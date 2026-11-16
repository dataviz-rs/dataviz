//! Backend-independent drawing primitives and the SVG writer.
//!
//! Every chart draws into a [`Scene`]; the scene is then written as SVG,
//! rasterized (see [`crate::plot::output`]) or shown in a window.

use super::color::Color;
use std::fmt::Write as _;
use std::sync::Arc;

/// One path segment.
#[derive(Clone, Debug, PartialEq)]
pub enum PathCmd {
    /// Start a new sub-path.
    MoveTo(f64, f64),
    /// Straight line.
    LineTo(f64, f64),
    /// Cubic Bézier curve (two control points, end point).
    CubicTo(f64, f64, f64, f64, f64, f64),
    /// Circular/elliptic arc (rx, ry, large-arc, sweep, end point), SVG semantics.
    ArcTo(f64, f64, bool, bool, f64, f64),
    /// Close the sub-path.
    Close,
}

/// Line cap style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum LineCap {
    /// Flat end at the end point.
    #[default]
    Butt,
    /// Rounded end.
    Round,
    /// Square end extending by half the width.
    Square,
}

/// Line join style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum LineJoin {
    /// Sharp corner.
    Miter,
    /// Rounded corner.
    #[default]
    Round,
    /// Cut corner.
    Bevel,
}

/// Outline style.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    /// Color (with opacity).
    pub color: Color,
    /// Width in pixels.
    pub width: f64,
    /// Dash pattern (on, off, on, off, … in pixels); empty for solid.
    pub dash: Vec<f64>,
    /// Cap style.
    pub cap: LineCap,
    /// Join style.
    pub join: LineJoin,
}

impl Stroke {
    /// Solid stroke.
    pub fn new(color: Color, width: f64) -> Self {
        Self {
            color,
            width,
            dash: Vec::new(),
            cap: LineCap::Butt,
            join: LineJoin::Round,
        }
    }

    /// With a dash pattern.
    pub fn dashed(mut self, dash: Vec<f64>) -> Self {
        self.dash = dash;
        self
    }

    /// With a cap style.
    pub fn cap(mut self, cap: LineCap) -> Self {
        self.cap = cap;
        self
    }

    /// With a join style.
    pub fn join(mut self, join: LineJoin) -> Self {
        self.join = join;
        self
    }
}

/// Horizontal text anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Anchor {
    /// Text starts at x.
    #[default]
    Start,
    /// Text is centred on x.
    Middle,
    /// Text ends at x.
    End,
}

/// Vertical text alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Baseline {
    /// y is the alphabetic baseline.
    #[default]
    Alphabetic,
    /// y is the top of the text.
    Top,
    /// y is the vertical centre of the text.
    Middle,
    /// y is the bottom of the text (descenders included).
    Bottom,
}

/// A text element.
#[derive(Clone, Debug, PartialEq)]
pub struct TextNode {
    /// Anchor position.
    pub x: f64,
    /// Anchor position.
    pub y: f64,
    /// Content (plain text).
    pub text: String,
    /// Font size in pixels.
    pub size: f64,
    /// Color.
    pub color: Color,
    /// Horizontal anchor.
    pub anchor: Anchor,
    /// Vertical alignment.
    pub baseline: Baseline,
    /// Rotation in degrees (clockwise) around the anchor.
    pub rotate: f64,
    /// Bold weight.
    pub bold: bool,
    /// Slanted (synthesized oblique; used for math variables).
    pub italic: bool,
}

/// An RGBA raster image (row-major, 4 bytes per pixel).
#[derive(Clone, Debug, PartialEq)]
pub struct RasterImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel data, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// A drawing primitive.
/// Triangles of a 3D plot in normalized box coordinates (independent of
/// the camera) and the camera of this frame: the GPU keeps the triangles
/// (by `id`) and only receives the camera when the view turns.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh3d {
    /// Identity of `vertices` (unchanged data keeps its id).
    pub(crate) id: u64,
    /// Per vertex: position (3), normal (3), RGBA color 0–1 (4).
    pub(crate) vertices: std::sync::Arc<Vec<f32>>,
    /// Lines, tubes and balls, 12 floats each: start (3), kind, end (3),
    /// size, straight RGBA 0–1 (4). Kind 0 line, 1 tube, 2 flat ball,
    /// 3 shaded ball; size in pixels (before perspective) when positive,
    /// else minus a radius in normalized box units.
    pub(crate) prims: std::sync::Arc<Vec<f32>>,
    /// Rotation rows (3 × vec4), then `[scale, centre x, centre y,
    /// distance]`, `[radius, fog, lighting, 0]`, background RGBA and the
    /// clip rectangle `[l, t, r, b]` (canvas units).
    pub(crate) view: [f32; 28],
}

/// A line segment: start and end point.
pub type Segment = ((f64, f64), (f64, f64));

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Node {
    /// Arbitrary path.
    Path {
        /// Segments.
        cmds: Vec<PathCmd>,
        /// Fill (None = no fill).
        fill: Option<Color>,
        /// Outline.
        stroke: Option<Stroke>,
    },
    /// Axis-aligned rectangle.
    Rect {
        /// Left.
        x: f64,
        /// Top.
        y: f64,
        /// Width.
        w: f64,
        /// Height.
        h: f64,
        /// Corner radius.
        rx: f64,
        /// Fill.
        fill: Option<Color>,
        /// Outline.
        stroke: Option<Stroke>,
    },
    /// Circle.
    Circle {
        /// Centre.
        cx: f64,
        /// Centre.
        cy: f64,
        /// Radius.
        r: f64,
        /// Fill.
        fill: Option<Color>,
        /// Outline.
        stroke: Option<Stroke>,
    },
    /// Shaded sphere (3D ball): lit from the upper left with a specular
    /// highlight, shaded per pixel by the rasterizer.
    Sphere {
        /// Centre.
        cx: f64,
        /// Centre.
        cy: f64,
        /// Radius.
        r: f64,
        /// Base color.
        color: Color,
    },
    /// Shaded tube segments (3D bonds, sticks): one lit cylinder of
    /// `width` per segment, with round ends.
    Tube {
        /// Segments (start, end).
        segs: Vec<Segment>,
        /// Diameter.
        width: f64,
        /// Base color.
        color: Color,
    },
    /// Opaque lit 3D faces kept on the GPU between frames (GPU back end;
    /// see [`Mesh3d`]). Other back ends never receive it.
    Mesh3d(Box<Mesh3d>),
    /// Text.
    Text(TextNode),
    /// Raster image scaled to the rectangle.
    Image {
        /// Left.
        x: f64,
        /// Top.
        y: f64,
        /// Width.
        w: f64,
        /// Height.
        h: f64,
        /// Pixels.
        image: Arc<RasterImage>,
        /// Interpolate when scaling (false = crisp pixels).
        smooth: bool,
    },
    /// Group of nodes with optional clipping, opacity and tooltip.
    Group {
        /// Children.
        children: Vec<Node>,
        /// Clip rectangle (x, y, w, h).
        clip: Option<(f64, f64, f64, f64)>,
        /// Group opacity.
        opacity: f64,
        /// Tooltip shown by SVG viewers (`<title>`).
        tooltip: Option<String>,
        /// Element id in SVG output (e.g. `s0-1`: plot 0, series 1).
        id: Option<String>,
    },
    /// The same node drawn at many positions (markers): `template` is
    /// drawn translated by each offset. Compact (8 bytes per position) and
    /// rasterized from a cached sprite.
    Instances {
        /// Node drawn around the origin.
        template: Box<Node>,
        /// Positions (canvas units).
        offsets: Vec<(f32, f32)>,
    },
    /// The same node drawn at the centres of the set cells of a grid (dense
    /// opaque markers): cell `(i, j)` is at `(x + (i + 0.5)·step,
    /// y + (j + 0.5)·step)`, set when bit `j·cols + i` of `bits` is 1.
    /// Its size does not depend on the number of data points.
    MarkerGrid {
        /// Node drawn around the origin.
        template: Box<Node>,
        /// Grid origin (canvas units).
        x: f64,
        /// Grid origin (canvas units).
        y: f64,
        /// Cell size (canvas units).
        step: f64,
        /// Cells per row.
        cols: usize,
        /// Occupancy bits, row-major.
        bits: Vec<u64>,
    },
}

/// Centres of the set cells of a [`Node::MarkerGrid`].
pub(crate) fn grid_positions(
    x: f64,
    y: f64,
    step: f64,
    cols: usize,
    bits: &[u64],
) -> impl Iterator<Item = (f32, f32)> + '_ {
    bits.iter().enumerate().flat_map(move |(wi, &w)| {
        let mut b = w;
        std::iter::from_fn(move || {
            (b != 0).then(|| {
                let k = wi * 64 + b.trailing_zeros() as usize;
                b &= b - 1;
                let (i, j) = (k % cols.max(1), k / cols.max(1));
                (
                    (x + (i as f64 + 0.5) * step) as f32,
                    (y + (j as f64 + 0.5) * step) as f32,
                )
            })
        })
    })
}

/// A complete drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Width in pixels.
    pub width: f64,
    /// Height in pixels.
    pub height: f64,
    /// Background (None = transparent).
    pub background: Option<Color>,
    /// Font family used for text (written into the SVG).
    pub font_family: String,
    /// Content.
    pub nodes: Vec<Node>,
    /// Accessible name (SVG `<title>`, read by screen readers).
    pub title: Option<String>,
    /// Accessible description (SVG `<desc>`): what the chart shows.
    pub description: Option<String>,
}

impl Scene {
    /// Empty scene.
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            width,
            height,
            background: Some(Color::WHITE),
            font_family: "sans-serif".into(),
            nodes: Vec::new(),
            title: None,
            description: None,
        }
    }

    /// Adds a node.
    pub fn push(&mut self, node: Node) {
        self.nodes.push(node);
    }

    /// Writes the scene as an SVG document.
    pub fn to_svg(&self) -> String {
        let mut w = SvgWriter {
            out: String::with_capacity(4096),
            clip_id: 0,
            spheres: Vec::new(),
            font: &self.font_family,
        };
        let a11y = match (&self.title, &self.description) {
            (None, None) => String::new(),
            (Some(_), None) => r#" role="img" aria-labelledby="dv-title""#.to_string(),
            (None, Some(_)) => r#" role="img" aria-describedby="dv-desc""#.to_string(),
            (Some(_), Some(_)) => {
                r#" role="img" aria-labelledby="dv-title" aria-describedby="dv-desc""#.to_string()
            }
        };
        let _ = write!(
            w.out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{0}" height="{1}" viewBox="0 0 {0} {1}" font-family="{2}"{3}>"#,
            num(self.width),
            num(self.height),
            esc(&font_list(&self.font_family)),
            a11y,
        );
        w.out.push('\n');
        if let Some(t) = &self.title {
            let _ = writeln!(w.out, r#"<title id="dv-title">{}</title>"#, esc(t));
        }
        if let Some(d) = &self.description {
            let _ = writeln!(w.out, r#"<desc id="dv-desc">{}</desc>"#, esc(d));
        }
        if let Some(bg) = self.background.filter(|c| c.is_visible()) {
            let _ = writeln!(
                w.out,
                r#"<rect width="100%" height="100%"{}/>"#,
                paint("fill", bg)
            );
        }
        for n in &self.nodes {
            w.node(n);
        }
        w.out.push_str("</svg>\n");
        w.out
    }
}

/// `font-family` value with generic fallback.
fn font_list(family: &str) -> String {
    if family == "sans-serif" || family.is_empty() {
        "sans-serif".into()
    } else {
        format!("'{}', sans-serif", family.replace('\'', ""))
    }
}

/// Compact number formatting (2 decimals, no trailing zeros).
pub(crate) fn num(v: f64) -> Num {
    Num(v)
}

/// A coordinate written with at most two decimals, trailing zeros trimmed
/// (`1`, `1.5`, `1.26`), without allocating.
#[derive(Clone, Copy)]
pub(crate) struct Num(f64);

impl std::fmt::Display for Num {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.0;
        if !v.is_finite() {
            return f.write_str("0");
        }
        if v.abs() >= 1e15 {
            // Out of the integer fast path's range.
            let s = format!("{v:.2}");
            return f.write_str(s.trim_end_matches('0').trim_end_matches('.'));
        }
        let c = (v * 100.0).round() as i64;
        if c == 0 {
            return f.write_str("0");
        }
        let (neg, c) = (c < 0, c.unsigned_abs());
        let (int, frac) = (c / 100, c % 100);
        let mut buf = [0u8; 24];
        let mut i = buf.len();
        if frac != 0 {
            let (d1, d2) = (frac / 10, frac % 10);
            if d2 != 0 {
                i -= 1;
                buf[i] = b'0' + d2 as u8;
            }
            i -= 1;
            buf[i] = b'0' + d1 as u8;
            i -= 1;
            buf[i] = b'.';
        }
        let mut n = int;
        loop {
            i -= 1;
            buf[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        if neg {
            i -= 1;
            buf[i] = b'-';
        }
        f.write_str(std::str::from_utf8(&buf[i..]).unwrap_or("0"))
    }
}

/// XML escaping for text and attributes.
pub(crate) fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

/// ` fill="#rrggbb" fill-opacity=".."` style attribute pair.
fn paint(attr: &str, c: Color) -> String {
    let mut s = format!(r#" {attr}="{}""#, c.to_hex());
    if c.a < 1.0 {
        let _ = write!(s, r#" {attr}-opacity="{}""#, num(c.a as f64));
    }
    s
}

fn fill_attr(fill: Option<Color>) -> String {
    match fill {
        Some(c) if c.is_visible() => paint("fill", c),
        _ => r#" fill="none""#.into(),
    }
}

fn stroke_attr(stroke: &Option<Stroke>) -> String {
    let Some(s) = stroke
        .as_ref()
        .filter(|s| s.color.is_visible() && s.width > 0.0)
    else {
        return String::new();
    };
    let mut a = paint("stroke", s.color);
    let _ = write!(a, r#" stroke-width="{}""#, num(s.width));
    if !s.dash.is_empty() {
        let d: Vec<String> = s.dash.iter().map(|v| num(*v).to_string()).collect();
        let _ = write!(a, r#" stroke-dasharray="{}""#, d.join(","));
    }
    match s.cap {
        LineCap::Butt => {}
        LineCap::Round => a.push_str(r#" stroke-linecap="round""#),
        LineCap::Square => a.push_str(r#" stroke-linecap="square""#),
    }
    match s.join {
        LineJoin::Miter => {}
        LineJoin::Round => a.push_str(r#" stroke-linejoin="round""#),
        LineJoin::Bevel => a.push_str(r#" stroke-linejoin="bevel""#),
    }
    a
}

pub(crate) fn path_data(cmds: &[PathCmd]) -> String {
    let mut d = String::with_capacity(cmds.len() * 12);
    for c in cmds {
        match *c {
            PathCmd::MoveTo(x, y) => {
                let _ = write!(d, "M{} {}", num(x), num(y));
            }
            PathCmd::LineTo(x, y) => {
                let _ = write!(d, "L{} {}", num(x), num(y));
            }
            PathCmd::CubicTo(a, b, c2, d2, x, y) => {
                let _ = write!(
                    d,
                    "C{} {} {} {} {} {}",
                    num(a),
                    num(b),
                    num(c2),
                    num(d2),
                    num(x),
                    num(y)
                );
            }
            PathCmd::ArcTo(rx, ry, large, sweep, x, y) => {
                let _ = write!(
                    d,
                    "A{} {} 0 {} {} {} {}",
                    num(rx),
                    num(ry),
                    large as u8,
                    sweep as u8,
                    num(x),
                    num(y)
                );
            }
            PathCmd::Close => d.push('Z'),
        }
    }
    d
}

struct SvgWriter<'a> {
    out: String,
    clip_id: usize,
    /// Sphere gradients written so far: (rgb, id).
    spheres: Vec<([u8; 3], usize)>,
    #[allow(dead_code)]
    font: &'a str,
}

impl SvgWriter<'_> {
    fn node(&mut self, n: &Node) {
        match n {
            Node::Path { cmds, fill, stroke } => {
                if cmds.len() < 2 {
                    return;
                }
                let _ = writeln!(
                    self.out,
                    r#"<path d="{}"{}{}/>"#,
                    path_data(cmds),
                    fill_attr(*fill),
                    stroke_attr(stroke)
                );
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
                let r = if *rx > 0.0 {
                    format!(r#" rx="{}""#, num(*rx))
                } else {
                    String::new()
                };
                let _ = writeln!(
                    self.out,
                    r#"<rect x="{}" y="{}" width="{}" height="{}"{r}{}{}/>"#,
                    num(x),
                    num(y),
                    num(w),
                    num(h),
                    fill_attr(*fill),
                    stroke_attr(stroke)
                );
            }
            Node::Circle {
                cx,
                cy,
                r,
                fill,
                stroke,
            } => {
                let _ = writeln!(
                    self.out,
                    r#"<circle cx="{}" cy="{}" r="{}"{}{}/>"#,
                    num(*cx),
                    num(*cy),
                    num(r.max(0.0)),
                    fill_attr(*fill),
                    stroke_attr(stroke)
                );
            }
            Node::Tube { segs, width, color } => {
                for n in tube_layers(segs, *width, *color) {
                    self.node(&n);
                }
            }
            // Only built for the GPU back end.
            Node::Mesh3d(_) => {}
            Node::Sphere { cx, cy, r, color } => {
                let rgb = [color.r, color.g, color.b];
                let id = match self.spheres.iter().find(|s| s.0 == rgb) {
                    Some(s) => s.1,
                    None => {
                        self.clip_id += 1;
                        let id = self.clip_id;
                        self.spheres.push((rgb, id));
                        let stop = |c: Color| format!("rgb({},{},{})", c.r, c.g, c.b);
                        let c = color.with_alpha(1.0);
                        let _ = writeln!(
                            self.out,
                            r#"<defs><radialGradient id="g{id}" fx="0.29" fy="0.22"><stop offset="0" stop-color="{}"/><stop offset="0.35" stop-color="{}"/><stop offset="1" stop-color="{}"/></radialGradient></defs>"#,
                            stop(c.lighten(0.55)),
                            stop(c),
                            stop(c.darken(0.5))
                        );
                        id
                    }
                };
                let op = if color.a < 1.0 {
                    format!(r#" fill-opacity="{}""#, num(color.a as f64))
                } else {
                    String::new()
                };
                let _ = writeln!(
                    self.out,
                    r#"<circle cx="{}" cy="{}" r="{}" fill="url(#g{id})"{op}/>"#,
                    num(*cx),
                    num(*cy),
                    num(r.max(0.0))
                );
            }
            Node::Text(t) => self.text(t),
            Node::Image {
                x,
                y,
                w,
                h,
                image,
                smooth,
            } => {
                let Some(png) = encode_png(image) else { return };
                let rendering = if *smooth {
                    "optimizeQuality"
                } else {
                    "optimizeSpeed"
                };
                let _ = writeln!(
                    self.out,
                    r#"<image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="none" image-rendering="{rendering}" style="image-rendering:{}" href="data:image/png;base64,{}"/>"#,
                    num(*x),
                    num(*y),
                    num(*w),
                    num(*h),
                    if *smooth { "auto" } else { "pixelated" },
                    base64(&png)
                );
            }
            Node::Group {
                children,
                clip,
                opacity,
                tooltip,
                id,
            } => {
                let mut attrs = String::new();
                if let Some(id) = id {
                    let _ = write!(attrs, r#" id="{}""#, esc(id));
                }
                if let Some((x, y, w, h)) = clip {
                    self.clip_id += 1;
                    let id = self.clip_id;
                    let _ = writeln!(
                        self.out,
                        r#"<clipPath id="c{id}"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath>"#,
                        num(*x),
                        num(*y),
                        num(w.max(0.0)),
                        num(h.max(0.0))
                    );
                    let _ = write!(attrs, r#" clip-path="url(#c{id})""#);
                }
                if *opacity < 1.0 {
                    let _ = write!(attrs, r#" opacity="{}""#, num(opacity.max(0.0)));
                }
                let _ = writeln!(self.out, "<g{attrs}>");
                if let Some(t) = tooltip {
                    let _ = writeln!(self.out, "<title>{}</title>", esc(t));
                }
                for c in children {
                    self.node(c);
                }
                self.out.push_str("</g>\n");
            }
            Node::MarkerGrid {
                template,
                x,
                y,
                step,
                cols,
                bits,
            } => {
                let offsets: Vec<(f32, f32)> = grid_positions(*x, *y, *step, *cols, bits).collect();
                self.node(&Node::Instances {
                    template: template.clone(),
                    offsets,
                });
            }
            Node::Instances { template, offsets } => {
                if offsets.is_empty() {
                    return;
                }
                self.clip_id += 1;
                let id = self.clip_id;
                let _ = write!(self.out, r#"<defs><g id="m{id}">"#);
                self.node(template);
                self.out.push_str("</g></defs>\n");
                for (x, y) in offsets {
                    let _ = writeln!(
                        self.out,
                        r##"<use href="#m{id}" x="{}" y="{}"/>"##,
                        num(*x as f64),
                        num(*y as f64)
                    );
                }
            }
        }
    }

    fn text(&mut self, t: &TextNode) {
        if t.text.is_empty() || t.size <= 0.0 || !t.x.is_finite() || !t.y.is_finite() {
            return;
        }
        let anchor = match t.anchor {
            Anchor::Start => "",
            Anchor::Middle => r#" text-anchor="middle""#,
            Anchor::End => r#" text-anchor="end""#,
        };
        // Baselines are resolved to an alphabetic baseline offset with
        // typical font metrics, which every SVG renderer supports.
        let dy = match t.baseline {
            Baseline::Alphabetic => 0.0,
            Baseline::Top => t.size * 0.76,
            Baseline::Middle => t.size * 0.35,
            Baseline::Bottom => -t.size * 0.24,
        };
        let rotate = if t.rotate != 0.0 {
            format!(
                r#" transform="rotate({} {} {})""#,
                num(t.rotate),
                num(t.x),
                num(t.y)
            )
        } else {
            String::new()
        };
        let weight = match (t.bold, t.italic) {
            (false, false) => "",
            (true, false) => r#" font-weight="bold""#,
            (false, true) => r#" font-style="italic""#,
            (true, true) => r#" font-weight="bold" font-style="italic""#,
        };
        let _ = writeln!(
            self.out,
            r#"<text x="{}" y="{}" font-size="{}"{anchor}{weight}{}{rotate}>{}</text>"#,
            num(t.x),
            num(t.y + dy),
            num(t.size),
            paint("fill", t.color),
            esc(&t.text)
        );
    }
}

pub(crate) fn encode_png(image: &RasterImage) -> Option<Vec<u8>> {
    use image::{ImageBuffer, ImageFormat, Rgba};
    let buf: ImageBuffer<Rgba<u8>, &[u8]> =
        ImageBuffer::from_raw(image.width, image.height, &image.rgba[..])?;
    let mut out = std::io::Cursor::new(Vec::new());
    buf.write_to(&mut out, ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

pub(crate) fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        s.push(T[(n >> 18) as usize & 63] as char);
        s.push(T[(n >> 12) as usize & 63] as char);
        s.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

/// Light direction of shaded spheres on screen (x right, y down, z
/// towards the viewer), normalized.
pub(crate) const SPHERE_LIGHT: [f32; 3] = [-0.4212, -0.5617, 0.7121];

/// Color of a sphere point with unit normal `(nx, ny, nz)` (screen axes).
#[inline]
pub(crate) fn sphere_shade(base: [f32; 3], nx: f32, ny: f32, nz: f32) -> [f32; 3] {
    let [lx, ly, lz] = SPHERE_LIGHT;
    let lam = (nx * lx + ny * ly + nz * lz).max(0.0);
    // Half vector of the light and the viewer (0, 0, 1).
    let (hx, hy, hz) = (lx * 0.5404, ly * 0.5404, (lz + 1.0) * 0.5404);
    let h = (nx * hx + ny * hy + nz * hz).max(0.0);
    let h2 = h * h;
    let h4 = h2 * h2;
    let h8 = h4 * h4;
    let spec = h8 * h8 * h4 * 0.75;
    let d = 0.5 + 0.5 * (lam * 1.25).min(1.0).powf(0.8);
    let mut out = [0.0; 3];
    for (o, b) in out.iter_mut().zip(base) {
        let c = b * d;
        *o = c + (255.0 - c) * spec;
    }
    out
}

/// Tube segments as overlapping strokes (a dark rim and lighter bands
/// offset towards the light), for back ends without per-pixel shading.
pub(crate) fn tube_layers(segs: &[Segment], width: f64, color: Color) -> Vec<Node> {
    let stroke = |c: Color, w: f64| {
        Stroke::new(c, w.max(0.3))
            .cap(LineCap::Round)
            .join(LineJoin::Round)
    };
    let path = |o: f64| {
        let mut cmds = Vec::with_capacity(segs.len() * 2);
        for &(a, b) in segs {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l = dx.hypot(dy).max(1e-9);
            let (mut nx, mut ny) = (-dy / l, dx / l);
            if nx * f64::from(SPHERE_LIGHT[0]) + ny * f64::from(SPHERE_LIGHT[1]) < 0.0 {
                (nx, ny) = (-nx, -ny);
            }
            cmds.push(PathCmd::MoveTo(a.0 + nx * o, a.1 + ny * o));
            cmds.push(PathCmd::LineTo(b.0 + nx * o, b.1 + ny * o));
        }
        cmds
    };
    // Thin tubes need fewer bands for the same look.
    let three = [
        (1.0, 0.0, color.darken(0.45)),
        (0.72, 0.1, color),
        (0.36, 0.22, color.lighten(0.38)),
    ];
    let two = [
        (1.0, 0.0, color.darken(0.35)),
        (0.5, 0.16, color.lighten(0.15)),
    ];
    let bands: &[(f64, f64, Color)] = if width < 7.0 { &two } else { &three };
    bands
        .iter()
        .map(|&(wf, of, c)| Node::Path {
            cmds: path(width * of),
            fill: None,
            stroke: Some(stroke(c, width * wf)),
        })
        .collect()
}

/// A sphere as stacked circles, for back ends without per-pixel shading.
pub(crate) fn sphere_layers(cx: f64, cy: f64, r: f64, color: Color) -> Vec<Node> {
    let rim = color.darken(0.5);
    let steps = ((r / 1.4) as usize).clamp(3, 18);
    let (ox, oy) = (SPHERE_LIGHT[0] as f64 * 0.62, SPHERE_LIGHT[1] as f64 * 0.62);
    let mut out = Vec::with_capacity(steps + 1);
    out.push(Node::Circle {
        cx,
        cy,
        r,
        fill: Some(rim),
        stroke: None,
    });
    for k in 1..=steps {
        let t = k as f64 / steps as f64;
        // Radius shrinks, centre moves towards the highlight.
        let rr = r * (1.0 - 0.9 * t);
        let off = r * t * 0.55;
        let c = if t < 0.72 {
            rim.mix(color, (t / 0.72).powf(0.7))
        } else {
            color.mix(color.lighten(0.6), (t - 0.72) / 0.28)
        };
        out.push(Node::Circle {
            cx: cx + ox * off,
            cy: cy + oy * off,
            r: rr,
            fill: Some(c.with_alpha(color.a)),
            stroke: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_format() {
        let n = |v: f64| num(v).to_string();
        assert_eq!(n(1.0), "1");
        assert_eq!(n(1.256), "1.26");
        assert_eq!(n(-0.001), "0");
        assert_eq!(n(f64::NAN), "0");
        assert_eq!(n(-12.5), "-12.5");
        assert_eq!(n(0.07), "0.07");
        assert_eq!(n(-0.3), "-0.3");
        assert_eq!(n(1200.0), "1200");
        assert_eq!(n(1e16), "10000000000000000");
    }

    #[test]
    fn base64_encoding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn svg_output() {
        let mut s = Scene::new(10.0, 20.0);
        s.push(Node::Text(TextNode {
            x: 1.0,
            y: 2.0,
            text: "a<b".into(),
            size: 10.0,
            color: Color::rgba(1, 2, 3, 0.5),
            anchor: Anchor::Middle,
            baseline: Baseline::Alphabetic,
            rotate: -90.0,
            bold: true,
            italic: false,
        }));
        s.push(Node::Group {
            children: vec![Node::Circle {
                cx: 1.0,
                cy: 1.0,
                r: 1.0,
                fill: Some(Color::RED),
                stroke: None,
            }],
            clip: Some((0.0, 0.0, 5.0, 5.0)),
            opacity: 0.5,
            tooltip: Some("tip".into()),
            id: None,
        });
        let svg = s.to_svg();
        assert!(svg.contains("a&lt;b"));
        assert!(svg.contains(r#"fill-opacity="0.5""#));
        assert!(svg.contains(r#"rotate(-90 1 2)"#));
        assert!(svg.contains(r#"clip-path="url(#c1)""#));
        assert!(svg.contains("<title>tip</title>"));
    }
}
