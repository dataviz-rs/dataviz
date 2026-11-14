//! Charts without x/y axes: pie/donut, treemap, sunburst, sankey, funnel,
//! gauge, network graph; plus parallel coordinates and calendar heatmaps.

use super::{
    short, Context, Extent, Hit, IntoLabels, IntoValues, LegendEntry, LegendSymbol, Rect, Series,
};
use crate::plot::axes::{Axes, AxesKind};
use crate::plot::axis::{AxisMap, TickSpec};
use crate::plot::color::{Color, Colormap};
use crate::plot::scene::{Anchor, Baseline, Node, PathCmd, Stroke, TextNode};
use crate::plot::time;
use std::any::Any;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn text_node(
    x: f64,
    y: f64,
    t: impl Into<String>,
    size: f64,
    color: Color,
    anchor: Anchor,
    bold: bool,
) -> Node {
    Node::Text(TextNode {
        x,
        y,
        text: t.into(),
        size,
        color,
        anchor,
        baseline: Baseline::Middle,
        rotate: 0.0,
        bold,
        italic: false,
    })
}

/// Annular sector path (screen angles in radians, counter-clockwise).
fn sector(c: (f64, f64), r0: f64, r1: f64, a0: f64, a1: f64) -> Vec<PathCmd> {
    let p = |r: f64, a: f64| (c.0 + r * a.cos(), c.1 - r * a.sin());
    let sweep = a1 - a0;
    if sweep.abs() >= TAU - 1e-9 {
        // Full ring: two half arcs each.
        let mut v = vec![
            PathCmd::MoveTo(c.0 + r1, c.1),
            PathCmd::ArcTo(r1, r1, false, false, c.0 - r1, c.1),
            PathCmd::ArcTo(r1, r1, false, false, c.0 + r1, c.1),
            PathCmd::Close,
        ];
        if r0 > 0.0 {
            v.extend([
                PathCmd::MoveTo(c.0 + r0, c.1),
                PathCmd::ArcTo(r0, r0, false, true, c.0 - r0, c.1),
                PathCmd::ArcTo(r0, r0, false, true, c.0 + r0, c.1),
                PathCmd::Close,
            ]);
        }
        return v;
    }
    let large = sweep.abs() > PI;
    // Counter-clockwise on screen = sweep flag 0 in SVG (y down).
    let ccw = sweep > 0.0;
    let (s1, e1) = (p(r1, a0), p(r1, a1));
    let mut v = vec![
        PathCmd::MoveTo(s1.0, s1.1),
        PathCmd::ArcTo(r1, r1, large, !ccw, e1.0, e1.1),
    ];
    if r0 > 0.0 {
        let (s0, e0) = (p(r0, a1), p(r0, a0));
        v.push(PathCmd::LineTo(s0.0, s0.1));
        v.push(PathCmd::ArcTo(r0, r0, large, ccw, e0.0, e0.1));
    } else {
        v.push(PathCmd::LineTo(c.0, c.1));
    }
    v.push(PathCmd::Close);
    v
}

// ----------------------------------------------------------------- pie ----

/// Pie or donut chart. Created by [`Axes::pie`].
#[derive(Clone, Debug)]
pub struct Pie {
    pub(crate) labels: Vec<String>,
    pub(crate) values: Vec<f64>,
    pub(crate) colors: Vec<Color>,
    pub(crate) hole: f64,
    pub(crate) start_deg: f64,
    pub(crate) explode: Vec<f64>,
    pub(crate) show_percent: bool,
    pub(crate) show_labels: bool,
    pub(crate) center_text: Option<String>,
}

impl Pie {
    /// Donut: inner radius as a fraction of the outer radius (0..0.95).
    pub fn donut(&mut self, hole: f64) -> &mut Self {
        self.hole = hole.clamp(0.0, 0.95);
        self
    }
    /// Angle of the first slice edge (degrees, counter-clockwise from east;
    /// default 90 = 12 o'clock, slices clockwise).
    pub fn start_angle(&mut self, deg: f64) -> &mut Self {
        self.start_deg = deg;
        self
    }
    /// Pulls slices out by a fraction of the radius.
    pub fn explode(&mut self, offsets: impl IntoValues) -> &mut Self {
        self.explode = offsets.into_values();
        self
    }
    /// Colors per slice.
    pub fn colors<C: Into<Color>>(&mut self, colors: impl IntoIterator<Item = C>) -> &mut Self {
        self.colors = colors.into_iter().map(Into::into).collect();
        self
    }
    /// Percentages on slices (default on).
    pub fn percent(&mut self, on: bool) -> &mut Self {
        self.show_percent = on;
        self
    }
    /// Labels next to slices (default off; the legend lists them).
    pub fn labels(&mut self, on: bool) -> &mut Self {
        self.show_labels = on;
        self
    }
    /// Text in the middle of a donut.
    pub fn center_text(&mut self, t: impl Into<String>) -> &mut Self {
        self.center_text = Some(t.into());
        self
    }

    fn slices(&self) -> Vec<(usize, f64, f64, f64)> {
        let v = |x: f64| if x.is_finite() && x > 0.0 { x } else { 0.0 };
        let total: f64 = self.values.iter().map(|x| v(*x)).sum();
        let mut a = self.start_deg.to_radians();
        let mut out = Vec::new();
        for (i, x) in self.values.iter().enumerate() {
            let sweep = if total > 0.0 {
                v(*x) / total * TAU
            } else {
                0.0
            };
            out.push((
                i,
                a,
                a - sweep,
                if total > 0.0 { v(*x) / total } else { 0.0 },
            ));
            a -= sweep;
        }
        out
    }

    fn color(&self, i: usize) -> Color {
        self.colors[i % self.colors.len().max(1)]
    }

    fn geometry(&self, r: Rect) -> ((f64, f64), f64) {
        let room = if self.show_labels { 0.72 } else { 0.92 };
        (r.center(), r.w.min(r.h) / 2.0 * room)
    }
}

impl Series for Pie {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let (c, r) = self.geometry(ctx.rect);
        let r0 = r * self.hole;
        let size = ctx.theme.annotation_size;
        for (i, a0, a1, frac) in self.slices() {
            if frac <= 0.0 {
                continue;
            }
            let mid = (a0 + a1) / 2.0;
            let off = self.explode.get(i).copied().unwrap_or(0.0) * r;
            let cc = (c.0 + off * mid.cos(), c.1 - off * mid.sin());
            ctx.push(Node::Group {
                children: vec![Node::Path {
                    cmds: sector(cc, r0, r, a0, a1),
                    fill: Some(self.color(i)),
                    stroke: Some(Stroke::new(ctx.theme.background, 1.5)),
                }],
                clip: None,
                opacity: 1.0,
                tooltip: Some(format!(
                    "{}: {} ({:.1}%)",
                    self.labels[i],
                    short(self.values[i]),
                    frac * 100.0
                )),
            });
            if self.show_percent && frac > 0.03 {
                let rr = if self.hole > 0.0 {
                    (r0 + r) / 2.0
                } else {
                    r * 0.62
                };
                let (x, y) = (cc.0 + rr * mid.cos(), cc.1 - rr * mid.sin());
                ctx.push(text_node(
                    x,
                    y,
                    format!("{:.1}%", frac * 100.0),
                    size,
                    self.color(i).contrasting_text(),
                    Anchor::Middle,
                    false,
                ));
            }
            if self.show_labels {
                let (p1, p2) = (
                    (cc.0 + r * mid.cos(), cc.1 - r * mid.sin()),
                    (cc.0 + r * 1.12 * mid.cos(), cc.1 - r * 1.12 * mid.sin()),
                );
                let right = mid.cos() >= 0.0;
                let p3 = (p2.0 + if right { 10.0 } else { -10.0 }, p2.1);
                ctx.polyline(&[p1, p2, p3], Stroke::new(ctx.theme.axis, 1.0));
                ctx.push(text_node(
                    p3.0 + if right { 3.0 } else { -3.0 },
                    p3.1,
                    self.labels[i].clone(),
                    ctx.theme.tick_size,
                    ctx.theme.text,
                    if right { Anchor::Start } else { Anchor::End },
                    false,
                ));
            }
        }
        if let Some(t) = &self.center_text {
            ctx.push(text_node(
                c.0,
                c.1,
                t.clone(),
                ctx.theme.axes_title_size,
                ctx.theme.text,
                Anchor::Middle,
                true,
            ));
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        self.labels
            .iter()
            .enumerate()
            .map(|(i, l)| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.color(i),
                    edge: None,
                },
            })
            .collect()
    }
    fn hit(&self, _x: &AxisMap, _y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let (c, r) = self.geometry(rect);
        let (dx, dy) = (px - c.0, c.1 - py);
        let d = dx.hypot(dy);
        if d > r * 1.1 || d < r * self.hole {
            return None;
        }
        let a = dy.atan2(dx);
        for (i, a0, a1, frac) in self.slices() {
            let (lo, hi) = (a1.min(a0), a1.max(a0));
            let norm = |x: f64| (x - lo).rem_euclid(TAU);
            if norm(a) <= hi - lo {
                let mid = (a0 + a1) / 2.0;
                return Some(Hit {
                    px: c.0 + r * 0.6 * mid.cos(),
                    py: c.1 - r * 0.6 * mid.sin(),
                    label: format!(
                        "{}: {} ({:.1}%)",
                        self.labels[i],
                        short(self.values[i]),
                        frac * 100.0
                    ),
                    distance: 0.0,
                });
            }
        }
        None
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ------------------------------------------------------------ hierarchy ----

/// A node of hierarchical data (treemaps, sunbursts).
#[derive(Clone, Debug, PartialEq)]
pub struct TreeNode {
    /// Label.
    pub label: String,
    /// Own value (leaves); groups sum their children.
    pub value: f64,
    /// Children.
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// A leaf with a value.
    pub fn leaf(label: impl Into<String>, value: f64) -> Self {
        Self {
            label: label.into(),
            value,
            children: Vec::new(),
        }
    }
    /// A group of children.
    pub fn group(label: impl Into<String>, children: Vec<TreeNode>) -> Self {
        Self {
            label: label.into(),
            value: 0.0,
            children,
        }
    }
    /// Total value (own value for leaves, sum of children otherwise).
    pub fn total(&self) -> f64 {
        if self.children.is_empty() {
            if self.value.is_finite() && self.value > 0.0 {
                self.value
            } else {
                0.0
            }
        } else {
            self.children.iter().map(TreeNode::total).sum()
        }
    }
}

/// Squarified treemap layout of `values` into `r`.
pub(crate) fn squarify(values: &[f64], r: Rect) -> Vec<Rect> {
    let total: f64 = values.iter().sum();
    let mut out = vec![
        Rect {
            x: r.x,
            y: r.y,
            w: 0.0,
            h: 0.0
        };
        values.len()
    ];
    if total <= 0.0 || r.w <= 0.0 || r.h <= 0.0 {
        return out;
    }
    let scale = r.w * r.h / total;
    let mut order: Vec<usize> = (0..values.len()).filter(|i| values[*i] > 0.0).collect();
    order.sort_by(|a, b| values[*b].total_cmp(&values[*a]));
    let mut rem = r;
    let mut i = 0;
    while i < order.len() {
        let short_side = rem.w.min(rem.h);
        let mut row = vec![order[i]];
        let worst = |row: &[usize]| {
            let s: f64 = row.iter().map(|k| values[*k] * scale).sum();
            let (mx, mn) = row
                .iter()
                .map(|k| values[*k] * scale)
                .fold((0.0f64, f64::INFINITY), |(a, b), v| (a.max(v), b.min(v)));
            let side2 = short_side * short_side;
            ((side2 * mx) / (s * s)).max((s * s) / (side2 * mn))
        };
        let mut j = i + 1;
        while j < order.len() {
            let mut cand = row.clone();
            cand.push(order[j]);
            if worst(&cand) <= worst(&row) {
                row = cand;
                j += 1;
            } else {
                break;
            }
        }
        let s: f64 = row.iter().map(|k| values[*k] * scale).sum();
        if rem.w >= rem.h {
            let w = s / rem.h;
            let mut y = rem.y;
            for k in &row {
                let h = values[*k] * scale / w;
                out[*k] = Rect { x: rem.x, y, w, h };
                y += h;
            }
            rem = Rect {
                x: rem.x + w,
                y: rem.y,
                w: (rem.w - w).max(0.0),
                h: rem.h,
            };
        } else {
            let h = s / rem.w;
            let mut x = rem.x;
            for k in &row {
                let w = values[*k] * scale / h;
                out[*k] = Rect { x, y: rem.y, w, h };
                x += w;
            }
            rem = Rect {
                x: rem.x,
                y: rem.y + h,
                w: rem.w,
                h: (rem.h - h).max(0.0),
            };
        }
        i = j;
    }
    out
}

/// Treemap. Created by [`Axes::treemap`].
#[derive(Clone, Debug)]
pub struct Treemap {
    pub(crate) root: Vec<TreeNode>,
    pub(crate) colors: Vec<Color>,
    pub(crate) padding: f64,
}

impl Treemap {
    /// Colors per top-level group.
    pub fn colors<C: Into<Color>>(&mut self, colors: impl IntoIterator<Item = C>) -> &mut Self {
        self.colors = colors.into_iter().map(Into::into).collect();
        self
    }

    fn cells(&self, r: Rect) -> Vec<(Rect, usize, String, f64, usize)> {
        // (rect, depth, label, value, top-level index)
        let mut out = Vec::new();
        fn rec(
            nodes: &[TreeNode],
            r: Rect,
            depth: usize,
            top: Option<usize>,
            pad: f64,
            out: &mut Vec<(Rect, usize, String, f64, usize)>,
        ) {
            let vals: Vec<f64> = nodes.iter().map(TreeNode::total).collect();
            for (i, (n, cell)) in nodes.iter().zip(squarify(&vals, r)).enumerate() {
                let t = top.unwrap_or(i);
                out.push((cell, depth, n.label.clone(), n.total(), t));
                if !n.children.is_empty() {
                    let head = if cell.h > 40.0 { 16.0 } else { 0.0 };
                    let inner = Rect {
                        x: cell.x + pad,
                        y: cell.y + pad + head,
                        w: (cell.w - 2.0 * pad).max(0.0),
                        h: (cell.h - 2.0 * pad - head).max(0.0),
                    };
                    rec(&n.children, inner, depth + 1, Some(t), pad, out);
                }
            }
        }
        rec(&self.root, r, 0, None, self.padding, &mut out);
        out
    }
}

impl Series for Treemap {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let size = ctx.theme.annotation_size;
        for (r, depth, label, value, top) in self.cells(ctx.rect) {
            if r.w < 1.0 || r.h < 1.0 {
                continue;
            }
            let base = self.colors[top % self.colors.len().max(1)];
            let color = base.lighten((depth as f64 * 0.18).min(0.6));
            ctx.push(Node::Group {
                children: vec![Node::Rect {
                    x: r.x,
                    y: r.y,
                    w: r.w,
                    h: r.h,
                    rx: 0.0,
                    fill: Some(color),
                    stroke: Some(Stroke::new(
                        ctx.theme.background,
                        1.0 + (1.0 - depth as f64 * 0.5).max(0.0),
                    )),
                }],
                clip: None,
                opacity: 1.0,
                tooltip: Some(format!("{label}: {}", short(value))),
            });
            let tc = color.contrasting_text();
            let has_children = self.cells_children(&label, depth);
            if has_children {
                if r.h > 40.0 && ctx.text_width(&label, size) < r.w - 6.0 {
                    ctx.push(Node::Text(TextNode {
                        x: r.x + 4.0,
                        y: r.y + 9.0,
                        text: label,
                        size,
                        color: tc,
                        anchor: Anchor::Start,
                        baseline: Baseline::Middle,
                        rotate: 0.0,
                        bold: true,
                        italic: false,
                    }));
                }
            } else if ctx.text_width(&label, size) < r.w - 6.0 && r.h > size * 2.6 {
                ctx.push(text_node(
                    r.x + r.w / 2.0,
                    r.y + r.h / 2.0 - size * 0.6,
                    label,
                    size,
                    tc,
                    Anchor::Middle,
                    true,
                ));
                ctx.push(text_node(
                    r.x + r.w / 2.0,
                    r.y + r.h / 2.0 + size * 0.7,
                    short(value),
                    size * 0.9,
                    tc,
                    Anchor::Middle,
                    false,
                ));
            } else if ctx.text_width(&label, size) < r.w - 4.0 && r.h > size * 1.3 {
                ctx.push(text_node(
                    r.x + r.w / 2.0,
                    r.y + r.h / 2.0,
                    label,
                    size,
                    tc,
                    Anchor::Middle,
                    false,
                ));
            }
        }
    }
    fn hit(&self, _x: &AxisMap, _y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        self.cells(rect)
            .into_iter()
            .filter(|(r, ..)| r.contains(px, py))
            .max_by_key(|(_, d, ..)| *d)
            .map(|(r, _, l, v, _)| Hit {
                px: r.x + r.w / 2.0,
                py: r.y + r.h / 2.0,
                label: format!("{l}: {}", short(v)),
                distance: 0.0,
            })
    }
    fn legend(&self) -> Vec<LegendEntry> {
        if self.root.iter().all(|n| n.children.is_empty()) {
            return Vec::new();
        }
        self.root
            .iter()
            .enumerate()
            .map(|(i, n)| LegendEntry {
                label: n.label.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.colors[i % self.colors.len().max(1)],
                    edge: None,
                },
            })
            .collect()
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Treemap {
    fn cells_children(&self, label: &str, depth: usize) -> bool {
        fn find(nodes: &[TreeNode], label: &str, depth: usize, d: usize) -> bool {
            nodes.iter().any(|n| {
                (d == depth && n.label == label && !n.children.is_empty())
                    || (d < depth && find(&n.children, label, depth, d + 1))
            })
        }
        find(&self.root, label, depth, 0)
    }
}

/// Sunburst (radial hierarchy). Created by [`Axes::sunburst`].
#[derive(Clone, Debug)]
pub struct Sunburst {
    pub(crate) root: Vec<TreeNode>,
    pub(crate) colors: Vec<Color>,
    pub(crate) hole: f64,
}

impl Sunburst {
    fn depth(nodes: &[TreeNode]) -> usize {
        nodes
            .iter()
            .map(|n| 1 + Self::depth(&n.children))
            .max()
            .unwrap_or(0)
    }
    fn arcs(&self) -> Vec<(usize, f64, f64, String, f64, usize)> {
        // (depth, a0, a1, label, value, top)
        let mut out = Vec::new();
        fn rec(
            nodes: &[TreeNode],
            a0: f64,
            a1: f64,
            depth: usize,
            top: Option<usize>,
            out: &mut Vec<(usize, f64, f64, String, f64, usize)>,
        ) {
            let total: f64 = nodes.iter().map(TreeNode::total).sum();
            if total <= 0.0 {
                return;
            }
            let mut a = a0;
            for (i, n) in nodes.iter().enumerate() {
                let sweep = (a1 - a0) * n.total() / total;
                let t = top.unwrap_or(i);
                out.push((depth, a, a + sweep, n.label.clone(), n.total(), t));
                rec(&n.children, a, a + sweep, depth + 1, Some(t), out);
                a += sweep;
            }
        }
        rec(&self.root, FRAC_PI_2, FRAC_PI_2 - TAU, 0, None, &mut out);
        out
    }
}

impl Series for Sunburst {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let c = ctx.rect.center();
        let r = ctx.rect.w.min(ctx.rect.h) / 2.0 * 0.95;
        let levels = Self::depth(&self.root).max(1) as f64;
        let ring = r * (1.0 - self.hole) / levels;
        let size = ctx.theme.annotation_size;
        for (d, a0, a1, label, value, top) in self.arcs() {
            let r0 = r * self.hole + d as f64 * ring;
            let base = self.colors[top % self.colors.len().max(1)];
            let color = base.lighten((d as f64 * 0.2).min(0.6));
            ctx.push(Node::Group {
                children: vec![Node::Path {
                    cmds: sector(c, r0, r0 + ring, a0, a1),
                    fill: Some(color),
                    stroke: Some(Stroke::new(ctx.theme.background, 1.0)),
                }],
                clip: None,
                opacity: 1.0,
                tooltip: Some(format!("{label}: {}", short(value))),
            });
            let mid = (a0 + a1) / 2.0;
            let rr = r0 + ring / 2.0;
            let arc_len = (a1 - a0).abs() * rr;
            let tw = ctx.text_width(&label, size);
            let (x, y) = (c.0 + rr * mid.cos(), c.1 - rr * mid.sin());
            if d > 0 && arc_len > size * 1.3 && ring > tw + 6.0 {
                // Narrow wedge: write along the radius, kept upright.
                let mut rot = -mid.to_degrees();
                if mid.cos() < 0.0 {
                    rot += 180.0;
                }
                ctx.push(Node::Text(TextNode {
                    x,
                    y,
                    text: label,
                    size,
                    color: color.contrasting_text(),
                    anchor: Anchor::Middle,
                    baseline: Baseline::Middle,
                    rotate: rot,
                    bold: false,
                    italic: false,
                }));
            } else if arc_len > tw + 4.0 && ring > size * 1.2 {
                ctx.push(text_node(
                    x,
                    y,
                    label,
                    size,
                    color.contrasting_text(),
                    Anchor::Middle,
                    false,
                ));
            }
        }
    }
    fn hit(&self, _x: &AxisMap, _y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let c = rect.center();
        let r = rect.w.min(rect.h) / 2.0 * 0.95;
        let levels = Self::depth(&self.root).max(1) as f64;
        let ring = r * (1.0 - self.hole) / levels;
        let (dx, dy) = (px - c.0, c.1 - py);
        let d = dx.hypot(dy);
        let level = ((d - r * self.hole) / ring).floor();
        if level < 0.0 {
            return None;
        }
        let a = dy.atan2(dx);
        self.arcs()
            .into_iter()
            .find(|(dep, a0, a1, ..)| {
                *dep as f64 == level && {
                    let (lo, hi) = (a1.min(*a0), a1.max(*a0));
                    (a - lo).rem_euclid(TAU) <= hi - lo
                }
            })
            .map(|(_, _, _, l, v, _)| Hit {
                px,
                py,
                label: format!("{l}: {}", short(v)),
                distance: 0.0,
            })
    }
    fn legend(&self) -> Vec<LegendEntry> {
        self.root
            .iter()
            .enumerate()
            .map(|(i, n)| LegendEntry {
                label: n.label.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.colors[i % self.colors.len().max(1)],
                    edge: None,
                },
            })
            .collect()
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// --------------------------------------------------------------- sankey ----

/// Sankey diagram of flows between nodes. Created by [`Axes::sankey`].
#[derive(Clone, Debug)]
pub struct Sankey {
    pub(crate) links: Vec<(String, String, f64)>,
    pub(crate) colors: Vec<Color>,
    pub(crate) node_width: f64,
    pub(crate) gap: f64,
}

struct SankeyLayout {
    names: Vec<String>,
    rects: Vec<Rect>,
    /// (source, target, value, y0 at source, y1 at target, thickness)
    bands: Vec<(usize, usize, f64, f64, f64, f64)>,
}

impl Sankey {
    /// Node bar width in pixels.
    pub fn node_width(&mut self, w: f64) -> &mut Self {
        self.node_width = w.max(1.0);
        self
    }
    /// Vertical gap between nodes in pixels.
    pub fn gap(&mut self, g: f64) -> &mut Self {
        self.gap = g.max(0.0);
        self
    }

    fn layout(&self, r: Rect, label_room: f64) -> SankeyLayout {
        let mut names: Vec<String> = Vec::new();
        let idx = |n: &str, names: &mut Vec<String>| match names.iter().position(|x| x == n) {
            Some(i) => i,
            None => {
                names.push(n.to_string());
                names.len() - 1
            }
        };
        let links: Vec<(usize, usize, f64)> = self
            .links
            .iter()
            .filter(|l| l.2.is_finite() && l.2 > 0.0)
            .map(|(a, b, v)| {
                let i = idx(a, &mut names);
                let j = idx(b, &mut names);
                (i, j, *v)
            })
            .collect();
        let n = names.len();
        // Column = longest path from a source (cycles are cut by the bound).
        let mut col = vec![0usize; n];
        for _ in 0..n {
            for &(a, b, _) in &links {
                if col[b] < col[a] + 1 && col[a] + 1 < n {
                    col[b] = col[a] + 1;
                }
            }
        }
        // Sinks go to the last column.
        let max_col = col.iter().copied().max().unwrap_or(0);
        for (i, c) in col.iter_mut().enumerate() {
            if !links.iter().any(|l| l.0 == i) && links.iter().any(|l| l.1 == i) {
                *c = max_col;
            }
        }
        let inflow = |i: usize| links.iter().filter(|l| l.1 == i).map(|l| l.2).sum::<f64>();
        let outflow = |i: usize| links.iter().filter(|l| l.0 == i).map(|l| l.2).sum::<f64>();
        let size: Vec<f64> = (0..n).map(|i| inflow(i).max(outflow(i))).collect();
        let usable = Rect {
            x: r.x,
            y: r.y,
            w: (r.w - label_room).max(10.0),
            h: r.h,
        };
        let ky = (0..=max_col)
            .map(|c| {
                let members: Vec<usize> = (0..n).filter(|i| col[*i] == c).collect();
                let total: f64 = members.iter().map(|i| size[*i]).sum();
                (usable.h - self.gap * (members.len().max(1) - 1) as f64) / total.max(1e-12)
            })
            .fold(f64::INFINITY, f64::min);
        let ky = if ky.is_finite() { ky.max(0.0) } else { 1.0 };
        let mut rects = vec![
            Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0
            };
            n
        ];
        for c in 0..=max_col {
            let members: Vec<usize> = (0..n).filter(|i| col[*i] == c).collect();
            let total: f64 = members.iter().map(|i| size[*i] * ky).sum::<f64>()
                + self.gap * (members.len().max(1) - 1) as f64;
            let mut y = usable.y + (usable.h - total) / 2.0;
            let x = if max_col == 0 {
                usable.x
            } else {
                usable.x + (usable.w - self.node_width) * c as f64 / max_col as f64
            };
            for i in members {
                rects[i] = Rect {
                    x,
                    y,
                    w: self.node_width,
                    h: size[i] * ky,
                };
                y += size[i] * ky + self.gap;
            }
        }
        let mut out_off = vec![0.0; n];
        let mut in_off = vec![0.0; n];
        let mut sorted = links.clone();
        sorted.sort_by(|a, b| rects[a.1].y.total_cmp(&rects[b.1].y));
        let mut bands = Vec::new();
        for (a, b, v) in sorted {
            let t = v * ky;
            let y0 = rects[a].y + out_off[a];
            out_off[a] += t;
            bands.push((a, b, v, y0, 0.0, t));
        }
        bands.sort_by(|x, y| rects[x.0].y.total_cmp(&rects[y.0].y));
        for band in &mut bands {
            band.4 = rects[band.1].y + in_off[band.1];
            in_off[band.1] += band.5;
        }
        SankeyLayout {
            names,
            rects,
            bands,
        }
    }

    fn label_room(&self, ctx_width: &dyn Fn(&str) -> f64) -> f64 {
        // Room for labels of the last column (drawn right of the nodes).
        self.links
            .iter()
            .map(|l| ctx_width(&l.1))
            .fold(0.0, f64::max)
            + 8.0
    }
}

impl Series for Sankey {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let size = ctx.theme.annotation_size;
        let fonts = ctx.fonts;
        let room = self.label_room(&|s| fonts.width(s, size));
        let lay = self.layout(ctx.rect, room.min(ctx.rect.w * 0.3));
        let color = |i: usize| self.colors[i % self.colors.len().max(1)];
        for (a, b, v, y0, y1, t) in &lay.bands {
            let (x0, x1) = (lay.rects[*a].right(), lay.rects[*b].x);
            let xm = (x0 + x1) / 2.0;
            let cmds = vec![
                PathCmd::MoveTo(x0, *y0),
                PathCmd::CubicTo(xm, *y0, xm, *y1, x1, *y1),
                PathCmd::LineTo(x1, y1 + t),
                PathCmd::CubicTo(xm, y1 + t, xm, y0 + t, x0, y0 + t),
                PathCmd::Close,
            ];
            ctx.push(Node::Group {
                children: vec![Node::Path {
                    cmds,
                    fill: Some(color(*a).with_alpha(0.4)),
                    stroke: None,
                }],
                clip: None,
                opacity: 1.0,
                tooltip: Some(format!(
                    "{} → {}: {}",
                    lay.names[*a],
                    lay.names[*b],
                    short(*v)
                )),
            });
        }
        for (i, r) in lay.rects.iter().enumerate() {
            ctx.push(Node::Rect {
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h.max(1.0),
                rx: 0.0,
                fill: Some(color(i)),
                stroke: None,
            });
            let (x, anchor) = (r.right() + 5.0, Anchor::Start);
            ctx.push(text_node(
                x,
                r.y + r.h / 2.0,
                lay.names[i].clone(),
                size,
                ctx.theme.text,
                anchor,
                false,
            ));
        }
    }
    fn hit(&self, _x: &AxisMap, _y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let lay = self.layout(rect, 60.0);
        lay.rects
            .iter()
            .enumerate()
            .find(|(_, r)| r.contains(px, py))
            .map(|(i, r)| Hit {
                px: r.x + r.w / 2.0,
                py: r.y + r.h / 2.0,
                label: lay.names[i].clone(),
                distance: 0.0,
            })
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// --------------------------------------------------------------- funnel ----

/// Funnel chart. Created by [`Axes::funnel`].
#[derive(Clone, Debug)]
pub struct Funnel {
    pub(crate) labels: Vec<String>,
    pub(crate) values: Vec<f64>,
    pub(crate) colors: Vec<Color>,
}

impl Series for Funnel {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let r = ctx.rect;
        let n = self.values.len().max(1);
        let max = self
            .values
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(0.0, f64::max)
            .max(1e-12);
        let r = Rect { w: r.w * 0.75, ..r };
        let first = self.values.first().copied().unwrap_or(1.0);
        let h = r.h / n as f64;
        let size = ctx.theme.annotation_size;
        let cx = r.x + r.w / 2.0;
        for (i, v) in self.values.iter().enumerate() {
            let w = r.w * 0.8 * v.max(0.0) / max;
            let next = self.values.get(i + 1).copied().unwrap_or(*v).max(0.0);
            let wn = r.w * 0.8 * next / max;
            let y = r.y + i as f64 * h;
            let c = self.colors[i % self.colors.len().max(1)];
            let pts = [
                (cx - w / 2.0, y + 1.0),
                (cx + w / 2.0, y + 1.0),
                (cx + wn / 2.0, y + h - 1.0),
                (cx - wn / 2.0, y + h - 1.0),
            ];
            ctx.polygon(&pts, Some(c), None);
            let pct = if first > 0.0 {
                format!(" ({:.0}%)", v / first * 100.0)
            } else {
                String::new()
            };
            let label = format!(
                "{}: {}{pct}",
                self.labels.get(i).cloned().unwrap_or_default(),
                short(*v)
            );
            if ctx.text_width(&label, size) < w.min(wn).max(wn) - 10.0 {
                ctx.push(text_node(
                    cx,
                    y + h / 2.0,
                    label,
                    size,
                    c.contrasting_text(),
                    Anchor::Middle,
                    false,
                ));
            } else {
                ctx.push(text_node(
                    cx + w.max(wn) / 2.0 + 8.0,
                    y + h / 2.0,
                    label,
                    size,
                    ctx.theme.text,
                    Anchor::Start,
                    false,
                ));
            }
        }
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ---------------------------------------------------------------- gauge ----

/// Gauge (speedometer) showing one value. Created by [`Axes::gauge`].
#[derive(Clone, Debug)]
pub struct Gauge {
    pub(crate) value: f64,
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) bands: Vec<(f64, Color)>,
    pub(crate) label: Option<String>,
    pub(crate) color: Color,
}

impl Gauge {
    /// Colored ranges: each `(upper bound, color)` from the minimum up.
    pub fn bands<C: Into<Color>>(
        &mut self,
        bands: impl IntoIterator<Item = (f64, C)>,
    ) -> &mut Self {
        self.bands = bands.into_iter().map(|(v, c)| (v, c.into())).collect();
        self
    }
    /// Caption under the value.
    pub fn label(&mut self, l: impl Into<String>) -> &mut Self {
        self.label = Some(l.into());
        self
    }
}

impl Series for Gauge {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let r = ctx.rect;
        let radius = (r.w / 2.0).min(r.h * 0.8) * 0.9;
        let c = (r.x + r.w / 2.0, r.y + r.h / 2.0 + radius * 0.35);
        let ang = |v: f64| {
            let t = if self.max > self.min {
                ((v - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
            } else {
                0.5
            };
            PI - t * PI
        };
        let inner = radius * 0.72;
        ctx.push(Node::Path {
            cmds: sector(c, inner, radius, PI, 0.0),
            fill: Some(ctx.theme.grid),
            stroke: None,
        });
        if self.bands.is_empty() {
            ctx.push(Node::Path {
                cmds: sector(c, inner, radius, PI, ang(self.value)),
                fill: Some(self.color),
                stroke: None,
            });
        } else {
            let mut lo = self.min;
            for (hi, col) in &self.bands {
                ctx.push(Node::Path {
                    cmds: sector(c, inner, radius, ang(lo), ang(*hi)),
                    fill: Some(*col),
                    stroke: None,
                });
                lo = *hi;
            }
            // Needle.
            let a = ang(self.value);
            let tip = (c.0 + radius * 0.95 * a.cos(), c.1 - radius * 0.95 * a.sin());
            let side = (6.0 * (a + FRAC_PI_2).cos(), -6.0 * (a + FRAC_PI_2).sin());
            ctx.polygon(
                &[
                    tip,
                    (c.0 + side.0, c.1 + side.1),
                    (c.0 - side.0, c.1 - side.1),
                ],
                Some(ctx.theme.text),
                None,
            );
            ctx.push(Node::Circle {
                cx: c.0,
                cy: c.1,
                r: 8.0,
                fill: Some(ctx.theme.text),
                stroke: None,
            });
        }
        let ts = ctx.theme.tick_size;
        for (v, a) in [(self.min, Anchor::Middle), (self.max, Anchor::Middle)] {
            let an = ang(v);
            let p = (
                c.0 + (radius + ts) * an.cos(),
                c.1 - (radius + ts) * an.sin() + ts,
            );
            ctx.push(text_node(p.0, p.1, short(v), ts, ctx.theme.text, a, false));
        }
        let vs = (radius * 0.28).clamp(12.0, 64.0);
        let ty = if self.bands.is_empty() {
            c.1 - vs * 0.3
        } else {
            c.1 + vs * 0.9
        };
        ctx.push(text_node(
            c.0,
            ty,
            short(self.value),
            vs,
            ctx.theme.text,
            Anchor::Middle,
            true,
        ));
        if let Some(l) = &self.label {
            ctx.push(text_node(
                c.0,
                ty + vs * 0.75,
                l.clone(),
                ctx.theme.label_size,
                ctx.theme.text.with_alpha(0.8),
                Anchor::Middle,
                false,
            ));
        }
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// -------------------------------------------------------------- network ----

/// Network (graph) diagram with force-directed layout. Created by [`Axes::network`].
#[derive(Clone, Debug)]
pub struct Network {
    pub(crate) nodes: Vec<String>,
    pub(crate) edges: Vec<(usize, usize, f64)>,
    pub(crate) groups: Option<Vec<usize>>,
    pub(crate) colors: Vec<Color>,
    pub(crate) iterations: usize,
    pub(crate) node_size: f64,
}

impl Network {
    /// Group index per node (colors nodes).
    pub fn groups(&mut self, g: impl IntoIterator<Item = usize>) -> &mut Self {
        self.groups = Some(g.into_iter().collect());
        self
    }
    /// Layout iterations (default 300).
    pub fn iterations(&mut self, n: usize) -> &mut Self {
        self.iterations = n;
        self
    }
    /// Node diameter in pixels (scaled up by degree).
    pub fn node_size(&mut self, s: f64) -> &mut Self {
        self.node_size = s.max(1.0);
        self
    }

    /// Fruchterman–Reingold layout in the unit square (deterministic).
    pub fn positions(&self) -> Vec<(f64, f64)> {
        let n = self.nodes.len();
        if n == 0 {
            return Vec::new();
        }
        let mut pos: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let a = i as f64 * 2.399963; // golden angle spiral
                let r = ((i as f64 + 0.5) / n as f64).sqrt() * 0.5;
                (0.5 + r * a.cos(), 0.5 + r * a.sin())
            })
            .collect();
        let k = (1.0 / n as f64).sqrt();
        let mut t = 0.1;
        for _ in 0..self.iterations {
            let mut disp = vec![(0.0, 0.0); n];
            for i in 0..n {
                for j in i + 1..n {
                    let (dx, dy) = (pos[i].0 - pos[j].0, pos[i].1 - pos[j].1);
                    let d = dx.hypot(dy).max(1e-4);
                    let f = k * k / d;
                    disp[i].0 += dx / d * f;
                    disp[i].1 += dy / d * f;
                    disp[j].0 -= dx / d * f;
                    disp[j].1 -= dy / d * f;
                }
            }
            for &(a, b, w) in &self.edges {
                if a >= n || b >= n || a == b {
                    continue;
                }
                let (dx, dy) = (pos[a].0 - pos[b].0, pos[a].1 - pos[b].1);
                let d = dx.hypot(dy).max(1e-4);
                let f = d * d / k * w.clamp(0.1, 10.0).sqrt();
                disp[a].0 -= dx / d * f;
                disp[a].1 -= dy / d * f;
                disp[b].0 += dx / d * f;
                disp[b].1 += dy / d * f;
            }
            for i in 0..n {
                // Gentle gravity keeps components together.
                disp[i].0 += (0.5 - pos[i].0) * 0.05;
                disp[i].1 += (0.5 - pos[i].1) * 0.05;
                let d = disp[i].0.hypot(disp[i].1).max(1e-9);
                pos[i].0 += disp[i].0 / d * d.min(t);
                pos[i].1 += disp[i].1 / d * d.min(t);
            }
            t *= 0.985;
        }
        // Normalize to the unit square.
        let (mut x0, mut x1, mut y0, mut y1) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for p in &pos {
            x0 = x0.min(p.0);
            x1 = x1.max(p.0);
            y0 = y0.min(p.1);
            y1 = y1.max(p.1);
        }
        let s = (x1 - x0).max(y1 - y0).max(1e-9);
        pos.iter()
            .map(|p| {
                (
                    (p.0 - x0) / s + (1.0 - (x1 - x0) / s) / 2.0,
                    (p.1 - y0) / s + (1.0 - (y1 - y0) / s) / 2.0,
                )
            })
            .collect()
    }

    fn canvas_positions(&self, r: Rect) -> Vec<(f64, f64)> {
        let pad = 30.0;
        let side = (r.w.min(r.h) - 2.0 * pad).max(10.0);
        let (ox, oy) = (r.x + (r.w - side) / 2.0, r.y + (r.h - side) / 2.0);
        self.positions()
            .into_iter()
            .map(|(x, y)| (ox + x * side, oy + y * side))
            .collect()
    }
}

impl Series for Network {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let pos = self.canvas_positions(ctx.rect);
        let maxw = self.edges.iter().map(|e| e.2).fold(0.0, f64::max).max(1e-9);
        for &(a, b, w) in &self.edges {
            if a < pos.len() && b < pos.len() {
                ctx.polyline(
                    &[pos[a], pos[b]],
                    Stroke::new(ctx.theme.axis.with_alpha(0.35), 0.8 + 2.5 * w / maxw),
                );
            }
        }
        let mut degree = vec![0usize; pos.len()];
        for &(a, b, _) in &self.edges {
            if a < pos.len() && b < pos.len() {
                degree[a] += 1;
                degree[b] += 1;
            }
        }
        let size = ctx.theme.annotation_size;
        for (i, p) in pos.iter().enumerate() {
            let g = self
                .groups
                .as_ref()
                .and_then(|g| g.get(i).copied())
                .unwrap_or(0);
            let c = self.colors[g % self.colors.len().max(1)];
            let r = self.node_size / 2.0 + (degree[i] as f64).sqrt() * 2.0;
            ctx.push(Node::Group {
                children: vec![Node::Circle {
                    cx: p.0,
                    cy: p.1,
                    r,
                    fill: Some(c),
                    stroke: Some(Stroke::new(ctx.theme.background, 1.5)),
                }],
                clip: None,
                opacity: 1.0,
                tooltip: Some(format!("{} (degree {})", self.nodes[i], degree[i])),
            });
            ctx.push(text_node(
                p.0,
                p.1 - r - size * 0.6,
                self.nodes[i].clone(),
                size,
                ctx.theme.text,
                Anchor::Middle,
                false,
            ));
        }
    }
    fn hit(&self, _x: &AxisMap, _y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        self.canvas_positions(rect)
            .into_iter()
            .enumerate()
            .map(|(i, p)| Hit {
                px: p.0,
                py: p.1,
                label: self.nodes[i].clone(),
                distance: (p.0 - px).hypot(p.1 - py),
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ------------------------------------------------- parallel coordinates ----

/// Parallel coordinates: one polyline per row across normalized dimensions.
/// Created by [`Axes::parallel_coordinates`].
#[derive(Clone, Debug)]
pub struct Parallel {
    pub(crate) dims: Vec<String>,
    pub(crate) rows: Vec<Vec<f64>>,
    pub(crate) classes: Option<Vec<usize>>,
    pub(crate) class_names: Vec<String>,
    pub(crate) colors: Vec<Color>,
    pub(crate) opacity: f32,
}

impl Parallel {
    /// Class index per row (colors the lines; names for the legend).
    pub fn classes(
        &mut self,
        classes: impl IntoIterator<Item = usize>,
        names: impl IntoLabels,
    ) -> &mut Self {
        self.classes = Some(classes.into_iter().collect());
        self.class_names = names.into_labels();
        self
    }
    /// Line opacity.
    pub fn opacity(&mut self, o: f32) -> &mut Self {
        self.opacity = o.clamp(0.0, 1.0);
        self
    }
    fn ranges(&self) -> Vec<(f64, f64)> {
        (0..self.dims.len())
            .map(|d| {
                let (a, b) = self
                    .rows
                    .iter()
                    .filter_map(|r| r.get(d).copied())
                    .filter(|v| v.is_finite())
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                        (a.min(v), b.max(v))
                    });
                if a.is_finite() {
                    (a, if b > a { b } else { a + 1.0 })
                } else {
                    (0.0, 1.0)
                }
            })
            .collect()
    }
}

impl Series for Parallel {
    fn extent(&self) -> Option<Extent> {
        let mut e = Extent::point(0.0, 0.0).include((self.dims.len().max(1) - 1) as f64, 1.0);
        e.sticky_y_min = true;
        e.sticky_y_max = true;
        Some(e)
    }
    fn draw(&self, ctx: &mut Context) {
        let ranges = self.ranges();
        for (d, _) in self.dims.iter().enumerate() {
            let x = ctx.x.px(d as f64);
            ctx.polyline(
                &[(x, ctx.rect.y), (x, ctx.rect.bottom())],
                Stroke::new(ctx.theme.axis, 1.0),
            );
            let (lo, hi) = ranges[d];
            let ts = ctx.theme.tick_size * 0.9;
            ctx.text(
                x,
                ctx.rect.y - 2.0,
                short(hi),
                ts,
                ctx.theme.text,
                Anchor::Middle,
                Baseline::Bottom,
            );
            ctx.text(
                x,
                ctx.rect.bottom() + 2.0 + ts * 1.4,
                short(lo),
                ts,
                ctx.theme.text,
                Anchor::Middle,
                Baseline::Top,
            );
        }
        for (i, row) in self.rows.iter().enumerate() {
            let class = self
                .classes
                .as_ref()
                .and_then(|c| c.get(i).copied())
                .unwrap_or(0);
            let c = self.colors[class % self.colors.len().max(1)].with_alpha(self.opacity);
            let pts: Vec<(f64, f64)> = row
                .iter()
                .enumerate()
                .take(self.dims.len())
                .map(|(d, v)| {
                    let (lo, hi) = ranges[d];
                    ctx.point(d as f64, (v - lo) / (hi - lo))
                })
                .collect();
            ctx.polyline(&pts, Stroke::new(c, 1.2));
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        self.class_names
            .iter()
            .enumerate()
            .map(|(i, n)| LegendEntry {
                label: n.clone(),
                symbol: LegendSymbol::Line {
                    color: self.colors[i % self.colors.len().max(1)],
                    width: 2.0,
                    dash: crate::plot::Dash::Solid,
                    marker: None,
                },
            })
            .collect()
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ---------------------------------------------------------------- axes ----

impl Axes {
    fn frameless(&mut self) {
        self.kind = AxesKind::Frameless;
    }

    fn palette_colors(&mut self, n: usize) -> Vec<Color> {
        (0..n.max(1)).map(|_| self.next_color()).collect()
    }

    /// Pie chart (use [`Pie::donut`] for a donut).
    pub fn pie(&mut self, labels: impl IntoLabels, values: impl IntoValues) -> &mut Pie {
        self.frameless();
        let labels = labels.into_labels();
        let colors = self.palette_colors(labels.len());
        let values = values.into_values();
        self.add(Pie {
            labels,
            values,
            colors,
            hole: 0.0,
            start_deg: 90.0,
            explode: Vec::new(),
            show_percent: true,
            show_labels: false,
            center_text: None,
        })
    }

    /// Treemap of hierarchical data (or a flat list of leaves).
    pub fn treemap(&mut self, root: Vec<TreeNode>) -> &mut Treemap {
        self.frameless();
        let colors = self.palette_colors(root.len());
        self.add(Treemap {
            root,
            colors,
            padding: 3.0,
        })
    }

    /// Sunburst of hierarchical data.
    pub fn sunburst(&mut self, root: Vec<TreeNode>) -> &mut Sunburst {
        self.frameless();
        let colors = self.palette_colors(root.len());
        self.add(Sunburst {
            root,
            colors,
            hole: 0.2,
        })
    }

    /// Sankey diagram from `(source, target, value)` links.
    pub fn sankey<A: ToString, B: ToString>(
        &mut self,
        links: impl IntoIterator<Item = (A, B, f64)>,
    ) -> &mut Sankey {
        self.frameless();
        let links: Vec<(String, String, f64)> = links
            .into_iter()
            .map(|(a, b, v)| (a.to_string(), b.to_string(), v))
            .collect();
        let colors = self.palette_colors(10);
        self.legend = crate::plot::LegendPosition::Hidden;
        self.add(Sankey {
            links,
            colors,
            node_width: 14.0,
            gap: 10.0,
        })
    }

    /// Funnel chart (stages from top to bottom).
    pub fn funnel(&mut self, stages: impl IntoLabels, values: impl IntoValues) -> &mut Funnel {
        self.frameless();
        let labels = stages.into_labels();
        let colors = self.palette_colors(labels.len());
        self.add(Funnel {
            labels,
            values: values.into_values(),
            colors,
        })
    }

    /// Gauge showing `value` on `min..max`.
    pub fn gauge(&mut self, value: f64, min: f64, max: f64) -> &mut Gauge {
        self.frameless();
        let color = self.next_color();
        self.add(Gauge {
            value,
            min,
            max,
            bands: Vec::new(),
            label: None,
            color,
        })
    }

    /// Network diagram: node labels and `(from, to, weight)` edges (indices).
    pub fn network(
        &mut self,
        nodes: impl IntoLabels,
        edges: impl IntoIterator<Item = (usize, usize, f64)>,
    ) -> &mut Network {
        self.frameless();
        let colors = self.palette_colors(10);
        self.add(Network {
            nodes: nodes.into_labels(),
            edges: edges.into_iter().collect(),
            groups: None,
            colors,
            iterations: 300,
            node_size: 10.0,
        })
    }

    /// Parallel coordinates plot: `rows[i][d]` values over `dimensions`.
    pub fn parallel_coordinates(
        &mut self,
        dimensions: impl IntoLabels,
        rows: Vec<Vec<f64>>,
    ) -> &mut Parallel {
        let dims = dimensions.into_labels();
        self.x.categories(dims.clone());
        self.y.hidden();
        self.y.limits(0.0, 1.0);
        self.y.grid = Some(false);
        self.x.grid = Some(false);
        let colors = self.palette_colors(10);
        self.add(Parallel {
            dims,
            rows,
            classes: None,
            class_names: Vec::new(),
            colors,
            opacity: 0.6,
        })
    }

    /// Calendar heatmap: one cell per day (weeks × weekdays), colored by value.
    pub fn calendar(
        &mut self,
        dates: impl IntoValues,
        values: impl IntoValues,
    ) -> &mut super::grid::Heatmap {
        let dates = dates.into_values();
        let values = values.into_values();
        let days: Vec<i64> = dates.iter().map(|t| (t / 86400.0).floor() as i64).collect();
        let (d0, d1) = days
            .iter()
            .fold((i64::MAX, i64::MIN), |(a, b), d| (a.min(*d), b.max(*d)));
        let (d0, d1) = if d0 > d1 { (0, 0) } else { (d0, d1) };
        // Weeks start on Monday; 1970-01-01 was a Thursday.
        let weekday = |d: i64| (d + 3).rem_euclid(7);
        let first_monday = d0 - weekday(d0);
        let weeks = ((d1 - first_monday) / 7 + 1) as usize;
        let mut z = vec![vec![f64::NAN; weeks]; 7];
        for (d, v) in days.iter().zip(&values) {
            let w = ((d - first_monday) / 7) as usize;
            let row = weekday(*d) as usize;
            let cell = &mut z[row][w];
            *cell = if cell.is_nan() { *v } else { *cell + v };
        }
        let mut month_ticks = Vec::new();
        let mut last_month = 0;
        for w in 0..weeks {
            let t = (first_monday + 7 * w as i64) as f64 * 86400.0;
            let m = time::breakdown(t + 6.0 * 86400.0).month;
            if m != last_month {
                month_ticks.push((w as f64, time::format_time(t + 6.0 * 86400.0, "%b")));
                last_month = m;
            }
        }
        let xl: Vec<String> = (0..weeks).map(|w| w.to_string()).collect();
        let yl = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        let h = self.heatmap_labeled(xl, yl, z);
        h.gaps = 2.0;
        h.colormap = Colormap::Greens;
        self.x.ticks = TickSpec::Labeled(month_ticks);
        self.x.tick_rotation = Some(0.0);
        self.equal_aspect = true;
        self.last_n::<super::grid::Heatmap>(1).pop().expect("added")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squarify_fills_the_rect() {
        let r = Rect {
            x: 0.0,
            y: 0.0,
            w: 600.0,
            h: 400.0,
        };
        let cells = squarify(&[6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0], r);
        let area: f64 = cells.iter().map(|c| c.w * c.h).sum();
        assert!((area - 240_000.0).abs() < 1.0);
        for c in &cells {
            assert!(
                c.x >= -1e-9
                    && c.y >= -1e-9
                    && c.x + c.w <= 600.0 + 1e-6
                    && c.y + c.h <= 400.0 + 1e-6
            );
        }
    }

    #[test]
    fn tree_totals() {
        let t = TreeNode::group(
            "a",
            vec![
                TreeNode::leaf("b", 2.0),
                TreeNode::group(
                    "c",
                    vec![TreeNode::leaf("d", 3.0), TreeNode::leaf("e", -1.0)],
                ),
            ],
        );
        assert_eq!(t.total(), 5.0);
    }

    #[test]
    fn network_layout_is_deterministic_and_bounded() {
        let n = Network {
            nodes: (0..6).map(|i| i.to_string()).collect(),
            edges: vec![(0, 1, 1.0), (1, 2, 1.0), (3, 4, 1.0)],
            groups: None,
            colors: vec![Color::RED],
            iterations: 100,
            node_size: 10.0,
        };
        let a = n.positions();
        assert_eq!(a, n.positions());
        assert!(a
            .iter()
            .all(|p| (0.0..=1.0 + 1e-9).contains(&p.0) && (0.0..=1.0 + 1e-9).contains(&p.1)));
    }
}
