//! GPU rasterizer (`gpu` feature): draws a [`Scene`] with wgpu on the
//! system's Vulkan, Metal, DirectX 12 or OpenGL driver.
//!
//! - Circles and rectangles: instanced quads with analytic anti-aliasing.
//! - Text: the CPU glyph coverage bitmaps, packed into an atlas texture.
//! - Any other fill or stroke (tiny-skia's stroker gives the outline): the
//!   nonzero rule with stencil-and-cover, anti-aliased by 4× MSAA.
//! - Clipping: analytic, per pixel, like the CPU rasterizer.
//! - Group opacity is applied to each child (the CPU composites the group).
//!
//! The device, pipelines, atlas and targets are created once and reused;
//! each render uploads the frame's geometry, draws it in one render pass and
//! reads the pixels back.

use super::color::Color;
use super::output::Error;
use super::raster::{self, GlyphDraw, GlyphKey};
use super::render::Out;
use super::scene::{LineCap, Node, RasterImage, Scene, Stroke};
use super::text::FontSet;
use resvg::tiny_skia::{self as sk, Pixmap, Transform};
use std::collections::HashMap;
use std::future::Future;
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Wake};

const ATLAS: u32 = 2048;
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

/// Runs a future to completion on the current thread.
fn block_on<F: Future>(f: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Arc::new(Unpark(std::thread::current())).into();
    let mut cx = Context::from_waker(&waker);
    let mut f = std::pin::pin!(f);
    loop {
        match f.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

/// Bytes of a float slice (for buffer uploads).
fn bytes(v: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding or invalid bit patterns as bytes; the
    // length covers exactly the slice's memory.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), std::mem::size_of_val(v)) }
}

/// Premultiplied, normalized RGBA.
fn premul(c: Color, alpha: f32) -> [f32; 4] {
    let a = (c.a * alpha).clamp(0.0, 1.0);
    [
        c.r as f32 / 255.0 * a,
        c.g as f32 / 255.0 * a,
        c.b as f32 / 255.0 * a,
        a,
    ]
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    samples: u32,
    image_layout: wgpu::BindGroupLayout,
    stencil: wgpu::RenderPipeline,
    cover: wgpu::RenderPipeline,
    tri: wgpu::RenderPipeline,
    mesh: wgpu::RenderPipeline,
    mesh_layout: wgpu::BindGroupLayout,
    shape: wgpu::RenderPipeline,
    glyph: wgpu::RenderPipeline,
    image: wgpu::RenderPipeline,
    sprite: wgpu::RenderPipeline,
    post: wgpu::RenderPipeline,
    nearest: wgpu::Sampler,
    linear: wgpu::Sampler,
    globals: wgpu::Buffer,
    bind: wgpu::BindGroup,
    atlas: wgpu::Texture,
    /// Bind group 1 for pipelines without an image.
    no_image: wgpu::BindGroup,
    state: Mutex<State>,
}

/// Per-size render targets and growable upload buffers.
#[derive(Default)]
struct State {
    targets: Option<Targets>,
    atlas: Atlas,
    buffers: [Option<wgpu::Buffer>; 7],
    /// Vertex buffers of retained 3D meshes by id, with vertex counts.
    meshes: std::collections::HashMap<u64, (wgpu::Buffer, u32)>,
}

struct Targets {
    size: (u32, u32),
    msaa: Option<wgpu::TextureView>,
    resolve_view: wgpu::TextureView,
    /// Converted image (see `fs_post`), copied to `readback`.
    post: wgpu::Texture,
    post_view: wgpu::TextureView,
    post_bind: wgpu::BindGroup,
    depth: wgpu::TextureView,
    readback: wgpu::Buffer,
    row: u32,
}

/// Shelf-packed glyph atlas.
#[derive(Default)]
struct Atlas {
    map: HashMap<GlyphKey, Option<(u32, u32)>>,
    x: u32,
    y: u32,
    row: u32,
    /// Set when the atlas was cleared during the current frame.
    reset: bool,
}

impl Atlas {
    fn place(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w > ATLAS || h > ATLAS {
            return None;
        }
        if self.x + w > ATLAS {
            self.x = 0;
            self.y += self.row;
            self.row = 0;
        }
        if self.y + h > ATLAS {
            // Full: start over (the frame is rebuilt, see `render`).
            self.map.clear();
            (self.x, self.y, self.row) = (0, 0, 0);
            self.reset = true;
        }
        let at = (self.x, self.y);
        self.x += w + 1;
        self.row = self.row.max(h + 1);
        Some(at)
    }
}

static GPU: OnceLock<Result<Gpu, String>> = OnceLock::new();

fn gpu() -> Result<&'static Gpu, Error> {
    GPU.get_or_init(Gpu::new)
        .as_ref()
        .map_err(|e| Error::Render(format!("GPU backend unavailable: {e}")))
}

/// Whether a GPU adapter can be used (initializes the GPU backend once).
pub(crate) fn available() -> bool {
    gpu().is_ok()
}

fn vertex_layout(
    stride: u64,
    attrs: &[wgpu::VertexAttribute],
    instance: bool,
) -> wgpu::VertexBufferLayout<'_> {
    wgpu::VertexBufferLayout {
        array_stride: stride,
        step_mode: if instance {
            wgpu::VertexStepMode::Instance
        } else {
            wgpu::VertexStepMode::Vertex
        },
        attributes: attrs,
    }
}

impl Gpu {
    fn new() -> Result<Gpu, String> {
        let instance = wgpu::Instance::default();
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        // A software rasterizer (llvmpipe, lavapipe, WARP) is slower than the
        // CPU backends; report no GPU instead.
        if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
            return Err(format!(
                "only a software adapter is available ({})",
                adapter.get_info().name
            ));
        }
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("dataviz"),
            required_limits: wgpu::Limits {
                // Large data: as big vertex buffers as the adapter allows.
                max_buffer_size: adapter.limits().max_buffer_size,
                ..wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits())
            },
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let ok4 = |f| {
            adapter
                .get_texture_format_features(f)
                .flags
                .sample_count_supported(4)
        };
        let samples = if ok4(COLOR) && ok4(DEPTH) { 4 } else { 1 };

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("dataviz"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&globals_layout), Some(&image_layout)],
            immediate_size: 0,
        });

        let keep = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        };
        let ds = |front: wgpu::StencilFaceState, back: wgpu::StencilFaceState, write: u32| {
            wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState {
                    front,
                    back,
                    read_mask: 0xff,
                    write_mask: write,
                },
                bias: Default::default(),
            }
        };
        let blend = Some(wgpu::ColorTargetState {
            format: COLOR,
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        });
        let pipeline = |vs: &str,
                        fs: &str,
                        buffers: &[Option<wgpu::VertexBufferLayout<'_>>],
                        depth: wgpu::DepthStencilState,
                        target: Option<wgpu::ColorTargetState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(vs),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: Some(depth),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[target],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        use wgpu::VertexFormat::{Float32, Float32x2, Float32x4, Uint32};
        let a = |location, format, offset| wgpu::VertexAttribute {
            format,
            offset,
            shader_location: location,
        };
        let stencil_attrs = [a(0, Float32x2, 0)];
        let cover_attrs = [a(0, Float32x4, 0), a(1, Float32x4, 16), a(2, Float32x4, 32)];
        let tri_attrs = [a(0, Float32x2, 0), a(1, Float32x4, 8), a(2, Float32x4, 24)];
        let shape_attrs = [
            a(0, Float32x4, 0),
            a(1, Float32x4, 16),
            a(2, Float32x4, 32),
            a(3, Float32x4, 48),
            a(4, Uint32, 64),
        ];
        let glyph_attrs = [
            a(0, Float32x4, 0),
            a(1, Float32x2, 16),
            a(2, Float32x4, 24),
            a(3, Float32x4, 40),
        ];
        let sprite_attrs = [
            a(0, Float32x4, 0),
            a(1, Float32x2, 16),
            a(2, Float32x4, 24),
            a(3, Float32, 40),
        ];
        let image_attrs = [a(0, Float32x4, 0), a(1, Float32x4, 16), a(2, Float32, 32)];

        let face = |pass_op| wgpu::StencilFaceState { pass_op, ..keep };
        let stencil = pipeline(
            "vs_stencil",
            "fs_none",
            &[Some(vertex_layout(8, &stencil_attrs, false))],
            ds(
                face(wgpu::StencilOperation::IncrementWrap),
                face(wgpu::StencilOperation::DecrementWrap),
                0xff,
            ),
            Some(wgpu::ColorTargetState {
                format: COLOR,
                blend: None,
                write_mask: wgpu::ColorWrites::empty(),
            }),
        );
        let nonzero = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::NotEqual,
            pass_op: wgpu::StencilOperation::Zero,
            ..keep
        };
        let cover = pipeline(
            "vs_cover",
            "fs_cover",
            &[Some(vertex_layout(48, &cover_attrs, true))],
            ds(nonzero, nonzero, 0xff),
            blend.clone(),
        );
        let tri = pipeline(
            "vs_tri",
            "fs_cover",
            &[Some(vertex_layout(40, &tri_attrs, false))],
            ds(keep, keep, 0),
            blend.clone(),
        );
        let shape = pipeline(
            "vs_shape",
            "fs_shape",
            &[Some(vertex_layout(68, &shape_attrs, true))],
            ds(keep, keep, 0),
            blend.clone(),
        );
        let glyph = pipeline(
            "vs_glyph",
            "fs_glyph",
            &[Some(vertex_layout(56, &glyph_attrs, true))],
            ds(keep, keep, 0),
            blend.clone(),
        );
        let sprite = pipeline(
            "vs_sprite",
            "fs_sprite",
            &[Some(vertex_layout(44, &sprite_attrs, true))],
            ds(keep, keep, 0),
            blend.clone(),
        );
        let image = pipeline(
            "vs_image",
            "fs_image",
            &[Some(vertex_layout(36, &image_attrs, true))],
            ds(keep, keep, 0),
            blend.clone(),
        );

        // Retained 3D meshes: own uniform (group 2), depth tested.
        let mesh_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let mesh_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mesh"),
            bind_group_layouts: &[
                Some(&globals_layout),
                Some(&image_layout),
                Some(&mesh_layout),
            ],
            immediate_size: 0,
        });
        let mesh_attrs = [
            a(0, wgpu::VertexFormat::Float32x3, 0),
            a(1, wgpu::VertexFormat::Float32x3, 12),
            a(2, Float32x4, 24),
        ];
        let mesh = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mesh"),
            layout: Some(&mesh_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_mesh"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout(40, &mesh_attrs, false))],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState {
                    front: keep,
                    back: keep,
                    read_mask: 0xff,
                    write_mask: 0,
                },
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: samples,
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_mesh"),
                compilation_options: Default::default(),
                targets: &[blend],
            }),
            multiview_mask: None,
            cache: None,
        });

        let post = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("post"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_post"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_post"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let sampler = |filter| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: filter,
                min_filter: filter,
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            })
        };
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            size: wgpu::Extent3d {
                width: ATLAS,
                height: ATLAS,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &globals_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
            ],
        });
        let dummy = device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: COLOR,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let nearest = sampler(wgpu::FilterMode::Nearest);
        let no_image = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &image_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&dummy),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&nearest),
                },
            ],
        });
        // Errors are reported through error scopes in `render`, never panics.
        device.on_uncaptured_error(Arc::new(|e| eprintln!("dataviz: GPU error: {e}")));
        Ok(Gpu {
            no_image,
            nearest,
            linear: sampler(wgpu::FilterMode::Linear),
            device,
            queue,
            samples,
            image_layout,
            stencil,
            cover,
            tri,
            mesh,
            mesh_layout,
            shape,
            glyph,
            image,
            sprite,
            post,
            globals,
            bind,
            atlas,
            state: Mutex::default(),
        })
    }

    fn targets(&self, w: u32, h: u32) -> Targets {
        let tex = |samples, format, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let att = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let resolve = tex(1, COLOR, att | wgpu::TextureUsages::TEXTURE_BINDING);
        let resolve_view = resolve.create_view(&Default::default());
        let post = tex(1, COLOR, att | wgpu::TextureUsages::COPY_SRC);
        let post_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.image_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&resolve_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.nearest),
                },
            ],
        });
        let row = (w * 4).div_ceil(256) * 256;
        Targets {
            size: (w, h),
            msaa: (self.samples > 1)
                .then(|| tex(self.samples, COLOR, att).create_view(&Default::default())),
            resolve_view,
            post_view: post.create_view(&Default::default()),
            post,
            post_bind,
            depth: tex(self.samples, DEPTH, att).create_view(&Default::default()),
            readback: self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: row as u64 * h as u64,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            row,
        }
    }

    /// Uploads `data` into growable buffer `slot`.
    fn upload<'a>(
        &self,
        slot: &'a mut Option<wgpu::Buffer>,
        data: &[f32],
    ) -> Result<Option<&'a wgpu::Buffer>, Error> {
        if data.is_empty() {
            return Ok(None);
        }
        let size = std::mem::size_of_val(data) as u64;
        let max = self.device.limits().max_buffer_size;
        if size > max {
            return Err(Error::Render(format!(
                "GPU: {} MB of geometry exceeds the {} MB buffer limit",
                size >> 20,
                max >> 20
            )));
        }
        if slot.as_ref().is_none_or(|b| b.size() < size) {
            *slot = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: size.next_power_of_two().clamp(4096, max.max(4096)),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let Some(b) = slot.as_ref() else {
            return Ok(None);
        };
        self.queue.write_buffer(b, 0, bytes(data));
        Ok(Some(b))
    }
}

/// Draw commands in painting order.
enum Cmd {
    Shapes(Range<u32>),
    Glyphs(Range<u32>),
    Fill {
        stencil: Range<u32>,
        cover: u32,
    },
    /// Vertices of convex fills (triangle list).
    Tris(Range<u32>),
    /// A retained 3D mesh (index into `Frame::meshes`).
    Mesh(usize),
    Image {
        instance: u32,
        bind: usize,
    },
    Sprites {
        instances: Range<u32>,
        bind: usize,
    },
}

/// Geometry of one frame.
#[derive(Default)]
struct Frame {
    cmds: Vec<Cmd>,
    stencil: Vec<f32>,
    cover: Vec<f32>,
    tris: Vec<f32>,
    /// Retained meshes: id, vertices, uniform (camera, light, clip, device).
    meshes: Vec<(u64, std::sync::Arc<Vec<f32>>, [f32; 32])>,
    shapes: Vec<f32>,
    glyphs: Vec<f32>,
    images: Vec<f32>,
    sprites: Vec<f32>,
    binds: Vec<wgpu::BindGroup>,
}

impl Frame {
    fn push_shape(
        &mut self,
        a: [f32; 4],
        fill: [f32; 4],
        stroke: [f32; 4],
        clip: [f32; 4],
        kind: u32,
    ) {
        let i = (self.shapes.len() / 17) as u32;
        self.shapes.extend_from_slice(&a);
        self.shapes.extend_from_slice(&fill);
        self.shapes.extend_from_slice(&stroke);
        self.shapes.extend_from_slice(&clip);
        self.shapes.push(f32::from_bits(kind));
        match self.cmds.last_mut() {
            Some(Cmd::Shapes(r)) if r.end == i => r.end += 1,
            _ => self.cmds.push(Cmd::Shapes(i..i + 1)),
        }
    }

    fn push_glyph(&mut self, dst: [f32; 4], uv: [f32; 2], color: [f32; 4], clip: [f32; 4]) {
        let i = (self.glyphs.len() / 14) as u32;
        self.glyphs.extend_from_slice(&dst);
        self.glyphs.extend_from_slice(&uv);
        self.glyphs.extend_from_slice(&color);
        self.glyphs.extend_from_slice(&clip);
        match self.cmds.last_mut() {
            Some(Cmd::Glyphs(r)) if r.end == i => r.end += 1,
            _ => self.cmds.push(Cmd::Glyphs(i..i + 1)),
        }
    }

    /// A convex polygon as a triangle fan, batched with the previous ones.
    /// Returns false when `poly` is not convex.
    fn push_convex(&mut self, poly: &[(f32, f32)], color: [f32; 4], clip: [f32; 4]) -> bool {
        let n = poly.len();
        if !(3..=64).contains(&n) {
            return false;
        }
        let mut sign = 0.0f32;
        for i in 0..n {
            let (a, b, c) = (poly[i], poly[(i + 1) % n], poly[(i + 2) % n]);
            let cross = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
            if !cross.is_finite() {
                return false;
            }
            if cross.abs() > 1e-6 {
                if sign == 0.0 {
                    sign = cross.signum();
                } else if cross.signum() != sign {
                    return false;
                }
            }
        }
        // A convex outline turns once: the direction changes sign at most
        // twice along each axis (rejects self-overlapping stars).
        let flips = |f: fn((f32, f32)) -> f32| {
            let mut k = 0;
            let mut last = 0.0f32;
            for i in 0..n {
                let d = f(poly[(i + 1) % n]) - f(poly[i]);
                if d != 0.0 {
                    if last != 0.0 && d.signum() != last {
                        k += 1;
                    }
                    last = d.signum();
                }
            }
            k
        };
        if sign == 0.0 || flips(|p| p.0) > 2 || flips(|p| p.1) > 2 {
            return sign == 0.0; // degenerate: nothing to draw
        }
        let start = (self.tris.len() / 10) as u32;
        let mut v = |p: (f32, f32)| {
            self.tris.extend_from_slice(&[p.0, p.1]);
            self.tris.extend_from_slice(&color);
            self.tris.extend_from_slice(&clip);
        };
        for i in 1..n - 1 {
            v(poly[0]);
            v(poly[i]);
            v(poly[i + 1]);
        }
        let end = (self.tris.len() / 10) as u32;
        match self.cmds.last_mut() {
            Some(Cmd::Tris(r)) if r.end == start => r.end = end,
            _ => self.cmds.push(Cmd::Tris(start..end)),
        }
        true
    }

    /// Nonzero fill of device-space contours.
    fn push_fill(&mut self, contours: &[Vec<(f32, f32)>], color: [f32; 4], clip: [f32; 4]) {
        if color[3] <= 0.0 {
            return;
        }
        if let [c] = contours {
            if self.push_convex(c, color, clip) {
                return;
            }
        }
        let start = (self.stencil.len() / 2) as u32;
        let (mut l, mut t, mut r, mut b) = (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        for c in contours.iter().filter(|c| c.len() >= 3) {
            for p in c {
                (l, t, r, b) = (l.min(p.0), t.min(p.1), r.max(p.0), b.max(p.1));
            }
        }
        // Each edge becomes the trapezoid between it and the bottom of the
        // bounding box; its orientation adds or removes one winding. Unlike a
        // triangle fan this does not overdraw jagged outlines many times.
        for c in contours.iter().filter(|c| c.len() >= 3) {
            let next = c.iter().skip(1).chain(std::iter::once(&c[0]));
            for (p, q) in c.iter().zip(next) {
                if p.0 == q.0 {
                    continue;
                }
                self.stencil
                    .extend_from_slice(&[p.0, p.1, q.0, q.1, q.0, b, p.0, p.1, q.0, b, p.0, b]);
            }
        }
        let end = (self.stencil.len() / 2) as u32;
        if end == start || !(l.is_finite() && t.is_finite() && r.is_finite() && b.is_finite()) {
            self.stencil.truncate(start as usize * 2);
            return;
        }
        // The cover quad must reach every pixel the fan touched (it also
        // resets the stencil there), so it is not reduced to the clip.
        let cover = (self.cover.len() / 12) as u32;
        self.cover.extend_from_slice(&[
            l.floor() - 1.0,
            t.floor() - 1.0,
            r.ceil() + 1.0,
            b.ceil() + 1.0,
        ]);
        self.cover.extend_from_slice(&color);
        self.cover.extend_from_slice(&clip);
        self.cmds.push(Cmd::Fill {
            stencil: start..end,
            cover,
        });
    }
}

/// Flattens a device-space path into closed polylines.
fn flatten(path: &sk::Path, tolerance: f32) -> Vec<Vec<(f32, f32)>> {
    let mut out: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut cur: Vec<(f32, f32)> = Vec::new();
    let mut last = (0.0f32, 0.0f32);
    let steps = |len: f32| ((len / tolerance).sqrt().ceil() as usize).clamp(1, 64);
    let dist = |a: (f32, f32), b: sk::Point| ((b.x - a.0).powi(2) + (b.y - a.1).powi(2)).sqrt();
    for seg in path.segments() {
        match seg {
            sk::PathSegment::MoveTo(p) => {
                if cur.len() > 1 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
                last = (p.x, p.y);
                cur.push(last);
            }
            sk::PathSegment::LineTo(p) => {
                last = (p.x, p.y);
                cur.push(last);
            }
            sk::PathSegment::QuadTo(c, p) => {
                let n = steps(dist(last, c) + dist((c.x, c.y), p));
                let p0 = last;
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    cur.push((
                        u * u * p0.0 + 2.0 * u * t * c.x + t * t * p.x,
                        u * u * p0.1 + 2.0 * u * t * c.y + t * t * p.y,
                    ));
                }
                last = (p.x, p.y);
            }
            sk::PathSegment::CubicTo(c1, c2, p) => {
                let n = steps(dist(last, c1) + dist((c1.x, c1.y), c2) + dist((c2.x, c2.y), p));
                let p0 = last;
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    cur.push((
                        a * p0.0 + b * c1.x + c * c2.x + d * p.x,
                        a * p0.1 + b * c1.y + c * c2.y + d * p.y,
                    ));
                }
                last = (p.x, p.y);
            }
            sk::PathSegment::Close => {
                if cur.len() > 1 {
                    let first = cur[0];
                    out.push(std::mem::take(&mut cur));
                    last = first;
                    cur.push(first);
                }
            }
        }
    }
    if cur.len() > 1 {
        out.push(cur);
    }
    out
}

/// Builds the frame geometry from the scene.
struct Builder<'a> {
    gpu: &'a Gpu,
    fonts: &'a FontSet,
    base: Transform,
    atlas: &'a mut Atlas,
    frame: Frame,
}

impl Builder<'_> {
    fn dev(&self, x: f64, y: f64) -> (f32, f32) {
        (
            x as f32 * self.base.sx + self.base.tx,
            y as f32 * self.base.sy + self.base.ty,
        )
    }

    fn fill_path(&mut self, path: &sk::Path, color: [f32; 4], clip: [f32; 4]) {
        let contours = flatten(path, 0.2);
        self.frame.push_fill(&contours, color, clip);
    }

    /// Stroke of a user-space path: outline by tiny-skia, filled here.
    fn stroke_path(&mut self, path: &sk::Path, s: &Stroke, alpha: f32, clip: [f32; 4]) {
        if !s.color.is_visible() || s.width <= 0.0 {
            return;
        }
        let k = self.base.sx;
        let Some(mut p) = path.clone().transform(self.base) else {
            return;
        };
        let mut st = raster::sk_stroke(s);
        st.width *= k;
        st.dash = None;
        if !s.dash.is_empty() {
            let arr: Vec<f32> = s.dash.iter().map(|v| (*v as f32 * k).max(0.001)).collect();
            let arr = if arr.len() % 2 == 1 {
                [arr.clone(), arr].concat()
            } else {
                arr
            };
            if let Some(dash) = sk::StrokeDash::new(arr, 0.0) {
                match p.dash(&dash, 1.0) {
                    Some(dp) => p = dp,
                    None => return,
                }
            }
        }
        // Lines thinner than a pixel: 1 px wide with less alpha.
        let fade = st.width.min(1.0);
        st.width = st.width.max(1.0);
        if let Some(outline) = p.stroke(&st, 1.0) {
            self.fill_path(&outline, premul(s.color, alpha * fade), clip);
        }
    }

    /// Shaded sphere: an analytic shape (kind 2) with the straight color.
    fn sphere(&mut self, cx: f64, cy: f64, r: f64, c: Color, clip: [f32; 4], alpha: f32) {
        let (px, py) = self.dev(cx, cy);
        let a = c.a * alpha;
        if !(px.is_finite() && py.is_finite()) || r <= 0.0 || a <= 0.0 {
            return;
        }
        let f = [
            c.r as f32 / 255.0,
            c.g as f32 / 255.0,
            c.b as f32 / 255.0,
            a,
        ];
        self.frame
            .push_shape([px, py, r as f32 * self.base.sx, 0.0], f, [0.0; 4], clip, 2);
    }

    fn rect(&mut self, l: f32, t: f32, r: f32, b: f32, c: [f32; 4], clip: [f32; 4]) {
        if c[3] > 0.0 && r > l && b > t {
            self.frame.push_shape([l, t, r, b], c, [0.0; 4], clip, 0);
        }
    }

    fn nodes(&mut self, nodes: &[Node], clip: [f32; 4], alpha: f32) {
        for n in nodes {
            self.node(n, clip, alpha);
        }
    }

    fn node(&mut self, n: &Node, clip: [f32; 4], alpha: f32) {
        let k = self.base.sx;
        match n {
            Node::Path { cmds, fill, stroke } => {
                if cmds.len() < 2 {
                    return;
                }
                // Axis-aligned solid strokes (grid, ticks, frames): rectangles.
                if let (None, Some(s)) = (fill, stroke.as_ref().filter(|s| s.dash.is_empty())) {
                    if let Some(segs) = raster::axis_aligned_segments(cmds) {
                        let c = premul(s.color, alpha);
                        let hw = s.width as f32 * k / 2.0;
                        let ext = if s.cap == LineCap::Butt { 0.0 } else { hw };
                        for ((a, b), (x, y)) in segs {
                            let (p, q) = (self.dev(a, b), self.dev(x, y));
                            let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
                            let (y0, y1) = (p.1.min(q.1), p.1.max(q.1));
                            if b == y {
                                self.rect(x0 - ext, y0 - hw, x1 + ext, y0 + hw, c, clip);
                            } else {
                                self.rect(x0 - hw, y0 - ext, x0 + hw, y1 + ext, c, clip);
                            }
                        }
                        return;
                    }
                }
                let Some(path) = raster::build_path(cmds) else {
                    return;
                };
                if let Some(f) = fill.filter(|c| c.is_visible()) {
                    if let Some(p) = path.clone().transform(self.base) {
                        self.fill_path(&p, premul(f, alpha), clip);
                    }
                }
                if let Some(s) = stroke {
                    self.stroke_path(&path, s, alpha, clip);
                }
            }
            Node::Rect {
                x,
                y,
                w,
                h,
                rx,
                fill,
                stroke,
            } => {
                let (x, w) = if *w < 0.0 { (x + w, -w) } else { (*x, *w) };
                let (y, h) = if *h < 0.0 { (y + h, -h) } else { (*y, *h) };
                if w <= 0.0 && h <= 0.0 {
                    return;
                }
                let thin = stroke
                    .as_ref()
                    .is_none_or(|s| s.dash.is_empty() && s.width <= 2.0);
                if *rx <= 0.0 && thin {
                    let (l, t) = self.dev(x, y);
                    let (r, b) = self.dev(x + w.max(0.01), y + h.max(0.01));
                    if let Some(f) = fill.filter(|c| c.is_visible()) {
                        self.rect(l, t, r, b, premul(f, alpha), clip);
                    }
                    if let Some(s) = stroke
                        .as_ref()
                        .filter(|s| s.color.is_visible() && s.width > 0.0)
                    {
                        let c = premul(s.color, alpha);
                        let sw = s.width as f32 * k;
                        let hw = sw / 2.0;
                        if r - l <= sw || b - t <= sw {
                            self.rect(l - hw, t - hw, r + hw, b + hw, c, clip);
                        } else {
                            self.rect(l - hw, t - hw, r + hw, t + hw, c, clip);
                            self.rect(l - hw, b - hw, r + hw, b + hw, c, clip);
                            self.rect(l - hw, t + hw, l + hw, b - hw, c, clip);
                            self.rect(r - hw, t + hw, r + hw, b - hw, c, clip);
                        }
                    }
                    return;
                }
                let Some(path) = raster::rounded_rect(
                    x as f32,
                    y as f32,
                    w.max(0.01) as f32,
                    h.max(0.01) as f32,
                    *rx as f32,
                ) else {
                    return;
                };
                if let Some(f) = fill.filter(|c| c.is_visible()) {
                    if let Some(p) = path.clone().transform(self.base) {
                        self.fill_path(&p, premul(f, alpha), clip);
                    }
                }
                if let Some(s) = stroke {
                    self.stroke_path(&path, s, alpha, clip);
                }
            }
            Node::Circle {
                cx,
                cy,
                r,
                fill,
                stroke,
            } => {
                if !cx.is_finite() || !cy.is_finite() || *r <= 0.0 {
                    return;
                }
                if stroke.as_ref().is_some_and(|s| !s.dash.is_empty()) {
                    if let Some(path) =
                        sk::PathBuilder::from_circle(*cx as f32, *cy as f32, *r as f32)
                    {
                        if let Some(f) = fill.filter(|c| c.is_visible()) {
                            if let Some(p) = path.clone().transform(self.base) {
                                self.fill_path(&p, premul(f, alpha), clip);
                            }
                        }
                        if let Some(s) = stroke {
                            self.stroke_path(&path, s, alpha, clip);
                        }
                    }
                    return;
                }
                let (px, py) = self.dev(*cx, *cy);
                let fc = fill.map_or([0.0; 4], |f| premul(f, alpha));
                let (sc, sw) = stroke
                    .as_ref()
                    .filter(|s| s.color.is_visible() && s.width > 0.0)
                    .map_or(([0.0; 4], 0.0), |s| {
                        (premul(s.color, alpha), s.width as f32 * k)
                    });
                if fc[3] > 0.0 || sc[3] > 0.0 {
                    self.frame
                        .push_shape([px, py, *r as f32 * k, sw], fc, sc, clip, 1);
                }
            }
            Node::Text(t) => {
                let color = premul(t.color, alpha);
                let (fonts, base) = (self.fonts, self.base);
                raster::layout_text(fonts, base, t, |g| match g {
                    GlyphDraw::Bitmap { key, bitmap, x, y } => {
                        let at = match self.atlas.map.get(&key) {
                            Some(at) => *at,
                            None => {
                                let at = self.atlas.place(bitmap.w as u32, bitmap.h as u32);
                                if let Some((ax, ay)) = at {
                                    self.gpu.queue.write_texture(
                                        wgpu::TexelCopyTextureInfo {
                                            texture: &self.gpu.atlas,
                                            mip_level: 0,
                                            origin: wgpu::Origin3d { x: ax, y: ay, z: 0 },
                                            aspect: wgpu::TextureAspect::All,
                                        },
                                        &bitmap.cov,
                                        wgpu::TexelCopyBufferLayout {
                                            offset: 0,
                                            bytes_per_row: Some(bitmap.w as u32),
                                            rows_per_image: Some(bitmap.h as u32),
                                        },
                                        wgpu::Extent3d {
                                            width: bitmap.w as u32,
                                            height: bitmap.h as u32,
                                            depth_or_array_layers: 1,
                                        },
                                    );
                                }
                                self.atlas.map.insert(key, at);
                                at
                            }
                        };
                        if let Some((ax, ay)) = at {
                            let (x, y) = (x as f32, y as f32);
                            let dst = [x, y, x + bitmap.w as f32, y + bitmap.h as f32];
                            self.frame
                                .push_glyph(dst, [ax as f32, ay as f32], color, clip);
                        }
                    }
                    GlyphDraw::Image { image, x, y, w, h } => {
                        self.image(x, y, w, h, &image, true, clip, alpha);
                    }
                    GlyphDraw::Outline { path, ts, bold } => {
                        if let Some(p) = (*path).clone().transform(ts) {
                            self.fill_path(&p, color, clip);
                        }
                        if let Some(width) = bold {
                            let st = sk::Stroke {
                                width,
                                ..sk::Stroke::default()
                            };
                            if let Some(p) = path
                                .stroke(&st, ts.sx.abs().max(ts.kx.abs()).max(0.01))
                                .and_then(|p| p.transform(ts))
                            {
                                self.fill_path(&p, color, clip);
                            }
                        }
                    }
                });
            }
            Node::Image {
                x,
                y,
                w,
                h,
                image,
                smooth,
            } => self.image(*x, *y, *w, *h, image, *smooth, clip, alpha),
            Node::Sphere { cx, cy, r, color } => self.sphere(*cx, *cy, *r, *color, clip, alpha),
            Node::Tube { segs, width, color } => {
                for n in crate::plot::scene::tube_layers(segs, *width, *color) {
                    self.node(&n, clip, alpha);
                }
            }
            Node::Mesh3d(m) => {
                let mut u = [0.0f32; 32];
                u[..24].copy_from_slice(&m.view[..24]);
                // Clip: the mesh's plot area within the current clip, in
                // device pixels.
                let (l, t) = self.dev(m.view[24] as f64, m.view[25] as f64);
                let (r, b) = self.dev(m.view[26] as f64, m.view[27] as f64);
                u[24..28].copy_from_slice(&[
                    l.max(clip[0]),
                    t.max(clip[1]),
                    r.min(clip[2]),
                    b.min(clip[3]),
                ]);
                u[28..32].copy_from_slice(&[self.base.sx, self.base.tx, self.base.ty, 0.0]);
                self.frame.meshes.push((m.id, m.vertices.clone(), u));
                self.frame.cmds.push(Cmd::Mesh(self.frame.meshes.len() - 1));
            }
            Node::Instances { template, offsets } => self.instances(template, offsets, clip, alpha),
            Node::MarkerGrid {
                template,
                x,
                y,
                step,
                cols,
                bits,
            } => {
                let offsets: Vec<(f32, f32)> =
                    super::scene::grid_positions(*x, *y, *step, *cols, bits).collect();
                self.instances(template, &offsets, clip, alpha)
            }
            Node::Group {
                children,
                clip: c,
                opacity,
                ..
            } => {
                let mut clip = clip;
                if let Some((x, y, w, h)) = *c {
                    let (l, t) = self.dev(x, y);
                    let (r, b) = self.dev(x + w.max(0.0), y + h.max(0.0));
                    clip = [
                        clip[0].max(l),
                        clip[1].max(t),
                        clip[2].min(r).max(l),
                        clip[3].min(b).max(t),
                    ];
                }
                self.nodes(children, clip, alpha * opacity.clamp(0.0, 1.0) as f32);
            }
        }
    }

    /// Markers: analytic circles/squares, otherwise CPU-rendered sprites at
    /// the 16 quarter-pixel offsets, drawn as one instanced batch.
    fn instances(&mut self, template: &Node, offsets: &[(f32, f32)], clip: [f32; 4], alpha: f32) {
        let k = self.base.sx;
        let dev = |b: &Self, x: f32, y: f32| (x * b.base.sx + b.base.tx, y * b.base.sy + b.base.ty);
        match template {
            Node::Circle {
                cx,
                cy,
                r,
                fill,
                stroke,
            } if stroke.as_ref().is_none_or(|s| s.dash.is_empty()) => {
                let fc = fill.map_or([0.0; 4], |f| premul(f, alpha));
                let (sc, sw) = stroke
                    .as_ref()
                    .filter(|s| s.color.is_visible() && s.width > 0.0)
                    .map_or(([0.0; 4], 0.0), |s| {
                        (premul(s.color, alpha), s.width as f32 * k)
                    });
                if fc[3] <= 0.0 && sc[3] <= 0.0 {
                    return;
                }
                for &(x, y) in offsets {
                    let (px, py) = dev(self, x + *cx as f32, y + *cy as f32);
                    if px.is_finite() && py.is_finite() {
                        self.frame
                            .push_shape([px, py, *r as f32 * k, sw], fc, sc, clip, 1);
                    }
                }
            }
            Node::Rect {
                x,
                y,
                w,
                h,
                rx,
                fill: Some(f),
                stroke: None,
            } if *rx <= 0.0 => {
                let c = premul(*f, alpha);
                let (x0, x1) = (x.min(x + w) as f32, x.max(x + w) as f32);
                let (y0, y1) = (y.min(y + h) as f32, y.max(y + h) as f32);
                for &(ox, oy) in offsets {
                    let (l, t) = dev(self, ox + x0, oy + y0);
                    let (r, b) = dev(self, ox + x1, oy + y1);
                    self.rect(l, t, r, b, c, clip);
                }
            }
            Node::Sphere { cx, cy, r, color } => {
                for &(x, y) in offsets {
                    self.sphere(cx + x as f64, cy + y as f64, *r, *color, clip, alpha);
                }
            }
            _ => {
                if raster::template_bounds(template).is_none() {
                    // Not a sprite: draw it at every offset.
                    let base = self.base;
                    for &(x, y) in offsets {
                        self.base = base.pre_translate(x, y);
                        self.node(template, clip, alpha);
                    }
                    self.base = base;
                    return;
                }
                // 4×4 atlas of the quarter-pixel variants.
                let variants: Vec<_> = (0..16u8)
                    .map(|i| raster::instance_sprite(self.fonts, template, k, i % 4, i / 4))
                    .collect();
                let Some(Some((first, ox, oy))) = variants.first() else {
                    return;
                };
                let (w, h, ox, oy) = (first.width(), first.height(), *ox, *oy);
                if variants.iter().any(|v| {
                    v.as_ref()
                        .is_none_or(|v| v.0.width() != w || v.0.height() != h)
                }) {
                    return;
                }
                let gpu = self.gpu;
                let size = wgpu::Extent3d {
                    width: w * 4,
                    height: h * 4,
                    depth_or_array_layers: 1,
                };
                let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: COLOR,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                for (i, v) in variants.iter().enumerate() {
                    let Some((sp, _, _)) = v else { continue };
                    gpu.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d {
                                x: (i as u32 % 4) * w,
                                y: (i as u32 / 4) * h,
                                z: 0,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        sp.data(),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(w * 4),
                            rows_per_image: Some(h),
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                let view = tex.create_view(&Default::default());
                let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &gpu.image_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&gpu.nearest),
                        },
                    ],
                });
                self.frame.binds.push(bind);
                let bind = self.frame.binds.len() - 1;
                let start = (self.frame.sprites.len() / 11) as u32;
                for &(x, y) in offsets {
                    let (gx, gy) = dev(self, x, y);
                    if !(gx.is_finite() && gy.is_finite()) {
                        continue;
                    }
                    let (ix, iy) = (gx.floor(), gy.floor());
                    let q = |f: f32| ((f * 4.0) as u32).min(3);
                    let (qx, qy) = (q(gx - ix), q(gy - iy));
                    let (l, t) = (ix - ox as f32, iy - oy as f32);
                    self.frame
                        .sprites
                        .extend_from_slice(&[l, t, l + w as f32, t + h as f32]);
                    self.frame
                        .sprites
                        .extend_from_slice(&[(qx * w) as f32, (qy * h) as f32]);
                    self.frame.sprites.extend_from_slice(&clip);
                    self.frame.sprites.push(alpha);
                }
                let end = (self.frame.sprites.len() / 11) as u32;
                if end > start {
                    self.frame.cmds.push(Cmd::Sprites {
                        instances: start..end,
                        bind,
                    });
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn image(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        img: &RasterImage,
        smooth: bool,
        clip: [f32; 4],
        alpha: f32,
    ) {
        if img.width == 0
            || img.height == 0
            || w <= 0.0
            || h <= 0.0
            || img.rgba.len() < (img.width * img.height * 4) as usize
        {
            return;
        }
        let gpu = self.gpu;
        let size = wgpu::Extent3d {
            width: img.width,
            height: img.height,
            depth_or_array_layers: 1,
        };
        let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let data: Vec<u8> = img
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|s| {
                let a = s[3] as u16;
                let m = |c: u8| ((c as u16 * a + 127) / 255) as u8;
                [m(s[0]), m(s[1]), m(s[2]), s[3]]
            })
            .collect();
        gpu.queue.write_texture(
            tex.as_image_copy(),
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(img.width * 4),
                rows_per_image: Some(img.height),
            },
            size,
        );
        let view = tex.create_view(&Default::default());
        let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &gpu.image_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(if smooth {
                        &gpu.linear
                    } else {
                        &gpu.nearest
                    }),
                },
            ],
        });
        let (l, t) = self.dev(x, y);
        let (r, b) = self.dev(x + w, y + h);
        let instance = (self.frame.images.len() / 9) as u32;
        self.frame.images.extend_from_slice(&[l, t, r, b]);
        self.frame.images.extend_from_slice(&clip);
        self.frame.images.push(alpha);
        self.frame.binds.push(bind);
        self.frame.cmds.push(Cmd::Image {
            instance,
            bind: self.frame.binds.len() - 1,
        });
    }
}

/// Destination of the read-back pixels.
enum Sink<'a> {
    Pixmap(&'a mut Option<Pixmap>),
    Out(Out<'a>),
}

/// Renders `scene` at `scale` into `pixmap` (premultiplied RGBA).
pub(crate) fn render(
    scene: &Scene,
    fonts: &FontSet,
    scale: f64,
    pixmap: &mut Option<Pixmap>,
) -> Result<(), Error> {
    render_sink(scene, fonts, scale, Sink::Pixmap(pixmap))
}

/// Renders `scene` at `scale`, converting the read-back pixels into `out`.
pub(crate) fn render_out(
    scene: &Scene,
    fonts: &FontSet,
    scale: f64,
    out: Out<'_>,
) -> Result<(), Error> {
    render_sink(scene, fonts, scale, Sink::Out(out))
}

fn render_sink(scene: &Scene, fonts: &FontSet, scale: f64, sink: Sink<'_>) -> Result<(), Error> {
    let gpu = gpu()?;
    let memory = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let r = render_inner(gpu, scene, fonts, scale, sink);
    let errors = (block_on(validation.pop()), block_on(memory.pop()));
    match errors {
        (Some(e), _) | (None, Some(e)) => Err(Error::Render(format!("GPU: {e}"))),
        _ => r,
    }
}

fn render_inner(
    gpu: &Gpu,
    scene: &Scene,
    fonts: &FontSet,
    scale: f64,
    sink: Sink<'_>,
) -> Result<(), Error> {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let w = (scene.width * scale).round().max(1.0) as u32;
    let h = (scene.height * scale).round().max(1.0) as u32;
    let max = gpu.device.limits().max_texture_dimension_2d;
    if w > max || h > max {
        return Err(Error::Render(format!(
            "image {w}×{h} exceeds the GPU limit {max}"
        )));
    }
    let mut state = gpu.state.lock().unwrap_or_else(|e| e.into_inner());
    let state = &mut *state;
    let clip = [0.0, 0.0, w as f32, h as f32];

    // Geometry; rebuilt once if the glyph atlas overflowed meanwhile.
    let mut frame = Frame::default();
    for _ in 0..2 {
        state.atlas.reset = false;
        let mut b = Builder {
            gpu,
            fonts,
            base: Transform::from_scale(scale as f32, scale as f32),
            atlas: &mut state.atlas,
            frame: Frame::default(),
        };
        b.nodes(&scene.nodes, clip, 1.0);
        frame = b.frame;
        if !state.atlas.reset {
            break;
        }
    }

    if state.targets.as_ref().is_none_or(|t| t.size != (w, h)) {
        state.targets = Some(gpu.targets(w, h));
    }
    let targets = state.targets.as_ref().expect("created above");
    let mode = match &sink {
        Sink::Pixmap(_) => 0.0,
        Sink::Out(Out::Orgb(_)) => 1.0,
        Sink::Out(Out::Pixels(_)) => 2.0,
    };
    gpu.queue
        .write_buffer(&gpu.globals, 0, bytes(&[w as f32, h as f32, mode, 0.0]));
    let [b_stencil, b_cover, b_shapes, b_glyphs, b_images, b_sprites, b_tris] = &mut state.buffers;
    let tris = gpu.upload(b_tris, &frame.tris)?;
    // Retained meshes: uploaded once per id; dropped when no longer drawn.
    state
        .meshes
        .retain(|id, _| frame.meshes.iter().any(|m| m.0 == *id));
    let mut mesh_binds = Vec::with_capacity(frame.meshes.len());
    for (id, verts, u) in &frame.meshes {
        if !state.meshes.contains_key(id) {
            let size = std::mem::size_of_val(verts.as_slice()) as u64;
            if size > gpu.device.limits().max_buffer_size {
                return Err(Error::Render(
                    "GPU: 3D mesh exceeds the buffer limit".into(),
                ));
            }
            let b = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mesh"),
                size: size.max(4),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            gpu.queue.write_buffer(&b, 0, bytes(verts));
            state.meshes.insert(*id, (b, (verts.len() / 10) as u32));
        }
        let ub = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh view"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.queue.write_buffer(&ub, 0, bytes(u));
        mesh_binds.push(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &gpu.mesh_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ub.as_entire_binding(),
            }],
        }));
    }
    let sprites = gpu.upload(b_sprites, &frame.sprites)?;
    let stencil = gpu.upload(b_stencil, &frame.stencil)?;
    let cover = gpu.upload(b_cover, &frame.cover)?;
    let shapes = gpu.upload(b_shapes, &frame.shapes)?;
    let glyphs = gpu.upload(b_glyphs, &frame.glyphs)?;
    let images = gpu.upload(b_images, &frame.images)?;

    let bg = scene.background.map_or([0.0; 4], |c| premul(c, 1.0));
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    {
        let (view, resolve_target) = match &targets.msaa {
            Some(m) => (m, Some(&targets.resolve_view)),
            None => (&targets.resolve_view, None),
        };
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: bg[0] as f64,
                        g: bg[1] as f64,
                        b: bg[2] as f64,
                        a: bg[3] as f64,
                    }),
                    store: if resolve_target.is_some() {
                        wgpu::StoreOp::Discard
                    } else {
                        wgpu::StoreOp::Store
                    },
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Discard,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &gpu.bind, &[]);
        pass.set_bind_group(1, &gpu.no_image, &[]);
        pass.set_stencil_reference(0);
        for cmd in &frame.cmds {
            match cmd {
                Cmd::Shapes(r) => {
                    if let Some(b) = shapes {
                        pass.set_pipeline(&gpu.shape);
                        pass.set_vertex_buffer(0, b.slice(..));
                        pass.draw(0..6, r.clone());
                    }
                }
                Cmd::Glyphs(r) => {
                    if let Some(b) = glyphs {
                        pass.set_pipeline(&gpu.glyph);
                        pass.set_vertex_buffer(0, b.slice(..));
                        pass.draw(0..6, r.clone());
                    }
                }
                Cmd::Fill {
                    stencil: s,
                    cover: c,
                } => {
                    if let (Some(sb), Some(cb)) = (stencil, cover) {
                        pass.set_pipeline(&gpu.stencil);
                        pass.set_vertex_buffer(0, sb.slice(..));
                        pass.draw(s.clone(), 0..1);
                        pass.set_pipeline(&gpu.cover);
                        pass.set_vertex_buffer(0, cb.slice(..));
                        pass.draw(0..6, *c..*c + 1);
                    }
                }
                Cmd::Mesh(i) => {
                    let (id, ..) = &frame.meshes[*i];
                    if let Some((b, n)) = state.meshes.get(id) {
                        pass.set_pipeline(&gpu.mesh);
                        pass.set_bind_group(2, &mesh_binds[*i], &[]);
                        pass.set_vertex_buffer(0, b.slice(..));
                        pass.draw(0..*n, 0..1);
                    }
                }
                Cmd::Tris(r) => {
                    if let Some(b) = tris {
                        pass.set_pipeline(&gpu.tri);
                        pass.set_vertex_buffer(0, b.slice(..));
                        pass.draw(r.clone(), 0..1);
                    }
                }
                Cmd::Sprites { instances, bind } => {
                    if let Some(b) = sprites {
                        pass.set_pipeline(&gpu.sprite);
                        pass.set_bind_group(1, &frame.binds[*bind], &[]);
                        pass.set_vertex_buffer(0, b.slice(..));
                        pass.draw(0..6, instances.clone());
                        pass.set_bind_group(1, &gpu.no_image, &[]);
                    }
                }
                Cmd::Image { instance, bind } => {
                    if let Some(b) = images {
                        pass.set_pipeline(&gpu.image);
                        pass.set_bind_group(1, &frame.binds[*bind], &[]);
                        pass.set_vertex_buffer(0, b.slice(..));
                        pass.draw(0..6, *instance..*instance + 1);
                        pass.set_bind_group(1, &gpu.no_image, &[]);
                    }
                }
            }
        }
    }
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("post"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.post_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&gpu.post);
        pass.set_bind_group(0, &gpu.bind, &[]);
        pass.set_bind_group(1, &targets.post_bind, &[]);
        pass.draw(0..3, 0..1);
    }
    enc.copy_texture_to_buffer(
        targets.post.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &targets.readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(targets.row),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([enc.finish()]);

    // Read back.
    let (tx, rx) = std::sync::mpsc::channel();
    targets
        .readback
        .map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| Error::Render(format!("GPU: {e}")))?;
    rx.recv()
        .map_err(|e| Error::Render(e.to_string()))?
        .map_err(|e| Error::Render(format!("GPU readback: {e}")))?;

    {
        let data = targets
            .readback
            .get_mapped_range(..)
            .map_err(|e| Error::Render(format!("GPU readback: {e}")))?;
        let (stride, wu, hu) = (targets.row as usize, w as usize, h as usize);
        // A plain copy: two threads for large images already hide most of
        // the memory latency; more would mostly burn CPU time.
        let threads = if (w as u64 * h as u64) >= 1_000_000 {
            2
        } else {
            1
        };
        match sink {
            Sink::Pixmap(pixmap) => {
                if pixmap
                    .as_ref()
                    .is_none_or(|p| p.width() != w || p.height() != h)
                {
                    *pixmap = Pixmap::new(w, h);
                }
                let pm = pixmap
                    .as_mut()
                    .ok_or_else(|| Error::Render("empty image".into()))?;
                for (dst, src) in pm
                    .data_mut()
                    .chunks_exact_mut(wu * 4)
                    .zip(data.chunks(stride))
                {
                    dst.copy_from_slice(&src[..wu * 4]);
                }
            }
            Sink::Out(Out::Orgb(o)) => raster::copy_rows_u32(&data, stride, wu, hu, o, threads),
            Sink::Out(Out::Pixels(p)) => {
                p.width = w;
                p.height = h;
                raster::copy_rows(&data, stride, wu * 4, hu, &mut p.rgba, threads)
            }
        }
    }
    targets.readback.unmap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    fn figure() -> Figure {
        let mut fig = Figure::new(640, 420);
        fig.title("GPU");
        let ax = fig.axes();
        let x: Vec<f64> = (0..5000).map(|i| i as f64 / 250.0).collect();
        ax.line(&x, x.iter().map(|v| v.sin() + (v * 31.0).cos() * 0.3))
            .label("dense");
        ax.line([0.0, 20.0], [-1.0, 1.0])
            .dash(Dash::Dashed)
            .width(2.5);
        ax.scatter(&x[..400], x[..400].iter().map(|v| v.cos()))
            .size(5.0)
            .opacity(0.6);
        ax.bar([4.0, 10.0, 16.0], [0.6, -0.8, 0.4]).label("bars");
        ax.text(10.0, 1.2, "text");
        ax.fill_between(
            &x[..1000],
            x[..1000].iter().map(|v| v.sin() * 0.5),
            vec![0.0; 1000],
        )
        .opacity(0.3);
        ax.x_label("x")
            .y_label("y")
            .legend(LegendPosition::LowerRight);
        fig
    }

    fn compare(a: &[u8], b: &[u8]) -> (f64, f64) {
        let (mut sum, mut big, mut n) = (0u64, 0u64, 0u64);
        for (p, q) in a.chunks(4).zip(b.chunks(4)) {
            let d = (0..3)
                .map(|i| (p[i] as i32 - q[i] as i32).unsigned_abs())
                .max()
                .unwrap_or(0);
            sum += d as u64;
            big += (d > 64) as u64;
            n += 1;
        }
        (sum as f64 / n as f64, big as f64 / n as f64)
    }

    /// The GPU backend must look like the CPU one (skipped without a GPU).
    #[test]
    fn gpu_matches_cpu() {
        if !Backend::gpu_available() {
            eprintln!("no GPU adapter: skipped");
            return;
        }
        let mut fig = figure();
        let cpu = fig
            .backend(Backend::Cpu { threads: 1 })
            .to_pixels()
            .unwrap();
        let gpu = fig.backend(Backend::Gpu).to_pixels().unwrap();
        assert_eq!((cpu.width, cpu.height), (gpu.width, gpu.height));
        let (mean, big) = compare(&cpu.rgba, &gpu.rgba);
        assert!(
            mean < 1.5 && big < 0.005,
            "mean {mean}, very different {big}"
        );

        // 0RGB output and the encoded PNG take the same path.
        let (mut a, mut b) = (Vec::new(), Vec::new());
        fig.backend(Backend::Cpu { threads: 1 })
            .render_0rgb(&mut a)
            .unwrap();
        fig.backend(Backend::Gpu).render_0rgb(&mut b).unwrap();
        let bytes = |v: &[u32]| {
            v.iter()
                .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8, 255])
                .collect::<Vec<u8>>()
        };
        let (mean, big) = compare(&bytes(&a), &bytes(&b));
        assert!(
            mean < 1.5 && big < 0.005,
            "0rgb: mean {mean}, very different {big}"
        );
        assert!(fig.encode(Format::Png).unwrap().starts_with(b"\x89PNG"));

        // Repeated renders and another size reuse/recreate targets correctly.
        let again = fig.to_pixels().unwrap();
        assert_eq!(again, gpu);
        fig.scale(2.0);
        let big_px = fig.to_pixels().unwrap();
        assert_eq!((big_px.width, big_px.height), (1280, 840));
    }

    /// Surfaces stay on the GPU as meshes: the image matches the CPU's,
    /// turning keeps the mesh and new data replaces it.
    #[test]
    fn retained_mesh_matches_cpu() {
        if !Backend::gpu_available() {
            eprintln!("no GPU adapter: skipped");
            return;
        }
        let mut fig = Figure::new(500, 400);
        fig.axes()
            .surface_fn(|x, y| (x * y).sin(), (-2.0, 2.0), (-2.0, 2.0), 40);
        let cpu = fig
            .backend(Backend::Cpu { threads: 1 })
            .to_pixels()
            .unwrap();
        let gpu = fig.backend(Backend::Gpu).to_pixels().unwrap();
        let (mean, big) = compare(&cpu.rgba, &gpu.rgba);
        assert!(
            mean < 3.0 && big < 0.02,
            "mean {mean}, very different {big}"
        );
        let id = |fig: &mut Figure| {
            fig.axes()
                .mesh_cache
                .lock()
                .unwrap()
                .as_ref()
                .map(|c| c.id())
        };
        let first = id(&mut fig);
        assert!(first.is_some());
        fig.axes().rotate(Axis3::Z, 30.0);
        let turned = fig.to_pixels().unwrap();
        assert_ne!(turned, gpu);
        assert_eq!(id(&mut fig), first, "turning must keep the mesh");
        fig.axes()
            .surface_fn(|x, y| x + y, (-2.0, 2.0), (-2.0, 2.0), 10);
        fig.to_pixels().unwrap();
        assert_ne!(id(&mut fig), first, "new data must replace the mesh");
    }
}
