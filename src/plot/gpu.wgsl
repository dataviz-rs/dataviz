// dataviz GPU rasterizer. Coordinates are device pixels (origin top-left),
// colors are premultiplied RGBA, every primitive is multiplied by the
// analytic coverage of its clip rectangle (l, t, r, b).

struct Globals {
    size: vec2<f32>,
    pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var atlas: texture_2d<f32>;

@group(1) @binding(0) var img: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

fn ndc(p: vec2<f32>) -> vec4<f32> {
    return vec4<f32>(p.x / g.size.x * 2.0 - 1.0, 1.0 - p.y / g.size.y * 2.0, 0.0, 1.0);
}

// Area of the pixel around centre `p` inside rectangle `r`.
fn box_cov(p: vec2<f32>, r: vec4<f32>) -> f32 {
    let cx = clamp(min(p.x + 0.5, r.z) - max(p.x - 0.5, r.x), 0.0, 1.0);
    let cy = clamp(min(p.y + 0.5, r.w) - max(p.y - 0.5, r.y), 0.0, 1.0);
    return cx * cy;
}

// Corner `i` (0..5, two triangles) of rectangle `r`.
fn corner(i: u32, r: vec4<f32>) -> vec2<f32> {
    var c = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let k = c[i];
    return vec2<f32>(mix(r.x, r.z, k.x), mix(r.y, r.w, k.y));
}

// ---- stencil pass of a nonzero fill (triangle fans) ----

@vertex
fn vs_stencil(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> {
    return ndc(p);
}

@fragment
fn fs_none() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}

// ---- cover pass: bounding quad where the stencil is non-zero ----

struct SolidOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) clip: vec4<f32>,
};

@vertex
fn vs_cover(@builtin(vertex_index) vi: u32,
            @location(0) rect: vec4<f32>,
            @location(1) color: vec4<f32>,
            @location(2) clip: vec4<f32>) -> SolidOut {
    var o: SolidOut;
    o.pos = ndc(corner(vi, rect));
    o.color = color;
    o.clip = clip;
    return o;
}

@fragment
fn fs_cover(in: SolidOut) -> @location(0) vec4<f32> {
    return in.color * box_cov(in.pos.xy, in.clip);
}

// ---- analytic shapes: kind 0 = rectangle, 1 = circle ----

struct ShapeOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) a: vec4<f32>,
    @location(1) fill: vec4<f32>,
    @location(2) stroke: vec4<f32>,
    @location(3) clip: vec4<f32>,
    @location(4) @interpolate(flat) kind: u32,
};

@vertex
fn vs_shape(@builtin(vertex_index) vi: u32,
            @location(0) a: vec4<f32>,
            @location(1) fill: vec4<f32>,
            @location(2) stroke: vec4<f32>,
            @location(3) clip: vec4<f32>,
            @location(4) kind: u32) -> ShapeOut {
    var r: vec4<f32>;
    if kind == 0u {
        r = a + vec4<f32>(-1.0, -1.0, 1.0, 1.0);
    } else {
        let e = a.z + a.w * 0.5 + 1.0;
        r = vec4<f32>(a.x - e, a.y - e, a.x + e, a.y + e);
    }
    // Fragments outside the clip are never needed.
    r = vec4<f32>(max(r.xy, clip.xy - 1.0), min(r.zw, clip.zw + 1.0));
    r = vec4<f32>(r.xy, max(r.zw, r.xy));
    var o: ShapeOut;
    o.pos = ndc(corner(vi, r));
    o.a = a;
    o.fill = fill;
    o.stroke = stroke;
    o.clip = clip;
    o.kind = kind;
    return o;
}

@fragment
fn fs_shape(in: ShapeOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy;
    var c: vec4<f32>;
    if in.kind == 0u {
        c = in.fill * box_cov(p, in.a);
    } else {
        let d = distance(p, in.a.xy);
        let r = in.a.z;
        let cf = clamp(r + 0.5 - d, 0.0, 1.0);
        let sw = in.a.w;
        var cs = 0.0;
        if sw > 0.0 {
            if sw < 1.0 {
                cs = clamp(1.0 - abs(d - r), 0.0, 1.0) * sw;
            } else {
                cs = clamp(sw * 0.5 + 0.5 - abs(d - r), 0.0, 1.0);
            }
        }
        // Stroke over fill, like drawing them one after the other.
        c = in.stroke * cs + in.fill * cf * (1.0 - in.stroke.a * cs);
    }
    return c * box_cov(p, in.clip);
}

// ---- glyphs: pixel-aligned quads sampling the coverage atlas ----

struct GlyphOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) dst: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) clip: vec4<f32>,
};

@vertex
fn vs_glyph(@builtin(vertex_index) vi: u32,
            @location(0) dst: vec4<f32>,
            @location(1) uv: vec2<f32>,
            @location(2) color: vec4<f32>,
            @location(3) clip: vec4<f32>) -> GlyphOut {
    var o: GlyphOut;
    o.pos = ndc(corner(vi, dst));
    o.dst = dst;
    o.uv = uv;
    o.color = color;
    o.clip = clip;
    return o;
}

@fragment
fn fs_glyph(in: GlyphOut) -> @location(0) vec4<f32> {
    let t = vec2<i32>(floor(in.uv + in.pos.xy - in.dst.xy));
    let k = textureLoad(atlas, t, 0).r;
    return in.color * k * box_cov(in.pos.xy, in.clip);
}

// ---- images: textured quad (premultiplied texture) ----

struct ImageOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) rect: vec4<f32>,
    @location(1) clip: vec4<f32>,
    @location(2) alpha: f32,
};

@vertex
fn vs_image(@builtin(vertex_index) vi: u32,
            @location(0) rect: vec4<f32>,
            @location(1) clip: vec4<f32>,
            @location(2) alpha: f32) -> ImageOut {
    var o: ImageOut;
    o.pos = ndc(corner(vi, rect));
    o.rect = rect;
    o.clip = clip;
    o.alpha = alpha;
    return o;
}

@fragment
fn fs_image(in: ImageOut) -> @location(0) vec4<f32> {
    let uv = (in.pos.xy - in.rect.xy) / (in.rect.zw - in.rect.xy);
    let c = textureSampleLevel(img, samp, uv, 0.0);
    return c * in.alpha * box_cov(in.pos.xy, in.clip);
}

// ---- conversion of the resolved image for read-back ----
// g.pad.x: 0 = premultiplied RGBA, 1 = 0x00RRGGBB on white (bytes B, G, R, 0),
// 2 = straight-alpha RGBA.

@vertex
fn vs_post(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_post(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(img, vec2<i32>(pos.xy), 0);
    if g.pad.x == 1.0 {
        let w = min(c.rgb + vec3<f32>(1.0 - c.a), vec3<f32>(1.0));
        return vec4<f32>(w.b, w.g, w.r, 0.0);
    }
    if g.pad.x == 2.0 {
        if c.a > 0.0 {
            return vec4<f32>(min(c.rgb / c.a, vec3<f32>(1.0)), c.a);
        }
        return vec4<f32>(0.0);
    }
    return c;
}
