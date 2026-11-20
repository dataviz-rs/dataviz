//! A single plot area: axes, series, legend, colorbar.

use super::axis::{Axis, AxisMap, TickSpec};
use super::color::{Color, Colormap};
use super::legend::{self, LegendPosition};
use super::scale::Scale;
use super::scene::{Anchor, Baseline, Node, PathCmd, Stroke, TextNode};
use super::series::bar::{BarMode, Bars};
use super::series::line::{FillBetween, Line};
use super::series::scatter::Scatter;
use super::series::{
    ColorScale, Context, Extent, IntoLabels, IntoValues, LegendEntry, Rect, Series,
};
use super::text::FontSet;
use super::theme::Theme;
use super::ticks;

/// What kind of coordinate frame an axes has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AxesKind {
    /// x/y axes (default).
    #[default]
    Cartesian,
    /// Angle/radius frame (polar and radar charts).
    Polar,
    /// No axes (pie, treemap, sankey, …).
    Frameless,
    /// 3D scene with a box, a camera and depth sorting.
    ThreeD,
}

/// Defaults copied from the figure theme when the axes is created.
#[derive(Clone, Debug)]
pub(crate) struct Defaults {
    pub palette: Vec<Color>,
    pub line_width: f64,
    pub marker_size: f64,
    pub colormap: Colormap,
}

impl Defaults {
    pub(crate) fn from_theme(t: &Theme) -> Self {
        Self {
            palette: t.palette.clone(),
            line_width: t.line_width,
            marker_size: t.marker_size,
            colormap: t.colormap,
        }
    }
}

/// One plot area with its axes and series. Get it from
/// [`Figure::axes`](crate::plot::Figure::axes) or
/// [`Figure::at`](crate::plot::Figure::at).
pub struct Axes {
    pub(crate) title: Option<String>,
    pub(crate) x: Axis,
    pub(crate) y: Axis,
    pub(crate) y2: Axis,
    pub(crate) series: Vec<Box<dyn Series>>,
    pub(crate) legend: LegendPosition,
    pub(crate) legend_columns: usize,
    pub(crate) next_color: usize,
    pub(crate) defaults: Defaults,
    pub(crate) kind: AxesKind,
    pub(crate) equal_aspect: bool,
    pub(crate) polar: super::series::polar::PolarFrame,
    pub(crate) show_colorbar: bool,
    /// y (and y2) fit the data inside the x range (`auto_scale_y`).
    pub(crate) fit_y: bool,
    /// Series hidden by [`Axes::set_visible`] (index = series index).
    pub(crate) hidden: Vec<bool>,
    /// Minimum distances of the plot area from the cell edges (left,
    /// right, top, bottom), set by the figure so that plots in the same
    /// grid row or column line up.
    pub(crate) align: Option<[f64; 4]>,
    /// Map projection (set by [`Axes::map`]).
    pub(crate) projection: Option<super::geo::Projection>,
    /// Draw the spines (frame) around the plot.
    pub(crate) frame: bool,
    /// Overview of all data below the plot with the shown x range.
    pub(crate) range_slider: bool,
    /// Camera and settings of the 3D scene.
    pub(crate) view3d: super::series::three_d::View3d,
    /// Changes of the series (added, changed, shown or hidden): keys the
    /// GPU mesh cache.
    pub(crate) revision: u64,
    /// Build 3D scenes of opaque faces as GPU meshes (set while rendering
    /// with the GPU back end).
    pub(crate) gpu_mesh: bool,
    pub(crate) mesh_cache: std::sync::Mutex<Option<super::series::three_d::MeshCache>>,
}

/// Limits of x, y and the secondary y-axis.
type AllLimits = ((f64, f64), (f64, f64), Option<(f64, f64)>);

/// Height of the range slider in pixels.
const SLIDER_H: f64 = 42.0;

/// Resolved layout of an axes (positions and mappings).
#[derive(Clone, Debug)]
pub struct Layout {
    /// Plot area.
    pub plot: Rect,
    /// x mapping.
    pub x: AxisMap,
    /// y mapping.
    pub y: AxisMap,
    /// Secondary y mapping (when used).
    pub y2: Option<AxisMap>,
    /// Legend rows: canvas rectangle and series index (for clicks).
    pub(crate) legend_items: Vec<(Rect, usize)>,
    /// Range slider: its rectangle and the mapping of all data's x range.
    pub(crate) slider: Option<(Rect, AxisMap)>,
    /// Area a 3D scene is drawn into (clip), for interactive output.
    pub(crate) clip3d: Option<Rect>,
}

impl Axes {
    pub(crate) fn new(theme: &Theme) -> Self {
        Self {
            title: None,
            x: Axis::default(),
            y: Axis::default(),
            y2: Axis::default(),
            series: Vec::new(),
            legend: LegendPosition::Best,
            legend_columns: 1,
            next_color: 0,
            defaults: Defaults::from_theme(theme),
            kind: AxesKind::Cartesian,
            equal_aspect: false,
            polar: Default::default(),
            show_colorbar: true,
            fit_y: false,
            hidden: Vec::new(),
            align: None,
            projection: None,
            frame: true,
            range_slider: false,
            view3d: Default::default(),
            revision: 0,
            gpu_mesh: false,
            mesh_cache: std::sync::Mutex::new(None),
        }
    }

    // ----- configuration -------------------------------------------------

    /// Title above the plot.
    pub fn title(&mut self, title: impl Into<String>) -> &mut Self {
        self.title = Some(title.into());
        self
    }
    /// x-axis label.
    pub fn x_label(&mut self, label: impl Into<String>) -> &mut Self {
        self.x.label(label);
        self
    }
    /// y-axis label.
    pub fn y_label(&mut self, label: impl Into<String>) -> &mut Self {
        self.y.label(label);
        self
    }
    /// Secondary (right) y-axis label.
    pub fn y2_label(&mut self, label: impl Into<String>) -> &mut Self {
        self.y2.label(label);
        self
    }
    /// x limits.
    pub fn x_limits(&mut self, min: f64, max: f64) -> &mut Self {
        self.x.limits(min, max);
        self
    }
    /// y limits.
    pub fn y_limits(&mut self, min: f64, max: f64) -> &mut Self {
        self.y.limits(min, max);
        self
    }
    /// Auto scale: the y-axes (without manual limits) fit only the data
    /// whose x lies in the visible x range, e.g. after zooming in or with a
    /// rolling time window (`x_limits`). Off by default (y fits all data).
    ///
    /// ```
    /// use dataviz::prelude::*;
    /// let mut fig = Figure::new(400, 300);
    /// let ax = fig.axes();
    /// ax.line([0.0, 1.0, 2.0, 3.0], [100.0, 1.0, 2.0, 3.0]);
    /// ax.x_limits(1.0, 3.0).auto_scale_y(true);
    /// let svg = fig.to_svg();   // y spans about 1..3, not 1..100
    /// assert!(!svg.contains(">100<"));
    /// ```
    pub fn auto_scale_y(&mut self, on: bool) -> &mut Self {
        self.fit_y = on;
        self
    }
    /// Whether [`Axes::auto_scale_y`] is on.
    pub fn auto_scales_y(&self) -> bool {
        self.fit_y
    }
    /// x scale.
    pub fn x_scale(&mut self, scale: Scale) -> &mut Self {
        self.x.scale(scale);
        self
    }
    /// y scale.
    pub fn y_scale(&mut self, scale: Scale) -> &mut Self {
        self.y.scale(scale);
        self
    }
    /// The x axis, for detailed configuration.
    pub fn x_axis(&mut self) -> &mut Axis {
        &mut self.x
    }
    /// The y axis, for detailed configuration.
    pub fn y_axis(&mut self) -> &mut Axis {
        &mut self.y
    }
    /// The secondary (right) y axis; series opt in with `.secondary(true)`.
    pub fn y2_axis(&mut self) -> &mut Axis {
        &mut self.y2
    }
    /// Legend position.
    pub fn legend(&mut self, position: LegendPosition) -> &mut Self {
        self.legend = position;
        self
    }
    /// Shows an overview of all data below the plot with the x range that
    /// is shown (a range slider). In a window it can be dragged to pan,
    /// its edges to zoom, and a click moves the view there.
    pub fn range_slider(&mut self, on: bool) -> &mut Self {
        self.range_slider = on;
        self
    }

    /// Limits of all the data (ignoring set limits and zoom).
    fn data_limits(&self) -> ((f64, f64), (f64, f64)) {
        let (mut x, mut y) = (self.x.clone(), self.y.clone());
        (x.min, x.max, y.min, y.max) = (None, None, None, None);
        let (x, y, _) = self.axis_limits_with(&x, &y, false);
        (x, y)
    }

    /// Shows or hides the frame (spines) around the plot.
    pub fn frame(&mut self, on: bool) -> &mut Self {
        self.frame = on;
        self
    }

    /// Number of legend columns (0 = as many as fit; default 1).
    pub fn legend_columns(&mut self, columns: usize) -> &mut Self {
        self.legend_columns = columns;
        self
    }
    /// Same pixel size for one unit on x and y.
    pub fn equal_aspect(&mut self, on: bool) -> &mut Self {
        self.equal_aspect = on;
        self
    }
    /// Grid on both axes on/off (overrides the theme).
    pub fn grid(&mut self, on: bool) -> &mut Self {
        self.x.grid(on);
        self.y.grid(on);
        self
    }
    /// Hides the colorbar of colormapped series.
    pub fn colorbar(&mut self, on: bool) -> &mut Self {
        self.show_colorbar = on;
        self
    }
    /// Frame kind (set automatically by polar, pie, … charts).
    pub fn kind(&mut self, kind: AxesKind) -> &mut Self {
        self.kind = kind;
        self
    }

    /// The next color of the cycle.
    pub fn next_color(&mut self) -> Color {
        let c = if self.defaults.palette.is_empty() {
            Color::BLUE
        } else {
            self.defaults.palette[self.next_color % self.defaults.palette.len()]
        };
        self.next_color += 1;
        c
    }

    /// Adds any series (including custom [`Series`] implementations) and
    /// returns it for further configuration.
    pub fn add<S: Series>(&mut self, series: S) -> &mut S {
        self.revision += 1;
        self.series.push(Box::new(series));
        self.series
            .last_mut()
            .and_then(|s| s.as_any_mut().downcast_mut::<S>())
            .expect("just added")
    }

    /// The last `n` series, which must all be of type `S`.
    pub(crate) fn last_n<S: Series>(&mut self, n: usize) -> Vec<&mut S> {
        self.revision += 1;
        let len = self.series.len();
        self.series[len - n..]
            .iter_mut()
            .filter_map(|s| s.as_any_mut().downcast_mut::<S>())
            .collect()
    }

    /// The bar series added by the last call of a multi-series bar method.
    pub fn last_series_bars(&mut self) -> Vec<&mut Bars> {
        let mut n = 0;
        for s in self.series.iter_mut().rev() {
            if s.as_any_mut().downcast_mut::<Bars>().is_none() {
                break;
            }
            n += 1;
        }
        self.last_n(n)
    }

    /// The `index`-th series if it has type `S` (e.g. to update data).
    pub fn series_mut<S: Series>(&mut self, index: usize) -> Option<&mut S> {
        self.revision += 1;
        self.series.get_mut(index)?.as_any_mut().downcast_mut::<S>()
    }

    /// Number of series.
    pub fn series_count(&self) -> usize {
        self.series.len()
    }

    /// Shows or hides the `index`-th series. A hidden series is not drawn,
    /// does not count for the automatic limits, and its legend row is
    /// dimmed (clicking a legend row in a window toggles this).
    pub fn set_visible(&mut self, index: usize, visible: bool) -> &mut Self {
        self.revision += 1;
        if self.hidden.len() <= index {
            self.hidden.resize(index + 1, false);
        }
        self.hidden[index] = !visible;
        self
    }

    /// Whether the `index`-th series is drawn (see [`Axes::set_visible`]).
    pub fn is_visible(&self, index: usize) -> bool {
        !self.hidden.get(index).copied().unwrap_or(false)
    }

    /// The series that are drawn.
    pub(crate) fn shown(&self) -> impl Iterator<Item = &dyn Series> {
        self.series
            .iter()
            .enumerate()
            .filter(|(i, _)| self.is_visible(*i))
            .map(|(_, s)| s.as_ref())
    }

    // ----- basic chart types ---------------------------------------------

    /// Line through `(x[i], y[i])`.
    ///
    /// ```
    /// use dataviz::prelude::*;
    /// let mut fig = Figure::new(640, 480);
    /// fig.axes().line([0, 1, 2, 3], [1.0, 3.0, 2.0, 4.0]).label("data").dash(Dash::Dashed);
    /// assert!(fig.to_svg().contains("<path"));
    /// ```
    pub fn line(&mut self, x: impl IntoValues, y: impl IntoValues) -> &mut Line {
        let c = self.next_color();
        let (w, m) = (self.defaults.line_width, self.defaults.marker_size);
        self.add(Line::new(x.into_values(), y.into_values(), c, w, m))
    }

    /// Line through `y` values at x = 0, 1, 2, …
    pub fn line_y(&mut self, y: impl IntoValues) -> &mut Line {
        let y = y.into_values();
        let x: Vec<f64> = (0..y.len()).map(|i| i as f64).collect();
        self.line(x, y)
    }

    /// Line over string categories on the x-axis.
    pub fn line_categories(
        &mut self,
        categories: impl IntoLabels,
        y: impl IntoValues,
    ) -> &mut Line {
        let x: Vec<f64> = categories
            .into_labels()
            .iter()
            .map(|c| self.x.category_index(c))
            .collect();
        self.line(x, y)
    }

    /// Plot of a function `y = f(x)` over `[x0, x1]`, sampled adaptively.
    pub fn function(&mut self, f: impl Fn(f64) -> f64, x0: f64, x1: f64) -> &mut Line {
        let (x, y) = super::series::misc::sample_function(&f, x0, x1, 400);
        let line = self.line(x, y);
        line.decimate = false;
        line
    }

    /// Area between the line and zero.
    pub fn area(&mut self, x: impl IntoValues, y: impl IntoValues) -> &mut Line {
        let l = self.line(x, y);
        l.fill = Some(0.0);
        l
    }

    /// Area between two curves.
    pub fn fill_between(
        &mut self,
        x: impl IntoValues,
        y1: impl IntoValues,
        y2: impl IntoValues,
    ) -> &mut FillBetween {
        let c = self.next_color();
        self.add(FillBetween::new(
            x.into_values(),
            y1.into_values(),
            y2.into_values(),
            c,
        ))
    }

    /// Stacked areas: one layer per `(label, values)` over shared `x`.
    /// With `percent`, every x sums to 100 %.
    pub fn stacked_area<L: Into<String>>(
        &mut self,
        x: impl IntoValues,
        layers: impl IntoIterator<Item = (L, Vec<f64>)>,
        percent: bool,
    ) -> Vec<&mut FillBetween> {
        let x = x.into_values();
        let layers: Vec<(String, Vec<f64>)> =
            layers.into_iter().map(|(l, v)| (l.into(), v)).collect();
        let n = x.len();
        let totals: Vec<f64> = (0..n)
            .map(|i| {
                layers
                    .iter()
                    .map(|(_, v)| v.get(i).copied().unwrap_or(0.0).max(0.0))
                    .sum()
            })
            .collect();
        let mut base = vec![0.0; n];
        for (label, values) in &layers {
            let top: Vec<f64> = (0..n)
                .map(|i| {
                    let mut v = values.get(i).copied().unwrap_or(0.0).max(0.0);
                    if percent {
                        v = if totals[i] > 0.0 {
                            v / totals[i] * 100.0
                        } else {
                            0.0
                        };
                    }
                    base[i] + v
                })
                .collect();
            let c = self.next_color();
            let f = self.add(FillBetween::new(x.clone(), top.clone(), base.clone(), c));
            f.opacity = 0.85;
            f.label = Some(label.clone());
            base = top;
        }
        if percent {
            self.y.limits(0.0, 100.0);
        }
        self.last_n(layers.len())
    }

    /// Scatter plot.
    pub fn scatter(&mut self, x: impl IntoValues, y: impl IntoValues) -> &mut Scatter {
        let c = self.next_color();
        let (m, cm) = (self.defaults.marker_size, self.defaults.colormap);
        self.add(Scatter::new(x.into_values(), y.into_values(), c, m, cm))
    }

    /// Vertical bars over string categories.
    pub fn bar(&mut self, categories: impl IntoLabels, heights: impl IntoValues) -> &mut Bars {
        let names = categories.into_labels();
        let pos: Vec<f64> = names.iter().map(|c| self.x.category_index(c)).collect();
        let c = self.next_color();
        let b = self.add(Bars::new(pos, heights.into_values(), c));
        b.width = 0.8;
        b.names = Some(names);
        b
    }

    /// Vertical bars at numeric x positions.
    pub fn bar_xy(&mut self, x: impl IntoValues, heights: impl IntoValues) -> &mut Bars {
        let c = self.next_color();
        self.add(Bars::new(x.into_values(), heights.into_values(), c))
    }

    /// Horizontal bars over string categories (listed top to bottom).
    pub fn barh(&mut self, categories: impl IntoLabels, widths: impl IntoValues) -> &mut Bars {
        let names = categories.into_labels();
        let pos: Vec<f64> = names.iter().map(|c| self.y.category_index(c)).collect();
        self.y.inverted = true;
        let c = self.next_color();
        let b = self.add(Bars::new(pos, widths.into_values(), c));
        b.width = 0.8;
        b.horizontal = true;
        b.names = Some(names);
        b
    }

    /// Several bar series over the same categories: grouped, stacked,
    /// percent-stacked or overlaid. Horizontal with `horizontal = true`.
    pub fn bars<L: Into<String>>(
        &mut self,
        categories: impl IntoLabels,
        series: impl IntoIterator<Item = (L, Vec<f64>)>,
        mode: BarMode,
        horizontal: bool,
    ) -> Vec<&mut Bars> {
        let names = categories.into_labels();
        let axis = if horizontal { &mut self.y } else { &mut self.x };
        let pos: Vec<f64> = names.iter().map(|c| axis.category_index(c)).collect();
        if horizontal {
            self.y.inverted = true;
        }
        let series: Vec<(String, Vec<f64>)> =
            series.into_iter().map(|(l, v)| (l.into(), v)).collect();
        let k = series.len().max(1);
        let n = names.len();
        let totals: Vec<f64> = (0..n)
            .map(|i| {
                series
                    .iter()
                    .map(|(_, v)| v.get(i).copied().unwrap_or(0.0).abs())
                    .sum()
            })
            .collect();
        let mut pos_base = vec![0.0; n];
        let mut neg_base = vec![0.0; n];
        for (j, (label, values)) in series.iter().enumerate() {
            let mut heights: Vec<f64> = (0..n)
                .map(|i| values.get(i).copied().unwrap_or(f64::NAN))
                .collect();
            let raw = heights.clone();
            if mode == BarMode::Percent {
                for i in 0..n {
                    heights[i] = if totals[i] > 0.0 {
                        heights[i] / totals[i] * 100.0
                    } else {
                        0.0
                    };
                }
            }
            let c = self.next_color();
            let b = self.add(Bars::new(pos.clone(), heights.clone(), c));
            b.horizontal = horizontal;
            b.label = Some(label.clone());
            b.names = Some(names.clone());
            match mode {
                BarMode::Grouped => {
                    b.width = 0.8 / k as f64;
                    b.offset = -0.4 + b.width * (j as f64 + 0.5);
                }
                BarMode::Overlay => {
                    b.width = 0.8;
                    b.color = b.color.with_alpha(0.6);
                }
                BarMode::Stacked | BarMode::Percent => {
                    b.width = 0.8;
                    let mut bottoms = vec![0.0; n];
                    for i in 0..n {
                        let h = heights[i];
                        if h >= 0.0 {
                            bottoms[i] = pos_base[i];
                            pos_base[i] += h;
                        } else if h < 0.0 {
                            bottoms[i] = neg_base[i];
                            neg_base[i] += h;
                        }
                    }
                    b.bottoms = bottoms;
                    if mode == BarMode::Percent {
                        b.label_values = Some(raw);
                    }
                }
            }
        }
        if mode == BarMode::Percent {
            let axis = if horizontal { &mut self.x } else { &mut self.y };
            axis.limits(0.0, 100.0);
        }
        self.last_n(series.len())
    }

    // ----- layout & drawing ------------------------------------------------

    fn uses_y2(&self) -> bool {
        self.shown().any(|s| s.secondary())
    }

    fn extent(&self, secondary: bool) -> Option<Extent> {
        self.shown()
            .filter(|s| s.secondary() == secondary)
            .filter_map(|s| s.extent())
            .reduce(|a, b| a.union(b))
    }

    pub(crate) fn color_scale(&self) -> Option<ColorScale> {
        if !self.show_colorbar {
            return None;
        }
        self.shown().find_map(|s| s.color_scale())
    }

    /// Legend rows with the index of the series each belongs to.
    pub(crate) fn legend_entries(&self) -> (Vec<LegendEntry>, Vec<usize>) {
        let mut entries = Vec::new();
        let mut owners = Vec::new();
        for (i, s) in self.series.iter().enumerate() {
            for e in s.legend() {
                entries.push(e);
                owners.push(i);
            }
        }
        (entries, owners)
    }

    /// Automatic limits of one axis.
    pub(crate) fn limits(
        axis: &Axis,
        lo: Option<(f64, bool)>,
        hi: Option<(f64, bool)>,
    ) -> (f64, f64) {
        let scale = &axis.scale;
        if *scale == Scale::Category {
            // Half a category around the first and last, or further when
            // the data reaches beyond (ridgelines).
            let n = axis.categories.len().max(1) as f64;
            let ext_lo = lo.map_or(
                -0.5,
                |(v, _)| if v.is_finite() { v.min(-0.5) } else { -0.5 },
            );
            let ext_hi = hi.map_or(n - 0.5, |(v, _)| {
                if v.is_finite() {
                    v.max(n - 0.5)
                } else {
                    n - 0.5
                }
            });
            let lo = axis.min.unwrap_or(ext_lo);
            let hi = axis.max.unwrap_or(ext_hi);
            return order(lo, hi);
        }
        let (mut a, sa) = lo.unwrap_or((f64::NAN, false));
        let (mut b, sb) = hi.unwrap_or((f64::NAN, false));
        if scale.is_log() {
            if a.is_nan() || a <= 0.0 {
                a = if b > 0.0 { b / 1000.0 } else { 1.0 };
            }
            if b.is_nan() || b <= 0.0 {
                b = a * 10.0;
            }
        }
        if !a.is_finite() || !b.is_finite() {
            let (da, db): (f64, f64) = if *scale == Scale::Time {
                (0.0, 86400.0)
            } else if scale.is_log() {
                (1.0, 10.0)
            } else {
                (0.0, 1.0)
            };
            a = if a.is_finite() { a } else { da };
            b = if b.is_finite() { b } else { db.max(a + 1.0) };
        }
        let (mut t0, mut t1) = (scale.forward(a), scale.forward(b));
        if t1 - t0 <= 0.0 || !(t1 - t0).is_finite() {
            let pad = if *scale == Scale::Time {
                3600.0
            } else if scale.is_log() {
                0.5
            } else if t0 == 0.0 {
                1.0
            } else {
                t0.abs() * 0.1
            };
            t0 -= pad;
            t1 += pad;
        } else {
            // Margins are a fraction of what is shown (breaks left out).
            let gaps: f64 = axis
                .breaks
                .iter()
                .map(|(a, b)| (scale.forward(*a).max(t0), scale.forward(*b).min(t1)))
                .filter(|(a, b)| b > a)
                .map(|(a, b)| b - a)
                .sum();
            let span = (t1 - t0 - gaps).max((t1 - t0) * 1e-6);
            if !sa {
                t0 -= span * axis.margin;
            }
            if !sb {
                t1 += span * axis.margin;
            }
        }
        let (mut lo, mut hi) = (scale.inverse(t0), scale.inverse(t1));
        if let Some(m) = axis.min {
            lo = m;
        }
        if let Some(m) = axis.max {
            hi = m;
        }
        if scale.is_log() {
            lo = lo.max(f64::MIN_POSITIVE);
            hi = hi.max(lo * 10.0);
        }
        order(lo, hi)
    }

    #[allow(clippy::type_complexity)]
    fn axis_limits(&self) -> AllLimits {
        self.axis_limits_with(&self.x, &self.y, self.fit_y)
    }

    fn axis_limits_with(&self, x_axis: &Axis, y_axis: &Axis, fit_y: bool) -> AllLimits {
        let e = self.extent(false);
        let xl = e.map(|e| (e.x_min, e.sticky_x_min));
        let xh = e.map(|e| (e.x_max, e.sticky_x_max));
        let e2 = self.extent(true);
        let (xl, xh) = match (e2, xl, xh) {
            (Some(e2), Some(a), Some(b)) => (
                Some((a.0.min(e2.x_min), a.1)),
                Some((b.0.max(e2.x_max), b.1)),
            ),
            (Some(e2), _, _) => (
                Some((e2.x_min, e2.sticky_x_min)),
                Some((e2.x_max, e2.sticky_x_max)),
            ),
            _ => (xl, xh),
        };
        let x = Self::limits(x_axis, xl, xh);
        // Auto scale: y from the data inside the visible x range.
        let visible = |secondary: bool, e: Option<Extent>| -> Option<Extent> {
            if !fit_y || self.kind != AxesKind::Cartesian {
                return e;
            }
            let mut r: Option<(f64, f64)> = None;
            for s in self.shown().filter(|s| s.secondary() == secondary) {
                let part = match s.y_range_in(x.0, x.1) {
                    Some(p) => p,
                    None => s.extent().map(|e| (e.y_min, e.y_max)),
                };
                if let Some((a, b)) = part {
                    r = Some(r.map_or((a, b), |(lo, hi)| (lo.min(a), hi.max(b))));
                }
            }
            match (r, e) {
                (Some((a, b)), Some(mut e)) => {
                    // A sticky edge (bar baseline) stays sticky if visible.
                    e.sticky_y_min &= a == e.y_min;
                    e.sticky_y_max &= b == e.y_max;
                    e.y_min = a;
                    e.y_max = b;
                    Some(e)
                }
                _ => e,
            }
        };
        let (e, e2) = (visible(false, e), visible(true, e2));
        let y = Self::limits(
            y_axis,
            e.map(|e| (e.y_min, e.sticky_y_min)),
            e.map(|e| (e.y_max, e.sticky_y_max)),
        );
        let y2 = self.uses_y2().then(|| {
            Self::limits(
                &self.y2,
                e2.map(|e| (e.y_min, e.sticky_y_min)),
                e2.map(|e| (e.y_max, e.sticky_y_max)),
            )
        });
        (x, y, y2)
    }

    fn tick_list(
        axis: &Axis,
        min: f64,
        max: f64,
        length_px: f64,
        spacing: f64,
    ) -> (Vec<(f64, String)>, Vec<f64>) {
        if !axis.visible {
            return (Vec::new(), Vec::new());
        }
        if !axis.breaks.is_empty() && axis.scale != Scale::Category {
            // Ticks for each visible segment, in proportion to its length.
            let (lo, hi) = order(min, max);
            let mut cuts: Vec<(f64, f64)> = axis
                .breaks
                .iter()
                .copied()
                .filter(|(a, b)| *a > lo && *b < hi)
                .collect();
            cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut segs = Vec::new();
            let mut start = lo;
            for (a, b) in cuts {
                if a > start {
                    segs.push((start, a));
                }
                start = start.max(b);
            }
            segs.push((start, hi));
            let f = |v: f64| axis.scale.forward(v);
            let total: f64 = segs.iter().map(|(a, b)| f(*b) - f(*a)).sum();
            let mut plain = axis.clone();
            plain.breaks.clear();
            let (mut major, mut minor) = (Vec::new(), Vec::new());
            for (a, b) in segs {
                let len = length_px * (f(b) - f(a)) / total.max(f64::MIN_POSITIVE);
                if len < spacing * 0.4 {
                    continue;
                }
                let (t, m) = Self::tick_list(&plain, a, b, len.max(spacing * 1.5), spacing);
                major.extend(t);
                minor.extend(m);
            }
            return (major, minor);
        }
        if axis.scale == Scale::Category {
            if let TickSpec::Labeled(l) = &axis.ticks {
                return (l.clone(), Vec::new());
            }
            let ticks = axis
                .categories
                .iter()
                .enumerate()
                .map(|(i, c)| (i as f64, c.clone()))
                .filter(|(v, _)| *v >= min.min(max) - 1e-9 && *v <= min.max(max) + 1e-9)
                .collect();
            return (ticks, Vec::new());
        }
        let target = ((length_px / spacing).round() as usize).clamp(2, 12);
        let target = match axis.ticks {
            TickSpec::Count(n) => n.max(1),
            _ => target,
        };
        let set = ticks::ticks(&axis.scale, min, max, target);
        let max_abs = min.abs().max(max.abs());
        let fmt = |v: f64| {
            ticks::format_value(
                v,
                &axis.format,
                &axis.scale,
                set.step,
                max_abs,
                set.time_format,
            )
        };
        let major: Vec<(f64, String)> = match &axis.ticks {
            TickSpec::None => Vec::new(),
            TickSpec::At(v) => v.iter().map(|v| (*v, fmt(*v))).collect(),
            TickSpec::Labeled(l) => l.clone(),
            _ => set.major.iter().map(|v| (*v, fmt(*v))).collect(),
        };
        let minor = if axis.minor_ticks {
            set.minor
        } else {
            Vec::new()
        };
        (major, minor)
    }

    /// Computes the plot rectangle and axis mappings inside `cell`.
    pub(crate) fn layout(&self, cell: Rect, theme: &Theme, fonts: &FontSet) -> (Layout, Decor) {
        let mut decor = Decor::default();
        let mut r = cell;
        if let Some(t) = &self.title {
            let h = fonts.height(theme.axes_title_size) + 6.0;
            decor.title = Some((t.clone(), cell.x + cell.w / 2.0, cell.y + h * 0.5));
            r.y += h;
            r.h -= h;
        }

        let (entries, owners) = self.legend_entries();
        let legend_on = !entries.is_empty() && self.legend != LegendPosition::Hidden;
        if legend_on {
            let horizontal = matches!(
                self.legend,
                LegendPosition::OutsideBottom | LegendPosition::OutsideTop
            );
            let cols = if horizontal && self.legend_columns == 1 {
                0
            } else {
                self.legend_columns
            };
            let l = legend::layout(&entries, theme, fonts, cols, r.w);
            match self.legend {
                LegendPosition::OutsideRight => r.w -= l.w + 10.0,
                LegendPosition::OutsideBottom => r.h -= l.h + 8.0,
                LegendPosition::OutsideTop => {
                    r.y += l.h + 8.0;
                    r.h -= l.h + 8.0;
                }
                _ => {}
            }
            decor.legend = Some((entries, owners, l));
        }

        let cs = self.color_scale();
        if let Some(cs) = &cs {
            let w = legend::colorbar_width(cs, theme, fonts);
            r.w -= w;
            decor.colorbar = Some((cs.clone(), w));
        }

        if self.kind == AxesKind::Polar {
            let (plot, x, y) = self.polar.maps(self, r, theme);
            return (
                Layout {
                    plot,
                    x,
                    y,
                    y2: None,
                    legend_items: Vec::new(),
                    slider: None,
                    clip3d: None,
                },
                decor,
            );
        }
        if self.kind != AxesKind::Cartesian {
            let plot = r.inset(4.0);
            let (x, y, _) = self.axis_limits();
            let lay = Layout {
                x: AxisMap::new(Scale::Linear, x.0, x.1, plot.x, plot.right()),
                y: AxisMap::new(Scale::Linear, y.0, y.1, plot.bottom(), plot.y),
                y2: None,
                plot,
                legend_items: Vec::new(),
                slider: None,
                clip3d: None,
            };
            return (lay, decor);
        }

        let (xl, yl, y2l) = self.axis_limits();
        let tick_gap = theme.tick_length.max(0.0) + 4.0;
        let ts = theme.tick_size;
        let th = fonts.height(ts);

        // Two passes: tick labels depend on the plot size and vice versa.
        let mut plot = r;
        let mut x_rot = 0.0;
        for _ in 0..2 {
            let (yt, _) = Self::tick_list(&self.y, yl.0, yl.1, plot.h, 48.0);
            let yw = yt
                .iter()
                .map(|(_, l)| fonts.width(l, ts))
                .fold(0.0, f64::max);
            let mut left = if self.y.visible { yw + tick_gap } else { 4.0 };
            if self.y.label.is_some() {
                left += fonts.height(theme.label_size) + 4.0;
            }
            let mut right = 8.0;
            if let Some(y2l) = y2l {
                let (t2, _) = Self::tick_list(&self.y2, y2l.0, y2l.1, plot.h, 48.0);
                right = t2
                    .iter()
                    .map(|(_, l)| fonts.width(l, ts))
                    .fold(0.0, f64::max)
                    + tick_gap
                    + 4.0;
                if self.y2.label.is_some() {
                    right += fonts.height(theme.label_size) + 4.0;
                }
            }
            let (xt, _) = Self::tick_list(&self.x, xl.0, xl.1, plot.w, 90.0);
            let widths: Vec<f64> = xt.iter().map(|(_, l)| fonts.width(l, ts)).collect();
            let maxw = widths.iter().copied().fold(0.0, f64::max);
            let spacing = if xt.len() > 1 {
                plot.w / xt.len() as f64
            } else {
                plot.w
            };
            x_rot = match self.x.tick_rotation {
                Some(r) => r,
                None if maxw + 6.0 > spacing && xt.len() > 1 => -45.0,
                None => 0.0,
            };
            let x_h = if x_rot == 0.0 {
                th
            } else {
                let a = x_rot.to_radians().abs();
                maxw * a.sin() + th * a.cos()
            };
            // Last x label may overhang the right edge.
            if x_rot == 0.0 {
                right = right.max(widths.last().copied().unwrap_or(0.0) / 2.0);
            }
            let mut bottom = if self.x.visible {
                x_h + tick_gap + 2.0
            } else {
                4.0
            };
            if self.x.label.is_some() {
                bottom += fonts.height(theme.label_size) + 4.0;
            }
            if self.range_slider {
                bottom += SLIDER_H + 10.0;
            }
            let top = th / 2.0 + 2.0;
            plot = Rect {
                x: r.x + left,
                y: r.y + top,
                w: (r.w - left - right).max(10.0),
                h: (r.h - top - bottom).max(10.0),
            };
        }
        decor.x_rotation = x_rot;
        if let Some([l, rt, t, b]) = self.align {
            let l = l.max(plot.x - cell.x);
            let rt = rt.max(cell.right() - plot.right());
            let t = t.max(plot.y - cell.y);
            let b = b.max(cell.bottom() - plot.bottom());
            plot = Rect {
                x: cell.x + l,
                y: cell.y + t,
                w: (cell.w - l - rt).max(10.0),
                h: (cell.h - t - b).max(10.0),
            };
        }

        let (mut xl, mut yl) = (xl, yl);
        if self.equal_aspect && self.x.scale == Scale::Linear && self.y.scale == Scale::Linear {
            let sx = plot.w / (xl.1 - xl.0);
            let sy = plot.h / (yl.1 - yl.0);
            if sx > sy {
                let c = (xl.0 + xl.1) / 2.0;
                let half = plot.w / sy / 2.0;
                xl = (c - half, c + half);
            } else {
                let c = (yl.0 + yl.1) / 2.0;
                let half = plot.h / sx / 2.0;
                yl = (c - half, c + half);
            }
        }
        let (x0, x1) = if self.x.inverted {
            (plot.right(), plot.x)
        } else {
            (plot.x, plot.right())
        };
        let (y0, y1) = if self.y.inverted {
            (plot.y, plot.bottom())
        } else {
            (plot.bottom(), plot.y)
        };
        let lay = Layout {
            x: AxisMap::new(self.x.scale.clone(), xl.0, xl.1, x0, x1).with_breaks(&self.x.breaks),
            y: AxisMap::new(self.y.scale.clone(), yl.0, yl.1, y0, y1).with_breaks(&self.y.breaks),
            y2: y2l.map(|l| {
                let (a, b) = if self.y2.inverted {
                    (plot.y, plot.bottom())
                } else {
                    (plot.bottom(), plot.y)
                };
                AxisMap::new(self.y2.scale.clone(), l.0, l.1, a, b)
            }),
            slider: self.range_slider.then(|| {
                let rect = Rect {
                    x: plot.x,
                    y: r.bottom() - SLIDER_H,
                    w: plot.w,
                    h: SLIDER_H,
                };
                let (dx, _) = self.data_limits();
                let map = AxisMap::new(self.x.scale.clone(), dx.0, dx.1, plot.x, plot.right())
                    .with_breaks(&self.x.breaks);
                (rect, map)
            }),
            clip3d: None,
            plot,
            legend_items: Vec::new(),
        };
        (lay, decor)
    }

    /// Draws everything into `out`.
    pub(crate) fn render(
        &self,
        cell: Rect,
        theme: &Theme,
        fonts: &FontSet,
        out: &mut Vec<Node>,
    ) -> Layout {
        let (mut lay, decor) = self.layout(cell, theme, fonts);
        let p = lay.plot;

        if self.kind == AxesKind::Cartesian {
            out.push(Node::Rect {
                x: p.x,
                y: p.y,
                w: p.w,
                h: p.h,
                rx: 0.0,
                fill: Some(theme.plot_background),
                stroke: None,
            });
            let g0 = out.len();
            self.draw_grid(&lay, theme, out);
            group_tail(out, g0, "grid");
        } else if self.kind == AxesKind::Polar {
            self.polar.draw_frame(self, &lay, theme, fonts, out, true);
        }

        // Series, clipped to the plot area; each in a group with an id
        // (`s<index>`, for interactive output).
        let mut data = Vec::new();
        if self.kind == AxesKind::ThreeD {
            let mut nodes = Vec::new();
            self.render3d(p, theme, fonts, &mut nodes);
            data.push(Node::Group {
                children: nodes,
                clip: None,
                opacity: 1.0,
                tooltip: None,
                id: Some("s3d".into()),
            });
        }
        for (k, s) in self.series.iter().enumerate() {
            if self.kind == AxesKind::ThreeD {
                break;
            }
            if !self.is_visible(k) {
                continue;
            }
            let y = if s.secondary() {
                lay.y2.as_ref().unwrap_or(&lay.y)
            } else {
                &lay.y
            };
            let mut nodes = Vec::new();
            let mut ctx = Context {
                x: &lay.x,
                y,
                rect: p,
                theme,
                fonts,
                out: &mut nodes,
            };
            s.draw(&mut ctx);
            data.push(Node::Group {
                children: nodes,
                clip: None,
                opacity: 1.0,
                tooltip: None,
                id: Some(format!("s{k}")),
            });
        }
        let anchors = anchor_points(&data);
        // Data stays inside its plot area (a zoomed 3D scene must not
        // spill over the neighbouring plots).
        let clip = match self.kind {
            AxesKind::Cartesian => Some((p.x, p.y, p.w, p.h)),
            // The whole cell below the title (box labels reach beyond the
            // plot rectangle), without the colorbar.
            AxesKind::ThreeD => {
                let right = cell.right() - decor.colorbar.as_ref().map_or(0.0, |c| c.1);
                lay.clip3d = Some(Rect {
                    x: cell.x,
                    y: p.y,
                    w: right - cell.x,
                    h: cell.bottom() - p.y,
                });
                Some((cell.x, p.y, right - cell.x, cell.bottom() - p.y))
            }
            _ => None,
        };
        out.push(Node::Group {
            children: data,
            clip,
            opacity: 1.0,
            tooltip: None,
            id: None,
        });

        if self.kind == AxesKind::Cartesian {
            self.draw_axes(&lay, &decor, theme, fonts, out);
            if let Some((rect, map)) = &lay.slider {
                self.draw_slider(&lay, *rect, map, theme, fonts, out);
            }
        } else if self.kind == AxesKind::Polar {
            self.polar.draw_frame(self, &lay, theme, fonts, out, false);
        }

        if let Some((t, x, y)) = &decor.title {
            out.push(text(
                *x,
                *y,
                t,
                theme.axes_title_size,
                theme.text,
                Anchor::Middle,
                Baseline::Middle,
                true,
            ));
        }
        if let Some((cs, w)) = &decor.colorbar {
            let r = Rect {
                x: cell.right() - w,
                y: p.y,
                w: *w,
                h: p.h,
            };
            legend::draw_colorbar(out, cs, r, theme);
        }
        if let Some((entries, owners, l)) = &decor.legend {
            let m = 8.0;
            let (x, y) = match self.legend {
                LegendPosition::OutsideRight => (
                    cell.right() - l.w - decor.colorbar.as_ref().map_or(0.0, |c| c.1),
                    p.y,
                ),
                LegendPosition::OutsideBottom => (p.x + (p.w - l.w) / 2.0, cell.bottom() - l.h),
                LegendPosition::OutsideTop => (p.x + (p.w - l.w) / 2.0, p.y - l.h - 8.0),
                LegendPosition::UpperLeft => (p.x + m, p.y + m),
                LegendPosition::UpperRight => (p.right() - l.w - m, p.y + m),
                LegendPosition::LowerLeft => (p.x + m, p.bottom() - l.h - m),
                LegendPosition::LowerRight => (p.right() - l.w - m, p.bottom() - l.h - m),
                LegendPosition::UpperCenter => (p.x + (p.w - l.w) / 2.0, p.y + m),
                LegendPosition::LowerCenter => (p.x + (p.w - l.w) / 2.0, p.bottom() - l.h - m),
                _ => best_corner(p, l.w, l.h, m, &anchors),
            };
            let dim: Vec<bool> = owners.iter().map(|&i| !self.is_visible(i)).collect();
            legend::draw(out, entries, &dim, l, x, y, theme);
            lay.legend_items = legend::item_rects(l, x, y)
                .into_iter()
                .map(|(r, e)| (r, owners[e]))
                .collect();
        }
        lay
    }

    /// The range slider: all data in small, the shown x range highlighted.
    fn draw_slider(
        &self,
        lay: &Layout,
        r: Rect,
        xmap: &AxisMap,
        theme: &Theme,
        fonts: &FontSet,
        out: &mut Vec<Node>,
    ) {
        let (_, dy) = self.data_limits();
        let ymap = AxisMap::new(
            self.y.scale.clone(),
            dy.0,
            dy.1,
            r.bottom() - 2.0,
            r.y + 2.0,
        );
        out.push(Node::Rect {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
            rx: 2.0,
            fill: Some(theme.plot_background),
            stroke: Some(Stroke::new(theme.axis.with_alpha(0.5), 1.0)),
        });
        let mut data = Vec::new();
        for s in self.shown().filter(|s| !s.secondary()) {
            let mut ctx = Context {
                x: xmap,
                y: &ymap,
                rect: r,
                theme,
                fonts,
                out: &mut data,
            };
            s.draw(&mut ctx);
        }
        out.push(Node::Group {
            children: data,
            clip: Some((r.x, r.y, r.w, r.h)),
            opacity: 0.6,
            tooltip: None,
            id: None,
        });
        // Shade what is not shown; mark the window's edges.
        let a = xmap.px(lay.x.min).clamp(r.x, r.right());
        let b = xmap.px(lay.x.max).clamp(r.x, r.right());
        let shade = Some(theme.text.with_alpha(0.12));
        for (x0, x1) in [(r.x, a), (b, r.right())] {
            if x1 - x0 > 0.5 {
                out.push(Node::Rect {
                    x: x0,
                    y: r.y,
                    w: x1 - x0,
                    h: r.h,
                    rx: 0.0,
                    fill: shade,
                    stroke: None,
                });
            }
        }
        let edge = Stroke::new(theme.text.with_alpha(0.55), 1.0);
        out.push(Node::Rect {
            x: a,
            y: r.y,
            w: (b - a).max(1.0),
            h: r.h,
            rx: 0.0,
            fill: None,
            stroke: Some(edge.clone()),
        });
        for x in [a, b] {
            out.push(Node::Rect {
                x: x - 3.0,
                y: r.y + r.h * 0.25,
                w: 6.0,
                h: r.h * 0.5,
                rx: 2.0,
                fill: Some(theme.plot_background),
                stroke: Some(edge.clone()),
            });
        }
    }

    fn draw_grid(&self, lay: &Layout, theme: &Theme, out: &mut Vec<Node>) {
        let p = lay.plot;
        let grid_stroke = theme.grid_dash.stroke(theme.grid, theme.grid_width);
        let minor_stroke = Stroke::new(theme.minor_grid, theme.grid_width * 0.7);
        let (xt, xm) = Self::tick_list(&self.x, lay.x.min, lay.x.max, p.w, 90.0);
        let (yt, ym) = Self::tick_list(&self.y, lay.y.min, lay.y.max, p.h, 48.0);
        let gx = self.x.grid.unwrap_or(theme.grid_x);
        let gy = self.y.grid.unwrap_or(theme.grid_y);
        let mut lines = Vec::new();
        if theme.minor_grid_lines {
            if gx {
                for v in &xm {
                    let x = lay.x.px(*v);
                    lines.push((x, p.y, x, p.bottom(), &minor_stroke));
                }
            }
            if gy {
                for v in &ym {
                    let y = lay.y.px(*v);
                    lines.push((p.x, y, p.right(), y, &minor_stroke));
                }
            }
        }
        if gx {
            for (v, _) in &xt {
                let x = lay.x.px(*v);
                lines.push((x, p.y, x, p.bottom(), &grid_stroke));
            }
        }
        if gy {
            for (v, _) in &yt {
                let y = lay.y.px(*v);
                lines.push((p.x, y, p.right(), y, &grid_stroke));
            }
        }
        for (x0, y0, x1, y1, s) in lines {
            if x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite() {
                out.push(Node::Path {
                    cmds: vec![PathCmd::MoveTo(x0, y0), PathCmd::LineTo(x1, y1)],
                    fill: None,
                    stroke: Some(s.clone()),
                });
            }
        }
    }

    fn draw_axes(
        &self,
        lay: &Layout,
        decor: &Decor,
        theme: &Theme,
        fonts: &FontSet,
        out: &mut Vec<Node>,
    ) {
        let p = lay.plot;
        let axis = Stroke::new(theme.axis, theme.axis_width);
        let sp = if self.frame {
            theme.spines
        } else {
            super::theme::Spines {
                left: false,
                right: false,
                top: false,
                bottom: false,
            }
        };
        let seg = |x0: f64, y0: f64, x1: f64, y1: f64, s: &Stroke, out: &mut Vec<Node>| {
            out.push(Node::Path {
                cmds: vec![PathCmd::MoveTo(x0, y0), PathCmd::LineTo(x1, y1)],
                fill: None,
                stroke: Some(s.clone()),
            });
        };
        if sp.left {
            seg(p.x, p.y, p.x, p.bottom(), &axis, out);
        }
        if sp.right || lay.y2.is_some() {
            seg(p.right(), p.y, p.right(), p.bottom(), &axis, out);
        }
        if sp.top {
            seg(p.x, p.y, p.right(), p.y, &axis, out);
        }
        if sp.bottom {
            seg(p.x, p.bottom(), p.right(), p.bottom(), &axis, out);
        }
        // Axis breaks: a gap across the plot and `//` marks on the spines.
        let mark = Stroke::new(theme.axis, theme.axis_width.max(1.0));
        let gap_w = 6.0;
        for bx in lay.x.break_positions() {
            out.push(Node::Rect {
                x: bx - gap_w / 2.0,
                y: p.y - 1.0,
                w: gap_w,
                h: p.h + 2.0,
                rx: 0.0,
                fill: Some(theme.plot_background),
                stroke: None,
            });
            for y in [p.y, p.bottom()] {
                for dx in [-gap_w / 2.0, gap_w / 2.0] {
                    seg(bx + dx - 3.0, y + 5.0, bx + dx + 3.0, y - 5.0, &mark, out);
                }
            }
        }
        for by in lay.y.break_positions() {
            out.push(Node::Rect {
                x: p.x - 1.0,
                y: by - gap_w / 2.0,
                w: p.w + 2.0,
                h: gap_w,
                rx: 0.0,
                fill: Some(theme.plot_background),
                stroke: None,
            });
            for x in [p.x, p.right()] {
                for dy in [-gap_w / 2.0, gap_w / 2.0] {
                    seg(x - 5.0, by + dy + 3.0, x + 5.0, by + dy - 3.0, &mark, out);
                }
            }
        }

        let tl = theme.tick_length;
        let ts = theme.tick_size;
        let gap = tl.max(0.0) + 4.0;

        // x ticks (grouped: interactive output redraws them when zoomed)
        let x0 = out.len();
        if self.x.visible {
            let (xt, xm) = Self::tick_list(&self.x, lay.x.min, lay.x.max, p.w, 90.0);
            for v in xm {
                let x = lay.x.px(v);
                seg(x, p.bottom(), x, p.bottom() + tl * 0.6, &axis, out);
            }
            let mut last_end = f64::NEG_INFINITY;
            let mut xt = xt;
            xt.sort_by(|a, b| lay.x.px(a.0).total_cmp(&lay.x.px(b.0)));
            for (v, label) in xt {
                let x = lay.x.px(v);
                if !x.is_finite() || x < p.x - 0.5 || x > p.right() + 0.5 {
                    continue;
                }
                if tl != 0.0 {
                    seg(x, p.bottom(), x, p.bottom() + tl, &axis, out);
                }
                if !self.x.tick_labels {
                    continue;
                }
                let w = fonts.width(&label, ts);
                if decor.x_rotation == 0.0 {
                    if x - w / 2.0 < last_end + 4.0 {
                        continue; // would overlap the previous label
                    }
                    last_end = x + w / 2.0;
                    out.push(text(
                        x,
                        p.bottom() + gap,
                        &label,
                        ts,
                        theme.text,
                        Anchor::Middle,
                        Baseline::Top,
                        false,
                    ));
                } else {
                    let mut t = text(
                        x,
                        p.bottom() + gap,
                        &label,
                        ts,
                        theme.text,
                        Anchor::End,
                        Baseline::Top,
                        false,
                    );
                    if let Node::Text(tn) = &mut t {
                        tn.rotate = decor.x_rotation;
                        tn.baseline = Baseline::Middle;
                        tn.y += ts * 0.3;
                    }
                    out.push(t);
                }
            }
        }
        group_tail(out, x0, "xt");
        // y ticks
        let y_axis = |axis_cfg: &Axis, map: &AxisMap, right: bool, out: &mut Vec<Node>| {
            if !axis_cfg.visible {
                return 0.0f64;
            }
            let (yt, ym) = Self::tick_list(axis_cfg, map.min, map.max, p.h, 48.0);
            let x = if right { p.right() } else { p.x };
            let dir = if right { 1.0 } else { -1.0 };
            for v in ym {
                let y = map.px(v);
                seg(x, y, x + dir * tl * 0.6, y, &axis, out);
            }
            let mut maxw: f64 = 0.0;
            let mut last = f64::INFINITY;
            for (v, label) in yt {
                let y = map.px(v);
                if !y.is_finite() || y < p.y - 0.5 || y > p.bottom() + 0.5 {
                    continue;
                }
                if tl != 0.0 {
                    seg(x, y, x + dir * tl, y, &axis, out);
                }
                let h = fonts.height(ts);
                if (last - y).abs() < h * 1.05 && last.is_finite() {
                    continue;
                }
                last = y;
                maxw = maxw.max(fonts.width(&label, ts));
                // Hidden labels keep their space (the axis label stays put).
                if !axis_cfg.tick_labels {
                    continue;
                }
                let anchor = if right { Anchor::Start } else { Anchor::End };
                out.push(text(
                    x + dir * gap,
                    y,
                    &label,
                    ts,
                    theme.text,
                    anchor,
                    Baseline::Middle,
                    false,
                ));
            }
            maxw
        };
        let y0 = out.len();
        let yw = y_axis(&self.y, &lay.y, false, out);
        group_tail(out, y0, "yt");
        let y2w = match &lay.y2 {
            Some(m) => {
                let y0 = out.len();
                let w = y_axis(&self.y2, m, true, out);
                group_tail(out, y0, "y2t");
                w
            }
            None => 0.0,
        };

        // axis labels
        let ls = theme.label_size;
        let lh = fonts.height(ls);
        if let Some(l) = self.x.label.as_ref().filter(|_| self.x.visible) {
            let th = fonts.height(ts);
            let (xt, _) = Self::tick_list(&self.x, lay.x.min, lay.x.max, p.w, 90.0);
            let maxw = xt
                .iter()
                .map(|(_, l)| fonts.width(l, ts))
                .fold(0.0, f64::max);
            let x_h = if decor.x_rotation == 0.0 {
                th
            } else {
                let a = decor.x_rotation.to_radians().abs();
                maxw * a.sin() + th * a.cos()
            };
            let y = p.bottom() + gap + x_h + 4.0;
            out.push(text(
                p.x + p.w / 2.0,
                y,
                l,
                ls,
                theme.text,
                Anchor::Middle,
                Baseline::Top,
                false,
            ));
        }
        if let Some(l) = self.y.label.as_ref().filter(|_| self.y.visible) {
            let x = p.x - gap - yw - 4.0 - lh / 2.0;
            let mut t = text(
                x,
                p.y + p.h / 2.0,
                l,
                ls,
                theme.text,
                Anchor::Middle,
                Baseline::Middle,
                false,
            );
            if let Node::Text(tn) = &mut t {
                tn.rotate = -90.0;
            }
            out.push(t);
        }
        if let (Some(l), Some(_)) = (&self.y2.label, &lay.y2) {
            let x = p.right() + gap + y2w + 4.0 + lh / 2.0;
            let mut t = text(
                x,
                p.y + p.h / 2.0,
                l,
                ls,
                theme.text,
                Anchor::Middle,
                Baseline::Middle,
                false,
            );
            if let Node::Text(tn) = &mut t {
                tn.rotate = 90.0;
            }
            out.push(t);
        }
    }

    /// Nearest data point to a canvas position (for hover).
    pub(crate) fn hit(&self, lay: &Layout, px: f64, py: f64) -> Option<super::series::Hit> {
        if self.kind == AxesKind::ThreeD {
            return self.hit3d(lay.plot, px, py);
        }
        self.shown()
            .filter_map(|s| {
                let y = if s.secondary() {
                    lay.y2.as_ref().unwrap_or(&lay.y)
                } else {
                    &lay.y
                };
                s.hit(&lay.x, y, lay.plot, px, py)
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
}

impl Axes {
    /// The data points under a vertical line at canvas `px` (one per
    /// series, nearest in x and within `tolerance` pixels), for the linked
    /// cursor of plots sharing an x range.
    #[cfg_attr(not(feature = "window"), allow(dead_code))]
    pub(crate) fn hits_at_x(
        &self,
        lay: &Layout,
        px: f64,
        tolerance: f64,
    ) -> Vec<super::series::Hit> {
        // x stretched around `px` so that the x distance decides which point
        // is nearest; y keeps its mapping so the hit lands on the data.
        const K: f64 = 1e6;
        let m = &lay.x;
        let x = AxisMap::new(
            m.scale.clone(),
            m.min,
            m.max,
            px + (m.p0 - px) * K,
            px + (m.p1 - px) * K,
        );
        self.shown()
            .filter_map(|s| {
                let y = if s.secondary() {
                    lay.y2.as_ref().unwrap_or(&lay.y)
                } else {
                    &lay.y
                };
                let mut h = s.hit(&x, y, lay.plot, px, lay.plot.y + lay.plot.h / 2.0)?;
                h.px = px + (h.px - px) / K;
                h.distance = (h.px - px).abs();
                Some(h)
            })
            .filter(|h| h.distance <= tolerance && h.py.is_finite())
            .collect()
    }

    /// Text of a cursor position on the x (`y = false`) or y axis.
    pub(crate) fn cursor_label(&self, lay: &Layout, along_y: bool, v: f64) -> String {
        let (axis, map) = if along_y {
            (&self.y, &lay.y)
        } else {
            (&self.x, &lay.x)
        };
        if !v.is_finite() {
            return String::new();
        }
        if !axis.categories.is_empty() {
            let i = v.round();
            if i >= 0.0 && (i as usize) < axis.categories.len() {
                return axis.categories[i as usize].clone();
            }
        }
        let span = (map.max - map.min).abs();
        match (&axis.format, &axis.scale) {
            (ticks::TickFormat::Auto, Scale::Time) => {
                let f = if span < 120.0 {
                    "%H:%M:%S"
                } else if span < 2.0 * 86400.0 {
                    "%d %b %H:%M:%S"
                } else {
                    "%Y-%m-%d %H:%M"
                };
                super::time::format_time(v, f)
            }
            (ticks::TickFormat::Auto, Scale::Linear) => {
                // About three significant digits more than the visible span.
                let d = (3.0 - span.log10().floor()).clamp(0.0, 12.0) as usize;
                let a = v.abs();
                if a != 0.0 && (a >= 1e9 || a < 1e-6) {
                    super::series::short(v)
                } else {
                    format!("{v:.d$}")
                }
            }
            (ticks::TickFormat::Auto, _) => super::series::short(v),
            (f, scale) => super::ticks::format_value(v, f, scale, span / 1000.0, span, None),
        }
    }
}

/// Decorations computed during layout.
#[derive(Default)]
pub(crate) struct Decor {
    title: Option<(String, f64, f64)>,
    legend: Option<(Vec<LegendEntry>, Vec<usize>, legend::LegendLayout)>,
    colorbar: Option<(ColorScale, f64)>,
    x_rotation: f64,
}

fn order(a: f64, b: f64) -> (f64, f64) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn text(
    x: f64,
    y: f64,
    t: &str,
    size: f64,
    color: Color,
    anchor: Anchor,
    baseline: Baseline,
    bold: bool,
) -> Node {
    Node::Text(TextNode {
        x,
        y,
        text: t.to_string(),
        size,
        color,
        anchor,
        baseline,
        rotate: 0.0,
        bold,
        italic: false,
    })
}

/// Sample positions of drawn data (for choosing the legend corner).
fn anchor_points(nodes: &[Node]) -> Vec<(f64, f64)> {
    let mut v = Vec::new();
    for n in nodes {
        match n {
            Node::Path { cmds, .. } => {
                for c in cmds {
                    match *c {
                        PathCmd::MoveTo(x, y) | PathCmd::LineTo(x, y) => v.push((x, y)),
                        PathCmd::CubicTo(_, _, _, _, x, y) | PathCmd::ArcTo(_, _, _, _, x, y) => {
                            v.push((x, y))
                        }
                        PathCmd::Close => {}
                    }
                }
            }
            Node::Rect { x, y, w, h, .. } => {
                for i in 0..=4 {
                    for j in 0..=4 {
                        v.push((x + w * i as f64 / 4.0, y + h * j as f64 / 4.0));
                    }
                }
            }
            Node::Circle { cx, cy, .. } => v.push((*cx, *cy)),
            Node::Mesh3d(_) => {}
            Node::Tube { segs, .. } => {
                for (a, b) in segs {
                    v.push(*a);
                    v.push(*b);
                    v.push(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0));
                }
            }
            Node::Sphere { cx, cy, r, .. } => {
                v.push((*cx, *cy));
                for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                    v.push((cx + dx * r, cy + dy * r));
                }
            }
            Node::Text(t) => v.push((t.x, t.y)),
            Node::Image { x, y, w, h, .. } => {
                for i in 0..=4 {
                    for j in 0..=4 {
                        v.push((x + w * i as f64 / 4.0, y + h * j as f64 / 4.0));
                    }
                }
            }
            Node::Group { children, .. } => v.extend(anchor_points(children)),
            // Legend placement only needs the density: sample large sets.
            Node::Instances { offsets, .. } => {
                let k = (offsets.len() / 20_000).max(1);
                v.extend(
                    offsets
                        .iter()
                        .step_by(k)
                        .map(|&(x, y)| (x as f64, y as f64)),
                )
            }
            Node::MarkerGrid {
                x,
                y,
                step,
                cols,
                bits,
                ..
            } => {
                let n: usize = bits.iter().map(|w| w.count_ones() as usize).sum();
                let k = (n / 20_000).max(1);
                v.extend(
                    crate::plot::scene::grid_positions(*x, *y, *step, *cols, bits)
                        .step_by(k)
                        .map(|(x, y)| (x as f64, y as f64)),
                )
            }
        }
    }
    v
}

fn best_corner(p: Rect, w: f64, h: f64, m: f64, pts: &[(f64, f64)]) -> (f64, f64) {
    let candidates = [
        (p.right() - w - m, p.y + m),
        (p.x + m, p.y + m),
        (p.x + m, p.bottom() - h - m),
        (p.right() - w - m, p.bottom() - h - m),
        (p.x + (p.w - w) / 2.0, p.y + m),
        (p.x + (p.w - w) / 2.0, p.bottom() - h - m),
    ];
    candidates
        .iter()
        .copied()
        .min_by_key(|&(x, y)| {
            pts.iter()
                .filter(|(a, b)| {
                    *a >= x - 4.0 && *a <= x + w + 4.0 && *b >= y - 4.0 && *b <= y + h + 4.0
                })
                .count()
        })
        .unwrap_or(candidates[0])
}

/// Moves the nodes pushed since `from` into one group with `id` (named
/// parts of a plot for interactive output); nothing when none were pushed.
fn group_tail(out: &mut Vec<Node>, from: usize, id: &str) {
    if out.len() > from {
        let children = out.split_off(from);
        out.push(Node::Group {
            children,
            clip: None,
            opacity: 1.0,
            tooltip: None,
            id: Some(id.into()),
        });
    }
}
