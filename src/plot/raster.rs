//! Direct rasterizer: draws a [`Scene`] into pixels with tiny-skia, without
//! going through SVG text and an SVG parser.
//!
//! - Text is drawn from glyph outlines of the same font used for layout
//!   (outlines are cached per glyph).
//! - Repeated identical circles (scatter markers) are rendered once into a
//!   sprite and blitted.
//! - Pixel buffers can be reused between frames (real-time charts).

use super::color::Color;
use super::output::Pixels;
use super::scene::{
    Anchor, Baseline, LineCap, LineJoin, Node, PathCmd, RasterImage, Scene, Stroke, TextNode,
};
use super::text::FontSet;
use ab_glyph::{Font, GlyphId, OutlineCurve, ScaleFont};
use resvg::tiny_skia::{
    self as sk, FillRule, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Transform,
};
use std::cell::RefCell;
use std::hash::{BuildHasherDefault, Hasher};

/// Small, fast non-cryptographic hasher (FxHash) for the internal caches;
/// keys are trusted, so DoS-resistant hashing is unnecessary.
#[derive(Default)]
struct FxHasher(u64);

impl FxHasher {
    #[inline]
    fn add(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

impl Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.add(*b as u64);
        }
    }
    fn write_u8(&mut self, n: u8) {
        self.add(n as u64);
    }
    fn write_u16(&mut self, n: u16) {
        self.add(n as u64);
    }
    fn write_u32(&mut self, n: u32) {
        self.add(n as u64);
    }
    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }
    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<FxHasher>>;

fn color(c: Color) -> sk::Color {
    sk::Color::from_rgba8(c.r, c.g, c.b, (c.a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color(c));
    p.anti_alias = true;
    p
}

fn sk_stroke(s: &Stroke) -> sk::Stroke {
    let mut st = sk::Stroke {
        width: s.width.max(0.0) as f32,
        line_cap: match s.cap {
            LineCap::Butt => sk::LineCap::Butt,
            LineCap::Round => sk::LineCap::Round,
            LineCap::Square => sk::LineCap::Square,
        },
        line_join: match s.join {
            LineJoin::Miter => sk::LineJoin::Miter,
            LineJoin::Round => sk::LineJoin::Round,
            LineJoin::Bevel => sk::LineJoin::Bevel,
        },
        ..sk::Stroke::default()
    };
    if !s.dash.is_empty() {
        let mut d: Vec<f32> = s.dash.iter().map(|v| (*v as f32).max(0.001)).collect();
        if d.len() % 2 == 1 {
            d.extend(d.clone());
        }
        st.dash = sk::StrokeDash::new(d, 0.0);
    }
    st
}

/// SVG endpoint arc → cubic Béziers (appended to `pb`).
fn arc_to(
    pb: &mut PathBuilder,
    from: (f64, f64),
    rx: f64,
    ry: f64,
    large: bool,
    sweep: bool,
    to: (f64, f64),
) {
    let (x1, y1) = from;
    let (x2, y2) = to;
    if (x1 - x2).abs() < 1e-9 && (y1 - y2).abs() < 1e-9 {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-9 || ry < 1e-9 {
        pb.line_to(x2 as f32, y2 as f32);
        return;
    }
    let (dx, dy) = ((x1 - x2) / 2.0, (y1 - y2) / 2.0);
    let lambda = (dx * dx) / (rx * rx) + (dy * dy) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * dy * dy - ry * ry * dx * dx).max(0.0);
    let den = rx * rx * dy * dy + ry * ry * dx * dx;
    let mut co = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
    if large == sweep {
        co = -co;
    }
    let (cxp, cyp) = (co * rx * dy / ry, -co * ry * dx / rx);
    let (cx, cy) = (cxp + (x1 + x2) / 2.0, cyp + (y1 + y2) / 2.0);
    let ang = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let t1 = ang(1.0, 0.0, (dx - cxp) / rx, (dy - cyp) / ry);
    let mut dt = ang(
        (dx - cxp) / rx,
        (dy - cyp) / ry,
        (-dx - cxp) / rx,
        (-dy - cyp) / ry,
    );
    if !sweep && dt > 0.0 {
        dt -= std::f64::consts::TAU;
    } else if sweep && dt < 0.0 {
        dt += std::f64::consts::TAU;
    }
    let segs = (dt.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let d = dt / segs as f64;
    let k = 4.0 / 3.0 * (d / 4.0).tan();
    let mut t = t1;
    for _ in 0..segs {
        let (c1, s1) = (t.cos(), t.sin());
        let (c2, s2) = ((t + d).cos(), (t + d).sin());
        let p = |c: f64, s: f64| (cx + rx * c, cy + ry * s);
        let (ax, ay) = p(c1, s1);
        let (bx, by) = p(c2, s2);
        pb.cubic_to(
            (ax - k * rx * s1) as f32,
            (ay + k * ry * c1) as f32,
            (bx + k * rx * s2) as f32,
            (by - k * ry * c2) as f32,
            bx as f32,
            by as f32,
        );
        t += d;
    }
}

fn build_path(cmds: &[PathCmd]) -> Option<sk::Path> {
    let mut pb = PathBuilder::with_capacity(cmds.len(), cmds.len());
    let mut cur = (0.0, 0.0);
    let mut start = (0.0, 0.0);
    for c in cmds {
        match *c {
            PathCmd::MoveTo(x, y) => {
                pb.move_to(x as f32, y as f32);
                cur = (x, y);
                start = cur;
            }
            PathCmd::LineTo(x, y) => {
                pb.line_to(x as f32, y as f32);
                cur = (x, y);
            }
            PathCmd::CubicTo(a, b, c2, d, x, y) => {
                pb.cubic_to(a as f32, b as f32, c2 as f32, d as f32, x as f32, y as f32);
                cur = (x, y);
            }
            PathCmd::ArcTo(rx, ry, large, sweep, x, y) => {
                arc_to(&mut pb, cur, rx, ry, large, sweep, (x, y));
                cur = (x, y);
            }
            PathCmd::Close => {
                pb.close();
                cur = start;
            }
        }
    }
    pb.finish()
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<sk::Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    if r <= 0.0 {
        return sk::Rect::from_xywh(x, y, w, h).map(PathBuilder::from_rect);
    }
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

thread_local! {
    /// Glyph outlines in font units (y up), per font and glyph.
    static GLYPHS: RefCell<HashMap<(usize, u16), Option<sk::Path>>> = RefCell::new(HashMap::default());
}

fn glyph_path(font: &ab_glyph::FontArc, key: usize, id: GlyphId) -> Option<sk::Path> {
    GLYPHS.with(|g| {
        let mut g = g.borrow_mut();
        if g.len() > 20_000 {
            g.clear();
        }
        g.entry((key, id.0))
            .or_insert_with(|| {
                let outline = font.outline(id)?;
                let mut pb = PathBuilder::new();
                let mut last: Option<ab_glyph::Point> = None;
                for c in &outline.curves {
                    let (p0, end) = match c {
                        OutlineCurve::Line(a, b) => (*a, *b),
                        OutlineCurve::Quad(a, _, c) => (*a, *c),
                        OutlineCurve::Cubic(a, _, _, d) => (*a, *d),
                    };
                    if last.is_none_or(|l| (l.x - p0.x).abs() > 1e-3 || (l.y - p0.y).abs() > 1e-3) {
                        if last.is_some() {
                            pb.close();
                        }
                        pb.move_to(p0.x, -p0.y);
                    }
                    match c {
                        OutlineCurve::Line(_, b) => pb.line_to(b.x, -b.y),
                        OutlineCurve::Quad(_, b, c) => pb.quad_to(b.x, -b.y, c.x, -c.y),
                        OutlineCurve::Cubic(_, b, c, d) => {
                            pb.cubic_to(b.x, -b.y, c.x, -c.y, d.x, -d.y)
                        }
                    }
                    last = Some(end);
                }
                if last.is_some() {
                    pb.close();
                }
                pb.finish()
            })
            .clone()
    })
}

/// Axis-aligned clip rectangle in device pixels (every scene clip is a
/// rectangle, so clipping is analytic instead of a per-pixel mask).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Clip {
    l: f32,
    t: f32,
    r: f32,
    b: f32,
}

impl Clip {
    fn new(l: f32, t: f32, r: f32, b: f32) -> Self {
        Clip {
            l,
            t,
            r: r.max(l),
            b: b.max(t),
        }
    }

    fn intersect(self, o: Clip) -> Clip {
        Clip::new(
            self.l.max(o.l),
            self.t.max(o.t),
            self.r.min(o.r),
            self.b.min(o.b),
        )
    }

    /// Coverage (0–255) of pixel `(px, py)` by the clip.
    #[inline]
    fn cov(&self, px: i32, py: i32) -> u32 {
        let (x, y) = (px as f32, py as f32);
        if x >= self.l && x + 1.0 <= self.r && y >= self.t && y + 1.0 <= self.b {
            return 255;
        }
        let cx = ((x + 1.0).min(self.r) - x.max(self.l)).clamp(0.0, 1.0);
        let cy = ((y + 1.0).min(self.b) - y.max(self.t)).clamp(0.0, 1.0);
        (cx * cy * 255.0 + 0.5) as u32
    }
}

/// Source-over of a premultiplied color `rgb·a` with alpha `a` (0–255).
#[inline]
fn blend(dst: &mut [u8], rgb: [u32; 3], a: u32) {
    if a >= 255 {
        dst[0] = rgb[0] as u8;
        dst[1] = rgb[1] as u8;
        dst[2] = rgb[2] as u8;
        dst[3] = 255;
        return;
    }
    let inv = 255 - a;
    for j in 0..3 {
        dst[j] = ((rgb[j] * a + dst[j] as u32 * inv + 127) / 255).min(255) as u8;
    }
    dst[3] = (a + (dst[3] as u32 * inv + 127) / 255).min(255) as u8;
}

/// Anti-aliased fill of the device rectangle `[x0,x1)×[y0,y1)`, clipped.
fn fill_rect_aa(pm: &mut Pixmap, x0: f32, y0: f32, x1: f32, y1: f32, c: Color, clip: Clip) {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let x0 = x0.max(clip.l).max(0.0);
    let y0 = y0.max(clip.t).max(0.0);
    let x1 = x1.min(clip.r).min(w);
    let y1 = y1.min(clip.b).min(h);
    if !(x0 < x1 && y0 < y1) || !c.is_visible() {
        return;
    }
    let ca = (c.a.clamp(0.0, 1.0) * 255.0).round() as u32;
    let rgb = [c.r as u32, c.g as u32, c.b as u32];
    let pw = pm.width() as usize;
    let dst = pm.data_mut();
    // Columns: partial left pixel, full interior, partial right pixel.
    let (px0, px1) = (x0.floor() as usize, x1.ceil() as usize);
    let (in0, in1) = (
        x0.ceil() as usize,
        (x1.floor() as usize).max(x0.ceil() as usize),
    );
    let col_cov = |px: usize| {
        let fx = px as f32;
        ((fx + 1.0).min(x1) - fx.max(x0)).clamp(0.0, 1.0)
    };
    let solid = [rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, 255];
    for py in y0.floor() as usize..y1.ceil() as usize {
        let fy = py as f32;
        let cy = ((fy + 1.0).min(y1) - fy.max(y0)).clamp(0.0, 1.0);
        let row = py * pw;
        let a_row = (cy * ca as f32 + 0.5) as u32;
        if a_row == 0 {
            continue;
        }
        let edge = |px: usize, dst: &mut [u8]| {
            let a = (col_cov(px) * cy * ca as f32 + 0.5) as u32;
            if a > 0 {
                blend(&mut dst[(row + px) * 4..(row + px) * 4 + 4], rgb, a);
            }
        };
        if px0 < in0.min(px1) {
            edge(px0, dst);
        }
        let interior = &mut dst[(row + in0.min(px1)) * 4..(row + in1.min(px1)) * 4];
        if a_row >= 255 {
            for px in interior.as_chunks_mut::<4>().0 {
                *px = solid;
            }
        } else {
            for px in interior.as_chunks_mut::<4>().0 {
                blend(px, rgb, a_row);
            }
        }
        if in1 < px1 && in1 >= in0 && (in1 > px0 || px0 >= in0) {
            edge(in1, dst);
        }
    }
}

/// `v / 255` rounded, for `v <= 255 * 255`.
#[inline]
fn div255(v: u32) -> u32 {
    (v + 128 + ((v + 128) >> 8)) >> 8
}

/// Source-over blit of a premultiplied sprite at integer position.
fn blit(pm: &mut Pixmap, sprite: &Pixmap, x: i32, y: i32, clip: Clip) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let (sw, sh) = (sprite.width() as i32, sprite.height() as i32);
    // Visible part of the sprite.
    let (sx0, sy0) = ((-x).max(0), (-y).max(0));
    let (sx1, sy1) = ((w - x).min(sw), (h - y).min(sh));
    if sx0 >= sx1 || sy0 >= sy1 {
        return;
    }
    // Entirely inside the clip: no per-pixel clip coverage.
    let inside = (x + sx0) as f32 >= clip.l
        && (x + sx1) as f32 <= clip.r
        && (y + sy0) as f32 >= clip.t
        && (y + sy1) as f32 <= clip.b;
    let src = sprite.data();
    let dst = pm.data_mut();
    for sy in sy0..sy1 {
        let py = y + sy;
        let srow = &src[((sy * sw + sx0) * 4) as usize..((sy * sw + sx1) * 4) as usize];
        let d0 = ((py * w + x + sx0) * 4) as usize;
        let drow = &mut dst[d0..d0 + srow.len()];
        for (k, (s, d)) in srow
            .as_chunks::<4>()
            .0
            .iter()
            .zip(drow.as_chunks_mut::<4>().0)
            .enumerate()
        {
            let sa = s[3] as u32;
            if sa == 0 {
                continue;
            }
            if inside {
                if sa == 255 {
                    *d = *s;
                    continue;
                }
                let inv = 255 - sa;
                for c in 0..4 {
                    d[c] = (s[c] as u32 + div255(d[c] as u32 * inv)).min(255) as u8;
                }
            } else {
                let m = clip.cov(x + sx0 + k as i32, py);
                if m == 0 {
                    continue;
                }
                let a = div255(sa * m);
                let inv = 255 - a;
                for c in 0..3 {
                    d[c] = (div255(s[c] as u32 * m) + div255(d[c] as u32 * inv)).min(255) as u8;
                }
                d[3] = (a + div255(d[3] as u32 * inv)).min(255) as u8;
            }
        }
    }
}

/// Blends a solid color through an alpha coverage map.
#[allow(clippy::too_many_arguments)]
fn blit_coverage(
    pm: &mut Pixmap,
    cov: &[u8],
    cw: i32,
    ch: i32,
    x: i32,
    y: i32,
    c: Color,
    clip: Clip,
) {
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let ca = (c.a.clamp(0.0, 1.0) * 255.0).round() as u32;
    let rgb = [c.r as u32, c.g as u32, c.b as u32];
    let dst = pm.data_mut();
    for sy in 0..ch {
        let py = y + sy;
        if py < 0 || py >= h {
            continue;
        }
        for sx in 0..cw {
            let px = x + sx;
            if px < 0 || px >= w {
                continue;
            }
            let k = cov[(sy * cw + sx) as usize] as u32;
            if k == 0 {
                continue;
            }
            let m = clip.cov(px, py);
            let a = k * ca / 255 * m / 255;
            if a == 0 {
                continue;
            }
            let inv = 255 - a;
            let di = ((py * w + px) * 4) as usize;
            for i in 0..3 {
                dst[di + i] = ((rgb[i] * a + dst[di + i] as u32 * inv + 127) / 255).min(255) as u8;
            }
            dst[di + 3] = (a + (dst[di + 3] as u32 * inv + 127) / 255).min(255) as u8;
        }
    }
}

/// Rasterized glyph: coverage, size and offset from the pen position.
#[derive(Clone)]
struct GlyphBitmap {
    cov: Vec<u8>,
    w: i32,
    h: i32,
    dx: i32,
    dy: i32,
}

type GlyphKey = (usize, u16, u32, u8, u8, bool);

thread_local! {
    static GLYPH_BITMAPS: RefCell<HashMap<GlyphKey, Option<GlyphBitmap>>> = RefCell::new(HashMap::default());
}

/// Coverage of a glyph at pixel scale `k` (font units → px) with subpixel
/// offset `(fx, fy)` in quarter pixels.
fn glyph_bitmap(path: &sk::Path, key: GlyphKey, k: f32, bold_px: f32) -> Option<GlyphBitmap> {
    GLYPH_BITMAPS.with(|g| {
        let mut g = g.borrow_mut();
        if g.len() > 50_000 {
            g.clear();
        }
        g.entry(key)
            .or_insert_with(|| {
                let (fx, fy) = (key.3 as f32 / 4.0, key.4 as f32 / 4.0);
                let b = path.bounds();
                let pad = 2.0 + bold_px;
                let left = (b.left() * k + fx - pad).floor();
                let top = (b.top() * k + fy - pad).floor();
                let w = ((b.right() * k + fx + pad).ceil() - left).max(1.0) as u32;
                let h = ((b.bottom() * k + fy + pad).ceil() - top).max(1.0) as u32;
                let mut pm = Pixmap::new(w, h)?;
                let ts = Transform::from_row(k, 0.0, 0.0, k, fx - left, fy - top);
                let p = paint(Color::BLACK);
                pm.fill_path(path, &p, FillRule::Winding, ts, None);
                if bold_px > 0.0 {
                    let st = sk::Stroke {
                        width: bold_px / k,
                        ..sk::Stroke::default()
                    };
                    pm.stroke_path(path, &p, &st, ts, None);
                }
                let cov = pm
                    .data()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|px| px[3])
                    .collect();
                Some(GlyphBitmap {
                    cov,
                    w: w as i32,
                    h: h as i32,
                    dx: left as i32,
                    dy: top as i32,
                })
            })
            .clone()
    })
}

/// Axis-aligned 2-point segments of a path (grid lines, ticks, frames).
type Segment = ((f64, f64), (f64, f64));

fn axis_aligned_segments(cmds: &[PathCmd]) -> Option<Vec<Segment>> {
    if !cmds.len().is_multiple_of(2) {
        return None;
    }
    cmds.chunks(2)
        .map(|c| match (c[0].clone(), c[1].clone()) {
            (PathCmd::MoveTo(a, b), PathCmd::LineTo(x, y)) if a == x || b == y => {
                Some(((a, b), (x, y)))
            }
            _ => None,
        })
        .collect()
}

/// Fills a straight-edged path (MoveTo/LineTo/Close, nonzero rule) with
/// signed-area accumulation: each edge adds its exact area contribution to
/// the cells it crosses, a running sum per row gives the coverage. Cost is
/// linear in edge length plus the bounding box, independent of how jagged
/// the outline is. Returns `false` when the path is not eligible.
fn fill_polygon(pm: &mut Pixmap, cmds: &[PathCmd], c: Color, ts: Transform, clip: Clip) -> bool {
    if cmds.len() < 16
        || ts.kx != 0.0
        || ts.ky != 0.0
        || !cmds.iter().all(|c| {
            matches!(
                c,
                PathCmd::MoveTo(..) | PathCmd::LineTo(..) | PathCmd::Close
            )
        })
    {
        return false;
    }
    if !c.is_visible() {
        return true;
    }
    // Device-space points, subpaths closed implicitly.
    let mut edges: Vec<((f32, f32), (f32, f32))> = Vec::with_capacity(cmds.len());
    let (mut start, mut cur) = (None::<(f32, f32)>, (0.0f32, 0.0f32));
    let (mut l, mut t, mut r, mut b) = (
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );
    for cmd in cmds {
        match *cmd {
            PathCmd::MoveTo(x, y) | PathCmd::LineTo(x, y) => {
                let p = (x as f32 * ts.sx + ts.tx, y as f32 * ts.sy + ts.ty);
                if !(p.0.is_finite() && p.1.is_finite()) {
                    return false;
                }
                (l, t, r, b) = (l.min(p.0), t.min(p.1), r.max(p.0), b.max(p.1));
                match (cmd, start) {
                    (PathCmd::MoveTo(..), Some(s0)) => {
                        edges.push((cur, s0));
                        start = Some(p);
                    }
                    (PathCmd::MoveTo(..), None) | (_, None) => start = Some(p),
                    _ => edges.push((cur, p)),
                }
                cur = p;
            }
            _ => {
                if let Some(s0) = start {
                    edges.push((cur, s0));
                    cur = s0;
                }
            }
        }
    }
    if let Some(s0) = start {
        edges.push((cur, s0));
    }
    // Pixel region: bounding box ∩ clip ∩ canvas.
    let x0 = l.max(clip.l).max(0.0).floor() as i32;
    let y0 = t.max(clip.t).max(0.0).floor() as i32;
    let x1 = (r.min(clip.r).min(pm.width() as f32).ceil() as i32).max(x0);
    let y1 = (b.min(clip.b).min(pm.height() as f32).ceil() as i32).max(y0);
    let (bw, bh) = ((x1 - x0) as usize, (y1 - y0) as usize);
    if bw == 0 || bh == 0 {
        return true;
    }
    let stride = bw + 2;
    COVERAGE.with(|cv| {
        let mut cv = cv.borrow_mut();
        let acc = &mut cv.acc;
        acc.clear();
        acc.resize(stride * bh, 0.0);
        let (fx0, fy0, fw) = (x0 as f32, y0 as f32, bw as f32);
        for &(p0, p1) in &edges {
            let (p0, p1) = ((p0.0 - fx0, p0.1 - fy0), (p1.0 - fx0, p1.1 - fy0));
            if p0.1 == p1.1 {
                continue;
            }
            let (dir, p0, p1) = if p0.1 < p1.1 {
                (1.0f32, p0, p1)
            } else {
                (-1.0, p1, p0)
            };
            if p1.1 <= 0.0 || p0.1 >= bh as f32 {
                continue;
            }
            let dxdy = (p1.0 - p0.0) / (p1.1 - p0.1);
            let ystart = p0.1.max(0.0);
            let mut x = p0.0 + (ystart - p0.1) * dxdy;
            let yend = p1.1.min(bh as f32);
            for y in ystart.floor() as usize..(yend.ceil() as usize).min(bh) {
                let row = y * stride;
                let dy = ((y + 1) as f32).min(yend) - (y as f32).max(ystart);
                let xnext = x + dxdy * dy;
                let d = dy * dir;
                // Clamp horizontally: left of the region counts at 0, right of
                // it lands in the padding cells.
                let (a, bb) = (x.clamp(0.0, fw + 0.5), xnext.clamp(0.0, fw + 0.5));
                let (xa, xb) = if a < bb { (a, bb) } else { (bb, a) };
                let xa_f = xa.floor();
                let xai = xa_f as usize;
                let xbi = xb.ceil() as usize;
                if xbi <= xai + 1 {
                    let xmf = 0.5 * (xa + xb) - xa_f;
                    acc[row + xai] += d - d * xmf;
                    acc[row + xai + 1] += d * xmf;
                } else {
                    let s = (xb - xa).recip();
                    let x0f = xa - xa_f;
                    let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
                    let x1f = xb - xb.ceil() + 1.0;
                    let am = 0.5 * s * x1f * x1f;
                    acc[row + xai] += d * a0;
                    if xbi == xai + 2 {
                        acc[row + xai + 1] += d * (1.0 - a0 - am);
                    } else {
                        let a1 = s * (1.5 - x0f);
                        acc[row + xai + 1] += d * (a1 - a0);
                        for v in &mut acc[row + xai + 2..row + xbi - 1] {
                            *v += d * s;
                        }
                        let a2 = a1 + (xbi - xai - 3) as f32 * s;
                        acc[row + xbi - 1] += d * (1.0 - a2 - am);
                    }
                    acc[row + xbi] += d * am;
                }
                x = xnext;
            }
        }
        let ca = c.a.clamp(0.0, 1.0) * 255.0;
        let rgb = [c.r as u32, c.g as u32, c.b as u32];
        let pw = pm.width() as usize;
        let dst = pm.data_mut();
        for y in 0..bh {
            let py = y0 + y as i32;
            let mut sum = 0.0f32;
            let row = &acc[y * stride..y * stride + bw];
            let drow = (py as usize * pw + x0 as usize) * 4;
            for (i, v) in row.iter().enumerate() {
                sum += v;
                let k = sum.abs().min(1.0);
                if k < 1.0 / 512.0 {
                    continue;
                }
                let px = x0 + i as i32;
                let m = clip.cov(px, py);
                let a = (k * ca * m as f32 / 255.0 + 0.5) as u32;
                if a > 0 {
                    blend(&mut dst[drow + i * 4..drow + i * 4 + 4], rgb, a);
                }
            }
        }
    });
    true
}

/// Reusable coverage buffer for [`stroke_polyline`].
#[derive(Default)]
struct Coverage {
    cov: Vec<u8>,
    acc: Vec<f32>,
    span: Vec<(i32, i32)>,
}

thread_local! {
    static COVERAGE: RefCell<Coverage> = RefCell::new(Coverage::default());
}

/// Draws a solid, round-joined polyline (MoveTo/LineTo only, at least 8
/// segments) directly: anti-aliased distance coverage per segment, merged
/// with `max` (round joins, no double blending) and blended once.
///
/// Much cheaper than generic stroking for long, jagged data lines. Returns
/// `false` when the path is not eligible.
fn stroke_polyline(
    pm: &mut Pixmap,
    cmds: &[PathCmd],
    s: &Stroke,
    ts: Transform,
    clip: Clip,
) -> bool {
    if cmds.len() < 9
        || !s.dash.is_empty()
        || s.join != LineJoin::Round
        || ts.kx != 0.0
        || ts.ky != 0.0
        || (ts.sx - ts.sy).abs() > 1e-6
        || !cmds
            .iter()
            .all(|c| matches!(c, PathCmd::MoveTo(..) | PathCmd::LineTo(..)))
    {
        return false;
    }
    if !s.color.is_visible() || s.width <= 0.0 {
        return true;
    }
    let (w, h) = (pm.width() as i32, pm.height() as i32);
    let wpx = (s.width * ts.sx as f64) as f32;
    // Lines thinner than a pixel: 1 px wide with proportionally less alpha.
    let (r, fade) = if wpx < 1.0 {
        (0.5, wpx)
    } else {
        (wpx / 2.0, 1.0)
    };
    let reach = r + 1.0;
    let pt = |x: f64, y: f64| (x as f32 * ts.sx + ts.tx, y as f32 * ts.sy + ts.ty);

    COVERAGE.with(|c| {
        let mut c = c.borrow_mut();
        let Coverage { cov, span, .. } = &mut *c;
        cov.resize((w * h) as usize, 0);
        span.clear();
        span.resize(h as usize, (i32::MAX, i32::MIN));

        // Subpaths as index ranges of points.
        let mut pts: Vec<(f32, f32)> = Vec::with_capacity(cmds.len());
        let mut starts = Vec::new();
        for cmd in cmds {
            match *cmd {
                PathCmd::MoveTo(x, y) => {
                    starts.push(pts.len());
                    pts.push(pt(x, y));
                }
                PathCmd::LineTo(x, y) => pts.push(pt(x, y)),
                _ => unreachable!(),
            }
        }
        if starts.first() != Some(&0) {
            starts.insert(0, 0);
        }
        starts.push(pts.len());

        for sub in starts.windows(2) {
            let sp = &pts[sub[0]..sub[1]];
            let n = sp.len();
            for i in 0..n.saturating_sub(1) {
                let (a, b) = (sp[i], sp[i + 1]);
                if !(a.0.is_finite() && a.1.is_finite() && b.0.is_finite() && b.1.is_finite()) {
                    continue;
                }
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let len = (dx * dx + dy * dy).sqrt();
                if len < 1e-6 && n > 2 {
                    continue;
                }
                let len = len.max(1e-6);
                let (ux, uy) = (dx / len, dy / len);
                // Butt/square caps only at the ends of the subpath.
                let ext = |end: bool| match (end, s.cap) {
                    (false, _) | (_, LineCap::Round) => None,
                    (true, LineCap::Butt) => Some(0.0f32),
                    (true, LineCap::Square) => Some(r),
                };
                let (cap0, cap1) = (ext(i == 0), ext(i + 2 == n));
                let y0 = ((a.1.min(b.1) - reach).floor() as i32).max(0);
                let y1 = ((a.1.max(b.1) + reach).ceil() as i32).min(h - 1);
                let (bx0, bx1) = (a.0.min(b.0) - reach, a.0.max(b.0) + reach);
                for py in y0..=y1 {
                    let yc = py as f32 + 0.5;
                    // x-range of the band |lateral| <= reach on this row.
                    let (mut lo, mut hi) = (bx0, bx1);
                    if uy.abs() > 1e-6 {
                        // lateral = (x-ax)*(-uy) + (y-ay)*ux
                        let base = (yc - a.1) * ux;
                        let e0 = a.0 + (base - reach) / uy;
                        let e1 = a.0 + (base + reach) / uy;
                        lo = lo.max(e0.min(e1));
                        hi = hi.min(e0.max(e1));
                    }
                    let x0 = ((lo - 0.5).floor() as i32).max(0);
                    let x1 = ((hi - 0.5).ceil() as i32).min(w - 1);
                    if x0 > x1 {
                        continue;
                    }
                    let row = (py * w) as usize;
                    let sp = &mut span[py as usize];
                    let mut touched = false;
                    for px in x0..=x1 {
                        let (qx, qy) = (px as f32 + 0.5 - a.0, yc - a.1);
                        let along = qx * ux + qy * uy;
                        let lat = (qy * ux - qx * uy).abs();
                        let edge = |d: f32| (r + 0.5 - d).clamp(0.0, 1.0);
                        let k = if along < 0.0 {
                            match cap0 {
                                Some(e) => edge(lat) * (0.5 + along + e).clamp(0.0, 1.0),
                                None => edge((lat * lat + along * along).sqrt()),
                            }
                        } else if along > len {
                            let o = along - len;
                            match cap1 {
                                Some(e) => edge(lat) * (0.5 - o + e).clamp(0.0, 1.0),
                                None => edge((lat * lat + o * o).sqrt()),
                            }
                        } else {
                            edge(lat)
                        };
                        if k > 0.0 {
                            let v = (k * fade * 255.0 + 0.5) as u8;
                            let cell = &mut cov[row + px as usize];
                            if v > *cell {
                                *cell = v;
                            }
                            touched = true;
                        }
                    }
                    if touched {
                        sp.0 = sp.0.min(x0);
                        sp.1 = sp.1.max(x1);
                    }
                }
            }
        }

        // Blend the coverage once and clear it for the next use.
        let ca = (s.color.a.clamp(0.0, 1.0) * 255.0).round() as u32;
        let rgb = [s.color.r as u32, s.color.g as u32, s.color.b as u32];
        let dst = pm.data_mut();
        for (py, &(x0, x1)) in span.iter().enumerate() {
            if x0 > x1 {
                continue;
            }
            let row = py * w as usize;
            for px in x0 as usize..=x1 as usize {
                let i = row + px;
                let k = std::mem::take(&mut cov[i]) as u32;
                if k == 0 {
                    continue;
                }
                let m = clip.cov(px as i32, py as i32);
                let a = (k * ca * m + 255 * 255 / 2) / (255 * 255);
                if a == 0 {
                    continue;
                }
                let inv = 255 - a;
                let di = i * 4;
                for j in 0..3 {
                    dst[di + j] =
                        ((rgb[j] * a + dst[di + j] as u32 * inv + 127) / 255).min(255) as u8;
                }
                dst[di + 3] = (a + (dst[di + 3] as u32 * inv + 127) / 255).min(255) as u8;
            }
        }
    });
    true
}

/// Fills and/or strokes a path with tiny-skia.
fn fill_stroke_path(
    pm: &mut Pixmap,
    path: &sk::Path,
    fill: Option<Color>,
    stroke: &Option<Stroke>,
    ts: Transform,
    mask: Option<&Mask>,
) {
    if let Some(f) = fill.filter(|c| c.is_visible()) {
        pm.fill_path(path, &paint(f), FillRule::Winding, ts, mask);
    }
    if let Some(s) = stroke
        .as_ref()
        .filter(|s| s.color.is_visible() && s.width > 0.0)
    {
        pm.stroke_path(path, &paint(s.color), &sk_stroke(s), ts, mask);
    }
}

struct Ctx<'a> {
    fonts: &'a FontSet,
    font_key: usize,
    base: Transform,
    /// Circle sprites: (r, fill, stroke) → pixmap and its centre offset.
    sprites: Vec<(SpriteKey, Pixmap, f32)>,
    /// Index of the last used sprite (markers usually repeat).
    last_sprite: usize,
    /// Last circle key and whether it is drawn as a sprite.
    last_count: Option<(CircleKey, bool)>,
    /// Current clip (device pixels).
    clip: Clip,
    /// Mask of `clip` for tiny-skia calls, built on demand.
    mask: Option<(Clip, Mask)>,
}

type CircleKey = (u32, u32, u32, u32);
type SpriteKey = (CircleKey, u8, u8);

fn circle_key(r: f64, fill: Option<Color>, stroke: &Option<Stroke>) -> Option<CircleKey> {
    if stroke.as_ref().is_some_and(|s| !s.dash.is_empty()) {
        return None;
    }
    let c = |c: Color| u32::from_be_bytes([c.r, c.g, c.b, (c.a * 255.0) as u8]);
    Some((
        (r * 64.0).round() as u32,
        fill.map_or(0, c),
        stroke.as_ref().map_or(0, |s| c(s.color)),
        stroke
            .as_ref()
            .map_or(0, |s| (s.width * 64.0).round() as u32),
    ))
}

impl Ctx<'_> {
    fn draw(
        &mut self,
        pm: &mut Pixmap,
        nodes: &[Node],
        circle_counts: &HashMap<(u32, u32, u32, u32), usize>,
    ) {
        for n in nodes {
            self.node(pm, n, circle_counts);
        }
    }

    /// Runs `f` with a mask for the current clip (None when nothing is
    /// clipped). The mask is only built for tiny-skia drawing calls and
    /// cached until the clip changes.
    fn with_mask<R>(
        &mut self,
        pm: &mut Pixmap,
        f: impl FnOnce(&Self, &mut Pixmap, Option<&Mask>) -> R,
    ) -> R {
        let full = Clip::new(0.0, 0.0, pm.width() as f32, pm.height() as f32);
        if self.clip.intersect(full) == full {
            return f(self, pm, None);
        }
        if self.mask.as_ref().is_none_or(|(c, m)| {
            *c != self.clip || m.width() != pm.width() || m.height() != pm.height()
        }) {
            let c = self.clip;
            self.mask = Mask::new(pm.width(), pm.height()).map(|mut m| {
                if let Some(r) =
                    sk::Rect::from_ltrb(c.l, c.t, c.r.max(c.l + 1e-3), c.b.max(c.t + 1e-3))
                {
                    m.fill_path(
                        &PathBuilder::from_rect(r),
                        FillRule::Winding,
                        true,
                        Transform::identity(),
                    );
                }
                (c, m)
            });
        }
        let cache = self.mask.take();
        let r = f(self, pm, cache.as_ref().map(|(_, m)| m));
        self.mask = cache;
        r
    }

    /// Solid axis-aligned rectangle (user space, scale/translate transform).
    fn fill_rect(&self, pm: &mut Pixmap, r: sk::Rect, c: Color, ts: Transform) {
        let (l, t) = (r.left() * ts.sx + ts.tx, r.top() * ts.sy + ts.ty);
        let (rr, b) = (r.right() * ts.sx + ts.tx, r.bottom() * ts.sy + ts.ty);
        fill_rect_aa(pm, l.min(rr), t.min(b), l.max(rr), t.max(b), c, self.clip);
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_stroke(
        &self,
        pm: &mut Pixmap,
        path: &sk::Path,
        fill: Option<Color>,
        stroke: &Option<Stroke>,
        ts: Transform,
        mask: Option<&Mask>,
    ) {
        fill_stroke_path(pm, path, fill, stroke, ts, mask);
    }

    fn node(&mut self, pm: &mut Pixmap, n: &Node, counts: &HashMap<(u32, u32, u32, u32), usize>) {
        let ts = self.base;
        match n {
            Node::Path { cmds, fill, stroke } => {
                if cmds.len() < 2 {
                    return;
                }
                // Axis-aligned solid strokes (grid, ticks, frame) as rectangles.
                if let (None, Some(s), true) = (
                    fill,
                    stroke.as_ref().filter(|s| s.dash.is_empty()),
                    ts.kx == 0.0 && ts.ky == 0.0,
                ) {
                    if let Some(segs) = axis_aligned_segments(cmds) {
                        if !s.color.is_visible() || s.width <= 0.0 {
                            return;
                        }
                        let hw = s.width / 2.0;
                        let ext = if s.cap == LineCap::Butt { 0.0 } else { hw };
                        for ((a, b), (x, y)) in segs {
                            let (x0, x1) = (a.min(x), a.max(x));
                            let (y0, y1) = (b.min(y), b.max(y));
                            let r = if b == y {
                                sk::Rect::from_ltrb(
                                    (x0 - ext) as f32,
                                    (y0 - hw) as f32,
                                    (x1 + ext) as f32,
                                    (y0 + hw) as f32,
                                )
                            } else {
                                sk::Rect::from_ltrb(
                                    (x0 - hw) as f32,
                                    (y0 - ext) as f32,
                                    (x0 + hw) as f32,
                                    (y1 + ext) as f32,
                                )
                            };
                            if let Some(r) = r {
                                self.fill_rect(pm, r, s.color, ts);
                            }
                        }
                        return;
                    }
                }
                // Straight-edged fills (areas, envelopes) and polylines
                // without tiny-skia.
                let filled = match fill.filter(|c| c.is_visible()) {
                    Some(c) => fill_polygon(pm, cmds, c, ts, self.clip),
                    None => true,
                };
                if filled {
                    let stroked = match stroke
                        .as_ref()
                        .filter(|s| s.color.is_visible() && s.width > 0.0)
                    {
                        Some(s) => stroke_polyline(pm, cmds, s, ts, self.clip),
                        None => true,
                    };
                    if stroked {
                        return;
                    }
                    if let Some(p) = build_path(cmds) {
                        self.with_mask(pm, |c, pm, m| c.fill_stroke(pm, &p, None, stroke, ts, m));
                    }
                    return;
                }
                if let Some(p) = build_path(cmds) {
                    self.with_mask(pm, |c, pm, m| c.fill_stroke(pm, &p, *fill, stroke, ts, m));
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
                if *rx <= 0.0 && stroke.is_none() {
                    if let (Some(f), Some(r)) = (
                        fill.filter(|c| c.is_visible()),
                        sk::Rect::from_xywh(
                            x as f32,
                            y as f32,
                            w.max(0.01) as f32,
                            h.max(0.01) as f32,
                        ),
                    ) {
                        self.fill_rect(pm, r, f, ts);
                    }
                    return;
                }
                // Square rect with a thin solid border: fill + 4 border strips.
                let thin = stroke
                    .as_ref()
                    .filter(|s| s.dash.is_empty() && (s.join == LineJoin::Miter || s.width <= 2.0));
                if let (true, Some(s), true) = (*rx <= 0.0, thin, ts.kx == 0.0 && ts.ky == 0.0) {
                    let rect = |x: f64, y: f64, w: f64, h: f64| {
                        sk::Rect::from_xywh(x as f32, y as f32, w as f32, h as f32)
                    };
                    let hw = s.width / 2.0;
                    if let Some(f) = fill.filter(|c| c.is_visible()) {
                        if let Some(r) = rect(x, y, w.max(0.01), h.max(0.01)) {
                            self.fill_rect(pm, r, f, ts);
                        }
                    }
                    if s.color.is_visible() && s.width > 0.0 {
                        let (ow, oh) = (w + s.width, h + s.width);
                        let strips = if w <= s.width || h <= s.width {
                            vec![(x - hw, y - hw, ow, oh)]
                        } else {
                            vec![
                                (x - hw, y - hw, ow, s.width),
                                (x - hw, y + h - hw, ow, s.width),
                                (x - hw, y + hw, s.width, h - s.width),
                                (x + w - hw, y + hw, s.width, h - s.width),
                            ]
                        };
                        for (a, b, c, d) in strips {
                            if let Some(r) = rect(a, b, c, d) {
                                self.fill_rect(pm, r, s.color, ts);
                            }
                        }
                    }
                    return;
                }
                if let Some(p) = rounded_rect(
                    x as f32,
                    y as f32,
                    w.max(0.01) as f32,
                    h.max(0.01) as f32,
                    *rx as f32,
                ) {
                    self.with_mask(pm, |c, pm, m| c.fill_stroke(pm, &p, *fill, stroke, ts, m));
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
                let key = circle_key(*r, *fill, stroke);
                let many = match (key, self.last_count) {
                    (Some(k), Some((lk, m))) if k == lk => m,
                    (Some(k), _) => {
                        let m = counts.get(&k).is_some_and(|c| *c >= 32);
                        self.last_count = Some((k, m));
                        m
                    }
                    (None, _) => false,
                };
                let scale = ts.sx;
                if many && ts.kx == 0.0 && ts.ky == 0.0 {
                    let key = key.expect("checked");
                    // Sprites at quarter-pixel offsets keep sub-pixel accuracy.
                    let gx = *cx as f32 * scale + ts.tx;
                    let gy = *cy as f32 * scale + ts.ty;
                    let (ix, iy) = (gx.floor(), gy.floor());
                    let q = |f: f32| ((f * 4.0) as u8).min(3);
                    let skey = (key, q(gx - ix), q(gy - iy));
                    let hit = self
                        .sprites
                        .get(self.last_sprite)
                        .is_some_and(|s| s.0 == skey);
                    if !hit {
                        self.last_sprite = match self.sprites.iter().position(|s| s.0 == skey) {
                            Some(i) => i,
                            None => {
                                let pad = stroke.as_ref().map_or(0.0, |s| s.width) + 1.0;
                                let half = ((*r + pad) * scale as f64).ceil() as f32;
                                let size = (2.0 * half) as u32 + 2;
                                let mut sp = Pixmap::new(size, size).expect("sprite size");
                                let (ox, oy) =
                                    ((skey.1 as f32 + 0.5) / 4.0, (skey.2 as f32 + 0.5) / 4.0);
                                if let Some(p) = PathBuilder::from_circle(
                                    half + ox,
                                    half + oy,
                                    (*r * scale as f64) as f32,
                                ) {
                                    let st = stroke.as_ref().map(|s| Stroke {
                                        width: s.width * scale as f64,
                                        ..s.clone()
                                    });
                                    fill_stroke_path(
                                        &mut sp,
                                        &p,
                                        *fill,
                                        &st,
                                        Transform::identity(),
                                        None,
                                    );
                                }
                                self.sprites.push((skey, sp, half));
                                self.sprites.len() - 1
                            }
                        };
                    }
                    let (_, sprite, half) = &self.sprites[self.last_sprite];
                    let half = *half as i32;
                    blit(pm, sprite, ix as i32 - half, iy as i32 - half, self.clip);
                } else if let Some(p) = PathBuilder::from_circle(*cx as f32, *cy as f32, *r as f32)
                {
                    self.with_mask(pm, |c, pm, m| c.fill_stroke(pm, &p, *fill, stroke, ts, m));
                }
            }
            Node::Text(t) if t.rotate == 0.0 => self.text(pm, t, None),
            Node::Text(t) => self.with_mask(pm, |c, pm, m| c.text(pm, t, m)),
            Node::Image {
                x,
                y,
                w,
                h,
                image,
                smooth,
            } => self.with_mask(pm, |c, pm, m| {
                c.image(pm, *x, *y, *w, *h, image, *smooth, m)
            }),
            Node::Group {
                children,
                clip,
                opacity,
                ..
            } => {
                let saved = self.clip;
                if let Some((x, y, w, h)) = *clip {
                    let (l, t) = (x as f32 * ts.sx + ts.tx, y as f32 * ts.sy + ts.ty);
                    let (r, b) = (l + w.max(0.0) as f32 * ts.sx, t + h.max(0.0) as f32 * ts.sy);
                    self.clip = self.clip.intersect(Clip::new(l, t, r, b));
                }
                if *opacity < 1.0 {
                    if let Some(mut layer) = Pixmap::new(pm.width(), pm.height()) {
                        self.draw(&mut layer, children, counts);
                        let p = PixmapPaint {
                            opacity: opacity.clamp(0.0, 1.0) as f32,
                            ..PixmapPaint::default()
                        };
                        pm.draw_pixmap(0, 0, layer.as_ref(), &p, Transform::identity(), None);
                    }
                } else {
                    self.draw(pm, children, counts);
                }
                self.clip = saved;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn image(
        &self,
        pm: &mut Pixmap,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        img: &RasterImage,
        smooth: bool,
        mask: Option<&Mask>,
    ) {
        if img.width == 0 || img.height == 0 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let Some(mut src) = Pixmap::new(img.width, img.height) else {
            return;
        };
        for (dst, s) in src
            .data_mut()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(img.rgba.as_chunks::<4>().0)
        {
            let a = s[3] as u16;
            *dst = [
                ((s[0] as u16 * a + 127) / 255) as u8,
                ((s[1] as u16 * a + 127) / 255) as u8,
                ((s[2] as u16 * a + 127) / 255) as u8,
                s[3],
            ];
        }
        let quality = if smooth {
            sk::FilterQuality::Bicubic
        } else {
            sk::FilterQuality::Nearest
        };
        let to_dest = Transform::from_row(
            (w / img.width as f64) as f32,
            0.0,
            0.0,
            (h / img.height as f64) as f32,
            x as f32,
            y as f32,
        );
        let shader = sk::Pattern::new(src.as_ref(), sk::SpreadMode::Pad, quality, 1.0, to_dest);
        let p = Paint {
            shader,
            anti_alias: false,
            ..Paint::default()
        };
        if let Some(r) = sk::Rect::from_xywh(x as f32, y as f32, w as f32, h as f32) {
            pm.fill_rect(r, &p, self.base, mask);
        }
    }

    fn text(&self, pm: &mut Pixmap, t: &TextNode, mask: Option<&Mask>) {
        let Some(font) = &self.fonts.font else { return };
        if t.text.is_empty()
            || t.size <= 0.0
            || !t.x.is_finite()
            || !t.y.is_finite()
            || !t.color.is_visible()
        {
            return;
        }
        let width = self.fonts.width(&t.text, t.size);
        let x0 = match t.anchor {
            Anchor::Start => t.x,
            Anchor::Middle => t.x - width / 2.0,
            Anchor::End => t.x - width,
        };
        // Same baseline offsets as the SVG writer.
        let dy = match t.baseline {
            Baseline::Alphabetic => 0.0,
            Baseline::Top => t.size * 0.76,
            Baseline::Middle => t.size * 0.35,
            Baseline::Bottom => -t.size * 0.24,
        };
        let baseline = t.y + dy;
        let scale = self.fonts.scale(t.size);
        let sf = font.as_scaled(scale);
        let k = sf.h_scale_factor();
        let mut ts = self.base;
        if t.rotate != 0.0 {
            ts = ts.pre_concat(Transform::from_rotate_at(
                t.rotate as f32,
                t.x as f32,
                t.y as f32,
            ));
        }
        let p = paint(t.color);
        let mut pen = x0 as f32;
        let mut prev = None;
        let simple = t.rotate == 0.0 && self.base.kx == 0.0 && self.base.ky == 0.0;
        for ch in t.text.chars() {
            let id = font.glyph_id(ch);
            if let Some(pv) = prev {
                pen += sf.kern(pv, id);
            }
            if simple {
                // Cached coverage bitmaps at quarter-pixel positions.
                let (s, tx, ty) = (self.base.sx, self.base.tx, self.base.ty);
                let gx = pen * s + tx;
                let gy = baseline as f32 * s + ty;
                let (ix, iy) = (gx.floor(), gy.floor());
                let (fx, fy) = (
                    ((gx - ix) * 4.0).floor() as u8,
                    ((gy - iy) * 4.0).floor() as u8,
                );
                let bold_px = if t.bold {
                    t.size as f32 * 0.07 * s
                } else {
                    0.0
                };
                let key: GlyphKey = (self.font_key, id.0, (k * s).to_bits(), fx, fy, t.bold);
                if let Some(path) = glyph_path(font, self.font_key, id) {
                    if let Some(g) = glyph_bitmap(&path, key, k * s, bold_px) {
                        blit_coverage(
                            pm,
                            &g.cov,
                            g.w,
                            g.h,
                            ix as i32 + g.dx,
                            iy as i32 + g.dy,
                            t.color,
                            self.clip,
                        );
                    }
                }
                pen += sf.h_advance(id);
                prev = Some(id);
                continue;
            }
            if let Some(path) = glyph_path(font, self.font_key, id) {
                let gt = ts.pre_concat(Transform::from_row(k, 0.0, 0.0, k, pen, baseline as f32));
                pm.fill_path(&path, &p, FillRule::Winding, gt, mask);
                if t.bold {
                    let st = sk::Stroke {
                        width: (t.size as f32 * 0.07) / k,
                        ..sk::Stroke::default()
                    };
                    pm.stroke_path(&path, &p, &st, gt, mask);
                }
            }
            pen += sf.h_advance(id);
            prev = Some(id);
        }
    }
}

fn count_circles(nodes: &[Node], out: &mut HashMap<(u32, u32, u32, u32), usize>) {
    for n in nodes {
        match n {
            Node::Circle {
                r, fill, stroke, ..
            } => {
                if let Some(k) = circle_key(*r, *fill, stroke) {
                    *out.entry(k).or_default() += 1;
                }
            }
            Node::Group { children, .. } => count_circles(children, out),
            _ => {}
        }
    }
}

/// Renders `scene` at `scale` into `pixmap` (resized if needed).
pub(crate) fn render_pixmap(
    scene: &Scene,
    fonts: &FontSet,
    scale: f64,
    pixmap: &mut Option<Pixmap>,
) -> Option<()> {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let w = (scene.width * scale).round().max(1.0) as u32;
    let h = (scene.height * scale).round().max(1.0) as u32;
    if pixmap
        .as_ref()
        .is_none_or(|p| p.width() != w || p.height() != h)
    {
        *pixmap = Pixmap::new(w, h);
    }
    let pm = pixmap.as_mut()?;
    pm.fill(scene.background.map_or(sk::Color::TRANSPARENT, color));
    let mut counts = HashMap::default();
    count_circles(&scene.nodes, &mut counts);
    let font_key = fonts.font.as_ref().map_or(0, |f| f as *const _ as usize);
    let mut ctx = Ctx {
        fonts,
        font_key,
        base: Transform::from_scale(scale as f32, scale as f32),
        sprites: Vec::new(),
        last_sprite: 0,
        last_count: None,
        clip: Clip::new(0.0, 0.0, pm.width() as f32, pm.height() as f32),
        mask: None,
    };
    ctx.draw(pm, &scene.nodes, &counts);
    Some(())
}

/// Converts a premultiplied pixmap into `0x00RRGGBB` composited on white.
pub(crate) fn to_0rgb(pm: &Pixmap, out: &mut Vec<u32>) {
    out.clear();
    out.extend(pm.data().as_chunks::<4>().0.iter().map(|p| {
        let inv = 255 - p[3] as u32;
        let c = |v: u8| (v as u32 + inv).min(255);
        c(p[0]) << 16 | c(p[1]) << 8 | c(p[2])
    }));
}

/// Copies a premultiplied pixmap into straight-alpha RGBA.
pub(crate) fn to_pixels(pm: &Pixmap, out: &mut Pixels) {
    out.width = pm.width();
    out.height = pm.height();
    let data = pm.data();
    out.rgba.resize(data.len(), 0);
    for (dst, s) in out
        .rgba
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(data.as_chunks::<4>().0)
    {
        let a = s[3];
        *dst = if a == 255 || a == 0 {
            *s
        } else {
            let un = |c: u8| ((c as u16 * 255 + a as u16 / 2) / a as u16).min(255) as u8;
            [un(s[0]), un(s[1]), un(s[2]), a]
        };
    }
}

/// Rasterizes a scene into straight-alpha RGBA pixels.
pub(crate) fn rasterize(scene: &Scene, fonts: &FontSet, scale: f64) -> Option<Pixels> {
    let mut pm = None;
    render_pixmap(scene, fonts, scale, &mut pm)?;
    let mut px = Pixels {
        width: 0,
        height: 0,
        rgba: Vec::new(),
    };
    to_pixels(pm.as_ref()?, &mut px);
    Some(px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_fill_matches_tiny_skia() {
        let mut seed = 12345u64;
        let mut rnd = |lo: f64, hi: f64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            lo + (seed % 10_000) as f64 / 10_000.0 * (hi - lo)
        };
        let c = Color::rgba(20, 120, 200, 0.8);
        for case in 0..30 {
            // Jagged, self-intersecting, partly off-canvas polygons; two subpaths.
            let mut cmds = vec![PathCmd::MoveTo(rnd(-20.0, 80.0), rnd(-20.0, 80.0))];
            for _ in 0..20 {
                cmds.push(PathCmd::LineTo(rnd(-20.0, 80.0), rnd(-20.0, 80.0)));
            }
            if case % 2 == 0 {
                cmds.push(PathCmd::Close);
                cmds.push(PathCmd::MoveTo(rnd(0.0, 60.0), rnd(0.0, 60.0)));
                for _ in 0..5 {
                    cmds.push(PathCmd::LineTo(rnd(0.0, 60.0), rnd(0.0, 60.0)));
                }
            }
            let clip = Clip::new(3.5, 2.0, 55.0, 58.25);
            let mut a = Pixmap::new(60, 60).unwrap();
            a.fill(sk::Color::WHITE);
            let mut b = a.clone();
            assert!(fill_polygon(&mut a, &cmds, c, Transform::identity(), clip));
            let mut m = Mask::new(60, 60).unwrap();
            let r = sk::Rect::from_ltrb(clip.l, clip.t, clip.r, clip.b).unwrap();
            m.fill_path(
                &PathBuilder::from_rect(r),
                FillRule::Winding,
                true,
                Transform::identity(),
            );
            let path = build_path(&cmds).unwrap();
            b.fill_path(
                &path,
                &paint(c),
                FillRule::Winding,
                Transform::identity(),
                Some(&m),
            );
            let (mut sum, mut big) = (0i64, 0usize);
            for (p, q) in a.data().iter().zip(b.data()) {
                let d = (*p as i32 - *q as i32).abs();
                sum += d as i64;
                big += (d > 48) as usize;
            }
            let mean = sum as f64 / a.data().len() as f64;
            // Exact area coverage vs supersampling: small differences, larger
            // ones only in single pixels where self-intersecting edges cross.
            assert!(mean < 1.0 && big < 40, "case {case}: mean {mean} big {big}");
        }
    }

    #[test]
    fn rect_fill_matches_tiny_skia() {
        let c = Color::rgba(200, 40, 90, 0.7);
        let full = Clip::new(0.0, 0.0, 20.0, 20.0);
        for &(x0, y0, x1, y1) in &[
            (2.3f32, 3.6f32, 2.8f32, 9.1f32),
            (1.0, 1.0, 7.0, 7.0),
            (0.5, 0.25, 15.75, 3.5),
            (-3.0, 4.2, 30.0, 4.9),
            (5.5, 5.5, 6.5, 6.5),
        ] {
            let mut a = Pixmap::new(20, 20).unwrap();
            let mut b = Pixmap::new(20, 20).unwrap();
            a.fill(sk::Color::WHITE);
            b.fill(sk::Color::WHITE);
            fill_rect_aa(&mut a, x0, y0, x1, y1, c, full);
            let r = sk::Rect::from_ltrb(x0, y0, x1, y1).unwrap();
            b.fill_rect(r, &paint(c), Transform::identity(), None);
            for (p, q) in a.data().iter().zip(b.data()) {
                assert!(
                    (*p as i32 - *q as i32).abs() <= 3,
                    "{x0} {y0} {x1} {y1}: {p} vs {q}"
                );
            }
        }
    }

    #[test]
    fn arc_conversion_reaches_endpoint() {
        let mut pb = PathBuilder::new();
        pb.move_to(10.0, 0.0);
        arc_to(&mut pb, (10.0, 0.0), 10.0, 10.0, false, true, (-10.0, 0.0));
        let p = pb.finish().unwrap();
        let b = p.bounds();
        assert!(
            (b.bottom() - 10.0).abs() < 0.1 && b.top().abs() < 0.1,
            "{b:?}"
        );
    }
}
