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

// ---- convex fills as plain triangles (one draw for many faces) ----

@vertex
fn vs_tri(@location(0) p: vec2<f32>,
          @location(1) color: vec4<f32>,
          @location(2) clip: vec4<f32>) -> SolidOut {
    var o: SolidOut;
    o.pos = ndc(p);
    o.color = color;
    o.clip = clip;
    return o;
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
    var o: ShapeOut;
    o.a = a;
    o.fill = fill;
    o.stroke = stroke;
    o.clip = clip;
    o.kind = kind;
    if kind == 3u {
        // Tube from a.xy to a.zw of radius stroke.x: a quad along it.
        let d = a.zw - a.xy;
        let len = length(d);
        var u = vec2<f32>(1.0, 0.0);
        if len > 1e-6 {
            u = d / len;
        }
        let nrm = vec2<f32>(-u.y, u.x);
        let e = stroke.x + 1.0;
        let k = corner(vi, vec4<f32>(0.0, 0.0, 1.0, 1.0));
        let q = a.xy + u * mix(-e, len + e, k.x) + nrm * mix(-e, e, k.y);
        o.pos = ndc(q);
        return o;
    }
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
    o.pos = ndc(corner(vi, r));
    return o;
}

@fragment
fn fs_shape(in: ShapeOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy;
    var c: vec4<f32>;
    if in.kind == 0u {
        c = in.fill * box_cov(p, in.a);
    } else if in.kind == 2u {
        // Shaded sphere (same model as the CPU rasterizer); `fill` is the
        // straight (not premultiplied) base color.
        let r = in.a.z;
        let dv = (p - in.a.xy) / r;
        let d = length(dv);
        let cov = clamp(r + 0.5 - d * r, 0.0, 1.0);
        var n = vec3<f32>(dv, sqrt(max(1.0 - dot(dv, dv), 0.0)));
        if d > 1.0 {
            n = vec3<f32>(dv / d, 0.0);
        }
        let l = vec3<f32>(-0.4212, -0.5617, 0.7121);
        let lam = max(dot(n, l), 0.0);
        let h = max(dot(n, vec3<f32>(l.x, l.y, l.z + 1.0) * 0.5404), 0.0);
        let spec = pow(h, 20.0) * 0.75;
        let base = in.fill.rgb * (0.5 + 0.5 * pow(min(lam * 1.25, 1.0), 0.8));
        let rgb = base + (vec3<f32>(1.0) - base) * spec;
        let a = in.fill.a * cov;
        c = vec4<f32>(rgb * a, a);
    } else if in.kind == 3u {
        // Lit tube with round ends (same model as the CPU rasterizer's
        // `fill_tube`); `fill` is the straight base color.
        let r = in.stroke.x;
        let dd = in.a.zw - in.a.xy;
        let len = length(dd);
        var u = vec2<f32>(1.0, 0.0);
        if len > 1e-6 {
            u = dd / len;
        }
        let q = p - in.a.xy;
        let along = dot(q, u);
        let lat = q.y * u.x - q.x * u.y;
        let o = max(max(-along, along - len), 0.0);
        let d = sqrt(lat * lat + o * o);
        let cov = clamp(r + 0.5 - d, 0.0, 1.0);
        let t = clamp(lat / r, -1.0, 1.0);
        let n = vec3<f32>(vec2<f32>(-u.y, u.x) * t, sqrt(max(1.0 - t * t, 0.0)));
        let l = vec3<f32>(-0.4212, -0.5617, 0.7121);
        let lam = max(dot(n, l), 0.0);
        let h = max(dot(n, vec3<f32>(l.x, l.y, l.z + 1.0) * 0.5404), 0.0);
        let spec = pow(h, 12.0) * 0.6;
        let t2 = t * t;
        let rim = 1.0 - 0.45 * t2 * t2 * t2;
        let base = in.fill.rgb * (0.5 + 0.5 * pow(min(lam * 1.25, 1.0), 0.8)) * rim;
        let rgb = base + (vec3<f32>(1.0) - base) * spec;
        let a = in.fill.a * cov;
        c = vec4<f32>(rgb * a, a);
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

// ---- marker sprites: pixel-aligned quads copying an RGBA sprite atlas ----

struct SpriteOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) dst: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) clip: vec4<f32>,
    @location(3) alpha: f32,
};

@vertex
fn vs_sprite(@builtin(vertex_index) vi: u32,
             @location(0) dst: vec4<f32>,
             @location(1) uv: vec2<f32>,
             @location(2) clip: vec4<f32>,
             @location(3) alpha: f32) -> SpriteOut {
    var o: SpriteOut;
    o.pos = ndc(corner(vi, dst));
    o.dst = dst;
    o.uv = uv;
    o.clip = clip;
    o.alpha = alpha;
    return o;
}

@fragment
fn fs_sprite(in: SpriteOut) -> @location(0) vec4<f32> {
    let t = vec2<i32>(floor(in.uv + in.pos.xy - in.dst.xy));
    return textureLoad(img, t, 0) * in.alpha * box_cov(in.pos.xy, in.clip);
}

// ---- retained 3D meshes: triangles in normalized box coordinates, turned
// and lit here with the camera of the frame (same model as the CPU scene) ----

struct MeshView {
    m0: vec4<f32>,
    m1: vec4<f32>,
    m2: vec4<f32>,
    // scale, centre x, centre y, perspective distance (canvas units)
    p: vec4<f32>,
    // radius, fog, lighting (0/1), unused
    q: vec4<f32>,
    bg: vec4<f32>,
    // clip rectangle in device pixels
    clip: vec4<f32>,
    // canvas → device: scale, offset x, offset y
    dev: vec4<f32>,
};

@group(2) @binding(0) var<uniform> mv: MeshView;

struct MeshOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_mesh(@location(0) p: vec3<f32>, @location(1) n: vec3<f32>, @location(2) c: vec4<f32>) -> MeshOut {
    let v = vec3<f32>(dot(mv.m0.xyz, p), dot(mv.m1.xyz, p), dot(mv.m2.xyz, p));
    let radius = mv.q.x;
    var f = 1.0;
    if mv.p.w > 0.0 {
        f = mv.p.w / max(mv.p.w - v.z, radius * 0.05);
    }
    let sx = (mv.p.y + v.x * mv.p.x * f) * mv.dev.x + mv.dev.y;
    let sy = (mv.p.z - v.y * mv.p.x * f) * mv.dev.x + mv.dev.z;
    var o: MeshOut;
    let q = ndc(vec2<f32>(sx, sy));
    // Nearer (larger view z) is smaller depth.
    o.pos = vec4<f32>(q.xy, clamp(0.5 - v.z / (4.0 * max(radius, 1e-6)), 0.0, 1.0), 1.0);
    var rgb = c.rgb;
    if mv.q.z > 0.5 {
        var nv = vec3<f32>(dot(mv.m0.xyz, n), dot(mv.m1.xyz, n), dot(mv.m2.xyz, n));
        if nv.z < 0.0 {
            nv = -nv;
        }
        let l = vec3<f32>(-0.42, 0.56, 0.71);
        let d = 0.42 + 0.62 * max(dot(nv, l), 0.0);
        let s = 0.32 * pow(max(dot(nv, normalize(l + vec3<f32>(0.0, 0.0, 1.0))), 0.0), 28.0);
        if d < 1.0 {
            rgb = rgb * d;
        } else {
            rgb = rgb + (vec3<f32>(1.0) - rgb) * (d - 1.0) * 0.6;
        }
        if s > 0.004 {
            rgb = rgb + (vec3<f32>(1.0) - rgb) * s;
        }
    }
    if mv.q.y > 0.0 {
        let near = clamp((v.z / max(radius, 1e-6) + 1.0) * 0.5, 0.0, 1.0);
        rgb = mix(rgb, mv.bg.rgb, mv.q.y * (1.0 - near));
    }
    o.color = vec4<f32>(rgb * c.a, c.a);
    return o;
}

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    return in.color * box_cov(in.pos.xy, mv.clip);
}
