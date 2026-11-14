//! Statistical charts: histogram, box plot, violin, error bars, KDE, ECDF,
//! regression.

use super::line::{FillBetween, Line, Step};
use super::Rect;
use super::{short, Context, Extent, Hit, IntoValues, LegendEntry, LegendSymbol, Series};
use crate::plot::axes::Axes;
use crate::plot::axis::AxisMap;
use crate::plot::color::Color;
use crate::plot::scene::Stroke;
use crate::plot::stats::{self, Bins};
use crate::plot::style::{Dash, Marker};
use std::any::Any;

/// A histogram. Created by [`Axes::hist`].
#[derive(Clone, Debug)]
pub struct Histogram {
    pub(crate) values: Vec<f64>,
    pub(crate) bins: Bins,
    pub(crate) density: bool,
    pub(crate) cumulative: bool,
    pub(crate) step: bool,
    pub(crate) horizontal: bool,
    pub(crate) color: Color,
    pub(crate) edge: Option<Color>,
    pub(crate) opacity: f32,
    pub(crate) label: Option<String>,
    /// Edges and heights, computed once (reset by the binning setters).
    pub(crate) cache: std::sync::OnceLock<(Vec<f64>, Vec<f64>)>,
}

impl Histogram {
    /// Binning rule (default: automatic).
    pub fn bins(&mut self, bins: Bins) -> &mut Self {
        self.cache = Default::default();
        self.bins = bins;
        self
    }
    /// Number of equal-width bins.
    pub fn bin_count(&mut self, n: usize) -> &mut Self {
        self.cache = Default::default();
        self.bins = Bins::Count(n);
        self
    }
    /// Normalizes so the area is 1 (probability density).
    pub fn density(&mut self, on: bool) -> &mut Self {
        self.cache = Default::default();
        self.density = on;
        self
    }
    /// Cumulative counts.
    pub fn cumulative(&mut self, on: bool) -> &mut Self {
        self.cache = Default::default();
        self.cumulative = on;
        self
    }
    /// Outline only (step histogram), useful for comparing distributions.
    pub fn step(&mut self, on: bool) -> &mut Self {
        self.step = on;
        self
    }
    /// Bars along the y-axis.
    pub fn horizontal(&mut self, on: bool) -> &mut Self {
        self.horizontal = on;
        self
    }
    /// Fill color.
    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.color = color.into();
        self
    }
    /// Bar outline color.
    pub fn edge(&mut self, color: impl Into<Color>) -> &mut Self {
        self.edge = Some(color.into());
        self
    }
    /// Fill opacity (overlapping histograms).
    pub fn opacity(&mut self, opacity: f32) -> &mut Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
    /// Legend label.
    pub fn label(&mut self, label: impl Into<String>) -> &mut Self {
        self.label = Some(label.into());
        self
    }

    /// Bin edges and heights as drawn.
    pub fn computed(&self) -> (Vec<f64>, Vec<f64>) {
        self.cache.get_or_init(|| self.compute()).clone()
    }

    fn compute(&self) -> (Vec<f64>, Vec<f64>) {
        let edges = stats::bin_edges(&self.values, &self.bins);
        let mut h = stats::histogram(&self.values, &edges);
        if self.cumulative {
            let mut acc = 0.0;
            for v in &mut h {
                acc += *v;
                *v = acc;
            }
        }
        if self.density {
            let total: f64 = if self.cumulative {
                h.last().copied().unwrap_or(0.0)
            } else {
                h.iter().sum()
            };
            if total > 0.0 {
                for (i, v) in h.iter_mut().enumerate() {
                    *v /= if self.cumulative {
                        total
                    } else {
                        total * (edges[i + 1] - edges[i])
                    };
                }
            }
        }
        (edges, h)
    }
}

impl Series for Histogram {
    fn extent(&self) -> Option<Extent> {
        let (e, h) = self.computed();
        if e.len() < 2 {
            return None;
        }
        let top = h.iter().copied().fold(0.0, f64::max);
        let mut ext = Extent::point(e[0], 0.0).include(e[e.len() - 1], top);
        ext.sticky_y_min = true;
        Some(if self.horizontal {
            ext.transposed()
        } else {
            ext
        })
    }

    fn draw(&self, ctx: &mut Context) {
        let (e, h) = self.computed();
        let pt = |ctx: &Context, a: f64, b: f64| {
            if self.horizontal {
                ctx.point(b, a)
            } else {
                ctx.point(a, b)
            }
        };
        let fill = self.color.with_alpha(self.color.a * self.opacity);
        if self.step {
            let mut pts = vec![pt(ctx, e[0], 0.0)];
            for i in 0..h.len() {
                pts.push(pt(ctx, e[i], h[i]));
                pts.push(pt(ctx, e[i + 1], h[i]));
            }
            if !self.cumulative {
                pts.push(pt(ctx, e[e.len() - 1], 0.0));
            }
            if self.opacity < 1.0 {
                ctx.polygon(&pts, Some(fill), None);
            }
            ctx.polyline(&pts, Stroke::new(self.color, 2.0));
            return;
        }
        let edge = self.edge.unwrap_or(ctx.theme.plot_background);
        for i in 0..h.len() {
            if h[i] <= 0.0 {
                continue;
            }
            let (x0, y0) = pt(ctx, e[i], 0.0);
            let (x1, y1) = pt(ctx, e[i + 1], h[i]);
            ctx.rect(x0, y0, x1, y1, Some(fill), Some(Stroke::new(edge, 0.8)));
        }
    }

    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.color.with_alpha(self.opacity.max(0.4)),
                    edge: None,
                },
            })
            .collect()
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let (e, h) = self.computed();
        let v = if self.horizontal {
            y.value(py)
        } else {
            x.value(px)
        };
        let i = e.windows(2).position(|w| v >= w[0] && v <= w[1])?;
        let (cx, cy) = if self.horizontal {
            (x.px(h[i]), y.px((e[i] + e[i + 1]) / 2.0))
        } else {
            (x.px((e[i] + e[i + 1]) / 2.0), y.px(h[i]))
        };
        Some(Hit {
            px: cx,
            py: cy,
            label: format!("[{}, {}): {}", short(e[i]), short(e[i + 1]), short(h[i])),
            distance: (cx - px).abs().min((cy - py).abs()),
        })
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Box plots (one per group). Created by [`Axes::boxplot`].
#[derive(Clone, Debug)]
pub struct BoxPlot {
    pub(crate) pos: Vec<f64>,
    pub(crate) stats: Vec<Option<stats::BoxStats>>,
    pub(crate) names: Vec<String>,
    pub(crate) width: f64,
    pub(crate) colors: Vec<Color>,
    pub(crate) horizontal: bool,
    pub(crate) show_mean: bool,
    pub(crate) show_outliers: bool,
    pub(crate) notch: bool,
    pub(crate) label: Option<String>,
}

impl BoxPlot {
    /// Box width in category units (default 0.5).
    pub fn width(&mut self, w: f64) -> &mut Self {
        self.width = w.max(0.0);
        self
    }
    /// One color for all boxes.
    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        let c = color.into();
        self.colors = vec![c];
        self
    }
    /// Colors per box.
    pub fn colors<C: Into<Color>>(&mut self, colors: impl IntoIterator<Item = C>) -> &mut Self {
        self.colors = colors.into_iter().map(Into::into).collect();
        self
    }
    /// Marks the mean with a diamond.
    pub fn show_mean(&mut self, on: bool) -> &mut Self {
        self.show_mean = on;
        self
    }
    /// Draws outliers (default on).
    pub fn show_outliers(&mut self, on: bool) -> &mut Self {
        self.show_outliers = on;
        self
    }
    /// Notched boxes (95 % confidence interval of the median).
    pub fn notch(&mut self, on: bool) -> &mut Self {
        self.notch = on;
        self
    }
    /// Legend label.
    pub fn label(&mut self, label: impl Into<String>) -> &mut Self {
        self.label = Some(label.into());
        self
    }

    fn color_of(&self, i: usize) -> Color {
        if self.colors.is_empty() {
            Color::BLUE
        } else {
            self.colors[i % self.colors.len()]
        }
    }
}

impl Series for BoxPlot {
    fn extent(&self) -> Option<Extent> {
        let mut e: Option<Extent> = None;
        for (i, s) in self.stats.iter().enumerate() {
            let Some(s) = s else { continue };
            let lo = s.outliers.iter().copied().fold(s.whisker_low, f64::min);
            let hi = s.outliers.iter().copied().fold(s.whisker_high, f64::max);
            let p = self.pos[i];
            let x = Extent::point(p - self.width / 2.0, lo).include(p + self.width / 2.0, hi);
            e = Some(e.map_or(x, |e| e.union(x)));
        }
        e.map(|e| if self.horizontal { e.transposed() } else { e })
    }

    fn draw(&self, ctx: &mut Context) {
        let hz = self.horizontal;
        let pt = |ctx: &Context, p: f64, v: f64| if hz { ctx.point(v, p) } else { ctx.point(p, v) };
        let line_c = ctx.theme.text.with_alpha(0.85);
        for (i, s) in self.stats.iter().enumerate() {
            let Some(s) = s else { continue };
            let color = self.color_of(i);
            let p = self.pos[i];
            let hw = self.width / 2.0;
            let stroke = Stroke::new(line_c, 1.2);
            // Whiskers and caps.
            for (a, b) in [(s.whisker_low, s.q1), (s.q3, s.whisker_high)] {
                ctx.polyline(&[pt(ctx, p, a), pt(ctx, p, b)], stroke.clone());
            }
            for v in [s.whisker_low, s.whisker_high] {
                ctx.polyline(
                    &[pt(ctx, p - hw / 2.0, v), pt(ctx, p + hw / 2.0, v)],
                    stroke.clone(),
                );
            }
            // Box (notched: indent at the median).
            let poly: Vec<(f64, f64)> = if self.notch && s.n > 1 {
                let ci = 1.57 * (s.q3 - s.q1) / (s.n as f64).sqrt();
                let (lo, hi) = ((s.median - ci).max(s.q1), (s.median + ci).min(s.q3));
                let nw = hw * 0.55;
                vec![
                    pt(ctx, p - hw, s.q1),
                    pt(ctx, p - hw, lo),
                    pt(ctx, p - nw, s.median),
                    pt(ctx, p - hw, hi),
                    pt(ctx, p - hw, s.q3),
                    pt(ctx, p + hw, s.q3),
                    pt(ctx, p + hw, hi),
                    pt(ctx, p + nw, s.median),
                    pt(ctx, p + hw, lo),
                    pt(ctx, p + hw, s.q1),
                ]
            } else {
                vec![
                    pt(ctx, p - hw, s.q1),
                    pt(ctx, p - hw, s.q3),
                    pt(ctx, p + hw, s.q3),
                    pt(ctx, p + hw, s.q1),
                ]
            };
            ctx.polygon(
                &poly,
                Some(color.with_alpha(0.75 * color.a)),
                Some(Stroke::new(line_c, 1.2)),
            );
            let nw = if self.notch { hw * 0.55 } else { hw };
            ctx.polyline(
                &[pt(ctx, p - nw, s.median), pt(ctx, p + nw, s.median)],
                Stroke::new(line_c, 2.0),
            );
            if self.show_mean {
                let (mx, my) = pt(ctx, p, s.mean);
                if let Some(n) =
                    Marker::Diamond.node(mx, my, 8.0, Color::WHITE, Some(Stroke::new(line_c, 1.0)))
                {
                    ctx.push(n);
                }
            }
            if self.show_outliers {
                for o in &s.outliers {
                    let (ox, oy) = pt(ctx, p, *o);
                    if let Some(n) = Marker::Circle.node(
                        ox,
                        oy,
                        6.0,
                        Color::TRANSPARENT,
                        Some(Stroke::new(line_c, 1.0)),
                    ) {
                        ctx.push(n);
                    }
                }
            }
        }
    }

    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.color_of(0),
                    edge: None,
                },
            })
            .collect()
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        for (i, s) in self.stats.iter().enumerate() {
            let Some(s) = s else { continue };
            let (cx, cy) = if self.horizontal {
                (x.px(s.median), y.px(self.pos[i]))
            } else {
                (x.px(self.pos[i]), y.px(s.median))
            };
            let d = (cx - px).hypot(cy - py);
            if best.as_ref().is_none_or(|b| d < b.distance) {
                best = Some(Hit {
                    px: cx,
                    py: cy,
                    label: format!(
                        "{}: median {} [Q1 {}, Q3 {}] n={}",
                        self.names.get(i).cloned().unwrap_or_default(),
                        short(s.median),
                        short(s.q1),
                        short(s.q3),
                        s.n
                    ),
                    distance: d,
                });
            }
        }
        best
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Violin plots (mirrored KDE per group). Created by [`Axes::violin`].
#[derive(Clone, Debug)]
pub struct Violin {
    pub(crate) pos: Vec<f64>,
    pub(crate) curves: Vec<(Vec<f64>, Vec<f64>)>,
    pub(crate) boxes: Vec<Option<stats::BoxStats>>,
    pub(crate) names: Vec<String>,
    pub(crate) width: f64,
    pub(crate) colors: Vec<Color>,
    pub(crate) horizontal: bool,
    pub(crate) inner_box: bool,
}

impl Violin {
    /// Maximum violin width in category units (default 0.8).
    pub fn width(&mut self, w: f64) -> &mut Self {
        self.width = w.max(0.0);
        self
    }
    /// Colors per violin.
    pub fn colors<C: Into<Color>>(&mut self, colors: impl IntoIterator<Item = C>) -> &mut Self {
        self.colors = colors.into_iter().map(Into::into).collect();
        self
    }
    /// Draws a slim box plot inside (default on).
    pub fn inner_box(&mut self, on: bool) -> &mut Self {
        self.inner_box = on;
        self
    }
}

impl Series for Violin {
    fn extent(&self) -> Option<Extent> {
        let mut e: Option<Extent> = None;
        for (i, (xs, _)) in self.curves.iter().enumerate() {
            if xs.is_empty() {
                continue;
            }
            let p = self.pos[i];
            let x = Extent::point(p - self.width / 2.0, xs[0])
                .include(p + self.width / 2.0, xs[xs.len() - 1]);
            e = Some(e.map_or(x, |e| e.union(x)));
        }
        e.map(|e| if self.horizontal { e.transposed() } else { e })
    }

    fn draw(&self, ctx: &mut Context) {
        let hz = self.horizontal;
        let pt = |ctx: &Context, p: f64, v: f64| if hz { ctx.point(v, p) } else { ctx.point(p, v) };
        let max_d = self
            .curves
            .iter()
            .flat_map(|(_, d)| d.iter().copied())
            .fold(0.0, f64::max);
        if max_d <= 0.0 {
            return;
        }
        let line_c = ctx.theme.text.with_alpha(0.8);
        for (i, (vals, dens)) in self.curves.iter().enumerate() {
            let color = if self.colors.is_empty() {
                Color::BLUE
            } else {
                self.colors[i % self.colors.len()]
            };
            let p = self.pos[i];
            let k = self.width / 2.0 / max_d;
            let mut poly: Vec<(f64, f64)> = vals
                .iter()
                .zip(dens)
                .map(|(v, d)| pt(ctx, p - d * k, *v))
                .collect();
            poly.extend(
                vals.iter()
                    .zip(dens)
                    .rev()
                    .map(|(v, d)| pt(ctx, p + d * k, *v)),
            );
            ctx.polygon(
                &poly,
                Some(color.with_alpha(0.7 * color.a)),
                Some(Stroke::new(color, 1.0)),
            );
            if let (true, Some(Some(s))) = (self.inner_box, self.boxes.get(i)) {
                ctx.polyline(
                    &[pt(ctx, p, s.whisker_low), pt(ctx, p, s.whisker_high)],
                    Stroke::new(line_c, 1.0),
                );
                let bw = self.width * 0.06;
                let b = [
                    pt(ctx, p - bw, s.q1),
                    pt(ctx, p - bw, s.q3),
                    pt(ctx, p + bw, s.q3),
                    pt(ctx, p + bw, s.q1),
                ];
                ctx.polygon(&b, Some(line_c), None);
                let (mx, my) = pt(ctx, p, s.median);
                if let Some(n) = Marker::Circle.node(mx, my, 5.0, Color::WHITE, None) {
                    ctx.push(n);
                }
            }
        }
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        self.boxes
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let s = s.as_ref()?;
                let (cx, cy) = if self.horizontal {
                    (x.px(s.median), y.px(self.pos[i]))
                } else {
                    (x.px(self.pos[i]), y.px(s.median))
                };
                Some(Hit {
                    px: cx,
                    py: cy,
                    label: format!(
                        "{}: median {} n={}",
                        self.names.get(i).cloned().unwrap_or_default(),
                        short(s.median),
                        s.n
                    ),
                    distance: (cx - px).hypot(cy - py),
                })
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Error bars around points. Created by [`Axes::error_bars`].
#[derive(Clone, Debug)]
pub struct ErrorBars {
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) y_err: Option<(Vec<f64>, Vec<f64>)>,
    pub(crate) x_err: Option<(Vec<f64>, Vec<f64>)>,
    pub(crate) color: Color,
    pub(crate) marker: Marker,
    pub(crate) marker_size: f64,
    pub(crate) cap: f64,
    pub(crate) width: f64,
    pub(crate) label: Option<String>,
}

impl ErrorBars {
    /// Symmetric x errors.
    pub fn x_err(&mut self, err: impl IntoValues) -> &mut Self {
        let e = err.into_values();
        self.x_err = Some((e.clone(), e));
        self
    }
    /// Asymmetric y errors (below, above).
    pub fn y_err_asymmetric(
        &mut self,
        below: impl IntoValues,
        above: impl IntoValues,
    ) -> &mut Self {
        self.y_err = Some((below.into_values(), above.into_values()));
        self
    }
    /// Asymmetric x errors (left, right).
    pub fn x_err_asymmetric(&mut self, left: impl IntoValues, right: impl IntoValues) -> &mut Self {
        self.x_err = Some((left.into_values(), right.into_values()));
        self
    }
    /// Color.
    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.color = color.into();
        self
    }
    /// Point marker (default circle; `Marker::None` for bars only).
    pub fn marker(&mut self, marker: Marker) -> &mut Self {
        self.marker = marker;
        self
    }
    /// Cap width in pixels.
    pub fn cap(&mut self, px: f64) -> &mut Self {
        self.cap = px.max(0.0);
        self
    }
    /// Legend label.
    pub fn label(&mut self, label: impl Into<String>) -> &mut Self {
        self.label = Some(label.into());
        self
    }

    fn err(e: &Option<(Vec<f64>, Vec<f64>)>, i: usize) -> (f64, f64) {
        e.as_ref()
            .map(|(a, b)| {
                (
                    a.get(i).copied().unwrap_or(0.0).abs(),
                    b.get(i).copied().unwrap_or(0.0).abs(),
                )
            })
            .unwrap_or((0.0, 0.0))
    }
}

impl Series for ErrorBars {
    fn extent(&self) -> Option<Extent> {
        let mut e: Option<Extent> = None;
        for i in 0..self.x.len().min(self.y.len()) {
            let (x, y) = (self.x[i], self.y[i]);
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            let (yl, yh) = Self::err(&self.y_err, i);
            let (xl, xh) = Self::err(&self.x_err, i);
            let p = Extent::point(x - xl, y - yl).include(x + xh, y + yh);
            e = Some(e.map_or(p, |e| e.union(p)));
        }
        e
    }

    fn draw(&self, ctx: &mut Context) {
        let s = Stroke::new(self.color, self.width);
        let c = self.cap / 2.0;
        for i in 0..self.x.len().min(self.y.len()) {
            let (x, y) = (self.x[i], self.y[i]);
            let (yl, yh) = Self::err(&self.y_err, i);
            let (xl, xh) = Self::err(&self.x_err, i);
            if yl > 0.0 || yh > 0.0 {
                let (a, b) = (ctx.point(x, y - yl), ctx.point(x, y + yh));
                ctx.polyline(&[a, b], s.clone());
                for p in [a, b] {
                    ctx.polyline(&[(p.0 - c, p.1), (p.0 + c, p.1)], s.clone());
                }
            }
            if xl > 0.0 || xh > 0.0 {
                let (a, b) = (ctx.point(x - xl, y), ctx.point(x + xh, y));
                ctx.polyline(&[a, b], s.clone());
                for p in [a, b] {
                    ctx.polyline(&[(p.0, p.1 - c), (p.0, p.1 + c)], s.clone());
                }
            }
            let (px, py) = ctx.point(x, y);
            if let Some(n) = self.marker.node(px, py, self.marker_size, self.color, None) {
                ctx.push(n);
            }
        }
    }

    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Line {
                    color: self.color,
                    width: 0.0,
                    dash: Dash::Solid,
                    marker: Some((self.marker, self.marker_size)),
                },
            })
            .collect()
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let pts = self.x.iter().copied().zip(self.y.iter().copied());
        let ((dx, dy), (cx, cy), d) = super::nearest_point(pts, x, y, px, py)?;
        let i = self
            .x
            .iter()
            .zip(&self.y)
            .position(|(a, b)| *a == dx && *b == dy)
            .unwrap_or(0);
        let (lo, hi) = Self::err(&self.y_err, i);
        let err = if lo == hi {
            format!(" ± {}", short(lo))
        } else {
            format!(" (−{}, +{})", short(lo), short(hi))
        };
        Some(Hit {
            px: cx,
            py: cy,
            label: format!("({}, {}{err})", short(dx), short(dy)),
            distance: d,
        })
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// Histogram of `values`.
    pub fn hist(&mut self, values: impl IntoValues) -> &mut Histogram {
        let color = self.next_color();
        self.add(Histogram {
            values: values.into_values(),
            bins: Bins::Auto,
            density: false,
            cumulative: false,
            step: false,
            horizontal: false,
            color,
            edge: None,
            opacity: 1.0,
            label: None,
            cache: Default::default(),
        })
    }

    fn groups<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
        horizontal: bool,
    ) -> (Vec<f64>, Vec<String>, Vec<Vec<f64>>) {
        let mut pos = Vec::new();
        let mut names = Vec::new();
        let mut data = Vec::new();
        for (l, v) in groups {
            let name = l.to_string();
            let axis = if horizontal { &mut self.y } else { &mut self.x };
            pos.push(axis.category_index(&name));
            names.push(name);
            data.push(v.into_values());
        }
        if horizontal {
            self.y.inverted = true;
        }
        (pos, names, data)
    }

    /// Box plots, one per `(name, values)` group.
    pub fn boxplot<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
    ) -> &mut BoxPlot {
        self.boxplot_oriented(groups, false)
    }

    /// Horizontal box plots.
    pub fn boxplot_horizontal<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
    ) -> &mut BoxPlot {
        self.boxplot_oriented(groups, true)
    }

    fn boxplot_oriented<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
        horizontal: bool,
    ) -> &mut BoxPlot {
        let (pos, names, data) = self.groups(groups, horizontal);
        let colors = (0..data.len()).map(|_| self.next_color()).collect();
        self.add(BoxPlot {
            pos,
            stats: data.iter().map(|d| stats::box_stats(d)).collect(),
            names,
            width: 0.5,
            colors,
            horizontal,
            show_mean: false,
            show_outliers: true,
            notch: false,
            label: None,
        })
    }

    /// Violin plots, one per `(name, values)` group.
    pub fn violin<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
    ) -> &mut Violin {
        let (pos, names, data) = self.groups(groups, false);
        let colors = (0..data.len()).map(|_| self.next_color()).collect();
        self.add(Violin {
            pos,
            curves: data.iter().map(|d| stats::kde(d, None, 120)).collect(),
            boxes: data.iter().map(|d| stats::box_stats(d)).collect(),
            names,
            width: 0.8,
            colors,
            horizontal: false,
            inner_box: true,
        })
    }

    /// Points with symmetric y error bars.
    pub fn error_bars(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        y_err: impl IntoValues,
    ) -> &mut ErrorBars {
        let color = self.next_color();
        let e = y_err.into_values();
        let ms = self.defaults.marker_size;
        self.add(ErrorBars {
            x: x.into_values(),
            y: y.into_values(),
            y_err: Some((e.clone(), e)),
            x_err: None,
            color,
            marker: Marker::Circle,
            marker_size: ms,
            cap: 8.0,
            width: 1.5,
            label: None,
        })
    }

    /// A line with a shaded band between `lower` and `upper` (confidence
    /// interval, min/max range, …). Returns the line.
    pub fn error_band(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        lower: impl IntoValues,
        upper: impl IntoValues,
    ) -> &mut Line {
        let x = x.into_values();
        let color = self.next_color();
        self.next_color -= 1;
        let band = self.add(FillBetween::new(
            x.clone(),
            lower.into_values(),
            upper.into_values(),
            color,
        ));
        band.opacity = 0.25;
        self.line(x, y)
    }

    /// Kernel density estimate of `values` as a line.
    pub fn kde(&mut self, values: impl IntoValues, bandwidth: Option<f64>) -> &mut Line {
        let (x, y) = stats::kde(&values.into_values(), bandwidth, 300);
        let l = self.line(x, y);
        l.decimate = false;
        l
    }

    /// Empirical cumulative distribution function of `values`.
    pub fn ecdf(&mut self, values: impl IntoValues) -> &mut Line {
        let (mut x, mut y) = stats::ecdf(&values.into_values());
        if let Some(&first) = x.first() {
            x.insert(0, first);
            y.insert(0, 0.0);
        }
        let l = self.line(x, y);
        l.step = Some(Step::Post);
        l
    }

    /// Least-squares polynomial fit of `degree` (1 = linear trend line),
    /// drawn over the x range; the label contains the equation and R².
    pub fn regression(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        degree: usize,
    ) -> &mut Line {
        let (x, y) = (x.into_values(), y.into_values());
        let c = stats::polyfit(&x, &y, degree)
            .filter(|c| c.iter().all(|v| v.is_finite()))
            .unwrap_or_default();
        let finite = x.iter().copied().filter(|v| v.is_finite());
        let (lo, hi) = finite.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(v), b.max(v))
        });
        let (xs, ys): (Vec<f64>, Vec<f64>) = if c.is_empty() || !lo.is_finite() {
            (Vec::new(), Vec::new())
        } else {
            (0..=200)
                .map(|i| {
                    let xv = lo + (hi - lo) * i as f64 / 200.0;
                    (xv, stats::polyval(&c, xv))
                })
                .unzip()
        };
        let label = if c.is_empty() {
            "fit".to_string()
        } else {
            let r2 = stats::r_squared(&x, &y, &c);
            let terms: Vec<String> = c
                .iter()
                .enumerate()
                .rev()
                .map(|(k, v)| match k {
                    0 => short(*v),
                    1 => format!("{}x", short(*v)),
                    _ => format!("{}x^{k}", short(*v)),
                })
                .collect();
            let r2 = if r2.is_finite() {
                format!(" (R² = {r2:.3})")
            } else {
                String::new()
            };
            format!("y = {}{r2}", terms.join(" + ").replace("+ -", "- "))
        };
        let l = self.line(xs, ys);
        l.label = Some(label);
        l.dash = Dash::Dashed;
        l.decimate = false;
        l
    }

    /// Trailing moving average of `y` over `window` points.
    pub fn moving_average(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        window: usize,
    ) -> &mut Line {
        let y = stats::moving_average(&y.into_values(), window);
        let l = self.line(x, y);
        l.label = Some(format!("MA({window})"));
        l
    }
}
