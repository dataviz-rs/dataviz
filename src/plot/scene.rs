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
            font: &self.font_family,
        };
        let _ = write!(
            w.out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{0}" height="{1}" viewBox="0 0 {0} {1}" font-family="{2}">"#,
            num(self.width),
            num(self.height),
            esc(&font_list(&self.font_family)),
        );
        w.out.push('\n');
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
            } => {
                let mut attrs = String::new();
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
        let weight = if t.bold { r#" font-weight="bold""# } else { "" };
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
        });
        let svg = s.to_svg();
        assert!(svg.contains("a&lt;b"));
        assert!(svg.contains(r#"fill-opacity="0.5""#));
        assert!(svg.contains(r#"rotate(-90 1 2)"#));
        assert!(svg.contains(r#"clip-path="url(#c1)""#));
        assert!(svg.contains("<title>tip</title>"));
    }
}
