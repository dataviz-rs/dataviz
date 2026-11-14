//! Business and editorial charts: bullet, dumbbell, slope, bump, waffle,
//! marimekko, chord diagram, streamgraph and horizon chart.
//!
//! Shapes of one color are merged into one path and curves are flattened
//! to straight segments, so every chart is a handful of nodes that the
//! rasterizer fills on its fast polygon path.

use super::{Context, Extent, IntoLabels, IntoValues, LegendEntry, LegendSymbol, Series};
use crate::plot::axes::{Axes, AxesKind};
use crate::plot::axis::TickSpec;
use crate::plot::color::Color;
use crate::plot::scene::{Anchor, Baseline, Node, PathCmd, Stroke};
use crate::plot::style::Marker;
use std::any::Any;
use std::f64::consts::{PI, TAU};

/// Shapes of one color collected into one path.
#[derive(Default)]
pub(crate) struct Shapes {
    cmds: Vec<PathCmd>,
}

impl Shapes {
    pub(crate) fn rect(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) || x0 == x1 || y0 == y1 {
            return;
        }
        self.cmds.extend([
            PathCmd::MoveTo(x0, y0),
            PathCmd::LineTo(x1, y0),
            PathCmd::LineTo(x1, y1),
            PathCmd::LineTo(x0, y1),
            PathCmd::Close,
        ]);
    }
    pub(crate) fn poly(&mut self, pts: &[(f64, f64)]) {
        let mut first = true;
        for &(x, y) in pts {
            if !(x.is_finite() && y.is_finite()) {
                continue;
            }
            self.cmds.push(if first {
                PathCmd::MoveTo(x, y)
            } else {
                PathCmd::LineTo(x, y)
            });
            first = false;
        }
        if !first {
            self.cmds.push(PathCmd::Close);
        }
    }
    pub(crate) fn line(&mut self, pts: &[(f64, f64)]) {
        for (i, &(x, y)) in pts.iter().enumerate() {
            self.cmds.push(if i == 0 {
                PathCmd::MoveTo(x, y)
            } else {
                PathCmd::LineTo(x, y)
            });
        }
    }
    pub(crate) fn fill(self, ctx: &mut Context, color: Color) {
        if !self.cmds.is_empty() {
            ctx.push(Node::Path {
                cmds: self.cmds,
                fill: Some(color),
                stroke: None,
            });
        }
    }
    pub(crate) fn stroke(self, ctx: &mut Context, stroke: Stroke) {
        if !self.cmds.is_empty() {
            ctx.push(Node::Path {
                cmds: self.cmds,
                fill: None,
                stroke: Some(stroke),
            });
        }
    }
}

fn finite_max(v: impl Iterator<Item = f64>) -> f64 {
    v.filter(|x| x.is_finite())
        .fold(f64::NEG_INFINITY, f64::max)
}

fn patch(label: &str, c: Color) -> LegendEntry {
    LegendEntry {
        label: label.to_string(),
        symbol: LegendSymbol::Patch {
            fill: c,
            edge: None,
        },
    }
}

fn dot(label: &str, c: Color) -> LegendEntry {
    LegendEntry {
        label: label.to_string(),
        symbol: LegendSymbol::Marker {
            marker: Marker::Circle,
            size: 9.0,
            color: c,
        },
    }
}

// ----- bullet ----------------------------------------------------------------

/// Bullet chart: a measure against a target over qualitative ranges.
/// Created by [`Axes::bullet`].
#[derive(Clone, Debug)]
pub struct Bullet {
    pub(crate) values: Vec<f64>,
    pub(crate) targets: Vec<f64>,
    pub(crate) ranges: Vec<Vec<f64>>,
    pub(crate) color: Color,
}

impl Bullet {
    /// Color of the measure bar.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
}

impl Bullet {
    /// Scale of row `i`: its largest range (or value / target).
    fn row_max(&self, i: usize) -> f64 {
        let m = finite_max(
            self.ranges
                .get(i)
                .into_iter()
                .flatten()
                .copied()
                .chain(self.values.get(i).copied())
                .chain(self.targets.get(i).copied()),
        );
        if m.is_finite() && m > 0.0 {
            m
        } else {
            1.0
        }
    }
}

impl Series for Bullet {
    fn extent(&self) -> Option<Extent> {
        // Every row is drawn in percent of its own scale (rows may have
        // different units).
        Some(Extent::point(0.0, -0.5).include(128.0, self.values.len() as f64 - 0.5))
    }
    fn draw(&self, ctx: &mut Context) {
        let k = self.ranges.iter().map(|r| r.len()).max().unwrap_or(0);
        let pct = |i: usize, v: f64| v / self.row_max(i) * 100.0;
        // Qualitative bands: darkest for the first (worst) range.
        let shade = |j: usize| {
            let t = if k <= 1 {
                0.0
            } else {
                j as f64 / (k - 1) as f64
            };
            ctx.theme
                .text
                .mix(ctx.theme.plot_background, 0.62 + 0.3 * t)
        };
        for j in (0..k).rev() {
            let mut s = Shapes::default();
            for (i, r) in self.ranges.iter().enumerate() {
                if let Some(&hi) = r.get(j) {
                    let (x0, y0) = ctx.point(0.0, i as f64 - 0.38);
                    let (x1, y1) = ctx.point(pct(i, hi), i as f64 + 0.38);
                    s.rect(x0, y0, x1, y1);
                }
            }
            s.fill(ctx, shade(j));
        }
        let mut bars = Shapes::default();
        let mut ticks = Shapes::default();
        let mut labels = Vec::new();
        for (i, &v) in self.values.iter().enumerate() {
            let (x0, y0) = ctx.point(0.0, i as f64 - 0.13);
            let (x1, y1) = ctx.point(pct(i, v), i as f64 + 0.13);
            bars.rect(x0, y0, x1, y1);
            let mut text = super::short(v);
            if let Some(&t) = self.targets.get(i) {
                let (tx, ty0) = ctx.point(pct(i, t), i as f64 - 0.28);
                let (_, ty1) = ctx.point(pct(i, t), i as f64 + 0.28);
                ticks.rect(tx - 1.5, ty0, tx + 1.5, ty1);
                text = format!("{text} / {}", super::short(t));
            }
            let (rx, ty) = ctx.point(100.0, i as f64);
            labels.push((rx + 6.0, ty, text));
        }
        bars.fill(ctx, self.color);
        ticks.fill(ctx, ctx.theme.text);
        for (x, y, t) in labels {
            ctx.text(
                x,
                y,
                t,
                ctx.theme.tick_size * 0.9,
                ctx.theme.text,
                Anchor::Start,
                Baseline::Middle,
            );
        }
    }
    fn kind(&self) -> &'static str {
        "bullet"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- dumbbell --------------------------------------------------------------

/// Dumbbell chart: two values per category joined by a line. Created by
/// [`Axes::dumbbell`].
#[derive(Clone, Debug)]
pub struct Dumbbell {
    pub(crate) start: Vec<f64>,
    pub(crate) end: Vec<f64>,
    pub(crate) colors: [Color; 2],
    pub(crate) labels: [Option<String>; 2],
    pub(crate) size: f64,
}

impl Dumbbell {
    /// Legend labels of the two ends.
    pub fn labels(&mut self, start: impl Into<String>, end: impl Into<String>) -> &mut Self {
        self.labels = [Some(start.into()), Some(end.into())];
        self
    }
    /// Colors of the two ends.
    pub fn colors(&mut self, start: impl Into<Color>, end: impl Into<Color>) -> &mut Self {
        self.colors = [start.into(), end.into()];
        self
    }
    /// Marker size in pixels.
    pub fn size(&mut self, s: f64) -> &mut Self {
        self.size = s;
        self
    }
}

impl Series for Dumbbell {
    fn extent(&self) -> Option<Extent> {
        let v: Vec<f64> = self
            .start
            .iter()
            .chain(&self.end)
            .copied()
            .filter(|v| v.is_finite())
            .collect();
        let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        lo.is_finite()
            .then(|| Extent::point(lo, -0.5).include(hi, self.start.len() as f64 - 0.5))
    }
    fn draw(&self, ctx: &mut Context) {
        let n = self.start.len().min(self.end.len());
        let mut bars = Shapes::default();
        for i in 0..n {
            let a = ctx.point(self.start[i], i as f64);
            let b = ctx.point(self.end[i], i as f64);
            bars.line(&[a, b]);
        }
        bars.stroke(
            ctx,
            Stroke::new(ctx.theme.text.with_alpha(0.3), (self.size * 0.4).max(1.5)),
        );
        for (k, v) in [&self.start, &self.end].into_iter().enumerate() {
            for (i, &x) in v.iter().take(n).enumerate() {
                let (px, py) = ctx.point(x, i as f64);
                ctx.push(Node::Circle {
                    cx: px,
                    cy: py,
                    r: self.size / 2.0,
                    fill: Some(self.colors[k]),
                    stroke: Some(Stroke::new(ctx.theme.plot_background, 1.0)),
                });
            }
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        self.labels
            .iter()
            .zip(self.colors)
            .filter_map(|(l, c)| l.as_ref().map(|l| dot(l, c)))
            .collect()
    }
    fn kind(&self) -> &'static str {
        "dumbbell"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- slope -----------------------------------------------------------------

/// Slope chart: each category's change between two times. Created by
/// [`Axes::slope`].
#[derive(Clone, Debug)]
pub struct Slope {
    pub(crate) names: Vec<String>,
    pub(crate) before: Vec<f64>,
    pub(crate) after: Vec<f64>,
    pub(crate) up: Color,
    pub(crate) down: Color,
    pub(crate) decimals: usize,
}

impl Slope {
    /// Colors of rising and falling lines.
    pub fn colors(&mut self, up: impl Into<Color>, down: impl Into<Color>) -> &mut Self {
        self.up = up.into();
        self.down = down.into();
        self
    }
    /// Decimals of the value labels.
    pub fn decimals(&mut self, d: usize) -> &mut Self {
        self.decimals = d;
        self
    }
}

/// Label y positions at least `gap` apart, as close to `want` as possible.
pub(crate) fn spread_labels(want: &[f64], gap: f64) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..want.len()).collect();
    idx.sort_by(|a, b| want[*a].total_cmp(&want[*b]));
    let mut pos: Vec<f64> = idx.iter().map(|&i| want[i]).collect();
    for _ in 0..50 {
        let mut moved = false;
        for k in 1..pos.len() {
            let d = pos[k] - pos[k - 1];
            if d < gap {
                let push = (gap - d) / 2.0;
                pos[k - 1] -= push;
                pos[k] += push;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    let mut out = vec![0.0; want.len()];
    for (k, &i) in idx.iter().enumerate() {
        out[i] = pos[k];
    }
    out
}

impl Series for Slope {
    fn extent(&self) -> Option<Extent> {
        let v: Vec<f64> = self
            .before
            .iter()
            .chain(&self.after)
            .copied()
            .filter(|v| v.is_finite())
            .collect();
        let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        lo.is_finite()
            .then(|| Extent::point(-0.55, lo).include(1.55, hi))
    }
    fn draw(&self, ctx: &mut Context) {
        let n = self
            .names
            .len()
            .min(self.before.len())
            .min(self.after.len());
        let (mut up, mut down) = (Shapes::default(), Shapes::default());
        let mut ends = Vec::with_capacity(n);
        for i in 0..n {
            let a = ctx.point(0.0, self.before[i]);
            let b = ctx.point(1.0, self.after[i]);
            if self.after[i] >= self.before[i] {
                up.line(&[a, b]);
            } else {
                down.line(&[a, b]);
            }
            ends.push((a, b));
        }
        up.stroke(ctx, Stroke::new(self.up, 2.2));
        down.stroke(ctx, Stroke::new(self.down, 2.2));
        let ts = ctx.theme.tick_size;
        let gap = ts * 1.25;
        let left = spread_labels(&ends.iter().map(|e| e.0 .1).collect::<Vec<_>>(), gap);
        let right = spread_labels(&ends.iter().map(|e| e.1 .1).collect::<Vec<_>>(), gap);
        let d = self.decimals;
        for i in 0..n {
            let c = if self.after[i] >= self.before[i] {
                self.up
            } else {
                self.down
            };
            for (p, y, side) in [(ends[i].0, left[i], -1.0), (ends[i].1, right[i], 1.0)] {
                ctx.push(Node::Circle {
                    cx: p.0,
                    cy: p.1,
                    r: 4.0,
                    fill: Some(c),
                    stroke: Some(Stroke::new(ctx.theme.plot_background, 1.0)),
                });
                let v = if side < 0.0 {
                    self.before[i]
                } else {
                    self.after[i]
                };
                let text = if side < 0.0 {
                    format!("{}  {v:.d$}", self.names[i])
                } else {
                    format!("{v:.d$}  {}", self.names[i])
                };
                let anchor = if side < 0.0 {
                    Anchor::End
                } else {
                    Anchor::Start
                };
                ctx.text(
                    p.0 + side * 9.0,
                    y,
                    text,
                    ts,
                    ctx.theme.text,
                    anchor,
                    Baseline::Middle,
                );
            }
        }
    }
    fn kind(&self) -> &'static str {
        "slope"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- bump ------------------------------------------------------------------

/// Bump chart: rankings over time. Created by [`Axes::bump`].
#[derive(Clone, Debug)]
pub struct Bump {
    pub(crate) names: Vec<String>,
    pub(crate) ranks: Vec<Vec<f64>>,
    pub(crate) colors: Vec<Color>,
    pub(crate) width: f64,
}

impl Bump {
    /// Line width.
    pub fn width(&mut self, w: f64) -> &mut Self {
        self.width = w;
        self
    }
}

impl Series for Bump {
    fn extent(&self) -> Option<Extent> {
        let t = self.ranks.iter().map(|r| r.len()).max().unwrap_or(0);
        let hi = finite_max(self.ranks.iter().flatten().copied());
        (t > 0 && hi.is_finite())
            .then(|| Extent::point(-0.3, 0.5).include(t as f64 - 1.0 + 0.85, hi + 0.5))
    }
    fn draw(&self, ctx: &mut Context) {
        for (k, r) in self.ranks.iter().enumerate() {
            let c = self.colors[k % self.colors.len()];
            // Smooth steps between periods (cosine easing), flattened.
            let mut pts = Vec::with_capacity(r.len() * 12);
            for t in 0..r.len() {
                if t + 1 < r.len() && r[t].is_finite() && r[t + 1].is_finite() {
                    for s in 0..12 {
                        let u = s as f64 / 12.0;
                        let e = (1.0 - (PI * u).cos()) / 2.0;
                        pts.push(ctx.point(t as f64 + u, r[t] + (r[t + 1] - r[t]) * e));
                    }
                } else if r[t].is_finite() {
                    pts.push(ctx.point(t as f64, r[t]));
                    pts.push((f64::NAN, f64::NAN));
                }
            }
            ctx.polyline(
                &pts,
                Stroke::new(c, self.width).join(crate::plot::scene::LineJoin::Round),
            );
            for (t, &v) in r.iter().enumerate() {
                if v.is_finite() {
                    let (x, y) = ctx.point(t as f64, v);
                    ctx.push(Node::Circle {
                        cx: x,
                        cy: y,
                        r: self.width * 1.3,
                        fill: Some(c),
                        stroke: Some(Stroke::new(ctx.theme.plot_background, 1.5)),
                    });
                }
            }
            if let Some((t, v)) = r.iter().enumerate().rev().find(|(_, v)| v.is_finite()) {
                let (x, y) = ctx.point(t as f64, *v);
                ctx.text(
                    x + self.width * 2.0 + 6.0,
                    y,
                    self.names[k].clone(),
                    ctx.theme.tick_size,
                    c,
                    Anchor::Start,
                    Baseline::Middle,
                );
            }
        }
    }
    fn kind(&self) -> &'static str {
        "bump"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- waffle ----------------------------------------------------------------

/// Waffle chart: shares as a grid of squares. Created by [`Axes::waffle`].
#[derive(Clone, Debug)]
pub struct Waffle {
    pub(crate) names: Vec<String>,
    pub(crate) counts: Vec<usize>,
    pub(crate) colors: Vec<Color>,
    pub(crate) rows: usize,
    pub(crate) cols: usize,
    pub(crate) shares: Vec<f64>,
}

impl Waffle {
    /// Grid size (default 10 × 10, one square per percent).
    pub fn grid(&mut self, rows: usize, cols: usize) -> &mut Self {
        self.rows = rows.max(1);
        self.cols = cols.max(1);
        self.counts = largest_remainder(&self.shares, self.rows * self.cols);
        self
    }
    /// Colors of the categories.
    pub fn colors(&mut self, c: Vec<Color>) -> &mut Self {
        if !c.is_empty() {
            self.colors = c;
        }
        self
    }
}

/// Integer counts summing to `total` in proportion to `shares`.
pub(crate) fn largest_remainder(shares: &[f64], total: usize) -> Vec<usize> {
    let sum: f64 = shares.iter().filter(|v| v.is_finite() && **v > 0.0).sum();
    if sum <= 0.0 {
        return vec![0; shares.len()];
    }
    let exact: Vec<f64> = shares
        .iter()
        .map(|v| v.max(0.0) / sum * total as f64)
        .collect();
    let mut counts: Vec<usize> = exact.iter().map(|v| v.floor() as usize).collect();
    let mut rest = total - counts.iter().sum::<usize>();
    let mut order: Vec<usize> = (0..shares.len()).collect();
    order.sort_by(|a, b| {
        (exact[*b] - exact[*b].floor()).total_cmp(&(exact[*a] - exact[*a].floor()))
    });
    for &i in order.iter().cycle().take(shares.len() * 2) {
        if rest == 0 {
            break;
        }
        counts[i] += 1;
        rest -= 1;
    }
    counts
}

impl Series for Waffle {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(self.cols as f64, self.rows as f64))
    }
    fn draw(&self, ctx: &mut Context) {
        let r = ctx.rect;
        let cell = (r.w / self.cols as f64).min(r.h / self.rows as f64);
        let (ox, oy) = (
            r.x + (r.w - cell * self.cols as f64) / 2.0,
            r.y + (r.h - cell * self.rows as f64) / 2.0,
        );
        let gap = (cell * 0.12).max(1.0);
        let mut k = 0;
        // Filled column by column from the bottom left, like reading a bar.
        for (c, &n) in self.counts.iter().enumerate() {
            let mut s = Shapes::default();
            for _ in 0..n {
                let (col, row) = (k / self.rows, self.rows - 1 - k % self.rows);
                let x = ox + col as f64 * cell;
                let y = oy + row as f64 * cell;
                s.rect(
                    x + gap / 2.0,
                    y + gap / 2.0,
                    x + cell - gap / 2.0,
                    y + cell - gap / 2.0,
                );
                k += 1;
            }
            s.fill(ctx, self.colors[c % self.colors.len()]);
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        let total: usize = self.counts.iter().sum::<usize>().max(1);
        self.names
            .iter()
            .zip(&self.counts)
            .enumerate()
            .map(|(i, (n, c))| {
                patch(
                    &format!("{n} ({:.0} %)", *c as f64 * 100.0 / total as f64),
                    self.colors[i % self.colors.len()],
                )
            })
            .collect()
    }
    fn kind(&self) -> &'static str {
        "waffle"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- marimekko -------------------------------------------------------------

/// Marimekko (mosaic) chart: column widths by column totals, segment
/// heights by shares within the column. Created by [`Axes::marimekko`].
#[derive(Clone, Debug)]
pub struct Marimekko {
    pub(crate) rows: Vec<String>,
    pub(crate) values: Vec<Vec<f64>>,
    pub(crate) colors: Vec<Color>,
}

impl Series for Marimekko {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(100.0, 100.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let totals: Vec<f64> = self
            .values
            .iter()
            .map(|c| c.iter().filter(|v| v.is_finite()).sum())
            .collect();
        let grand: f64 = totals.iter().sum::<f64>().max(1e-300);
        let mut shapes: Vec<Shapes> = (0..self.rows.len()).map(|_| Shapes::default()).collect();
        let mut labels = Vec::new();
        let mut x = 0.0;
        let bg = ctx.theme.plot_background;
        for (c, col) in self.values.iter().enumerate() {
            let w = totals[c] / grand * 100.0;
            let mut y = 0.0;
            for (r, &v) in col.iter().enumerate().take(self.rows.len()) {
                let h = if totals[c] > 0.0 {
                    v.max(0.0) / totals[c] * 100.0
                } else {
                    0.0
                };
                let (x0, y0) = ctx.point(x, y);
                let (x1, y1) = ctx.point(x + w, y + h);
                shapes[r].rect(x0 + 1.0, y0 - 1.0, x1 - 1.0, y1 + 1.0);
                let text = format!("{h:.0}%");
                if (x1 - x0).abs() > ctx.text_width(&text, ctx.theme.tick_size) + 6.0
                    && (y0 - y1).abs() > ctx.theme.tick_size + 4.0
                {
                    labels.push((
                        (x0 + x1) / 2.0,
                        (y0 + y1) / 2.0,
                        text,
                        self.colors[r % self.colors.len()],
                    ));
                }
                y += h;
            }
            x += w;
        }
        for (r, s) in shapes.into_iter().enumerate() {
            s.fill(ctx, self.colors[r % self.colors.len()]);
        }
        let _ = bg;
        for (x, y, t, c) in labels {
            ctx.text(
                x,
                y,
                t,
                ctx.theme.tick_size,
                c.contrasting_text(),
                Anchor::Middle,
                Baseline::Middle,
            );
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        self.rows
            .iter()
            .enumerate()
            .map(|(i, n)| patch(n, self.colors[i % self.colors.len()]))
            .collect()
    }
    fn kind(&self) -> &'static str {
        "marimekko"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- chord -----------------------------------------------------------------

/// Chord diagram: flows between groups around a circle. Created by
/// [`Axes::chord`].
#[derive(Clone, Debug)]
pub struct Chord {
    pub(crate) names: Vec<String>,
    pub(crate) matrix: Vec<Vec<f64>>,
    pub(crate) colors: Vec<Color>,
    pub(crate) gap: f64,
    pub(crate) opacity: f32,
}

impl Chord {
    /// Gap between groups in degrees.
    pub fn gap(&mut self, deg: f64) -> &mut Self {
        self.gap = deg.max(0.0);
        self
    }
    /// Opacity of the ribbons.
    pub fn opacity(&mut self, a: f32) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }
}

impl Series for Chord {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(-1.0, -1.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let n = self.names.len().min(self.matrix.len());
        if n == 0 {
            return;
        }
        let val = |i: usize, j: usize| {
            self.matrix[i]
                .get(j)
                .copied()
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(0.0)
        };
        // d3's layout: group i's arc is its row total, split into one
        // slot per target; the ribbon i–j joins slot (i, j) and slot (j, i).
        let totals: Vec<f64> = (0..n).map(|i| (0..n).map(|j| val(i, j)).sum()).collect();
        let grand: f64 = totals.iter().sum::<f64>().max(1e-300);
        let gap = self.gap.to_radians();
        let span = (TAU - gap * n as f64).max(0.1);
        let (cx, cy) = (ctx.rect.x + ctx.rect.w / 2.0, ctx.rect.y + ctx.rect.h / 2.0);
        let ts = ctx.theme.tick_size;
        let longest = self
            .names
            .iter()
            .map(|s| ctx.text_width(s, ts))
            .fold(0.0, f64::max);
        let r_out =
            (ctx.rect.w.min(ctx.rect.h) / 2.0 - longest.min(ctx.rect.w * 0.2) - 10.0).max(20.0);
        let r_in = r_out * 0.93;
        let at = |a: f64, r: f64| (cx + r * a.cos(), cy + r * a.sin());
        let arc = |a0: f64, a1: f64, r: f64, out: &mut Vec<(f64, f64)>| {
            let steps = (((a1 - a0).abs() * r / 4.0) as usize).clamp(2, 128);
            for s in 0..=steps {
                out.push(at(a0 + (a1 - a0) * s as f64 / steps as f64, r));
            }
        };
        let scale = span / grand;
        let mut start = vec![0.0; n];
        let mut slot = vec![vec![(0.0, 0.0); n]; n];
        let mut a = -PI / 2.0;
        for i in 0..n {
            start[i] = a;
            // Targets in circular order after i, so ribbons do not cross
            // inside a group.
            for k in 0..n {
                let j = (i + n - k) % n;
                let w = val(i, j) * scale;
                slot[i][j] = (a, a + w);
                a += w;
            }
            a += gap;
        }
        let quad = |p0: (f64, f64), p2: (f64, f64), out: &mut Vec<(f64, f64)>| {
            for s in 1..20 {
                let t = s as f64 / 20.0;
                let u = 1.0 - t;
                out.push((
                    u * u * p0.0 + 2.0 * u * t * cx + t * t * p2.0,
                    u * u * p0.1 + 2.0 * u * t * cy + t * t * p2.1,
                ));
            }
        };
        let mut ribbons: Vec<Shapes> = (0..n).map(|_| Shapes::default()).collect();
        for i in 0..n {
            for j in i..n {
                let (a0, a1) = slot[i][j];
                let (b0, b1) = slot[j][i];
                if a1 - a0 <= 1e-9 && b1 - b0 <= 1e-9 {
                    continue;
                }
                let mut pts = Vec::new();
                arc(a0, a1, r_in, &mut pts);
                if i == j {
                    quad(at(a1, r_in), at(a0, r_in), &mut pts);
                } else {
                    quad(at(a1, r_in), at(b0, r_in), &mut pts);
                    arc(b0, b1, r_in, &mut pts);
                    quad(at(b1, r_in), at(a0, r_in), &mut pts);
                }
                // Colored by the larger side of the flow.
                let owner = if val(i, j) >= val(j, i) { i } else { j };
                ribbons[owner].poly(&pts);
            }
        }
        for (i, s) in ribbons.into_iter().enumerate() {
            s.fill(
                ctx,
                self.colors[i % self.colors.len()].with_alpha(self.opacity),
            );
        }
        for i in 0..n {
            let a0 = start[i];
            let a1 = a0 + totals[i] * scale;
            if a1 <= a0 {
                continue;
            }
            let mut pts = Vec::new();
            arc(a0, a1, r_out, &mut pts);
            arc(a1, a0, r_in * 1.005, &mut pts);
            let c = self.colors[i % self.colors.len()];
            ctx.polygon(&pts, Some(c), None);
            let m = (a0 + a1) / 2.0;
            let (lx, ly) = at(m, r_out + 8.0);
            let anchor = if m.cos() > 0.15 {
                Anchor::Start
            } else if m.cos() < -0.15 {
                Anchor::End
            } else {
                Anchor::Middle
            };
            let base = if m.sin() > 0.5 {
                Baseline::Top
            } else if m.sin() < -0.5 {
                Baseline::Bottom
            } else {
                Baseline::Middle
            };
            ctx.text(
                lx,
                ly,
                self.names[i].clone(),
                ts,
                ctx.theme.text,
                anchor,
                base,
            );
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        Vec::new()
    }
    fn kind(&self) -> &'static str {
        "chord"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- streamgraph -----------------------------------------------------------

/// Streamgraph: stacked layers around a moving baseline that keeps them
/// as flat as possible. Created by [`Axes::streamgraph`].
#[derive(Clone, Debug)]
pub struct Streamgraph {
    pub(crate) x: Vec<f64>,
    pub(crate) names: Vec<String>,
    /// Lower and upper edge of each layer at each x.
    pub(crate) bands: Vec<(Vec<f64>, Vec<f64>)>,
    pub(crate) colors: Vec<Color>,
    pub(crate) labels: bool,
}

impl Streamgraph {
    /// Names inside the layers where they fit (default on).
    pub fn labels(&mut self, on: bool) -> &mut Self {
        self.labels = on;
        self
    }
}

/// Lower edges of stacked layers with the "wiggle" baseline (Byron &
/// Wattenberg 2008).
pub(crate) fn wiggle_stack(x: &[f64], layers: &[Vec<f64>]) -> Vec<(Vec<f64>, Vec<f64>)> {
    let m = x.len();
    let n = layers.len();
    let v = |i: usize, t: usize| {
        layers[i]
            .get(t)
            .copied()
            .filter(|v| v.is_finite())
            .unwrap_or(0.0)
            .max(0.0)
    };
    let mut g0 = vec![0.0; m];
    // Start centred, then follow the weighted slope.
    let total0: f64 = (0..n).map(|i| v(i, 0)).sum();
    if m > 0 {
        g0[0] = -total0 / 2.0;
    }
    for t in 1..m {
        let dx = (x[t] - x[t - 1]).abs().max(1e-300);
        let total: f64 = (0..n).map(|i| v(i, t)).sum();
        let mut s1 = 0.0;
        for i in 0..n {
            let d = (v(i, t) - v(i, t - 1)) / dx;
            let below: f64 = (0..i).map(|j| (v(j, t) - v(j, t - 1)) / dx).sum();
            s1 += v(i, t) * (below + d / 2.0);
        }
        let slope = if total > 0.0 { -s1 / total } else { 0.0 };
        g0[t] = g0[t - 1] + slope * dx;
    }
    // Keep the middle of the stream near zero.
    let mean_mid: f64 = (0..m)
        .map(|t| g0[t] + (0..n).map(|i| v(i, t)).sum::<f64>() / 2.0)
        .sum::<f64>()
        / m.max(1) as f64;
    let mut out = Vec::with_capacity(n);
    let mut base: Vec<f64> = g0.iter().map(|g| g - mean_mid).collect();
    for i in 0..n {
        let top: Vec<f64> = (0..m).map(|t| base[t] + v(i, t)).collect();
        out.push((base.clone(), top.clone()));
        base = top;
    }
    out
}

impl Series for Streamgraph {
    fn extent(&self) -> Option<Extent> {
        let (lo, hi) = self
            .bands
            .iter()
            .flat_map(|(a, b)| a.iter().chain(b))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        let (x0, x1) = self
            .x
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        (lo.is_finite() && x0.is_finite()).then(|| Extent::point(x0, lo).include(x1, hi))
    }
    fn draw(&self, ctx: &mut Context) {
        for (i, (lo, hi)) in self.bands.iter().enumerate() {
            let mut pts: Vec<(f64, f64)> = self
                .x
                .iter()
                .zip(hi)
                .map(|(x, y)| ctx.point(*x, *y))
                .collect();
            pts.extend(self.x.iter().zip(lo).rev().map(|(x, y)| ctx.point(*x, *y)));
            let c = self.colors[i % self.colors.len()];
            ctx.polygon(
                &pts,
                Some(c),
                Some(Stroke::new(ctx.theme.plot_background.with_alpha(0.6), 0.6)),
            );
        }
        if self.labels {
            let ts = ctx.theme.tick_size;
            for (i, (lo, hi)) in self.bands.iter().enumerate() {
                let Some((t, _)) = lo
                    .iter()
                    .zip(hi)
                    .map(|(a, b)| b - a)
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                else {
                    continue;
                };
                let (x, y0) = ctx.point(self.x[t], lo[t]);
                let (_, y1) = ctx.point(self.x[t], hi[t]);
                let w = ctx.text_width(&self.names[i], ts);
                if (y0 - y1).abs() > ts * 1.4 {
                    let x = x.clamp(ctx.rect.x + w / 2.0 + 4.0, ctx.rect.right() - w / 2.0 - 4.0);
                    let c = self.colors[i % self.colors.len()];
                    ctx.text(
                        x,
                        (y0 + y1) / 2.0,
                        self.names[i].clone(),
                        ts,
                        c.contrasting_text(),
                        Anchor::Middle,
                        Baseline::Middle,
                    );
                }
            }
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        if self.labels {
            return Vec::new();
        }
        self.names
            .iter()
            .enumerate()
            .map(|(i, n)| patch(n, self.colors[i % self.colors.len()]))
            .collect()
    }
    fn kind(&self) -> &'static str {
        "streamgraph"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- horizon ---------------------------------------------------------------

/// Horizon chart: many time series in little height, values folded into
/// bands of rising color intensity (negative values in red). Created by
/// [`Axes::horizon`].
#[derive(Clone, Debug)]
pub struct Horizon {
    pub(crate) x: Vec<f64>,
    pub(crate) rows: Vec<Vec<f64>>,
    pub(crate) bands: usize,
    pub(crate) step: f64,
    pub(crate) pos: Color,
    pub(crate) neg: Color,
}

impl Horizon {
    /// Number of bands (default 3).
    pub fn bands(&mut self, n: usize) -> &mut Self {
        self.bands = n.clamp(1, 8);
        self.step = horizon_step(&self.rows, self.bands);
        self
    }
    /// Colors of positive and negative values.
    pub fn colors(&mut self, pos: impl Into<Color>, neg: impl Into<Color>) -> &mut Self {
        self.pos = pos.into();
        self.neg = neg.into();
        self
    }
}

fn horizon_step(rows: &[Vec<f64>], bands: usize) -> f64 {
    let m = finite_max(rows.iter().flatten().map(|v| v.abs()));
    if m.is_finite() && m > 0.0 {
        m / bands as f64
    } else {
        1.0
    }
}

impl Series for Horizon {
    fn extent(&self) -> Option<Extent> {
        let (x0, x1) = self
            .x
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        x0.is_finite()
            .then(|| Extent::point(x0, -0.5).include(x1, self.rows.len() as f64 - 0.5))
    }
    fn draw(&self, ctx: &mut Context) {
        let bg = ctx.theme.plot_background;
        for k in 0..self.bands {
            let t = (k + 1) as f64 / self.bands as f64;
            for (sign, base) in [(1.0, self.pos), (-1.0, self.neg)] {
                let mut s = Shapes::default();
                for (r, row) in self.rows.iter().enumerate() {
                    // Row r occupies [r - 0.5, r + 0.5]; bands grow upwards
                    // from its bottom edge (the axis is inverted: top = 0).
                    let y0 = r as f64 + 0.48;
                    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(row.len() + 2);
                    pts.push(ctx.point(self.x[0], y0));
                    for (x, v) in self.x.iter().zip(row) {
                        let part = ((sign * v - k as f64 * self.step) / self.step).clamp(0.0, 1.0);
                        let part = if v.is_finite() { part } else { 0.0 };
                        pts.push(ctx.point(*x, y0 - part * 0.96));
                    }
                    pts.push(ctx.point(*self.x.last().unwrap(), y0));
                    s.poly(&pts);
                }
                s.fill(ctx, bg.mix(base, 0.25 + 0.75 * t));
            }
        }
    }
    fn kind(&self) -> &'static str {
        "horizon"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// The first `n` palette colors (not advancing the series colors).
    fn palette_first(&self, n: usize) -> Vec<Color> {
        (0..n.max(1))
            .map(|i| self.defaults.palette[i % self.defaults.palette.len()])
            .collect()
    }

    fn category_rows(&mut self, names: &[String]) {
        self.y.categories(names.to_vec());
        self.y.inverted = true;
        self.y.grid = Some(false);
    }

    /// Bullet chart (Few): per row the measure as a bar, the target as a
    /// tick and qualitative ranges (e.g. poor / fair / good upper limits)
    /// as grey bands.
    pub fn bullet(
        &mut self,
        names: impl IntoLabels,
        values: impl IntoValues,
        targets: impl IntoValues,
        ranges: &[Vec<f64>],
    ) -> &mut Bullet {
        let names = names.into_labels();
        self.category_rows(&names);
        let color = self.defaults.palette[0];
        self.x.format =
            crate::plot::TickFormat::Custom(std::sync::Arc::new(|v| format!("{v:.0} %")));
        // Room right of 100 % for the "value / target" labels.
        self.x.limits(0.0, 128.0);
        self.x.margin = 0.0;
        self.x.ticks = TickSpec::At(vec![0.0, 25.0, 50.0, 75.0, 100.0]);
        self.x.grid = Some(false);
        self.add(Bullet {
            values: values.into_values(),
            targets: targets.into_values(),
            ranges: ranges.to_vec(),
            color,
        })
    }

    /// Dumbbell chart: a start and an end value per category.
    pub fn dumbbell(
        &mut self,
        names: impl IntoLabels,
        start: impl IntoValues,
        end: impl IntoValues,
    ) -> &mut Dumbbell {
        let names = names.into_labels();
        self.category_rows(&names);
        let p = self.palette_first(2);
        self.add(Dumbbell {
            start: start.into_values(),
            end: end.into_values(),
            colors: [p[0].mix(Color::WHITE, 0.35), p[0]],
            labels: [None, None],
            size: 11.0,
        })
    }

    /// Slope chart: each category's value at two times (`before_label`,
    /// `after_label` on the x-axis), rising lines in one color, falling in
    /// another, labelled at both ends.
    pub fn slope(
        &mut self,
        names: impl IntoLabels,
        before: impl IntoValues,
        after: impl IntoValues,
        before_label: &str,
        after_label: &str,
    ) -> &mut Slope {
        self.x.ticks =
            TickSpec::Labeled(vec![(0.0, before_label.into()), (1.0, after_label.into())]);
        self.y.visible = false;
        self.y.grid = Some(false);
        self.x.grid = Some(false);
        self.add(Slope {
            names: names.into_labels(),
            before: before.into_values(),
            after: after.into_values(),
            up: Color::rgb(36, 132, 92),
            down: Color::rgb(200, 66, 60),
            decimals: 0,
        })
    }

    /// Bump chart: `ranks[k][t]` is the rank (1 = top) of series `k` in
    /// period `t`; `periods` names the periods.
    pub fn bump(
        &mut self,
        names: impl IntoLabels,
        periods: impl IntoLabels,
        ranks: &[Vec<f64>],
    ) -> &mut Bump {
        let names = names.into_labels();
        let periods = periods.into_labels();
        self.x.ticks = TickSpec::Labeled(
            periods
                .iter()
                .enumerate()
                .map(|(i, p)| (i as f64, p.clone()))
                .collect(),
        );
        self.y.inverted = true;
        let top = finite_max(ranks.iter().flatten().copied()).max(1.0) as usize;
        self.y.ticks = TickSpec::Labeled((1..=top).map(|r| (r as f64, format!("#{r}"))).collect());
        self.x.grid = Some(false);
        let colors = self.palette_first(names.len());
        self.add(Bump {
            names,
            ranks: ranks.to_vec(),
            colors,
            width: 3.0,
        })
    }

    /// Waffle chart: shares of a whole as a 10 × 10 grid of squares.
    pub fn waffle(&mut self, names: impl IntoLabels, values: impl IntoValues) -> &mut Waffle {
        self.kind = AxesKind::Frameless;
        let names = names.into_labels();
        let shares = values.into_values();
        let colors = self.palette_first(names.len());
        self.legend(crate::plot::LegendPosition::OutsideRight);
        self.add(Waffle {
            counts: largest_remainder(&shares, 100),
            names,
            colors,
            rows: 10,
            cols: 10,
            shares,
        })
    }

    /// Marimekko (mosaic) chart: `values[column][row]`; column widths show
    /// the column totals, segments the shares within each column.
    pub fn marimekko(
        &mut self,
        columns: impl IntoLabels,
        rows: impl IntoLabels,
        values: &[Vec<f64>],
    ) -> &mut Marimekko {
        let rows = rows.into_labels();
        let columns = columns.into_labels();
        let colors = self.palette_first(rows.len());
        self.y.format =
            crate::plot::TickFormat::Custom(std::sync::Arc::new(|v| format!("{v:.0} %")));
        // Column names under the column centres, with their share.
        let totals: Vec<f64> = values
            .iter()
            .map(|c| c.iter().filter(|v| v.is_finite()).sum())
            .collect();
        let grand: f64 = totals.iter().sum::<f64>().max(1e-300);
        let mut at = 0.0;
        let mut ticks = Vec::new();
        for (c, t) in totals.iter().enumerate() {
            let w = t / grand * 100.0;
            ticks.push((
                at + w / 2.0,
                format!(
                    "{} ({:.0} %)",
                    columns.get(c).cloned().unwrap_or_default(),
                    w
                ),
            ));
            at += w;
        }
        self.x.ticks = TickSpec::Labeled(ticks);
        self.x.grid = Some(false);
        self.x.limits(0.0, 100.0);
        self.y.limits(0.0, 100.0);
        self.x.margin = 0.0;
        self.y.margin = 0.0;
        self.legend(crate::plot::LegendPosition::OutsideRight);
        self.add(Marimekko {
            rows,
            values: values.to_vec(),
            colors,
        })
    }

    /// Chord diagram of flows `matrix[from][to]` between named groups.
    pub fn chord(&mut self, names: impl IntoLabels, matrix: &[Vec<f64>]) -> &mut Chord {
        self.kind = AxesKind::Frameless;
        let names = names.into_labels();
        let colors = self.palette_first(names.len());
        self.add(Chord {
            names,
            matrix: matrix.to_vec(),
            colors,
            gap: 2.0,
            opacity: 0.68,
        })
    }

    /// Streamgraph of named layers over `x` (layers are non-negative).
    pub fn streamgraph<S: AsRef<str>>(
        &mut self,
        x: impl IntoValues,
        layers: &[(S, Vec<f64>)],
    ) -> &mut Streamgraph {
        let x = x.into_values();
        let values: Vec<Vec<f64>> = layers.iter().map(|l| l.1.clone()).collect();
        let bands = wiggle_stack(&x, &values);
        self.y.visible = false;
        self.y.grid = Some(false);
        let colors = self.palette_first(layers.len());
        self.add(Streamgraph {
            x,
            names: layers.iter().map(|l| l.0.as_ref().to_string()).collect(),
            bands,
            colors,
            labels: true,
        })
    }

    /// Horizon chart: one thin row per named series over `x`.
    pub fn horizon<S: AsRef<str>>(
        &mut self,
        x: impl IntoValues,
        rows: &[(S, Vec<f64>)],
    ) -> &mut Horizon {
        let names: Vec<String> = rows.iter().map(|r| r.0.as_ref().to_string()).collect();
        self.category_rows(&names);
        let rows: Vec<Vec<f64>> = rows.iter().map(|r| r.1.clone()).collect();
        let step = horizon_step(&rows, 3);
        self.add(Horizon {
            x: x.into_values(),
            rows,
            bands: 3,
            step,
            pos: Color::rgb(33, 102, 172),
            neg: Color::rgb(178, 24, 43),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn largest_remainder_sums() {
        let c = largest_remainder(&[1.0, 1.0, 1.0], 100);
        assert_eq!(c.iter().sum::<usize>(), 100);
        assert_eq!(
            largest_remainder(&[50.0, 30.0, 20.0], 100),
            vec![50, 30, 20]
        );
    }

    #[test]
    fn wiggle_layers_stack() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let s = wiggle_stack(&x, &[vec![1.0, 2.0, 3.0, 2.0], vec![2.0, 2.0, 1.0, 1.0]]);
        for t in 0..4 {
            assert!((s[0].1[t] - s[1].0[t]).abs() < 1e-12);
            assert!((s[1].1[t] - s[1].0[t] - [2.0, 2.0, 1.0, 1.0][t]).abs() < 1e-12);
        }
    }

    #[test]
    fn spread_keeps_gap() {
        let y = spread_labels(&[10.0, 11.0, 12.0, 40.0], 5.0);
        let mut s = y.clone();
        s.sort_by(f64::total_cmp);
        assert!(s.windows(2).all(|w| w[1] - w[0] >= 4.999));
    }

    #[test]
    fn all_render() {
        let mut fig = Figure::new(1200, 800);
        fig.grid(3, 3);
        fig.at(0, 0).bullet(
            ["A", "B"],
            [70.0, 40.0],
            [80.0, 60.0],
            &[vec![50.0, 75.0, 100.0], vec![30.0, 60.0, 90.0]],
        );
        fig.at(0, 1)
            .dumbbell(["A", "B"], [1.0, 2.0], [3.0, 1.5])
            .labels("2015", "2025");
        fig.at(0, 2).slope(
            ["A", "B", "C"],
            [1.0, 5.0, 3.0],
            [4.0, 2.0, 3.5],
            "2015",
            "2025",
        );
        fig.at(1, 0).bump(
            ["a", "b", "c"],
            ["Q1", "Q2", "Q3"],
            &[
                vec![1.0, 2.0, 3.0],
                vec![2.0, 1.0, 1.0],
                vec![3.0, 3.0, 2.0],
            ],
        );
        fig.at(1, 1).waffle(["x", "y"], [62.0, 38.0]);
        fig.at(1, 2)
            .marimekko(["A", "B"], ["r", "s"], &[vec![3.0, 1.0], vec![1.0, 1.0]]);
        fig.at(2, 0).chord(
            ["a", "b", "c"],
            &[
                vec![0.0, 5.0, 2.0],
                vec![3.0, 0.0, 1.0],
                vec![1.0, 4.0, 2.0],
            ],
        );
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        fig.at(2, 1).streamgraph(
            &x,
            &[
                ("a", x.iter().map(|v| (v / 5.0).sin() + 1.5).collect()),
                ("b", x.iter().map(|v| (v / 7.0).cos() + 1.2).collect()),
            ],
        );
        fig.at(2, 2).horizon(
            &x,
            &[
                ("a", x.iter().map(|v| (v / 4.0).sin() * 3.0).collect()),
                ("b", x.iter().map(|v| v / 10.0 - 2.0).collect()),
            ],
        );
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
        fig.save(std::env::temp_dir().join("dataviz_general.svg"))
            .unwrap();
    }
}
