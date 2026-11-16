//! Plot series (one per chart type) and the trait they implement.

use super::axis::AxisMap;
use super::color::{Color, Colormap};
use super::scene::{Anchor, Baseline, Node, PathCmd, Stroke, TextNode};
use super::style::{Dash, Marker};
use super::text::FontSet;
use super::theme::Theme;
use std::any::Any;

pub mod bar;
pub mod bio;
pub mod bio3d;
pub mod contour;
pub mod distribution;
pub mod engineering;
pub mod evaluation;
pub mod finance;
pub mod frameless;
pub mod general;
pub mod genome;
pub mod grid;
pub mod line;
pub mod misc;
pub mod ml;
pub mod network;
pub mod polar;
pub mod scatter;
pub mod signal;
pub mod stats;
pub mod three_d;
pub mod trading;
pub mod volume;

/// Data extent of a series, with "sticky" edges that get no margin
/// (e.g. the zero baseline of bars).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extent {
    /// Minimum x.
    pub x_min: f64,
    /// Maximum x.
    pub x_max: f64,
    /// Minimum y.
    pub y_min: f64,
    /// Maximum y.
    pub y_max: f64,
    /// No margin below `x_min`.
    pub sticky_x_min: bool,
    /// No margin above `x_max`.
    pub sticky_x_max: bool,
    /// No margin below `y_min`.
    pub sticky_y_min: bool,
    /// No margin above `y_max`.
    pub sticky_y_max: bool,
}

impl Extent {
    /// Extent of finite points accepted by `accept_x`/`accept_y`.
    pub fn of_points<I: IntoIterator<Item = (f64, f64)>>(points: I) -> Option<Self> {
        let mut e: Option<Extent> = None;
        for (x, y) in points {
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            e = Some(match e {
                None => Extent::point(x, y),
                Some(e) => e.include(x, y),
            });
        }
        e
    }

    /// Extent of one point.
    pub fn point(x: f64, y: f64) -> Self {
        Self {
            x_min: x,
            x_max: x,
            y_min: y,
            y_max: y,
            sticky_x_min: false,
            sticky_x_max: false,
            sticky_y_min: false,
            sticky_y_max: false,
        }
    }

    /// Extends to include `(x, y)` (non-finite coordinates are ignored).
    pub fn include(mut self, x: f64, y: f64) -> Self {
        if x.is_finite() {
            self.x_min = self.x_min.min(x);
            self.x_max = self.x_max.max(x);
        }
        if y.is_finite() {
            self.y_min = self.y_min.min(y);
            self.y_max = self.y_max.max(y);
        }
        self
    }

    /// Union of two extents.
    pub fn union(self, o: Extent) -> Self {
        let pick = |a: f64, sa: bool, b: f64, sb: bool, lower: bool| {
            if a == b {
                (a, sa || sb)
            } else if (a < b) == lower {
                (a, sa)
            } else {
                (b, sb)
            }
        };
        let (x_min, sx0) = pick(self.x_min, self.sticky_x_min, o.x_min, o.sticky_x_min, true);
        let (x_max, sx1) = pick(
            self.x_max,
            self.sticky_x_max,
            o.x_max,
            o.sticky_x_max,
            false,
        );
        let (y_min, sy0) = pick(self.y_min, self.sticky_y_min, o.y_min, o.sticky_y_min, true);
        let (y_max, sy1) = pick(
            self.y_max,
            self.sticky_y_max,
            o.y_max,
            o.sticky_y_max,
            false,
        );
        Self {
            x_min,
            x_max,
            y_min,
            y_max,
            sticky_x_min: sx0,
            sticky_x_max: sx1,
            sticky_y_min: sy0,
            sticky_y_max: sy1,
        }
    }

    /// Makes the y edges at `v` sticky (e.g. a zero baseline).
    pub fn sticky_y_at(mut self, v: f64) -> Self {
        if self.y_min == v {
            self.sticky_y_min = true;
        }
        if self.y_max == v {
            self.sticky_y_max = true;
        }
        self
    }

    /// Makes the x edges at `v` sticky.
    pub fn sticky_x_at(mut self, v: f64) -> Self {
        if self.x_min == v {
            self.sticky_x_min = true;
        }
        if self.x_max == v {
            self.sticky_x_max = true;
        }
        self
    }

    /// Swaps x and y (for horizontal variants).
    pub fn transposed(self) -> Self {
        Self {
            x_min: self.y_min,
            x_max: self.y_max,
            y_min: self.x_min,
            y_max: self.x_max,
            sticky_x_min: self.sticky_y_min,
            sticky_x_max: self.sticky_y_max,
            sticky_y_min: self.sticky_x_min,
            sticky_y_max: self.sticky_x_max,
        }
    }
}

/// How a series appears in the legend.
#[derive(Clone, Debug, PartialEq)]
pub enum LegendSymbol {
    /// A line sample, optionally with a marker.
    Line {
        /// Color.
        color: Color,
        /// Width.
        width: f64,
        /// Dash.
        dash: Dash,
        /// Marker shape and size.
        marker: Option<(Marker, f64)>,
    },
    /// A filled square.
    Patch {
        /// Fill.
        fill: Color,
        /// Border.
        edge: Option<Color>,
    },
    /// A marker.
    Marker {
        /// Shape.
        marker: Marker,
        /// Diameter.
        size: f64,
        /// Color.
        color: Color,
    },
}

/// One legend row.
#[derive(Clone, Debug, PartialEq)]
pub struct LegendEntry {
    /// Text.
    pub label: String,
    /// Symbol.
    pub symbol: LegendSymbol,
}

/// The data point nearest to the mouse, for hover tooltips.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// Canvas position of the point.
    pub px: f64,
    /// Canvas position of the point.
    pub py: f64,
    /// Tooltip text.
    pub label: String,
    /// Distance from the mouse in pixels.
    pub distance: f64,
}

/// A continuous color scale shown as a colorbar.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorScale {
    /// Colormap.
    pub colormap: Colormap,
    /// Value at the low end.
    pub min: f64,
    /// Value at the high end.
    pub max: f64,
    /// Colorbar label.
    pub label: Option<String>,
}

impl ColorScale {
    /// Color for a value.
    pub fn color(&self, v: f64) -> Color {
        let span = self.max - self.min;
        let t = if span == 0.0 {
            0.5
        } else {
            (v - self.min) / span
        };
        self.colormap.at(t)
    }
}

/// The plot rectangle in canvas pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Left.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl Rect {
    /// Centre.
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Right edge.
    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    /// Shrinks by `d` on every side.
    pub fn inset(&self, d: f64) -> Rect {
        Rect {
            x: self.x + d,
            y: self.y + d,
            w: (self.w - 2.0 * d).max(1.0),
            h: (self.h - 2.0 * d).max(1.0),
        }
    }

    /// Whether `(x, y)` is inside.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }
}

/// Everything a series needs to draw itself.
pub struct Context<'a> {
    /// x mapping.
    pub x: &'a AxisMap,
    /// y mapping (the secondary axis for series on it).
    pub y: &'a AxisMap,
    /// Plot area.
    pub rect: Rect,
    /// Theme.
    pub theme: &'a Theme,
    pub(crate) fonts: &'a FontSet,
    pub(crate) out: &'a mut Vec<Node>,
}

impl Context<'_> {
    /// Canvas position of a data point.
    pub fn point(&self, x: f64, y: f64) -> (f64, f64) {
        (self.x.px_clamped(x), self.y.px_clamped(y))
    }

    /// Adds a node.
    pub fn push(&mut self, node: Node) {
        self.out.push(node);
    }

    /// Width of a text in pixels.
    pub fn text_width(&self, text: &str, size: f64) -> f64 {
        self.fonts.width(text, size)
    }

    /// Adds a text.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        x: f64,
        y: f64,
        text: impl Into<String>,
        size: f64,
        color: Color,
        anchor: Anchor,
        baseline: Baseline,
    ) {
        self.out.push(Node::Text(TextNode {
            x,
            y,
            text: text.into(),
            size,
            color,
            anchor,
            baseline,
            rotate: 0.0,
            bold: false,
            italic: false,
        }));
    }

    /// Adds a polyline through canvas points; non-finite points split it.
    pub fn polyline(&mut self, points: &[(f64, f64)], stroke: Stroke) {
        for run in points.split(|(x, y)| !x.is_finite() || !y.is_finite()) {
            if run.len() < 2 {
                continue;
            }
            let cmds = run
                .iter()
                .enumerate()
                .map(|(i, &(x, y))| {
                    if i == 0 {
                        PathCmd::MoveTo(x, y)
                    } else {
                        PathCmd::LineTo(x, y)
                    }
                })
                .collect();
            self.out.push(Node::Path {
                cmds,
                fill: None,
                stroke: Some(stroke.clone()),
            });
        }
    }

    /// Adds a closed, filled polygon through canvas points.
    pub fn polygon(&mut self, points: &[(f64, f64)], fill: Option<Color>, stroke: Option<Stroke>) {
        let pts: Vec<_> = points
            .iter()
            .copied()
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .collect();
        if pts.len() < 3 {
            return;
        }
        let mut cmds: Vec<PathCmd> = pts
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                if i == 0 {
                    PathCmd::MoveTo(x, y)
                } else {
                    PathCmd::LineTo(x, y)
                }
            })
            .collect();
        cmds.push(PathCmd::Close);
        self.out.push(Node::Path { cmds, fill, stroke });
    }

    /// Adds a rectangle between two canvas corners.
    pub fn rect(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        fill: Option<Color>,
        stroke: Option<Stroke>,
    ) {
        if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
            return;
        }
        self.out.push(Node::Rect {
            x: x0.min(x1),
            y: y0.min(y1),
            w: (x1 - x0).abs(),
            h: (y1 - y0).abs(),
            rx: 0.0,
            fill,
            stroke,
        });
    }
}

/// A drawable data series. Implemented by every chart type; implement it to
/// add custom chart types.
pub trait Series: Any {
    /// Data extent (None = no data).
    fn extent(&self) -> Option<Extent>;
    /// Draws the series.
    fn draw(&self, ctx: &mut Context);
    /// Legend rows.
    fn legend(&self) -> Vec<LegendEntry> {
        Vec::new()
    }
    /// Nearest point to a canvas position, for hover tooltips.
    fn hit(&self, _x: &AxisMap, _y: &AxisMap, _rect: Rect, _px: f64, _py: f64) -> Option<Hit> {
        None
    }
    /// Whether the series uses the secondary y-axis.
    fn secondary(&self) -> bool {
        false
    }
    /// Continuous color scale to show as a colorbar.
    fn color_scale(&self) -> Option<ColorScale> {
        None
    }
    /// y-range of the data whose x lies in `[x0, x1]`, for
    /// [`Axes::auto_scale_y`](crate::plot::Axes::auto_scale_y). `None` (the
    /// default) means the whole extent is used; `Some(None)` means no data
    /// in that range.
    fn y_range_in(&self, _x0: f64, _x1: f64) -> Option<Option<(f64, f64)>> {
        None
    }
    /// What kind of chart the series is ("line", "scatter", …), for text
    /// descriptions. Default: the type name in lower case.
    fn kind(&self) -> &'static str {
        let full = std::any::type_name::<Self>();
        let name = full.rsplit("::").next().unwrap_or(full);
        match name {
            "Line" => "line",
            "Scatter" => "scatter",
            "Bars" => "bar",
            "Candles" => "candlestick",
            "Histogram" => "histogram",
            "FillBetween" => "area",
            "BoxPlot" => "box",
            "Violin" => "violin",
            "Heatmap" => "heatmap",
            "Pie" => "pie",
            "GeoMap" => "map",
            "Strip" => "strip",
            "Ridgeline" => "ridgeline",
            "Annotate" => "annotation",
            "SmithGrid" => "smith",
            "SequenceLogo" => "sequence logo",
            "GeneTrack" => "gene track",
            "ForestPlot" => "forest",
            "Dendrogram" => "dendrogram",
            _ => "data",
        }
    }
    /// A sentence about what the series shows, for text descriptions
    /// ([`Figure::alt_text`](crate::plot::Figure::alt_text)); `None` (the
    /// default) describes it by its name and y-range.
    fn describe(&self) -> Option<String> {
        None
    }
    /// Name shown for the series (legend label) in interactive output.
    fn name(&self) -> Option<String> {
        self.legend().into_iter().next().map(|e| e.label)
    }
    /// Up to about `max` data points (x, y) of the series, evenly spread,
    /// for hover readouts in interactive output (HTML). Default: none.
    fn data_points(&self, _max: usize) -> Vec<(f64, f64)> {
        Vec::new()
    }
    /// Every data point in row order, gaps included (NaN), for linked
    /// brushing ([`crate::plot::figure::Figure::link_brushing`]): row `i`
    /// of series with the same number of rows is the same record.
    /// Default: none (the series does not take part).
    fn row_points(&self) -> Vec<(f64, f64)> {
        Vec::new()
    }
    /// Data range of a 3D series (None for 2D series).
    fn bounds3d(&self) -> Option<three_d::Bounds3> {
        None
    }
    /// Draws a 3D series into the shared scene of its axes.
    fn draw3d(&self, _scene: &mut three_d::Scene3d) {}
    /// Nearest point of a 3D series to a canvas position.
    fn hit3d(&self, _proj: &three_d::Projection3d, _px: f64, _py: f64) -> Option<Hit> {
        None
    }
    /// For downcasting to the concrete type.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Up to about `max` of the points `(x[i], y[i])`, evenly spread (the
/// first and last always included).
pub(crate) fn spread_points(x: &[f64], y: &[f64], max: usize) -> Vec<(f64, f64)> {
    let n = x.len().min(y.len());
    if n == 0 || max == 0 {
        return Vec::new();
    }
    let step = n.div_ceil(max).max(1);
    let mut v: Vec<(f64, f64)> = (0..n).step_by(step).map(|i| (x[i], y[i])).collect();
    if !(n - 1).is_multiple_of(step) {
        v.push((x[n - 1], y[n - 1]));
    }
    v.retain(|(a, b)| a.is_finite() && b.is_finite());
    v
}

/// Points `(x[i], y[i])` of the common length, non-finite values kept.
pub(crate) fn row_pairs(x: &[f64], y: &[f64]) -> Vec<(f64, f64)> {
    x.iter().zip(y).map(|(a, b)| (*a, *b)).collect()
}

/// Conversion of numeric types to `f64`.
pub trait ToF64 {
    /// The value as `f64`.
    fn to_f64(&self) -> f64;
}

macro_rules! to_f64 {
    ($($t:ty),*) => {$(
        impl ToF64 for $t {
            fn to_f64(&self) -> f64 {
                *self as f64
            }
        }
    )*};
}
to_f64!(f64, f32, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

/// Missing values (`None`, e.g. nulls of Arrow and Polars arrays) become
/// gaps (NaN).
impl<T: ToF64> ToF64 for Option<T> {
    fn to_f64(&self) -> f64 {
        self.as_ref().map_or(f64::NAN, |v| v.to_f64())
    }
}

impl ToF64 for bool {
    fn to_f64(&self) -> f64 {
        if *self {
            1.0
        } else {
            0.0
        }
    }
}

impl<T: ToF64 + ?Sized> ToF64 for &T {
    fn to_f64(&self) -> f64 {
        (**self).to_f64()
    }
}

/// Anything that yields numbers: slices, vectors, arrays, ranges, iterators.
pub trait IntoValues {
    /// Collects the values.
    fn into_values(self) -> Vec<f64>;
}

impl<I> IntoValues for I
where
    I: IntoIterator,
    I::Item: ToF64,
{
    fn into_values(self) -> Vec<f64> {
        self.into_iter().map(|v| v.to_f64()).collect()
    }
}

/// `v` shortened to `n` items, keeping its allocation (no copy).
pub(crate) fn truncated(mut v: Vec<f64>, n: usize) -> Vec<f64> {
    v.truncate(n);
    v
}

/// y-range of the points `i` with `x[i]` in `[x0, x1]`, where `ys(i)`
/// gives the low and high y of point `i` (helper for `Series::y_range_in`).
pub(crate) fn y_range_where(
    x: &[f64],
    x0: f64,
    x1: f64,
    ys: impl Fn(usize) -> (f64, f64),
) -> Option<(f64, f64)> {
    let sorted = x.windows(2).all(|w| w[0] <= w[1]);
    let (a, b) = if sorted {
        (
            x.partition_point(|v| *v < x0),
            x.partition_point(|v| *v <= x1),
        )
    } else {
        (0, x.len())
    };
    let mut r: Option<(f64, f64)> = None;
    for (i, xv) in x.iter().enumerate().take(b).skip(a) {
        if !(*xv >= x0 && *xv <= x1) {
            continue;
        }
        let (lo, hi) = ys(i);
        if lo.is_finite() && hi.is_finite() {
            r = Some(r.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi))));
        }
    }
    r
}

/// Anything that yields labels.
pub trait IntoLabels {
    /// Collects the labels.
    fn into_labels(self) -> Vec<String>;
}

impl<I> IntoLabels for I
where
    I: IntoIterator,
    I::Item: ToString,
{
    fn into_labels(self) -> Vec<String> {
        self.into_iter().map(|v| v.to_string()).collect()
    }
}

/// Nearest of `points` (data coordinates) to a canvas position: the data
/// point, its canvas position and the distance.
#[allow(clippy::type_complexity)]
pub(crate) fn nearest_point(
    points: impl Iterator<Item = (f64, f64)>,
    x: &AxisMap,
    y: &AxisMap,
    px: f64,
    py: f64,
) -> Option<((f64, f64), (f64, f64), f64)> {
    points
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .map(|p| {
            let (cx, cy) = (x.px(p.0), y.px(p.1));
            (p, (cx, cy), (cx - px).hypot(cy - py))
        })
        .filter(|(_, _, d)| d.is_finite())
        .min_by(|a, b| a.2.total_cmp(&b.2))
}

/// Indices of the x-sorted values `xs` within `r` pixels of canvas `px`,
/// plus one more on each side (the nearest points outside), so that a hit
/// test looks at a few points instead of all. `None` when `xs` is not
/// sorted (then every point has to be checked).
pub(crate) fn x_window(
    xs: &[f64],
    shift: f64,
    x: &AxisMap,
    px: f64,
    r: f64,
) -> Option<std::ops::Range<usize>> {
    if !xs.is_sorted_by(|a, b| a <= b) {
        return None;
    }
    let (a, b) = (x.value(px - r) - shift, x.value(px + r) - shift);
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    if !lo.is_finite() || !hi.is_finite() {
        return Some(0..xs.len());
    }
    let start = xs.partition_point(|v| *v < lo).saturating_sub(1);
    let end = (xs.partition_point(|v| *v <= hi) + 1).min(xs.len());
    Some(start..end.max(start))
}

/// Short number for tooltips and labels.
pub(crate) fn short(v: f64) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    let a = v.abs();
    if a != 0.0 && (a >= 1e6 || a < 1e-3) {
        let s = format!("{v:.3e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = m.trim_end_matches('0').trim_end_matches('.');
        format!("{m}e{e}")
    } else {
        let s = format!("{v:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}
