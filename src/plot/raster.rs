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
    self as sk, FillRule, Mask, Paint, PathBuilder, Pixmap, PixmapMut, PixmapPaint, Transform,
};
use std::cell::RefCell;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::{Arc, LazyLock, RwLock};

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

pub(crate) fn sk_stroke(s: &Stroke) -> sk::Stroke {
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

pub(crate) fn build_path(cmds: &[PathCmd]) -> Option<sk::Path> {
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

pub(crate) fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<sk::Path> {
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

/// A cache shared by the rendering threads (read-mostly).
type SharedCache<K, V> = LazyLock<RwLock<HashMap<K, Option<Arc<V>>>>>;

/// Looks `key` up in `cache`, computing and inserting it on a miss.
fn cached<K: std::hash::Hash + Eq, V>(
    cache: &RwLock<HashMap<K, Option<Arc<V>>>>,
    key: K,
    limit: usize,
    make: impl FnOnce() -> Option<V>,
) -> Option<Arc<V>> {
    if let Some(v) = cache.read().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return v.clone();
    }
    let v = make().map(Arc::new);
    let mut map = cache.write().unwrap_or_else(|e| e.into_inner());
    if map.len() > limit {
        map.clear();
    }
    map.entry(key).or_insert(v).clone()
}

/// Glyph outlines in font units (y up), per font and glyph.
static GLYPHS: SharedCache<(usize, u16), sk::Path> = LazyLock::new(Default::default);

fn glyph_path(font: &ab_glyph::FontArc, key: usize, id: GlyphId) -> Option<Arc<sk::Path>> {
    cached(&GLYPHS, (key, id.0), 20_000, || {
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
                OutlineCurve::Cubic(_, b, c, d) => pb.cubic_to(b.x, -b.y, c.x, -c.y, d.x, -d.y),
            }
            last = Some(end);
        }
        if last.is_some() {
            pb.close();
        }
        pb.finish()
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
fn fill_rect_aa(pm: &mut PixmapMut<'_>, x0: f32, y0: f32, x1: f32, y1: f32, c: Color, clip: Clip) {
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
    // Premultiplied source and inverse alpha for coverage `k` (0..1).
    let src_for = |k: f32| {
        let a = (k * ca as f32 + 0.5) as u32;
        (
            [
                div255(rgb[0] * a),
                div255(rgb[1] * a),
                div255(rgb[2] * a),
                a,
            ],
            255 - a,
        )
    };
    // Columns: partial left pixel, full interior, partial right pixel; their
    // coverage is the same on every row.
    let (px0, px1) = (x0.floor() as usize, x1.ceil() as usize);
    let in0 = (x0.ceil() as usize).min(px1);
    let in1 = (x1.floor() as usize).clamp(in0, px1);
    let col = |px: usize| ((px as f32 + 1.0).min(x1) - (px as f32).max(x0)).clamp(0.0, 1.0);
    let left = (px0 < in0).then(|| col(px0));
    let right = (in1 < px1 && (in1 > px0 || left.is_none())).then(|| col(in1));
    let solid = [rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, 255];
    let put = |p: &mut [u8; 4], (s, inv): ([u32; 4], u32)| {
        for j in 0..4 {
            p[j] = (s[j] + div255(p[j] as u32 * inv)).min(255) as u8;
        }
    };
    let mut row_fill = |py: usize, cy: f32| {
        let row = &mut dst[py * pw * 4..(py + 1) * pw * 4];
        let px = row.as_chunks_mut::<4>().0;
        if let Some(k) = left {
            put(&mut px[px0], src_for(k * cy));
        }
        let interior = &mut px[in0..in1];
        let (s, inv) = src_for(cy);
        if s[3] >= 255 {
            interior.fill(solid);
        } else if s[3] > 0 {
            for p in interior {
                put(p, (s, inv));
            }
        }
        if let Some(k) = right {
            put(&mut px[in1], src_for(k * cy));
        }
    };
    let (ry0, ry1) = (y0.floor() as usize, y1.ceil() as usize);
    let row_cov = |py: usize| ((py as f32 + 1.0).min(y1) - (py as f32).max(y0)).clamp(0.0, 1.0);
    for py in ry0..ry1 {
        let cy = if py > ry0 && py + 1 < ry1 {
            1.0
        } else {
            row_cov(py)
        };
        row_fill(py, cy);
    }
}

/// `v / 255` rounded, for `v <= 255 * 255`.
#[inline]
fn div255(v: u32) -> u32 {
    (v + 128 + ((v + 128) >> 8)) >> 8
}

/// Source-over blit of a premultiplied sprite at integer position.
fn blit(pm: &mut PixmapMut<'_>, sprite: &Pixmap, x: i32, y: i32, clip: Clip) {
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
    pm: &mut PixmapMut<'_>,
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
pub(crate) struct GlyphBitmap {
    pub cov: Vec<u8>,
    pub w: i32,
    pub h: i32,
    pub dx: i32,
    pub dy: i32,
}

pub(crate) type GlyphKey = (usize, u16, u32, u8, u8, bool);

static GLYPH_BITMAPS: SharedCache<GlyphKey, GlyphBitmap> = LazyLock::new(Default::default);

/// Coverage of a glyph at pixel scale `k` (font units → px) with subpixel
/// offset `(fx, fy)` in quarter pixels.
fn glyph_bitmap(path: &sk::Path, key: GlyphKey, k: f32, bold_px: f32) -> Option<Arc<GlyphBitmap>> {
    cached(&GLYPH_BITMAPS, key, 50_000, || {
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
}

/// One glyph of laid-out text.
pub(crate) enum GlyphDraw {
    /// Cached coverage bitmap with its top-left corner at device `(x, y)`.
    Bitmap {
        #[cfg_attr(not(feature = "gpu"), allow(dead_code))]
        key: GlyphKey,
        bitmap: Arc<GlyphBitmap>,
        x: i32,
        y: i32,
    },
    /// Outline (font units) drawn with transform `ts` (rotated text); `bold`
    /// is the extra stroke width in font units.
    Outline {
        path: Arc<sk::Path>,
        ts: Transform,
        bold: Option<f32>,
    },
}

/// Lays out `t` with the base transform `base` (scale + translation) and
/// calls `draw` for each glyph. Shared by the CPU and GPU backends.
pub(crate) fn layout_text(
    fonts: &FontSet,
    font_key: usize,
    base: Transform,
    t: &TextNode,
    mut draw: impl FnMut(GlyphDraw),
) {
    let Some(font) = &fonts.font else { return };
    if t.text.is_empty()
        || t.size <= 0.0
        || !t.x.is_finite()
        || !t.y.is_finite()
        || !t.color.is_visible()
    {
        return;
    }
    let width = fonts.width(&t.text, t.size);
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
    let sf = font.as_scaled(fonts.scale(t.size));
    let k = sf.h_scale_factor();
    let mut ts = base;
    if t.rotate != 0.0 {
        ts = ts.pre_concat(Transform::from_rotate_at(
            t.rotate as f32,
            t.x as f32,
            t.y as f32,
        ));
    }
    let simple = t.rotate == 0.0 && base.kx == 0.0 && base.ky == 0.0;
    let mut pen = x0 as f32;
    let mut prev = None;
    for ch in t.text.chars() {
        let id = font.glyph_id(ch);
        if let Some(pv) = prev {
            pen += sf.kern(pv, id);
        }
        if let Some(path) = glyph_path(font, font_key, id) {
            if simple {
                // Cached coverage bitmaps at quarter-pixel positions.
                let s = base.sx;
                let gx = pen * s + base.tx;
                let gy = baseline as f32 * s + base.ty;
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
                let key: GlyphKey = (font_key, id.0, (k * s).to_bits(), fx, fy, t.bold);
                if let Some(g) = glyph_bitmap(&path, key, k * s, bold_px) {
                    let (x, y) = (ix as i32 + g.dx, iy as i32 + g.dy);
                    draw(GlyphDraw::Bitmap {
                        key,
                        bitmap: g,
                        x,
                        y,
                    });
                }
            } else {
                let gt = ts.pre_concat(Transform::from_row(k, 0.0, 0.0, k, pen, baseline as f32));
                let bold = t.bold.then(|| (t.size as f32 * 0.07) / k);
                draw(GlyphDraw::Outline { path, ts: gt, bold });
            }
        }
        pen += sf.h_advance(id);
        prev = Some(id);
    }
}

/// Axis-aligned 2-point segments of a path (grid lines, ticks, frames).
type Segment = ((f64, f64), (f64, f64));

pub(crate) fn axis_aligned_segments(cmds: &[PathCmd]) -> Option<Vec<Segment>> {
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
fn fill_polygon(
    pm: &mut PixmapMut<'_>,
    cmds: &[PathCmd],
    c: Color,
    ts: Transform,
    clip: Clip,
) -> bool {
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
        // The buffer is all zeros between uses (the scan below clears what it
        // reads), so it is only grown, never cleared.
        let acc = &mut cv.acc;
        if acc.len() < stride * bh {
            acc.resize(stride * bh, 0.0);
        }
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
        let solid = [rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, 255];
        let (cl, cr) = (clip.l.ceil() as i32, clip.r.floor() as i32);
        let (ct, cb) = (clip.t.ceil() as i32, clip.b.floor() as i32);
        for y in 0..bh {
            let py = y0 + y as i32;
            let row_inside = py >= ct && py < cb;
            let base = y * stride;
            let drow = (py as usize * pw + x0 as usize) * 4;
            let mut put = |i: usize, k: f32| {
                let px = x0 + i as i32;
                let m = if row_inside && px >= cl && px < cr {
                    255
                } else {
                    clip.cov(px, py)
                };
                let a = (k * ca * m as f32 / 255.0 + 0.5) as u32;
                let d: &mut [u8; 4] = (&mut dst[drow + i * 4..drow + i * 4 + 4])
                    .try_into()
                    .expect("4 bytes");
                if a >= 255 {
                    *d = solid;
                } else if a > 0 {
                    blend(d, rgb, a);
                }
            };
            let mut sum = 0.0f32;
            for i in 0..bw {
                sum += std::mem::take(&mut acc[base + i]);
                let k = sum.abs().min(1.0);
                if k >= 1.0 / 512.0 {
                    put(i, k);
                }
            }
            acc[base + bw] = 0.0;
            acc[base + bw + 1] = 0.0;
        }
    });
    true
}

/// Fills a rectilinear path (all edges horizontal or vertical, nonzero
/// rule) as rectangles: the horizontal edges are cut into vertical slabs at
/// their end points and each slab is filled where its winding number is
/// non-zero. The cost is proportional to the filled area, not to the
/// bounding box (column envelopes of dense lines). Interior slab borders
/// must be whole device pixels (no seams); `false` when not eligible.
fn fill_rectilinear(
    pm: &mut PixmapMut<'_>,
    cmds: &[PathCmd],
    c: Color,
    ts: Transform,
    clip: Clip,
) -> bool {
    if cmds.len() < 16 || ts.kx != 0.0 || ts.ky != 0.0 {
        return false;
    }
    // Horizontal edges (x0 < x1, y, direction) in device space.
    let mut hs: Vec<(f32, f32, f32, i32)> = Vec::with_capacity(cmds.len() / 2);
    let mut push = |p: (f32, f32), q: (f32, f32)| -> bool {
        if p.1 == q.1 {
            if p.0 != q.0 {
                let dir = if q.0 > p.0 { 1 } else { -1 };
                hs.push((p.0.min(q.0), p.0.max(q.0), p.1, dir));
            }
            true
        } else {
            p.0 == q.0
        }
    };
    let dev = |x: f64, y: f64| (x as f32 * ts.sx + ts.tx, y as f32 * ts.sy + ts.ty);
    let (mut start, mut cur) = (None::<(f32, f32)>, (0.0f32, 0.0f32));
    for cmd in cmds {
        match *cmd {
            PathCmd::MoveTo(x, y) => {
                if let Some(s0) = start {
                    if !push(cur, s0) {
                        return false;
                    }
                }
                cur = dev(x, y);
                start = Some(cur);
            }
            PathCmd::LineTo(x, y) => {
                let p = dev(x, y);
                if !push(cur, p) {
                    return false;
                }
                cur = p;
            }
            PathCmd::Close => {
                if let Some(s0) = start {
                    if !push(cur, s0) {
                        return false;
                    }
                    cur = s0;
                }
            }
            _ => return false,
        }
    }
    if let Some(s0) = start {
        if !push(cur, s0) {
            return false;
        }
    }
    if hs.is_empty()
        || hs
            .iter()
            .any(|h| !(h.0.is_finite() && h.1.is_finite() && h.2.is_finite()))
    {
        return false;
    }
    // Slab borders: every end point; interior ones must be whole pixels.
    let mut xs: Vec<f32> = hs.iter().flat_map(|h| [h.0, h.1]).collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup();
    let interior = &xs[1..xs.len() - 1];
    if interior.iter().any(|x| (x - x.round()).abs() > 1e-3) {
        return false;
    }
    // (slab, y, dir) for every slab an edge spans.
    let mut cuts: Vec<(u32, f32, i32)> = Vec::with_capacity(hs.len());
    for &(x0, x1, y, dir) in &hs {
        let a = xs.partition_point(|v| *v < x0);
        let b = xs.partition_point(|v| *v < x1);
        for slab in a..b {
            cuts.push((slab as u32, y, dir));
        }
    }
    cuts.sort_unstable_by(|p, q| p.0.cmp(&q.0).then(p.1.total_cmp(&q.1)));
    let mut i = 0;
    while i < cuts.len() {
        let slab = cuts[i].0;
        let (l, r) = (xs[slab as usize], xs[slab as usize + 1]);
        let mut wind = 0;
        let mut top = 0.0f32;
        while i < cuts.len() && cuts[i].0 == slab {
            let (_, y, dir) = cuts[i];
            let before = wind;
            wind += dir;
            if before == 0 && wind != 0 {
                top = y;
            } else if before != 0 && wind == 0 && y > top {
                fill_rect_aa(pm, l, top, r, y, c, clip);
            }
            i += 1;
        }
    }
    true
}

/// Reusable coverage buffer for [`stroke_polyline`].
#[derive(Default)]
struct Coverage {
    cov: Vec<u8>,
    acc: Vec<f32>,
    /// Evaluated runs (first index, length, vertical?), so the blend pass
    /// visits exactly the pixels near the line.
    spans: Vec<(u32, u32, bool)>,
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
    pm: &mut PixmapMut<'_>,
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
        let Coverage { cov, spans, .. } = &mut *c;
        cov.resize((w * h) as usize, 0);
        spans.clear();

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
                // Steep segments with round ends: walk columns (a steep, short
                // segment covers a few columns but many rows).
                if cap0.is_none() && cap1.is_none() && uy.abs() > ux.abs() {
                    let px0 = ((bx0 - 0.5).floor() as i32).max(0);
                    let px1 = ((bx1 - 0.5).ceil() as i32).min(w - 1);
                    let (by0, by1) = (a.1.min(b.1) - reach, a.1.max(b.1) + reach);
                    for px in px0..=px1 {
                        let qx = px as f32 + 0.5 - a.0;
                        // y-range of the band |lateral| <= reach in this column.
                        let (mut lo, mut hi) = (by0, by1);
                        if ux.abs() > 1e-6 {
                            let e0 = a.1 + (qx * uy - reach) / ux;
                            let e1 = a.1 + (qx * uy + reach) / ux;
                            lo = lo.max(e0.min(e1));
                            hi = hi.min(e0.max(e1));
                        }
                        let ya = ((lo - 0.5).floor() as i32).max(0);
                        let yb = ((hi - 0.5).ceil() as i32).min(h - 1);
                        let mut idx = ya as usize * w as usize + px as usize;
                        if ya <= yb {
                            spans.push((idx as u32, (yb - ya + 1) as u32, true));
                        }
                        for py in ya..=yb {
                            let qy = py as f32 + 0.5 - a.1;
                            let along = qx * ux + qy * uy;
                            let lat = qy * ux - qx * uy;
                            let o = (-along).max(along - len).max(0.0);
                            let d = if o > 0.0 {
                                (lat * lat + o * o).sqrt()
                            } else {
                                lat.abs()
                            };
                            let k = (r + 0.5 - d).clamp(0.0, 1.0);
                            let v = (k * fade * 255.0 + 0.5) as u8;
                            let cell = &mut cov[idx];
                            *cell = (*cell).max(v);
                            idx += w as usize;
                        }
                    }
                    continue;
                }
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
                    spans.push(((row + x0 as usize) as u32, (x1 - x0 + 1) as u32, false));
                    if cap0.is_none() && cap1.is_none() {
                        // Round ends: distance to the segment, branch-free so
                        // the row vectorizes.
                        let qy = yc - a.1;
                        let cells = &mut cov[row + x0 as usize..=row + x1 as usize];
                        for (i, cell) in cells.iter_mut().enumerate() {
                            let qx = (x0 + i as i32) as f32 + 0.5 - a.0;
                            let along = qx * ux + qy * uy;
                            let lat = qy * ux - qx * uy;
                            let o = (-along).max(along - len).max(0.0);
                            let d = (lat * lat + o * o).sqrt();
                            let k = (r + 0.5 - d).clamp(0.0, 1.0);
                            let v = (k * fade * 255.0 + 0.5) as u8;
                            *cell = (*cell).max(v);
                        }
                        continue;
                    }
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
                        }
                    }
                }
            }
        }

        // Blend the coverage once and clear it for the next use.
        let ca = (s.color.a.clamp(0.0, 1.0) * 255.0).round() as u32;
        let rgb = [s.color.r as u32, s.color.g as u32, s.color.b as u32];
        let dst = pm.data_mut();
        let wu = w as usize;
        for &(start, len, vertical) in spans.iter() {
            let step = if vertical { wu } else { 1 };
            let (mut px, mut py) = (start as usize % wu, start as usize / wu);
            let (ex, ey) = if vertical {
                (px + 1, py + len as usize)
            } else {
                (px + len as usize, py + 1)
            };
            // Spans inside the clip need no per-pixel clip coverage.
            let inside = px as f32 >= clip.l
                && ex as f32 <= clip.r
                && py as f32 >= clip.t
                && ey as f32 <= clip.b;
            let mut i = start as usize;
            for _ in 0..len {
                let k = std::mem::take(&mut cov[i]) as u32;
                if k != 0 {
                    let m = if inside {
                        255
                    } else {
                        clip.cov(px as i32, py as i32)
                    };
                    let a = (k * ca * m + 255 * 255 / 2) / (255 * 255);
                    if a > 0 {
                        let inv = 255 - a;
                        let di = i * 4;
                        for j in 0..3 {
                            dst[di + j] = ((rgb[j] * a + dst[di + j] as u32 * inv + 127) / 255)
                                .min(255) as u8;
                        }
                        dst[di + 3] = (a + (dst[di + 3] as u32 * inv + 127) / 255).min(255) as u8;
                    }
                }
                i += step;
                if vertical {
                    py += 1;
                } else {
                    px += 1;
                }
            }
        }
    });
    true
}

/// Fills and/or strokes a path with tiny-skia.
fn fill_stroke_path(
    pm: &mut PixmapMut<'_>,
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
    /// Sprites of `Node::Instances` templates.
    instance_sprites: Vec<InstanceSprite>,
    /// Opaque background while nothing has been drawn yet: a rectangle
    /// filled with it (a plot area on the same background) is skipped.
    pristine: Option<Color>,
}

/// A template rendered at one quarter-pixel offset: (template address,
/// qx, qy, pixels, position of the template origin in the sprite).
type InstanceSprite = (usize, u8, u8, Sprite, i32, i32);

/// A sprite with the non-empty span of each row, so blitting skips the
/// transparent corners of round markers.
struct Sprite {
    pm: Pixmap,
    spans: Vec<(u16, u16)>,
}

impl Sprite {
    fn new(pm: Pixmap) -> Self {
        let w = pm.width() as usize;
        let spans = pm
            .data()
            .chunks(w * 4)
            .map(|row| {
                let a = |i: &usize| row[i * 4 + 3] != 0;
                let s0 = (0..w).find(a).unwrap_or(w);
                let s1 = (0..w).rev().find(a).map_or(s0, |i| i + 1);
                (s0 as u16, s1.max(s0) as u16)
            })
            .collect();
        Sprite { pm, spans }
    }

    /// Source-over at integer position `(x, y)`.
    fn blit(&self, pm: &mut PixmapMut<'_>, x: i32, y: i32, clip: Clip) {
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        let sw = self.pm.width() as i32;
        let inside = x as f32 >= clip.l
            && (x + sw) as f32 <= clip.r
            && y as f32 >= clip.t
            && (y + self.spans.len() as i32) as f32 <= clip.b
            && x >= 0
            && x + sw <= w;
        if !inside {
            return blit(pm, &self.pm, x, y, clip);
        }
        let src = self.pm.data();
        let dst = pm.data_mut();
        for (sy, &(s0, s1)) in self.spans.iter().enumerate() {
            let py = y + sy as i32;
            if py < 0 || py >= h || s0 >= s1 {
                continue;
            }
            let (s0, s1) = (s0 as usize, s1 as usize);
            let srow = &src[(sy * sw as usize + s0) * 4..(sy * sw as usize + s1) * 4];
            let d0 = ((py * w + x) as usize + s0) * 4;
            let drow = &mut dst[d0..d0 + srow.len()];
            for (s, d) in srow
                .as_chunks::<4>()
                .0
                .iter()
                .zip(drow.as_chunks_mut::<4>().0)
            {
                match s[3] {
                    255 => *d = *s,
                    0 => {}
                    sa => {
                        let inv = 255 - sa as u32;
                        for c in 0..4 {
                            d[c] = (s[c] as u32 + div255(d[c] as u32 * inv)).min(255) as u8;
                        }
                    }
                }
            }
        }
    }
}

/// Bounds of a marker template around its origin, including the stroke;
/// `None` for nodes that are not drawn as sprites.
pub(crate) fn template_bounds(n: &Node) -> Option<(f64, f64, f64, f64)> {
    let sw = |s: &Option<Stroke>| s.as_ref().map_or(0.0, |s| s.width);
    match n {
        Node::Circle {
            cx, cy, r, stroke, ..
        } => {
            let e = r + sw(stroke) / 2.0;
            Some((cx - e, cy - e, cx + e, cy + e))
        }
        Node::Rect {
            x, y, w, h, stroke, ..
        } => {
            let e = sw(stroke) / 2.0;
            let (x0, x1) = (x.min(x + w), x.max(x + w));
            let (y0, y1) = (y.min(y + h), y.max(y + h));
            Some((x0 - e, y0 - e, x1 + e, y1 + e))
        }
        Node::Path { cmds, stroke, .. } => {
            let (mut l, mut t, mut r, mut b) = (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            );
            let mut add = |x: f64, y: f64, e: f64| {
                (l, t, r, b) = (l.min(x - e), t.min(y - e), r.max(x + e), b.max(y + e));
            };
            for c in cmds {
                match *c {
                    PathCmd::MoveTo(x, y) | PathCmd::LineTo(x, y) => add(x, y, 0.0),
                    PathCmd::CubicTo(a, bb, c2, d, x, y) => {
                        add(a, bb, 0.0);
                        add(c2, d, 0.0);
                        add(x, y, 0.0);
                    }
                    PathCmd::ArcTo(rx, ry, _, _, x, y) => add(x, y, 2.0 * rx.abs().max(ry.abs())),
                    PathCmd::Close => {}
                }
            }
            // Miter joins can reach a few stroke widths beyond the outline.
            let e = sw(stroke) * 2.0;
            (l.is_finite() && t.is_finite()).then_some((l - e, t - e, r + e, b + e))
        }
        _ => None,
    }
}

/// Renders `template` at device scale `scale` with its origin at
/// quarter-pixel offset `(qx, qy)`: the sprite and the origin's integer
/// position in it. Shared by the CPU and GPU backends.
pub(crate) fn instance_sprite(
    fonts: &FontSet,
    template: &Node,
    scale: f32,
    qx: u8,
    qy: u8,
) -> Option<(Pixmap, i32, i32)> {
    let (l, t, r, b) = template_bounds(template)?;
    let pad = 2.0;
    let x0 = (-l as f32 * scale).ceil() + pad;
    let y0 = (-t as f32 * scale).ceil() + pad;
    let w = (x0 + r as f32 * scale + pad + 1.0).ceil().max(1.0) as u32;
    let h = (y0 + b as f32 * scale + pad + 1.0).ceil().max(1.0) as u32;
    if w > 4096 || h > 4096 {
        return None;
    }
    let mut sp = Pixmap::new(w, h)?;
    let (fx, fy) = ((qx as f32 + 0.5) / 4.0, (qy as f32 + 0.5) / 4.0);
    let mut ctx = Ctx::new(
        fonts,
        0,
        Transform::from_row(scale, 0.0, 0.0, scale, x0 + fx, y0 + fy),
        w,
        h,
    );
    ctx.node(&mut sp.as_mut(), template, &HashMap::default());
    Some((sp, x0 as i32, y0 as i32))
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

impl<'a> Ctx<'a> {
    fn new(fonts: &'a FontSet, font_key: usize, base: Transform, w: u32, h: u32) -> Self {
        Ctx {
            fonts,
            font_key,
            base,
            sprites: Vec::new(),
            last_sprite: 0,
            last_count: None,
            clip: Clip::new(0.0, 0.0, w as f32, h as f32),
            mask: None,
            instance_sprites: Vec::new(),
            pristine: None,
        }
    }

    /// `Node::Instances`: the template from cached quarter-pixel sprites,
    /// or drawn at each offset when it cannot be a sprite.
    fn instances(
        &mut self,
        pm: &mut PixmapMut<'_>,
        template: &Node,
        offsets: impl IntoIterator<Item = (f32, f32)>,
        counts: &HashMap<(u32, u32, u32, u32), usize>,
    ) {
        let ts = self.base;
        let key = template as *const Node as usize;
        if ts.kx != 0.0 || ts.ky != 0.0 || template_bounds(template).is_none() {
            for (x, y) in offsets {
                self.base = ts.pre_translate(x, y);
                self.node(pm, template, counts);
            }
            self.base = ts;
            return;
        }
        let mut last = usize::MAX;
        for (x, y) in offsets {
            let gx = x * ts.sx + ts.tx;
            let gy = y * ts.sy + ts.ty;
            if !(gx.is_finite() && gy.is_finite()) {
                continue;
            }
            let (ix, iy) = (gx.floor(), gy.floor());
            let q = |f: f32| ((f * 4.0) as u8).min(3);
            let (qx, qy) = (q(gx - ix), q(gy - iy));
            let hit = self
                .instance_sprites
                .get(last)
                .is_some_and(|s| s.0 == key && s.1 == qx && s.2 == qy);
            if !hit {
                last = match self
                    .instance_sprites
                    .iter()
                    .position(|s| s.0 == key && s.1 == qx && s.2 == qy)
                {
                    Some(i) => i,
                    None => {
                        let Some((sp, ox, oy)) =
                            instance_sprite(self.fonts, template, ts.sx, qx, qy)
                        else {
                            continue;
                        };
                        self.instance_sprites
                            .push((key, qx, qy, Sprite::new(sp), ox, oy));
                        self.instance_sprites.len() - 1
                    }
                };
            }
            let (_, _, _, sp, ox, oy) = &self.instance_sprites[last];
            sp.blit(pm, ix as i32 - ox, iy as i32 - oy, self.clip);
        }
    }

    fn draw(
        &mut self,
        pm: &mut PixmapMut<'_>,
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
        pm: &mut PixmapMut<'_>,
        f: impl FnOnce(&Self, &mut PixmapMut<'_>, Option<&Mask>) -> R,
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
    fn fill_rect(&self, pm: &mut PixmapMut<'_>, r: sk::Rect, c: Color, ts: Transform) {
        let (l, t) = (r.left() * ts.sx + ts.tx, r.top() * ts.sy + ts.ty);
        let (rr, b) = (r.right() * ts.sx + ts.tx, r.bottom() * ts.sy + ts.ty);
        fill_rect_aa(pm, l.min(rr), t.min(b), l.max(rr), t.max(b), c, self.clip);
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_stroke(
        &self,
        pm: &mut PixmapMut<'_>,
        path: &sk::Path,
        fill: Option<Color>,
        stroke: &Option<Stroke>,
        ts: Transform,
        mask: Option<&Mask>,
    ) {
        fill_stroke_path(pm, path, fill, stroke, ts, mask);
    }

    fn node(
        &mut self,
        pm: &mut PixmapMut<'_>,
        n: &Node,
        counts: &HashMap<(u32, u32, u32, u32), usize>,
    ) {
        if let Some(bg) = self.pristine {
            if let Node::Rect {
                rx,
                fill: Some(f),
                stroke: None,
                ..
            } = n
            {
                if *rx <= 0.0 && *f == bg {
                    return;
                }
            }
            self.pristine = None;
        }
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
                    Some(c) => {
                        fill_rectilinear(pm, cmds, c, ts, self.clip)
                            || fill_polygon(pm, cmds, c, ts, self.clip)
                    }
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
                                        &mut sp.as_mut(),
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
            Node::Instances { template, offsets } => {
                self.instances(pm, template, offsets.iter().copied(), counts)
            }
            Node::MarkerGrid {
                template,
                x,
                y,
                step,
                cols,
                bits,
            } => {
                let pos = super::scene::grid_positions(*x, *y, *step, *cols, bits);
                self.instances(pm, template, pos, counts)
            }
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
                        self.draw(&mut layer.as_mut(), children, counts);
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
        pm: &mut PixmapMut<'_>,
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

    fn text(&self, pm: &mut PixmapMut<'_>, t: &TextNode, mask: Option<&Mask>) {
        let p = paint(t.color);
        layout_text(self.fonts, self.font_key, self.base, t, |g| match g {
            GlyphDraw::Bitmap {
                bitmap: g, x, y, ..
            } => blit_coverage(pm, &g.cov, g.w, g.h, x, y, t.color, self.clip),
            GlyphDraw::Outline { path, ts, bold } => {
                pm.fill_path(&path, &p, FillRule::Winding, ts, mask);
                if let Some(width) = bold {
                    let st = sk::Stroke {
                        width,
                        ..sk::Stroke::default()
                    };
                    pm.stroke_path(&path, &p, &st, ts, mask);
                }
            }
        });
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

/// Renders `scene` at `scale` into `pixmap` (resized if needed), on one
/// thread.
pub(crate) fn render_pixmap(
    scene: &Scene,
    fonts: &FontSet,
    scale: f64,
    pixmap: &mut Option<Pixmap>,
) -> Option<()> {
    render_pixmap_threads(scene, fonts, scale, pixmap, 1)
}

/// Like [`render_pixmap`], splitting the image into horizontal bands drawn
/// by up to `threads` threads (each band at least 64 rows). Every band
/// draws the whole scene clipped to its rows, so the result equals the
/// single-threaded one.
pub(crate) fn render_pixmap_threads(
    scene: &Scene,
    fonts: &FontSet,
    scale: f64,
    pixmap: &mut Option<Pixmap>,
    threads: usize,
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
    render_bytes(scene, fonts, scale, pm.data_mut(), w, h, threads);
    Some(())
}

/// Device size of `scene` at `scale`.
pub(crate) fn device_size(scene: &Scene, scale: f64) -> (u32, u32) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let w = (scene.width * scale).round().max(1.0) as u32;
    let h = (scene.height * scale).round().max(1.0) as u32;
    (w, h)
}

/// Renders into `data` (`w × h` premultiplied RGBA, any prior content) on
/// up to `threads` threads.
pub(crate) fn render_bytes(
    scene: &Scene,
    fonts: &FontSet,
    scale: f64,
    data: &mut [u8],
    w: u32,
    h: u32,
    threads: usize,
) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let mut counts = HashMap::default();
    count_circles(&scene.nodes, &mut counts);
    let font_key = fonts.font.as_ref().map_or(0, |f| f as *const _ as usize);
    let background = scene.background.map_or(sk::Color::TRANSPARENT, color);
    let bands = threads.clamp(1, (h as usize / 64).max(1));
    let rows = (h as usize).div_ceil(bands);
    let draw_band = |y0: usize, data: &mut [u8]| {
        let bh = (data.len() / (w as usize * 4)) as u32;
        let Some(mut band) = PixmapMut::from_bytes(data, w, bh) else {
            return;
        };
        band.fill(background);
        let base = Transform::from_row(scale as f32, 0.0, 0.0, scale as f32, 0.0, -(y0 as f32));
        let mut ctx = Ctx::new(fonts, font_key, base, w, bh);
        ctx.pristine = scene.background.filter(|c| c.a >= 1.0);
        ctx.draw(&mut band, &scene.nodes, &counts);
    };
    let chunk = rows * w as usize * 4;
    if bands == 1 {
        draw_band(0, data);
    } else {
        std::thread::scope(|sc| {
            for (i, data) in data.chunks_mut(chunk).enumerate() {
                let draw_band = &draw_band;
                sc.spawn(move || draw_band(i * rows, data));
            }
        });
    }
}

/// Converts premultiplied RGBA rows (`stride` bytes apart) row by row on
/// up to `threads` threads: `f(src_row, dst_row)` with `n` destination
/// items per row.
fn par_rows<D: Send>(
    src: &[u8],
    stride: usize,
    h: usize,
    dst: &mut [D],
    n: usize,
    threads: usize,
    f: impl Fn(&[u8], &mut [D]) + Sync,
) {
    let threads = threads.clamp(1, (dst.len() / 65_536).max(1));
    let rows = h.div_ceil(threads).max(1);
    let run = |y0: usize, d: &mut [D]| {
        for (y, row) in d.chunks_mut(n).enumerate() {
            let at = (y0 + y) * stride;
            f(&src[at..at + stride.min(src.len() - at)], row);
        }
    };
    if threads == 1 {
        return run(0, dst);
    }
    std::thread::scope(|sc| {
        for (i, d) in dst.chunks_mut(rows * n).enumerate() {
            let run = &run;
            sc.spawn(move || run(i * rows, d));
        }
    });
}

/// Copies `h` rows of `row` bytes (`stride` bytes apart) into `out`.
#[cfg(feature = "gpu")]
pub(crate) fn copy_rows(
    src: &[u8],
    stride: usize,
    row: usize,
    h: usize,
    out: &mut Vec<u8>,
    threads: usize,
) {
    out.resize(row * h, 0);
    par_rows(src, stride, h, out, row, threads, |s, d| {
        d.copy_from_slice(&s[..d.len()])
    });
}

/// Copies rows of little-endian `u32` pixels into `out`.
#[cfg(feature = "gpu")]
pub(crate) fn copy_rows_u32(
    src: &[u8],
    stride: usize,
    w: usize,
    h: usize,
    out: &mut Vec<u32>,
    threads: usize,
) {
    out.resize(w * h, 0);
    par_rows(src, stride, h, out, w, threads, |s, d| {
        for (o, p) in d.iter_mut().zip(s.as_chunks::<4>().0) {
            *o = u32::from_le_bytes(*p);
        }
    });
}

/// Converts `w × h` premultiplied RGBA pixels stored in `buf` (as bytes)
/// to `0x00RRGGBB` composited on white, in place.
pub(crate) fn premul_to_0rgb_in_place(buf: &mut [u32], threads: usize) {
    let n = buf.len();
    let threads = threads.clamp(1, (n / 65_536).max(1));
    let convert = |d: &mut [u32]| {
        #[cfg(target_arch = "x86_64")]
        let done = if std::arch::is_x86_feature_detected!("avx2") {
            // SAFETY: AVX2 support was checked at run time.
            unsafe { premul_to_0rgb_in_place_avx2(d) }
        } else {
            0
        };
        #[cfg(not(target_arch = "x86_64"))]
        let done = 0;
        for o in &mut d[done..] {
            let p = o.to_ne_bytes();
            let inv = 255 - p[3] as u32;
            let c = |v: u8| (v as u32 + inv).min(255);
            *o = c(p[0]) << 16 | c(p[1]) << 8 | c(p[2]);
        }
    };
    if threads == 1 {
        return convert(buf);
    }
    let per = n.div_ceil(threads);
    std::thread::scope(|sc| {
        for d in buf.chunks_mut(per) {
            let convert = &convert;
            sc.spawn(move || convert(d));
        }
    });
}

/// In-place AVX2 form of [`premul_to_0rgb_in_place`] (8 pixels per step;
/// opaque groups are a byte shuffle). Returns the pixels converted.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn premul_to_0rgb_in_place_avx2(d: &mut [u32]) -> usize {
    use std::arch::x86_64::*;
    let n = d.len() / 8 * 8;
    let shuf = _mm256_setr_epi8(
        2, 1, 0, -1, 6, 5, 4, -1, 10, 9, 8, -1, 14, 13, 12, -1, 2, 1, 0, -1, 6, 5, 4, -1, 10, 9, 8,
        -1, 14, 13, 12, -1,
    );
    let alpha = _mm256_set1_epi32(0xff00_0000u32 as i32);
    let mut i = 0;
    while i < n {
        // SAFETY: i + 8 <= n elements are in bounds.
        unsafe {
            let p = d.as_mut_ptr().add(i).cast::<__m256i>();
            let v = _mm256_loadu_si256(p);
            if _mm256_movemask_epi8(_mm256_cmpeq_epi32(_mm256_and_si256(v, alpha), alpha)) == -1 {
                _mm256_storeu_si256(p, _mm256_shuffle_epi8(v, shuf));
            } else {
                for o in &mut d[i..i + 8] {
                    let q = o.to_ne_bytes();
                    let inv = 255 - q[3] as u32;
                    let c = |v: u8| (v as u32 + inv).min(255);
                    *o = c(q[0]) << 16 | c(q[1]) << 8 | c(q[2]);
                }
            }
        }
        i += 8;
    }
    n
}

/// Demultiplies premultiplied RGBA bytes in place (straight alpha).
pub(crate) fn demultiply_in_place(buf: &mut [u8]) {
    for p in buf.as_chunks_mut::<4>().0 {
        let a = p[3];
        if a != 255 && a != 0 {
            let un = |c: u8| ((c as u16 * 255 + a as u16 / 2) / a as u16).min(255) as u8;
            *p = [un(p[0]), un(p[1]), un(p[2]), a];
        }
    }
}

/// Premultiplied RGBA rows → straight-alpha [`Pixels`].
pub(crate) fn premul_to_pixels(
    src: &[u8],
    stride: usize,
    w: usize,
    h: usize,
    out: &mut Pixels,
    threads: usize,
) {
    out.width = w as u32;
    out.height = h as u32;
    out.rgba.resize(w * h * 4, 0);
    par_rows(src, stride, h, &mut out.rgba, w * 4, threads, |s, d| {
        // Runs of opaque pixels are plain copies (16 bytes at a time).
        let mut done = 0;
        for (dc, sc) in d
            .as_chunks_mut::<16>()
            .0
            .iter_mut()
            .zip(s.as_chunks::<16>().0)
        {
            if sc[3] != 255 || sc[7] != 255 || sc[11] != 255 || sc[15] != 255 {
                break;
            }
            *dc = *sc;
            done += 16;
        }
        let (d, s) = (&mut d[done..], &s[done..]);
        for (dst, s) in d
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(s.as_chunks::<4>().0)
        {
            let a = s[3];
            *dst = if a == 255 || a == 0 {
                *s
            } else {
                let un = |c: u8| ((c as u16 * 255 + a as u16 / 2) / a as u16).min(255) as u8;
                [un(s[0]), un(s[1]), un(s[2]), a]
            };
        }
    });
}

/// Copies a premultiplied pixmap into straight-alpha RGBA.
pub(crate) fn to_pixels(pm: &Pixmap, out: &mut Pixels, threads: usize) {
    let w = pm.width() as usize;
    premul_to_pixels(pm.data(), w * 4, w, pm.height() as usize, out, threads);
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
    to_pixels(pm.as_ref()?, &mut px, 1);
    Some(px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_match_single_thread() {
        use crate::prelude::*;
        let mut fig = Figure::new(500, 400);
        fig.title("Bands");
        let ax = fig.axes();
        let x: Vec<f64> = (0..3000).map(|i| i as f64 / 100.0).collect();
        ax.line(&x, x.iter().map(|v| v.sin() * (v * 13.0).cos()));
        ax.scatter(&x[..300], x[..300].iter().map(|v| v.cos()))
            .size(6.0);
        ax.bar([5.0, 15.0, 25.0], [0.5, -0.7, 0.3]);
        ax.text(12.0, 0.8, "label");
        ax.y_label("rotated y label");
        let scene = fig.to_scene();
        let fonts = FontSet::resolve(None);
        let (mut a, mut b) = (None, None);
        render_pixmap_threads(&scene, &fonts, 1.5, &mut a, 1).unwrap();
        render_pixmap_threads(&scene, &fonts, 1.5, &mut b, 7).unwrap();
        let (a, b) = (a.unwrap(), b.unwrap());
        // Own fillers give identical bands; tiny-skia paths (rotated text
        // here) may differ slightly where its edge clipper splits curves at
        // a band border.
        let diff: Vec<i32> = a
            .data()
            .iter()
            .zip(b.data())
            .map(|(p, q)| (*p as i32 - *q as i32).abs())
            .collect();
        let off = diff.iter().filter(|d| **d > 2).count();
        assert!(off * 2000 < diff.len(), "{off} differing channels");
        assert!(diff.iter().max() <= Some(&64));
    }

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
            assert!(fill_polygon(
                &mut a.as_mut(),
                &cmds,
                c,
                Transform::identity(),
                clip
            ));
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
    #[allow(clippy::needless_range_loop)]
    fn rectilinear_fill_matches_polygon_fill() {
        let mut seed = 99u64;
        let mut rnd = |lo: f64, hi: f64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            lo + (seed % 10_000) as f64 / 10_000.0 * (hi - lo)
        };
        let c = Color::rgba(200, 30, 60, 0.9);
        for case in 0..20 {
            // Column envelope: top chain left→right, bottom chain back.
            let x0 = 3.3 + case as f64 * 0.04;
            let cols = 40;
            let tops: Vec<f64> = (0..cols).map(|_| rnd(2.0, 30.0)).collect();
            let bots: Vec<f64> = tops.iter().map(|t| t + rnd(0.5, 25.0)).collect();
            let edge = |i: usize| if i == 0 { x0 } else { 4.0 + i as f64 };
            let right = |i: usize| {
                if i + 1 == cols {
                    4.0 + cols as f64 - 0.4
                } else {
                    5.0 + i as f64
                }
            };
            let mut cmds = vec![PathCmd::MoveTo(edge(0), tops[0])];
            for i in 0..cols {
                cmds.push(PathCmd::LineTo(edge(i), tops[i]));
                cmds.push(PathCmd::LineTo(right(i), tops[i]));
            }
            for i in (0..cols).rev() {
                cmds.push(PathCmd::LineTo(right(i), bots[i]));
                cmds.push(PathCmd::LineTo(edge(i), bots[i]));
            }
            cmds.push(PathCmd::Close);
            let clip = Clip::new(2.5, 1.0, 58.0, 62.0);
            let mut a = Pixmap::new(64, 64).unwrap();
            a.fill(sk::Color::WHITE);
            let mut b = a.clone();
            assert!(fill_rectilinear(
                &mut a.as_mut(),
                &cmds,
                c,
                Transform::identity(),
                clip
            ));
            assert!(fill_polygon(
                &mut b.as_mut(),
                &cmds,
                c,
                Transform::identity(),
                clip
            ));
            let worst = a
                .data()
                .iter()
                .zip(b.data())
                .map(|(p, q)| p.abs_diff(*q))
                .max();
            assert!(worst <= Some(2), "case {case}: {worst:?}");
        }
        // Fractional interior borders are refused (they would leave seams).
        let cmds: Vec<PathCmd> = [
            (0.0, 0.0),
            (2.5, 0.0),
            (2.5, 1.0),
            (5.0, 1.0),
            (5.0, 3.0),
            (0.0, 3.0),
        ]
        .iter()
        .cycle()
        .take(18)
        .enumerate()
        .map(|(i, &(x, y))| {
            if i == 0 {
                PathCmd::MoveTo(x, y)
            } else {
                PathCmd::LineTo(x, y)
            }
        })
        .collect();
        let mut a = Pixmap::new(8, 8).unwrap();
        assert!(!fill_rectilinear(
            &mut a.as_mut(),
            &cmds,
            c,
            Transform::identity(),
            Clip::new(0.0, 0.0, 8.0, 8.0)
        ));
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
            fill_rect_aa(&mut a.as_mut(), x0, y0, x1, y1, c, full);
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
