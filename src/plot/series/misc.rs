//! Miscellaneous series: function sampling, stems, waterfalls, annotations, …

/// Samples `f` on `[x0, x1]` with at least `n` points, refining where the
/// curve bends; non-finite values split the curve.
pub(crate) fn sample_function(
    f: &dyn Fn(f64) -> f64,
    x0: f64,
    x1: f64,
    n: usize,
) -> (Vec<f64>, Vec<f64>) {
    let n = n.max(2);
    let mut xs: Vec<f64> = (0..n)
        .map(|i| x0 + (x1 - x0) * i as f64 / (n - 1) as f64)
        .collect();
    for _ in 0..3 {
        let ys: Vec<f64> = xs.iter().map(|x| f(*x)).collect();
        let span = ys
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(v), b.max(v))
            });
        let tol = (span.1 - span.0).abs() * 0.002;
        let mut out = Vec::with_capacity(xs.len() * 2);
        for i in 0..xs.len() - 1 {
            out.push(xs[i]);
            let xm = (xs[i] + xs[i + 1]) / 2.0;
            let ym = f(xm);
            let lin = (ys[i] + ys[i + 1]) / 2.0;
            if !(ym - lin).abs().is_finite() || (ym - lin).abs() > tol {
                out.push(xm);
            }
        }
        out.push(*xs.last().unwrap());
        if out.len() == xs.len() || out.len() > 20_000 {
            break;
        }
        xs = out;
    }
    let ys = xs
        .iter()
        .map(|x| f(*x))
        .map(|y| if y.is_finite() { y } else { f64::NAN })
        .collect();
    (xs, ys)
}

use super::Rect;
use super::{
    nearest_point, short, Context, Extent, Hit, IntoLabels, IntoValues, LegendEntry, LegendSymbol,
    Series,
};
use crate::plot::axes::Axes;
use crate::plot::axis::AxisMap;
use crate::plot::color::Color;
use crate::plot::scene::{Anchor, Baseline, Node, PathCmd, Stroke, TextNode};
use crate::plot::style::{Dash, Marker};
use std::any::Any;

/// Stem plot: vertical lines from a baseline to markers. Created by [`Axes::stem`].
#[derive(Clone, Debug)]
pub struct Stem {
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) baseline: f64,
    pub(crate) color: Color,
    pub(crate) marker: Marker,
    pub(crate) marker_size: f64,
    pub(crate) label: Option<String>,
}

impl Stem {
    /// Baseline value (default 0).
    pub fn baseline(&mut self, y: f64) -> &mut Self {
        self.baseline = y;
        self
    }
    /// Color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
    /// Head marker (`Marker::None` for plain lines; lollipop charts use circles).
    pub fn marker(&mut self, m: Marker) -> &mut Self {
        self.marker = m;
        self
    }
    /// Head size.
    pub fn marker_size(&mut self, s: f64) -> &mut Self {
        self.marker_size = s.max(0.0);
        self
    }
    /// Legend label.
    pub fn label(&mut self, l: impl Into<String>) -> &mut Self {
        self.label = Some(l.into());
        self
    }
}

impl Series for Stem {
    fn extent(&self) -> Option<Extent> {
        let e = Extent::of_points(self.x.iter().copied().zip(self.y.iter().copied()))?;
        Some(e.include(e.x_min, self.baseline).sticky_y_at(self.baseline))
    }
    fn draw(&self, ctx: &mut Context) {
        let s = Stroke::new(self.color, 1.5);
        let (x0, x1) = (ctx.rect.x, ctx.rect.right());
        let by = ctx.y.px_clamped(self.baseline);
        ctx.polyline(&[(x0, by), (x1, by)], Stroke::new(ctx.theme.axis, 1.0));
        for (x, y) in self.x.iter().zip(&self.y) {
            let (px, py) = ctx.point(*x, *y);
            ctx.polyline(&[(px, by), (px, py)], s.clone());
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
                symbol: LegendSymbol::Marker {
                    marker: self.marker,
                    size: self.marker_size,
                    color: self.color,
                },
            })
            .collect()
    }
    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let ((dx, dy), (cx, cy), d) = nearest_point(
            self.x.iter().copied().zip(self.y.iter().copied()),
            x,
            y,
            px,
            py,
        )?;
        Some(Hit {
            px: cx,
            py: cy,
            label: format!("({}, {})", short(dx), short(dy)),
            distance: d,
        })
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Waterfall chart: running total of positive and negative changes.
/// Created by [`Axes::waterfall`].
#[derive(Clone, Debug)]
pub struct Waterfall {
    pub(crate) pos: Vec<f64>,
    pub(crate) names: Vec<String>,
    pub(crate) deltas: Vec<f64>,
    pub(crate) totals: Vec<bool>,
    pub(crate) up: Color,
    pub(crate) down: Color,
    pub(crate) total: Color,
    pub(crate) labels: bool,
}

impl Waterfall {
    /// Marks the bar at `index` as a (sub)total: it shows the running sum
    /// instead of a change.
    pub fn total_at(&mut self, index: usize) -> &mut Self {
        if index < self.totals.len() {
            self.totals[index] = true;
        }
        self
    }
    /// Colors for increases, decreases and totals.
    pub fn colors(
        &mut self,
        up: impl Into<Color>,
        down: impl Into<Color>,
        total: impl Into<Color>,
    ) -> &mut Self {
        self.up = up.into();
        self.down = down.into();
        self.total = total.into();
        self
    }
    /// Value labels (default on).
    pub fn labels(&mut self, on: bool) -> &mut Self {
        self.labels = on;
        self
    }

    /// (start, end, kind) per bar; kind 0 = up, 1 = down, 2 = total.
    fn bars(&self) -> Vec<(f64, f64, u8)> {
        let mut acc = 0.0;
        self.deltas
            .iter()
            .zip(&self.totals)
            .map(|(d, t)| {
                if *t {
                    (0.0, acc, 2)
                } else {
                    let s = acc;
                    acc += if d.is_finite() { *d } else { 0.0 };
                    (s, acc, if *d >= 0.0 { 0 } else { 1 })
                }
            })
            .collect()
    }
}

impl Series for Waterfall {
    fn extent(&self) -> Option<Extent> {
        let b = self.bars();
        if b.is_empty() {
            return None;
        }
        let mut e = Extent::point(self.pos[0] - 0.4, 0.0);
        for (i, (s, t, _)) in b.iter().enumerate() {
            e = e
                .include(self.pos[i] - 0.4, *s)
                .include(self.pos[i] + 0.4, *t);
        }
        Some(e.sticky_y_at(0.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let b = self.bars();
        let size = ctx.theme.annotation_size;
        for (i, (s, t, kind)) in b.iter().enumerate() {
            let p = self.pos[i];
            let color = [self.up, self.down, self.total][*kind as usize];
            let (x0, y0) = ctx.point(p - 0.4, *s);
            let (x1, y1) = ctx.point(p + 0.4, *t);
            ctx.rect(x0, y0, x1, y1, Some(color), None);
            if i + 1 < b.len() {
                let (nx, _) = ctx.point(self.pos[i + 1] - 0.4, *t);
                let mut st = Stroke::new(ctx.theme.axis.with_alpha(0.6), 1.0);
                st.dash = vec![3.0, 2.0];
                ctx.polyline(&[(x1, y1), (nx, y1)], st);
            }
            if self.labels {
                let v = if *kind == 2 { *t } else { t - s };
                let text = if *kind == 0 {
                    format!("+{}", short(v))
                } else {
                    short(v)
                };
                let top = y0.min(y1);
                ctx.text(
                    (x0 + x1) / 2.0,
                    top - 3.0,
                    text,
                    size,
                    ctx.theme.text,
                    Anchor::Middle,
                    Baseline::Bottom,
                );
            }
        }
    }
    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, _py: f64) -> Option<Hit> {
        let b = self.bars();
        b.iter()
            .enumerate()
            .map(|(i, (s, t, k))| {
                let (cx, cy) = (x.px(self.pos[i]), y.px(*t));
                let v = if *k == 2 { *t } else { t - s };
                Hit {
                    px: cx,
                    py: cy,
                    label: format!("{}: {} (total {})", self.names[i], short(v), short(*t)),
                    distance: (cx - px).abs(),
                }
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Vector field: arrows `(u, v)` at `(x, y)`. Created by [`Axes::quiver`].
#[derive(Clone, Debug)]
pub struct Quiver {
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) u: Vec<f64>,
    pub(crate) v: Vec<f64>,
    pub(crate) scale: Option<f64>,
    pub(crate) color: Color,
    pub(crate) colormap: Option<crate::plot::color::Colormap>,
}

impl Quiver {
    /// Data units per arrow length unit (default: longest arrow ≈ grid spacing).
    pub fn scale(&mut self, s: f64) -> &mut Self {
        self.scale = Some(s);
        self
    }
    /// Arrow color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
    /// Colors arrows by magnitude.
    pub fn colormap(&mut self, cm: crate::plot::color::Colormap) -> &mut Self {
        self.colormap = Some(cm);
        self
    }
    fn n(&self) -> usize {
        self.x
            .len()
            .min(self.y.len())
            .min(self.u.len())
            .min(self.v.len())
    }
    fn auto_scale(&self) -> f64 {
        if let Some(s) = self.scale {
            return s;
        }
        let ext = Extent::of_points(self.x.iter().copied().zip(self.y.iter().copied()));
        let span = ext.map_or(1.0, |e| (e.x_max - e.x_min).max(e.y_max - e.y_min));
        let maxm = (0..self.n())
            .map(|i| self.u[i].hypot(self.v[i]))
            .filter(|m| m.is_finite())
            .fold(0.0, f64::max);
        let spacing = span / (self.n() as f64).sqrt().max(1.0);
        if maxm > 0.0 {
            spacing * 0.9 / maxm
        } else {
            1.0
        }
    }
}

impl Series for Quiver {
    fn extent(&self) -> Option<Extent> {
        let k = self.auto_scale();
        let pts = (0..self.n()).flat_map(|i| {
            [
                (self.x[i], self.y[i]),
                (self.x[i] + self.u[i] * k, self.y[i] + self.v[i] * k),
            ]
        });
        Extent::of_points(pts)
    }
    fn draw(&self, ctx: &mut Context) {
        let k = self.auto_scale();
        let maxm = (0..self.n())
            .map(|i| self.u[i].hypot(self.v[i]))
            .filter(|m| m.is_finite())
            .fold(0.0, f64::max);
        for i in 0..self.n() {
            let (a, b) = ctx.point(self.x[i], self.y[i]);
            let (c, d) = ctx.point(self.x[i] + self.u[i] * k, self.y[i] + self.v[i] * k);
            let color = match self.colormap {
                Some(cm) if maxm > 0.0 => cm.at(self.u[i].hypot(self.v[i]) / maxm),
                _ => self.color,
            };
            arrow(ctx.out, (a, b), (c, d), color, 1.3, 6.0);
        }
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Draws an arrow from `a` to `b` (canvas coordinates).
pub(crate) fn arrow(
    out: &mut Vec<Node>,
    a: (f64, f64),
    b: (f64, f64),
    color: Color,
    width: f64,
    head: f64,
) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy);
    if !len.is_finite() || len < 0.5 {
        return;
    }
    let (ux, uy) = (dx / len, dy / len);
    let h = head.min(len * 0.6);
    let base = (b.0 - ux * h, b.1 - uy * h);
    out.push(Node::Path {
        cmds: vec![PathCmd::MoveTo(a.0, a.1), PathCmd::LineTo(base.0, base.1)],
        fill: None,
        stroke: Some(Stroke::new(color, width)),
    });
    let (px, py) = (-uy * h * 0.45, ux * h * 0.45);
    out.push(Node::Path {
        cmds: vec![
            PathCmd::MoveTo(b.0, b.1),
            PathCmd::LineTo(base.0 + px, base.1 + py),
            PathCmd::LineTo(base.0 - px, base.1 - py),
            PathCmd::Close,
        ],
        fill: Some(color),
        stroke: None,
    });
}

/// Gantt chart: tasks as horizontal bars from start to end (often times).
/// Created by [`Axes::gantt`].
#[derive(Clone, Debug)]
pub struct Gantt {
    pub(crate) pos: Vec<f64>,
    pub(crate) names: Vec<String>,
    pub(crate) start: Vec<f64>,
    pub(crate) end: Vec<f64>,
    pub(crate) colors: Vec<Color>,
    pub(crate) progress: Option<Vec<f64>>,
}

impl Gantt {
    /// Completed fraction (0..1) per task, drawn as a darker part.
    pub fn progress(&mut self, p: impl IntoValues) -> &mut Self {
        self.progress = Some(p.into_values());
        self
    }
    /// Colors per task.
    pub fn colors<C: Into<Color>>(&mut self, colors: impl IntoIterator<Item = C>) -> &mut Self {
        self.colors = colors.into_iter().map(Into::into).collect();
        self
    }
}

impl Series for Gantt {
    fn extent(&self) -> Option<Extent> {
        let mut e: Option<Extent> = None;
        for i in 0..self.pos.len().min(self.start.len()).min(self.end.len()) {
            let p = Extent::point(self.start[i], self.pos[i] - 0.4)
                .include(self.end[i], self.pos[i] + 0.4);
            e = Some(e.map_or(p, |e| e.union(p)));
        }
        e
    }
    fn draw(&self, ctx: &mut Context) {
        for i in 0..self.pos.len().min(self.start.len()).min(self.end.len()) {
            let c = self.colors[i % self.colors.len().max(1)];
            let (x0, y0) = ctx.point(self.start[i], self.pos[i] - 0.35);
            let (x1, y1) = ctx.point(self.end[i], self.pos[i] + 0.35);
            ctx.push(Node::Rect {
                x: x0.min(x1),
                y: y0.min(y1),
                w: (x1 - x0).abs(),
                h: (y1 - y0).abs(),
                rx: 3.0,
                fill: Some(c.lighten(0.35)),
                stroke: None,
            });
            if let Some(p) = self.progress.as_ref().and_then(|p| p.get(i)) {
                let xm = x0 + (x1 - x0) * p.clamp(0.0, 1.0);
                ctx.push(Node::Rect {
                    x: x0.min(xm),
                    y: y0.min(y1),
                    w: (xm - x0).abs(),
                    h: (y1 - y0).abs(),
                    rx: 3.0,
                    fill: Some(c),
                    stroke: None,
                });
            } else {
                ctx.push(Node::Rect {
                    x: x0.min(x1),
                    y: y0.min(y1),
                    w: (x1 - x0).abs(),
                    h: (y1 - y0).abs(),
                    rx: 3.0,
                    fill: Some(c),
                    stroke: None,
                });
            }
        }
    }
    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        (0..self.pos.len().min(self.start.len()).min(self.end.len()))
            .map(|i| {
                let (cx, cy) = (x.px((self.start[i] + self.end[i]) / 2.0), y.px(self.pos[i]));
                Hit {
                    px: cx,
                    py: cy,
                    label: self.names[i].clone(),
                    distance: (cy - py).abs() + (cx - px).abs() * 0.01,
                }
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Annotations drawn in data coordinates.
#[derive(Clone, Debug)]
pub enum Annotation {
    /// Horizontal line at y.
    HLine(f64),
    /// Vertical line at x.
    VLine(f64),
    /// Shaded band between two y values.
    HSpan(f64, f64),
    /// Shaded band between two x values.
    VSpan(f64, f64),
    /// Text at a point.
    Text(f64, f64, String),
    /// Arrow from (x0, y0) to (x1, y1) with optional text at the start.
    Arrow(f64, f64, f64, f64, Option<String>),
    /// Rectangle (x0, y0, x1, y1).
    Rect(f64, f64, f64, f64),
}

/// An annotation with its style. Created by [`Axes::hline`], [`Axes::text`], …
#[derive(Clone, Debug)]
pub struct Annotate {
    pub(crate) kind: Annotation,
    pub(crate) color: Color,
    pub(crate) width: f64,
    pub(crate) dash: Dash,
    pub(crate) opacity: f32,
    pub(crate) size: Option<f64>,
    pub(crate) anchor: Anchor,
    pub(crate) label: Option<String>,
    pub(crate) bold: bool,
}

impl Annotate {
    fn new(kind: Annotation, color: Color) -> Self {
        Self {
            kind,
            color,
            width: 1.5,
            dash: Dash::Solid,
            opacity: 0.15,
            size: None,
            anchor: Anchor::Start,
            label: None,
            bold: false,
        }
    }
    /// Color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
    /// Line width.
    pub fn width(&mut self, w: f64) -> &mut Self {
        self.width = w;
        self
    }
    /// Dash style.
    pub fn dash(&mut self, d: Dash) -> &mut Self {
        self.dash = d;
        self
    }
    /// Fill opacity of spans and rectangles.
    pub fn opacity(&mut self, o: f32) -> &mut Self {
        self.opacity = o.clamp(0.0, 1.0);
        self
    }
    /// Font size of text.
    pub fn size(&mut self, s: f64) -> &mut Self {
        self.size = Some(s);
        self
    }
    /// Horizontal text anchor.
    pub fn anchor(&mut self, a: Anchor) -> &mut Self {
        self.anchor = a;
        self
    }
    /// Bold text.
    pub fn bold(&mut self, b: bool) -> &mut Self {
        self.bold = b;
        self
    }
    /// Legend label (lines and spans).
    pub fn label(&mut self, l: impl Into<String>) -> &mut Self {
        self.label = Some(l.into());
        self
    }
}

impl Series for Annotate {
    fn extent(&self) -> Option<Extent> {
        match &self.kind {
            Annotation::Text(x, y, _) => Some(Extent::point(*x, *y)),
            Annotation::Arrow(a, b, c, d, _) | Annotation::Rect(a, b, c, d) => {
                Some(Extent::point(*a, *b).include(*c, *d))
            }
            // Reference lines and spans do not change the limits.
            _ => None,
        }
    }
    fn draw(&self, ctx: &mut Context) {
        let r = ctx.rect;
        let stroke = self.dash.stroke(self.color, self.width);
        let fill = self.color.with_alpha(self.opacity);
        match &self.kind {
            Annotation::HLine(y) => {
                let py = ctx.y.px(*y);
                ctx.polyline(&[(r.x, py), (r.right(), py)], stroke);
            }
            Annotation::VLine(x) => {
                let px = ctx.x.px(*x);
                ctx.polyline(&[(px, r.y), (px, r.bottom())], stroke);
            }
            Annotation::HSpan(a, b) => {
                let (ya, yb) = (ctx.y.px_clamped(*a), ctx.y.px_clamped(*b));
                ctx.rect(r.x, ya, r.right(), yb, Some(fill), None);
            }
            Annotation::VSpan(a, b) => {
                let (xa, xb) = (ctx.x.px_clamped(*a), ctx.x.px_clamped(*b));
                ctx.rect(xa, r.y, xb, r.bottom(), Some(fill), None);
            }
            Annotation::Rect(a, b, c, d) => {
                let (x0, y0) = ctx.point(*a, *b);
                let (x1, y1) = ctx.point(*c, *d);
                ctx.rect(x0, y0, x1, y1, Some(fill), Some(stroke));
            }
            Annotation::Text(x, y, t) => {
                let (px, py) = ctx.point(*x, *y);
                let size = self.size.unwrap_or(ctx.theme.annotation_size);
                ctx.push(Node::Text(TextNode {
                    x: px,
                    y: py,
                    text: t.clone(),
                    size,
                    color: self.color,
                    anchor: self.anchor,
                    baseline: Baseline::Middle,
                    rotate: 0.0,
                    bold: self.bold,
                }));
            }
            Annotation::Arrow(a, b, c, d, t) => {
                let p0 = ctx.point(*a, *b);
                let p1 = ctx.point(*c, *d);
                arrow(ctx.out, p0, p1, self.color, self.width, 8.0);
                if let Some(t) = t {
                    let size = self.size.unwrap_or(ctx.theme.annotation_size);
                    let anchor = if p0.0 <= p1.0 {
                        Anchor::End
                    } else {
                        Anchor::Start
                    };
                    let dx = if p0.0 <= p1.0 { -4.0 } else { 4.0 };
                    ctx.push(Node::Text(TextNode {
                        x: p0.0 + dx,
                        y: p0.1,
                        text: t.clone(),
                        size,
                        color: ctx.theme.text,
                        anchor,
                        baseline: Baseline::Middle,
                        rotate: 0.0,
                        bold: self.bold,
                    }));
                }
            }
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        let Some(l) = &self.label else {
            return Vec::new();
        };
        let symbol = match self.kind {
            Annotation::HSpan(..) | Annotation::VSpan(..) | Annotation::Rect(..) => {
                LegendSymbol::Patch {
                    fill: self.color.with_alpha(self.opacity.max(0.3)),
                    edge: None,
                }
            }
            _ => LegendSymbol::Line {
                color: self.color,
                width: self.width,
                dash: self.dash.clone(),
                marker: None,
            },
        };
        vec![LegendEntry {
            label: l.clone(),
            symbol,
        }]
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// Stem plot.
    pub fn stem(&mut self, x: impl IntoValues, y: impl IntoValues) -> &mut Stem {
        let color = self.next_color();
        let ms = self.defaults.marker_size;
        self.add(Stem {
            x: x.into_values(),
            y: y.into_values(),
            baseline: 0.0,
            color,
            marker: Marker::Circle,
            marker_size: ms,
            label: None,
        })
    }

    /// Lollipop chart over categories (stems with circles).
    pub fn lollipop(&mut self, categories: impl IntoLabels, values: impl IntoValues) -> &mut Stem {
        let x: Vec<f64> = categories
            .into_labels()
            .iter()
            .map(|c| self.x.category_index(c))
            .collect();
        let s = self.stem(x, values);
        s.marker_size *= 1.6;
        s
    }

    /// Waterfall chart of changes per category; use
    /// [`Waterfall::total_at`] to insert (sub)totals.
    pub fn waterfall(
        &mut self,
        categories: impl IntoLabels,
        deltas: impl IntoValues,
    ) -> &mut Waterfall {
        let names = categories.into_labels();
        let pos = names.iter().map(|c| self.x.category_index(c)).collect();
        let deltas = deltas.into_values();
        let n = deltas.len();
        self.add(Waterfall {
            pos,
            names,
            deltas,
            totals: vec![false; n],
            up: Color::rgb(38, 166, 91),
            down: Color::rgb(220, 53, 69),
            total: Color::rgb(70, 110, 180),
            labels: true,
        })
    }

    /// Vector field (arrows of `(u, v)` at `(x, y)`).
    pub fn quiver(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        u: impl IntoValues,
        v: impl IntoValues,
    ) -> &mut Quiver {
        let color = self.next_color();
        self.add(Quiver {
            x: x.into_values(),
            y: y.into_values(),
            u: u.into_values(),
            v: v.into_values(),
            scale: None,
            color,
            colormap: None,
        })
    }

    /// Gantt chart: one bar per task from `start` to `end` (listed top to bottom).
    pub fn gantt(
        &mut self,
        tasks: impl IntoLabels,
        start: impl IntoValues,
        end: impl IntoValues,
    ) -> &mut Gantt {
        let names = tasks.into_labels();
        let pos = names.iter().map(|c| self.y.category_index(c)).collect();
        self.y.inverted = true;
        self.y.grid = Some(false);
        let colors = (0..names.len()).map(|_| self.next_color()).collect();
        self.add(Gantt {
            pos,
            names,
            start: start.into_values(),
            end: end.into_values(),
            colors,
            progress: None,
        })
    }

    fn annotate(&mut self, kind: Annotation) -> &mut Annotate {
        let c = self.defaults.palette.get(3).copied().unwrap_or(Color::RED);
        self.add(Annotate::new(kind, c))
    }

    /// Horizontal reference line.
    pub fn hline(&mut self, y: f64) -> &mut Annotate {
        let a = self.annotate(Annotation::HLine(y));
        a.dash = Dash::Dashed;
        a
    }
    /// Vertical reference line.
    pub fn vline(&mut self, x: f64) -> &mut Annotate {
        let a = self.annotate(Annotation::VLine(x));
        a.dash = Dash::Dashed;
        a
    }
    /// Shaded horizontal band.
    pub fn hspan(&mut self, y0: f64, y1: f64) -> &mut Annotate {
        self.annotate(Annotation::HSpan(y0, y1))
    }
    /// Shaded vertical band.
    pub fn vspan(&mut self, x0: f64, x1: f64) -> &mut Annotate {
        self.annotate(Annotation::VSpan(x0, x1))
    }
    /// Text at a data position.
    pub fn text(&mut self, x: f64, y: f64, text: impl Into<String>) -> &mut Annotate {
        let a = self.annotate(Annotation::Text(x, y, text.into()));
        a.color = Color::rgb(34, 34, 34);
        a
    }
    /// Arrow pointing from `(x0, y0)` to `(x1, y1)`, with optional text at the tail.
    pub fn arrow(&mut self, from: (f64, f64), to: (f64, f64), text: Option<&str>) -> &mut Annotate {
        let a = self.annotate(Annotation::Arrow(
            from.0,
            from.1,
            to.0,
            to.1,
            text.map(str::to_string),
        ));
        a.color = Color::rgb(60, 60, 60);
        a
    }
    /// Rectangle in data coordinates.
    pub fn rectangle(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> &mut Annotate {
        self.annotate(Annotation::Rect(x0, y0, x1, y1))
    }
}
