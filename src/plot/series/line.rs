//! Line, area and fill-between series.

use super::Rect;
use super::{nearest_point, short, Context, Extent, Hit, LegendEntry, LegendSymbol, Series};
use crate::plot::axis::AxisMap;
use crate::plot::color::Color;
use crate::plot::scene::Stroke;
use crate::plot::style::{Dash, Marker};
use std::any::Any;

/// Step interpolation for step plots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    /// The step happens at the start of the interval (value held until x).
    Pre,
    /// The step happens halfway between points.
    Mid,
    /// The step happens at the end of the interval (value held from x).
    Post,
}

/// A line through `(x, y)` points. Created by [`Axes::line`](crate::plot::Axes::line).
#[derive(Clone, Debug)]
pub struct Line {
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) label: Option<String>,
    pub(crate) color: Color,
    pub(crate) width: f64,
    pub(crate) dash: Dash,
    pub(crate) marker: Marker,
    pub(crate) marker_size: f64,
    pub(crate) step: Option<Step>,
    pub(crate) smooth: bool,
    pub(crate) fill: Option<f64>,
    pub(crate) fill_opacity: f32,
    pub(crate) secondary: bool,
    pub(crate) decimate: bool,
}

impl Line {
    pub(crate) fn new(
        x: Vec<f64>,
        y: Vec<f64>,
        color: Color,
        width: f64,
        marker_size: f64,
    ) -> Self {
        let n = x.len().min(y.len());
        Self {
            x: super::truncated(x, n),
            y: super::truncated(y, n),
            label: None,
            color,
            width,
            dash: Dash::Solid,
            marker: Marker::None,
            marker_size,
            step: None,
            smooth: false,
            fill: None,
            fill_opacity: 0.3,
            secondary: false,
            decimate: true,
        }
    }

    /// Legend label.
    pub fn label(&mut self, label: impl Into<String>) -> &mut Self {
        self.label = Some(label.into());
        self
    }
    /// Line color.
    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.color = color.into();
        self
    }
    /// Line width in pixels (0 hides the line, e.g. markers only).
    pub fn width(&mut self, width: f64) -> &mut Self {
        self.width = width.max(0.0);
        self
    }
    /// Dash style.
    pub fn dash(&mut self, dash: Dash) -> &mut Self {
        self.dash = dash;
        self
    }
    /// Marker at every point.
    pub fn marker(&mut self, marker: Marker) -> &mut Self {
        self.marker = marker;
        self
    }
    /// Marker diameter in pixels.
    pub fn marker_size(&mut self, size: f64) -> &mut Self {
        self.marker_size = size.max(0.0);
        self
    }
    /// Step plot.
    pub fn step(&mut self, step: Step) -> &mut Self {
        self.step = Some(step);
        self
    }
    /// Smooth curve through the points (monotone cubic, no overshoot).
    pub fn smooth(&mut self, smooth: bool) -> &mut Self {
        self.smooth = smooth;
        self
    }
    /// Fills the area between the line and `y = baseline`.
    pub fn fill_to(&mut self, baseline: f64) -> &mut Self {
        self.fill = Some(baseline);
        self
    }
    /// Opacity of the fill (`0..1`).
    pub fn fill_opacity(&mut self, opacity: f32) -> &mut Self {
        self.fill_opacity = opacity.clamp(0.0, 1.0);
        self
    }
    /// Plots against the secondary (right) y-axis.
    pub fn secondary(&mut self, on: bool) -> &mut Self {
        self.secondary = on;
        self
    }
    /// Reduces very long lines to at most ~2 points per pixel column
    /// (min/max decimation; on by default, lossless visually).
    pub fn decimate(&mut self, on: bool) -> &mut Self {
        self.decimate = on;
        self
    }

    /// Replaces the data.
    pub fn set_data(&mut self, x: impl super::IntoValues, y: impl super::IntoValues) -> &mut Self {
        let (x, y) = (x.into_values(), y.into_values());
        let n = x.len().min(y.len());
        self.x = super::truncated(x, n);
        self.y = super::truncated(y, n);
        self
    }
    /// Appends a point (streaming data).
    pub fn push(&mut self, x: f64, y: f64) -> &mut Self {
        self.x.push(x);
        self.y.push(y);
        self
    }
    /// Drops old points so that at most `n` remain (rolling window).
    pub fn keep_last(&mut self, n: usize) -> &mut Self {
        if self.x.len() > n {
            let k = self.x.len() - n;
            self.x.drain(..k);
            self.y.drain(..k);
        }
        self
    }
    /// Drops the points with `x < x_min` (rolling time window: pass
    /// `now - window`). Points are expected in increasing x order, as
    /// produced by [`Line::push`].
    ///
    /// ```
    /// use dataviz::prelude::*;
    /// use dataviz::plot::series::line::Line;
    /// let mut fig = Figure::new(400, 300);
    /// fig.axes().line([0.0, 1.0, 2.0, 3.0], [5.0, 6.0, 7.0, 8.0]);
    /// let line = fig.axes().series_mut::<Line>(0).unwrap();
    /// line.keep_since(1.5);
    /// assert_eq!(line.data().0, &[2.0, 3.0]);
    /// ```
    pub fn keep_since(&mut self, x_min: f64) -> &mut Self {
        let k = self.x.partition_point(|x| *x < x_min);
        if k > 0 {
            self.x.drain(..k);
            self.y.drain(..k);
        }
        self
    }
    /// The data points.
    pub fn data(&self) -> (&[f64], &[f64]) {
        (&self.x, &self.y)
    }

    fn points(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.x.iter().copied().zip(self.y.iter().copied())
    }
}

/// Canvas points → path, applying step / smoothing.
pub(crate) fn line_path(
    pts: &[(f64, f64)],
    step: Option<Step>,
    smooth: bool,
) -> Vec<Vec<(f64, f64)>> {
    let mut runs = Vec::new();
    for run in pts.split(|(x, y)| !x.is_finite() || !y.is_finite()) {
        if run.is_empty() {
            continue;
        }
        let mut out = Vec::with_capacity(run.len() * 2);
        match step {
            Some(mode) => {
                out.push(run[0]);
                for w in run.windows(2) {
                    let ((x0, y0), (x1, y1)) = (w[0], w[1]);
                    match mode {
                        Step::Pre => out.push((x0, y1)),
                        Step::Post => out.push((x1, y0)),
                        Step::Mid => {
                            let xm = (x0 + x1) / 2.0;
                            out.push((xm, y0));
                            out.push((xm, y1));
                        }
                    }
                    out.push((x1, y1));
                }
            }
            None if smooth && run.len() > 2 => out = monotone_spline(run),
            None => out.extend_from_slice(run),
        }
        runs.push(out);
    }
    runs
}

/// Monotone cubic (Fritsch–Carlson) interpolation sampled as a polyline.
fn monotone_spline(p: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let n = p.len();
    let increasing = p.windows(2).all(|w| w[1].0 > w[0].0);
    let decreasing = p.windows(2).all(|w| w[1].0 < w[0].0);
    if !increasing && !decreasing {
        return catmull_rom(p);
    }
    let d: Vec<f64> = p
        .windows(2)
        .map(|w| (w[1].1 - w[0].1) / (w[1].0 - w[0].0))
        .collect();
    let mut m = vec![0.0; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] <= 0.0 {
            0.0
        } else {
            (d[i - 1] + d[i]) / 2.0
        };
    }
    for i in 0..n - 1 {
        if d[i] == 0.0 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
        } else {
            let a = m[i] / d[i];
            let b = m[i + 1] / d[i];
            let s = a * a + b * b;
            if s > 9.0 {
                let t = 3.0 / s.sqrt();
                m[i] = t * a * d[i];
                m[i + 1] = t * b * d[i];
            }
        }
    }
    let mut out = Vec::new();
    for i in 0..n - 1 {
        let (x0, y0) = p[i];
        let (x1, y1) = p[i + 1];
        let h = x1 - x0;
        let steps = ((h.abs() / 3.0).ceil() as usize).clamp(2, 64);
        for s in 0..steps {
            let t = s as f64 / steps as f64;
            let (t2, t3) = (t * t, t * t * t);
            let y = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
                + (t3 - 2.0 * t2 + t) * h * m[i]
                + (-2.0 * t3 + 3.0 * t2) * y1
                + (t3 - t2) * h * m[i + 1];
            out.push((x0 + t * h, y));
        }
    }
    out.push(p[n - 1]);
    out
}

fn catmull_rom(p: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let n = p.len();
    let mut out = Vec::new();
    for i in 0..n - 1 {
        let p0 = p[i.saturating_sub(1)];
        let (p1, p2) = (p[i], p[i + 1]);
        let p3 = p[(i + 2).min(n - 1)];
        for s in 0..16 {
            let t = s as f64 / 16.0;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, c: f64, d: f64| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            out.push((f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out.push(p[n - 1]);
    out
}

/// Envelope polygon of a dense polyline (canvas points sorted by x):
/// per pixel column the vertical extent of the line, widened by half the
/// line width. `None` when the line is not dense enough (< 2 vertices per
/// column on average) or not sorted by x.
#[allow(clippy::needless_range_loop)]
pub(crate) fn envelope(pts: &[(f64, f64)], width: f64) -> Option<Vec<(f64, f64)>> {
    if pts.len() < 64 || !pts.windows(2).all(|w| w[1].0 >= w[0].0) {
        return None;
    }
    let (x0, x1) = (pts[0].0, pts[pts.len() - 1].0);
    let c0 = x0.floor() as i64;
    let cols = (x1.floor() as i64 - c0 + 1).max(1) as usize;
    if (pts.len() as f64) < cols as f64 * 2.0 || cols > 20_000 {
        return None;
    }
    let mut lo = vec![f64::INFINITY; cols];
    let mut hi = vec![f64::NEG_INFINITY; cols];
    let mut put = |c: i64, y: f64| {
        let i = (c - c0).clamp(0, cols as i64 - 1) as usize;
        lo[i] = lo[i].min(y);
        hi[i] = hi[i].max(y);
    };
    let hw = (width / 2.0).max(0.5);
    // A sample reaches every column within half a line width horizontally
    // (the stroke is as wide sideways as vertically).
    let mut spread = |x: f64, y: f64| {
        for c in (x - hw).floor() as i64..=(x + hw).floor() as i64 {
            put(c, y);
        }
    };
    for w in pts.windows(2) {
        let ((ax, ay), (bx, by)) = (w[0], w[1]);
        let (ca, cb) = (ax.floor() as i64, bx.floor() as i64);
        spread(ax, ay);
        spread(bx, by);
        // Columns crossed by the segment get the interpolated y at the edges.
        if cb > ca {
            for c in ca + 1..=cb {
                let t = (c as f64 - ax) / (bx - ax);
                spread(c as f64, ay + (by - ay) * t);
            }
        }
    }
    let mut poly = Vec::with_capacity(cols * 4);
    for i in 0..cols {
        let x = (c0 + i as i64) as f64;
        let (l, r) = (
            if i == 0 { x0 } else { x },
            if i + 1 == cols { x1 } else { x + 1.0 },
        );
        poly.push((l, lo[i] - hw));
        poly.push((r, lo[i] - hw));
    }
    for i in (0..cols).rev() {
        let x = (c0 + i as i64) as f64;
        let (l, r) = (
            if i == 0 { x0 } else { x },
            if i + 1 == cols { x1 } else { x + 1.0 },
        );
        poly.push((r, hi[i] + hw));
        poly.push((l, hi[i] + hw));
    }
    Some(poly)
}

/// Min/max decimation per pixel column for x-sorted canvas points.
/// Streaming form of [`decimate`] for x-sorted points: keeps the first,
/// lowest, highest and last point of every pixel column (and every
/// non-finite break) while reading the points once. `None` when the points
/// are not sorted by x (the caller then falls back to [`decimate`]).
pub(crate) fn decimate_stream(pts: impl Iterator<Item = (f64, f64)>) -> Option<Vec<(f64, f64)>> {
    let mut out = Vec::new();
    // Current column: (column, first, lowest, highest, last).
    let mut cur: Option<(f64, [(f64, f64); 4])> = None;
    let flush = |out: &mut Vec<(f64, f64)>, c: [(f64, f64); 4]| {
        // first, then lo/hi in x order, then last; duplicates removed.
        let (lo, hi) = (c[1], c[2]);
        let mid = if lo.0 <= hi.0 { [lo, hi] } else { [hi, lo] };
        for p in [c[0], mid[0], mid[1], c[3]] {
            if out.last() != Some(&p) {
                out.push(p);
            }
        }
    };
    let mut last_x = f64::NEG_INFINITY;
    for p in pts {
        if !p.0.is_finite() || !p.1.is_finite() {
            if let Some((_, c)) = cur.take() {
                flush(&mut out, c);
            }
            out.push(p);
            continue;
        }
        if p.0 < last_x {
            return None;
        }
        last_x = p.0;
        let col = p.0.floor();
        match &mut cur {
            Some((c, g)) if *c == col => {
                if p.1 < g[1].1 {
                    g[1] = p;
                }
                if p.1 > g[2].1 {
                    g[2] = p;
                }
                g[3] = p;
            }
            _ => {
                if let Some((_, c)) = cur.take() {
                    flush(&mut out, c);
                }
                cur = Some((col, [p; 4]));
            }
        }
    }
    if let Some((_, c)) = cur {
        flush(&mut out, c);
    }
    Some(out)
}

pub(crate) fn decimate(pts: Vec<(f64, f64)>, width_px: f64) -> Vec<(f64, f64)> {
    if (pts.len() as f64) < width_px * 4.0
        || !pts
            .windows(2)
            .all(|w| w[1].0 >= w[0].0 || !w[1].0.is_finite() || !w[0].0.is_finite())
    {
        return pts;
    }
    let mut out = Vec::with_capacity(width_px as usize * 4);
    let mut i = 0;
    while i < pts.len() {
        if !pts[i].0.is_finite() || !pts[i].1.is_finite() {
            out.push(pts[i]);
            i += 1;
            continue;
        }
        let col = pts[i].0.floor();
        let start = i;
        let (mut lo, mut hi) = (i, i);
        while i < pts.len()
            && pts[i].0.is_finite()
            && pts[i].1.is_finite()
            && pts[i].0.floor() == col
        {
            if pts[i].1 < pts[lo].1 {
                lo = i;
            }
            if pts[i].1 > pts[hi].1 {
                hi = i;
            }
            i += 1;
        }
        let end = i - 1;
        let mut keep = vec![start, lo, hi, end];
        keep.sort_unstable();
        keep.dedup();
        out.extend(keep.into_iter().map(|k| pts[k]));
    }
    out
}

impl Series for Line {
    fn y_range_in(&self, x0: f64, x1: f64) -> Option<Option<(f64, f64)>> {
        let r = super::y_range_where(&self.x, x0, x1, |i| (self.y[i], self.y[i]));
        Some(match (r, self.fill) {
            (Some((a, b)), Some(base)) => Some((a.min(base), b.max(base))),
            (r, _) => r,
        })
    }
    fn extent(&self) -> Option<Extent> {
        let e = Extent::of_points(self.points())?;
        Some(match self.fill {
            Some(b) => e.include(e.x_min, b).sticky_y_at(b),
            None => e,
        })
    }

    fn draw(&self, ctx: &mut Context) {
        let mapped = || self.points().map(|(x, y)| ctx.point(x, y));
        // Large x-sorted data is reduced while it is mapped, without holding
        // every mapped point in memory.
        let streamed = (self.decimate
            && self.step.is_none()
            && !self.smooth
            && self.x.len() as f64 >= ctx.rect.w * 4.0)
            .then(|| decimate_stream(mapped()))
            .flatten();
        let pts = match streamed {
            Some(p) => p,
            None => {
                let pts: Vec<(f64, f64)> = mapped().collect();
                if self.decimate && self.step.is_none() && !self.smooth {
                    decimate(pts, ctx.rect.w)
                } else {
                    pts
                }
            }
        };
        let runs = line_path(&pts, self.step, self.smooth);
        if let Some(base) = self.fill {
            let by = ctx.y.px_clamped(base);
            for run in &runs {
                if run.len() < 2 {
                    continue;
                }
                let mut poly = run.clone();
                poly.push((run[run.len() - 1].0, by));
                poly.push((run[0].0, by));
                ctx.polygon(
                    &poly,
                    Some(self.color.with_alpha(self.fill_opacity * self.color.a)),
                    None,
                );
            }
        }
        if self.width > 0.0 {
            let stroke = self.dash.stroke(self.color, self.width);
            let plain =
                self.decimate && self.step.is_none() && !self.smooth && self.dash == Dash::Solid;
            for run in &runs {
                // Dense, x-sorted solid lines are drawn as their pixel-column
                // envelope (one filled polygon) instead of stroking thousands
                // of overlapping segments: same picture, a fraction of the cost.
                match plain.then(|| envelope(run, self.width)).flatten() {
                    Some(poly) => ctx.polygon(&poly, Some(self.color), None),
                    None => ctx.polyline(run, stroke.clone()),
                }
            }
        }
        if self.marker != Marker::None {
            let template = (self.x.len() >= 64)
                .then(|| {
                    self.marker
                        .node(0.0, 0.0, self.marker_size, self.color, None)
                })
                .flatten();
            match template {
                Some(template) => {
                    let offsets = self
                        .points()
                        .map(|(x, y)| ctx.point(x, y))
                        .filter(|p| p.0.is_finite() && p.1.is_finite())
                        .map(|(x, y)| (x as f32, y as f32))
                        .collect();
                    ctx.push(crate::plot::scene::Node::Instances {
                        template: Box::new(template),
                        offsets,
                    });
                }
                None => {
                    let raw: Vec<(f64, f64)> =
                        self.points().map(|(x, y)| ctx.point(x, y)).collect();
                    for (x, y) in raw {
                        if let Some(n) = self.marker.node(x, y, self.marker_size, self.color, None)
                        {
                            ctx.push(n);
                        }
                    }
                }
            }
        }
    }

    fn legend(&self) -> Vec<LegendEntry> {
        let Some(label) = &self.label else {
            return Vec::new();
        };
        let symbol = if self.width == 0.0 && self.marker != Marker::None {
            LegendSymbol::Marker {
                marker: self.marker,
                size: self.marker_size,
                color: self.color,
            }
        } else if self.fill.is_some() && self.width == 0.0 {
            LegendSymbol::Patch {
                fill: self.color.with_alpha(self.fill_opacity),
                edge: None,
            }
        } else {
            LegendSymbol::Line {
                color: self.color,
                width: self.width,
                dash: self.dash.clone(),
                marker: (self.marker != Marker::None).then_some((self.marker, self.marker_size)),
            }
        };
        vec![LegendEntry {
            label: label.clone(),
            symbol,
        }]
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let ((dx, dy), (cx, cy), d) = nearest_point(self.points(), x, y, px, py)?;
        let name = self
            .label
            .as_deref()
            .map(|l| format!("{l}: "))
            .unwrap_or_default();
        Some(Hit {
            px: cx,
            py: cy,
            label: format!("{name}({}, {})", short(dx), short(dy)),
            distance: d,
        })
    }

    fn secondary(&self) -> bool {
        self.secondary
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Area between two curves sharing x values (also used for stacked areas
/// and error bands). Created by [`Axes::fill_between`](crate::plot::Axes::fill_between).
#[derive(Clone, Debug)]
pub struct FillBetween {
    pub(crate) x: Vec<f64>,
    pub(crate) y1: Vec<f64>,
    pub(crate) y2: Vec<f64>,
    pub(crate) label: Option<String>,
    pub(crate) color: Color,
    pub(crate) opacity: f32,
    pub(crate) edge_width: f64,
    pub(crate) step: Option<Step>,
    pub(crate) secondary: bool,
}

impl FillBetween {
    pub(crate) fn new(x: Vec<f64>, y1: Vec<f64>, y2: Vec<f64>, color: Color) -> Self {
        let n = x.len().min(y1.len()).min(y2.len());
        Self {
            x: super::truncated(x, n),
            y1: super::truncated(y1, n),
            y2: super::truncated(y2, n),
            label: None,
            color,
            opacity: 0.35,
            edge_width: 0.0,
            step: None,
            secondary: false,
        }
    }

    /// Legend label.
    pub fn label(&mut self, label: impl Into<String>) -> &mut Self {
        self.label = Some(label.into());
        self
    }
    /// Fill color.
    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.color = color.into();
        self
    }
    /// Fill opacity.
    pub fn opacity(&mut self, opacity: f32) -> &mut Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
    /// Outline width (0 = no outline).
    pub fn edge_width(&mut self, width: f64) -> &mut Self {
        self.edge_width = width.max(0.0);
        self
    }
    /// Step interpolation.
    pub fn step(&mut self, step: Step) -> &mut Self {
        self.step = Some(step);
        self
    }
    /// Plots against the secondary y-axis.
    pub fn secondary(&mut self, on: bool) -> &mut Self {
        self.secondary = on;
        self
    }
}

impl Series for FillBetween {
    fn y_range_in(&self, x0: f64, x1: f64) -> Option<Option<(f64, f64)>> {
        Some(super::y_range_where(&self.x, x0, x1, |i| {
            (self.y1[i].min(self.y2[i]), self.y1[i].max(self.y2[i]))
        }))
    }
    fn extent(&self) -> Option<Extent> {
        let a = Extent::of_points(self.x.iter().copied().zip(self.y1.iter().copied()));
        let b = Extent::of_points(self.x.iter().copied().zip(self.y2.iter().copied()));
        let e = match (a, b) {
            (Some(a), Some(b)) => a.union(b),
            (a, b) => a.or(b)?,
        };
        // Constant baselines (stacked areas start at 0) get no margin.
        let mut e = e;
        if self.y1.iter().all(|v| *v == e.y_min) || self.y2.iter().all(|v| *v == e.y_min) {
            e.sticky_y_min = true;
        }
        Some(e)
    }

    fn draw(&self, ctx: &mut Context) {
        let mut run_top = Vec::new();
        let mut run_bot = Vec::new();
        let flush = |top: &mut Vec<(f64, f64)>, bot: &mut Vec<(f64, f64)>, ctx: &mut Context| {
            if top.len() >= 2 {
                let t = line_path(top, self.step, false).concat();
                let mut b = line_path(bot, self.step, false).concat();
                b.reverse();
                let mut poly = t.clone();
                poly.extend(b.iter().copied());
                ctx.polygon(
                    &poly,
                    Some(self.color.with_alpha(self.opacity * self.color.a)),
                    None,
                );
                if self.edge_width > 0.0 {
                    let s = Stroke::new(self.color, self.edge_width);
                    ctx.polyline(&t, s.clone());
                    b.reverse();
                    ctx.polyline(&b, s);
                }
            }
            top.clear();
            bot.clear();
        };
        for i in 0..self.x.len() {
            let (x, a, b) = (self.x[i], self.y1[i], self.y2[i]);
            if x.is_finite() && a.is_finite() && b.is_finite() {
                run_top.push(ctx.point(x, a));
                run_bot.push(ctx.point(x, b));
            } else {
                flush(&mut run_top, &mut run_bot, ctx);
            }
        }
        flush(&mut run_top, &mut run_bot, ctx);
    }

    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.color.with_alpha(self.opacity.max(0.5)),
                    edge: None,
                },
            })
            .collect()
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let pts = self.x.iter().copied().zip(self.y1.iter().copied());
        let ((dx, dy), (cx, cy), d) = nearest_point(pts, x, y, px, py)?;
        let name = self
            .label
            .as_deref()
            .map(|l| format!("{l}: "))
            .unwrap_or_default();
        Some(Hit {
            px: cx,
            py: cy,
            label: format!("{name}({}, {})", short(dx), short(dy)),
            distance: d,
        })
    }

    fn secondary(&self) -> bool {
        self.secondary
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_paths() {
        let p = [(0.0, 0.0), (1.0, 1.0)];
        assert_eq!(
            line_path(&p, Some(Step::Post), false)[0],
            vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]
        );
        assert_eq!(
            line_path(&p, Some(Step::Pre), false)[0],
            vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
        );
        assert_eq!(line_path(&p, Some(Step::Mid), false)[0].len(), 4);
    }

    #[test]
    fn spline_passes_through_points_without_overshoot() {
        let p = [(0.0, 0.0), (10.0, 10.0), (20.0, 10.0), (30.0, 0.0)];
        let s = monotone_spline(&p);
        assert_eq!(s[0], p[0]);
        assert_eq!(*s.last().unwrap(), p[3]);
        assert!(s.iter().all(|(_, y)| *y <= 10.0 + 1e-9 && *y >= -1e-9));
    }

    #[test]
    fn streaming_decimation_matches() {
        let mut pts: Vec<(f64, f64)> = (0..20_000)
            .map(|i| (i as f64 / 37.0, ((i * 7919) % 101) as f64 - (i % 13) as f64))
            .collect();
        pts[5000].1 = f64::NAN;
        pts[5001].1 = f64::NAN;
        let a = decimate(pts.clone(), 100.0);
        let b = decimate_stream(pts.iter().copied()).unwrap();
        let same = |p: &(f64, f64), q: &(f64, f64)| {
            p.0 == q.0 && (p.1 == q.1 || p.1.is_nan() && q.1.is_nan())
        };
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(p, q)| same(p, q)));
        // Unsorted input is refused.
        assert!(decimate_stream([(1.0, 0.0), (0.0, 0.0)].into_iter()).is_none());
    }

    #[test]
    fn envelope_covers_the_line() {
        let pts: Vec<(f64, f64)> = (0..2000)
            .map(|i| (i as f64 / 10.0, (i % 7) as f64 * 3.0))
            .collect();
        let poly = envelope(&pts, 2.0).unwrap();
        let top = poly.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let bottom = poly.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        assert_eq!((top, bottom), (-1.0, 19.0));
        assert!(envelope(&pts[..50], 2.0).is_none());
        let sparse: Vec<(f64, f64)> = (0..100).map(|i| (i as f64 * 10.0, 0.0)).collect();
        assert!(envelope(&sparse, 2.0).is_none());
    }

    #[test]
    fn decimation_keeps_extremes() {
        let pts: Vec<(f64, f64)> = (0..10_000)
            .map(|i| (i as f64 / 100.0, (i % 7) as f64))
            .collect();
        let d = decimate(pts, 100.0);
        assert!(d.len() <= 100 * 4 + 4);
        assert!(d.iter().any(|p| p.1 == 6.0) && d.iter().any(|p| p.1 == 0.0));
    }
}
