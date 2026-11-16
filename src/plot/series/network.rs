//! Network diagrams with fixed layouts: arc diagrams (nodes on a line,
//! edges as arcs) and hierarchical edge bundling (nodes on a circle, edges
//! bent through their groups so that related connections run together).

use super::{Context, Extent, Hit, IntoLabels, LegendEntry, LegendSymbol, Rect, Series};
use crate::plot::axes::{Axes, AxesKind};
use crate::plot::axis::AxisMap;
use crate::plot::color::Color;
use crate::plot::scene::{Anchor, Baseline, Node, PathCmd, Stroke, TextNode};
use std::any::Any;
use std::f64::consts::{FRAC_PI_2, TAU};

/// Group of node `i` (0 without groups).
fn group(groups: &[usize], i: usize) -> usize {
    groups.get(i).copied().unwrap_or(0)
}

fn degrees(n: usize, edges: &[(usize, usize, f64)]) -> Vec<f64> {
    let mut d = vec![0.0; n];
    for &(a, b, w) in edges {
        if a < n && b < n {
            let w = if w.is_finite() { w.abs() } else { 0.0 };
            d[a] += w;
            d[b] += w;
        }
    }
    d
}

fn legend_of(names: &[String], colors: &[Color]) -> Vec<LegendEntry> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| LegendEntry {
            label: n.clone(),
            symbol: LegendSymbol::Patch {
                fill: colors[i % colors.len().max(1)],
                edge: None,
            },
        })
        .collect()
}

/// Arc diagram (see [`Axes::arc_diagram`]).
#[derive(Clone, Debug)]
pub struct ArcDiagram {
    nodes: Vec<String>,
    edges: Vec<(usize, usize, f64)>,
    groups: Vec<usize>,
    group_names: Vec<String>,
    colors: Vec<Color>,
    sort: bool,
    opacity: f32,
}

impl ArcDiagram {
    /// Group of each node: colors nodes and their arcs; with
    /// [`ArcDiagram::sort_by_group`] groups sit together.
    pub fn groups(&mut self, g: impl IntoIterator<Item = usize>) -> &mut Self {
        self.groups = g.into_iter().collect();
        self
    }
    /// Names of the groups for the legend.
    pub fn group_names(&mut self, names: impl IntoLabels) -> &mut Self {
        self.group_names = names.into_labels();
        self
    }
    /// Orders the nodes by group (default true).
    pub fn sort_by_group(&mut self, on: bool) -> &mut Self {
        self.sort = on;
        self
    }
    /// Arc opacity (default 0.5).
    pub fn opacity(&mut self, a: f32) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }

    /// Order of the nodes along the line.
    fn order(&self) -> Vec<usize> {
        let mut o: Vec<usize> = (0..self.nodes.len()).collect();
        if self.sort && !self.groups.is_empty() {
            o.sort_by_key(|&i| group(&self.groups, i));
        }
        o
    }

    /// Canvas x of each node, the baseline y and the node radii.
    fn layout(&self, rect: Rect, label_room: f64) -> (Vec<f64>, f64, Vec<f64>) {
        let n = self.nodes.len().max(1);
        let pad = rect.w * 0.04;
        let step = (rect.w - 2.0 * pad) / (n.saturating_sub(1).max(1)) as f64;
        let mut x = vec![0.0; self.nodes.len()];
        for (k, &i) in self.order().iter().enumerate() {
            x[i] = rect.x
                + pad
                + if n == 1 {
                    (rect.w - 2.0 * pad) / 2.0
                } else {
                    k as f64 * step
                };
        }
        let deg = degrees(self.nodes.len(), &self.edges);
        let max = deg.iter().copied().fold(0.0, f64::max).max(1e-9);
        let rmax = (step * 0.42).clamp(2.0, 9.0);
        let r = deg
            .iter()
            .map(|d| 2.0 + (rmax - 2.0) * (d / max).sqrt())
            .collect();
        // Centred on the height of the widest arc (semicircles).
        let n = self.nodes.len();
        let top = self
            .edges
            .iter()
            .filter(|e| e.0 < n && e.1 < n)
            .map(|e| (x[e.0] - x[e.1]).abs() / 2.0)
            .fold(0.0, f64::max);
        let free = rect.h - label_room - 8.0;
        let base = rect.y + 4.0 + top.min(free) + (free - top.min(free)) / 2.0;
        (x, base, r)
    }
}

impl Series for ArcDiagram {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }

    fn draw(&self, ctx: &mut Context) {
        let n = self.nodes.len();
        if n == 0 {
            return;
        }
        let size = ctx.theme.annotation_size;
        // Labels below the line, turned when they would collide.
        let pitch = ctx.rect.w * 0.92 / n.max(1) as f64;
        let widest = self
            .nodes
            .iter()
            .map(|s| ctx.text_width(s, size))
            .fold(0.0, f64::max);
        let turned = widest > pitch * 0.9;
        let room = if turned {
            widest.min(ctx.rect.h * 0.3) + 10.0
        } else {
            size * 2.0
        };
        let (x, base, r) = self.layout(ctx.rect, room);
        let maxw = self
            .edges
            .iter()
            .map(|e| e.2.abs())
            .fold(0.0, f64::max)
            .max(1e-9);
        // Widest arcs first: small ones stay visible on top.
        let mut edges: Vec<_> = self.edges.iter().filter(|e| e.0 < n && e.1 < n).collect();
        edges.sort_by(|a, b| (x[b.0] - x[b.1]).abs().total_cmp(&(x[a.0] - x[a.1]).abs()));
        let height = base - ctx.rect.y - 4.0;
        let span = ctx.rect.w.max(1.0).min(2.0 * height);
        for &&(a, b, w) in &edges {
            let (x0, x1) = (x[a].min(x[b]), x[a].max(x[b]));
            let rx = (x1 - x0) / 2.0;
            // Semicircles, flattened when the widest would not fit.
            let ry = rx.min(height * (x1 - x0) / span * 1.0).min(rx);
            let c = self.colors[group(&self.groups, a) % self.colors.len().max(1)];
            let mut cmds = Vec::with_capacity(34);
            for k in 0..=32 {
                let t = std::f64::consts::PI * k as f64 / 32.0;
                let p = (x0 + rx - rx * t.cos(), base - ry * t.sin());
                cmds.push(if k == 0 {
                    PathCmd::MoveTo(p.0, p.1)
                } else {
                    PathCmd::LineTo(p.0, p.1)
                });
            }
            // Dense diagrams: thin lines (the fast anti-aliased path).
            let width = if edges.len() > 400 {
                0.5 + 0.75 * (w.abs() / maxw).sqrt()
            } else {
                0.8 + 3.2 * (w.abs() / maxw).sqrt()
            };
            ctx.push(Node::Path {
                cmds,
                fill: None,
                stroke: Some(Stroke::new(c.with_alpha(self.opacity), width)),
            });
        }
        ctx.polyline(
            &[(ctx.rect.x, base), (ctx.rect.x + ctx.rect.w, base)],
            Stroke::new(ctx.theme.axis.with_alpha(0.25), 0.8),
        );
        for i in 0..n {
            let c = self.colors[group(&self.groups, i) % self.colors.len().max(1)];
            ctx.push(Node::Circle {
                cx: x[i],
                cy: base,
                r: r[i],
                fill: Some(c),
                stroke: Some(Stroke::new(ctx.theme.background, 1.2)),
            });
            ctx.push(Node::Text(TextNode {
                x: x[i],
                y: base + r[i] + if turned { 4.0 } else { size * 0.9 },
                text: self.nodes[i].clone(),
                size,
                color: ctx.theme.text,
                anchor: if turned {
                    Anchor::Start
                } else {
                    Anchor::Middle
                },
                baseline: Baseline::Middle,
                rotate: if turned { 90.0 } else { 0.0 },
                bold: false,
                italic: false,
            }));
        }
    }

    fn legend(&self) -> Vec<LegendEntry> {
        legend_of(&self.group_names, &self.colors)
    }

    fn hit(&self, _x: &AxisMap, _y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let (x, base, _) = self.layout(rect, 0.0);
        let deg = degrees(self.nodes.len(), &self.edges);
        x.iter()
            .enumerate()
            .map(|(i, &nx)| Hit {
                px: nx,
                py: base,
                label: format!("{} (weight {})", self.nodes[i], super::short(deg[i])),
                distance: (nx - px).abs() + (py - base).abs().min(30.0) * 0.1,
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn kind(&self) -> &'static str {
        "arc diagram"
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Hierarchical edge bundling (see [`Axes::edge_bundling`]).
#[derive(Clone, Debug)]
pub struct EdgeBundling {
    nodes: Vec<String>,
    groups: Vec<usize>,
    group_names: Vec<String>,
    edges: Vec<(usize, usize, f64)>,
    colors: Vec<Color>,
    tension: f64,
    opacity: f32,
}

impl EdgeBundling {
    /// Names of the groups (labels on the outer ring and the legend).
    pub fn group_names(&mut self, names: impl IntoLabels) -> &mut Self {
        self.group_names = names.into_labels();
        self
    }
    /// How strongly edges follow the hierarchy: 0 straight lines … 1 fully
    /// bundled (default 0.85).
    pub fn tension(&mut self, t: f64) -> &mut Self {
        self.tension = t.clamp(0.0, 1.0);
        self
    }
    /// Edge opacity (default 0.6).
    pub fn opacity(&mut self, a: f32) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }

    /// Angle of each node (grouped, a gap between groups) and of each group's
    /// middle.
    fn angles(&self) -> (Vec<f64>, Vec<(usize, f64, f64)>) {
        let n = self.nodes.len();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&i| group(&self.groups, i));
        let ngroups = order
            .windows(2)
            .filter(|w| group(&self.groups, w[0]) != group(&self.groups, w[1]))
            .count()
            + 1;
        let gap = 1.5;
        let slots = n as f64 + gap * ngroups as f64;
        let mut a = vec![0.0; n];
        let mut spans: Vec<(usize, f64, f64)> = Vec::new();
        let mut pos = gap / 2.0;
        let mut prev = None;
        for &i in &order {
            let g = group(&self.groups, i);
            if prev.is_some_and(|p| p != g) {
                pos += gap;
            }
            prev = Some(g);
            a[i] = pos / slots * TAU - FRAC_PI_2;
            match spans.last_mut() {
                Some(s) if s.0 == g => s.2 = a[i],
                _ => spans.push((g, a[i], a[i])),
            }
            pos += 1.0;
        }
        (a, spans)
    }

    fn geometry(&self, rect: Rect, label_room: f64) -> ((f64, f64), f64) {
        let c = rect.center();
        let r = (rect.w.min(rect.h) / 2.0 - label_room).max(10.0);
        (c, r)
    }
}

/// Points of a uniform cubic B-spline through control points `p` (clamped:
/// it starts and ends at the first and last point).
fn bspline(p: &[(f64, f64)], per: usize) -> Vec<(f64, f64)> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let mut q = Vec::with_capacity(p.len() + 4);
    q.push(p[0]);
    q.push(p[0]);
    q.extend_from_slice(p);
    q.push(p[p.len() - 1]);
    q.push(p[p.len() - 1]);
    let mut out = Vec::with_capacity((q.len() - 3) * per + 1);
    for i in 0..q.len() - 3 {
        let (a, b, c, d) = (q[i], q[i + 1], q[i + 2], q[i + 3]);
        for k in 0..per {
            let t = k as f64 / per as f64;
            let (t2, t3) = (t * t, t * t * t);
            let w0 = (1.0 - t).powi(3) / 6.0;
            let w1 = (3.0 * t3 - 6.0 * t2 + 4.0) / 6.0;
            let w2 = (-3.0 * t3 + 3.0 * t2 + 3.0 * t + 1.0) / 6.0;
            let w3 = t3 / 6.0;
            out.push((
                w0 * a.0 + w1 * b.0 + w2 * c.0 + w3 * d.0,
                w0 * a.1 + w1 * b.1 + w2 * c.1 + w3 * d.1,
            ));
        }
    }
    out.push(p[p.len() - 1]);
    out
}

impl Series for EdgeBundling {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }

    fn draw(&self, ctx: &mut Context) {
        let n = self.nodes.len();
        if n == 0 {
            return;
        }
        let size = ctx.theme.annotation_size;
        let widest = self
            .nodes
            .iter()
            .map(|s| ctx.text_width(s, size))
            .fold(0.0, f64::max);
        let room = widest
            + 14.0
            + if self.group_names.is_empty() {
                0.0
            } else {
                size * 2.2
            };
        let (c, r) = self.geometry(ctx.rect, room.min(ctx.rect.w.min(ctx.rect.h) * 0.3));
        let (ang, spans) = self.angles();
        let at = |t: f64, rr: f64| (c.0 + rr * t.cos(), c.1 + rr * t.sin());
        // Group hubs halfway in, at the middle of their arc.
        let hub = |g: usize| {
            spans
                .iter()
                .find(|s| s.0 == g)
                .map(|s| at((s.1 + s.2) / 2.0, r * 0.5))
                .unwrap_or(c)
        };
        let maxw = self
            .edges
            .iter()
            .map(|e| e.2.abs())
            .fold(0.0, f64::max)
            .max(1e-9);
        let beta = self.tension;
        for &(a, b, w) in self
            .edges
            .iter()
            .filter(|e| e.0 < n && e.1 < n && e.0 != e.1)
        {
            let (ga, gb) = (group(&self.groups, a), group(&self.groups, b));
            let (pa, pb) = (at(ang[a], r), at(ang[b], r));
            let mut ctrl = vec![pa, hub(ga)];
            if ga != gb {
                ctrl.push(c);
                ctrl.push(hub(gb));
            }
            ctrl.push(pb);
            // Straightened towards the chord by 1 − β (Holten 2006).
            let m = ctrl.len() - 1;
            for (k, p) in ctrl.iter_mut().enumerate() {
                let t = k as f64 / m as f64;
                let s = (pa.0 + (pb.0 - pa.0) * t, pa.1 + (pb.1 - pa.1) * t);
                *p = (
                    beta * p.0 + (1.0 - beta) * s.0,
                    beta * p.1 + (1.0 - beta) * s.1,
                );
            }
            // About one point per 8 px of curve.
            let len: f64 = ctrl
                .windows(2)
                .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
                .sum();
            let per = ((len / 8.0) as usize / m.max(1)).clamp(2, 10);
            let pts = bspline(&ctrl, per);
            let col = self.colors[ga % self.colors.len().max(1)];
            // Thin lines (at most 1.25 px: the fast anti-aliased path), as is
            // usual for bundles: density shows through the overlap.
            let width = 0.5 + 0.75 * (w.abs() / maxw).sqrt();
            let st = Stroke::new(col.with_alpha(self.opacity), width);
            ctx.polyline(&pts, st);
        }
        for i in 0..n {
            let g = group(&self.groups, i);
            let col = self.colors[g % self.colors.len().max(1)];
            let (x, y) = at(ang[i], r);
            ctx.push(Node::Circle {
                cx: x,
                cy: y,
                r: 2.6,
                fill: Some(col),
                stroke: None,
            });
            let t = ang[i];
            let left = t.cos() < 0.0;
            let (lx, ly) = at(t, r + 6.0);
            ctx.push(Node::Text(TextNode {
                x: lx,
                y: ly,
                text: self.nodes[i].clone(),
                size,
                color: ctx.theme.text,
                anchor: if left { Anchor::End } else { Anchor::Start },
                baseline: Baseline::Middle,
                rotate: t.to_degrees() + if left { 180.0 } else { 0.0 },
                bold: false,
                italic: false,
            }));
        }
        // Group arcs outside the labels.
        if !self.group_names.is_empty() {
            let ro = r + widest + 12.0;
            for &(g, a0, a1) in &spans {
                let col = self.colors[g % self.colors.len().max(1)];
                let (a0, a1) = (a0 - 0.02, a1 + 0.02);
                let k = ((a1 - a0) * ro / 4.0).ceil().max(2.0) as usize;
                let arc: Vec<(f64, f64)> = (0..=k)
                    .map(|j| at(a0 + (a1 - a0) * j as f64 / k as f64, ro))
                    .collect();
                ctx.polyline(&arc, Stroke::new(col, 3.0));
            }
        }
    }

    fn legend(&self) -> Vec<LegendEntry> {
        legend_of(&self.group_names, &self.colors)
    }

    fn hit(&self, _x: &AxisMap, _y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let (c, r) = self.geometry(rect, 0.0);
        let (ang, _) = self.angles();
        let deg = degrees(self.nodes.len(), &self.edges);
        let t = (py - c.1).atan2(px - c.0);
        ang.iter()
            .enumerate()
            .map(|(i, &a)| {
                let d = (a - t).rem_euclid(TAU);
                Hit {
                    px: c.0 + r * a.cos(),
                    py: c.1 + r * a.sin(),
                    label: format!("{} (weight {})", self.nodes[i], super::short(deg[i])),
                    distance: d.min(TAU - d) * r,
                }
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn kind(&self) -> &'static str {
        "hierarchical edge bundling"
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// Arc diagram: nodes on a line, `(from, to, weight)` edges (node
    /// indices) as arcs above it — sequences, co-occurrence, dependencies.
    pub fn arc_diagram(
        &mut self,
        nodes: impl IntoLabels,
        edges: impl IntoIterator<Item = (usize, usize, f64)>,
    ) -> &mut ArcDiagram {
        self.kind = AxesKind::Frameless;
        let colors = (0..10).map(|_| self.next_color()).collect();
        self.add(ArcDiagram {
            nodes: nodes.into_labels(),
            edges: edges.into_iter().collect(),
            groups: Vec::new(),
            group_names: Vec::new(),
            colors,
            sort: true,
            opacity: 0.5,
        })
    }

    /// Hierarchical edge bundling: nodes on a circle by `groups` (one group
    /// index per node), `(from, to, weight)` edges bent through their
    /// groups so that related connections run together (Holten 2006).
    pub fn edge_bundling(
        &mut self,
        nodes: impl IntoLabels,
        groups: impl IntoIterator<Item = usize>,
        edges: impl IntoIterator<Item = (usize, usize, f64)>,
    ) -> &mut EdgeBundling {
        self.kind = AxesKind::Frameless;
        self.equal_aspect = true;
        let groups: Vec<usize> = groups.into_iter().collect();
        let ng = groups.iter().copied().max().map_or(1, |m| m + 1);
        let colors = (0..ng.max(1)).map(|_| self.next_color()).collect();
        self.add(EdgeBundling {
            nodes: nodes.into_labels(),
            groups,
            group_names: Vec::new(),
            edges: edges.into_iter().collect(),
            colors,
            tension: 0.85,
            opacity: 0.6,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn arc_diagram_and_edge_bundling_draw() {
        let names: Vec<String> = (0..24).map(|i| format!("n{i}")).collect();
        let groups: Vec<usize> = (0..24).map(|i| i % 4).collect();
        let edges: Vec<(usize, usize, f64)> = (0..40)
            .map(|k| ((k * 7) % 24, (k * 11 + 3) % 24, 1.0 + (k % 5) as f64))
            .collect();
        let mut fig = Figure::new(900, 450);
        fig.grid(1, 2);
        fig.at(0, 0)
            .arc_diagram(&names, edges.clone())
            .groups(groups.clone())
            .group_names(["a", "b", "c", "d"]);
        fig.at(0, 1)
            .edge_bundling(&names, groups, edges)
            .group_names(["a", "b", "c", "d"]);
        let svg = fig.to_svg();
        assert!(svg.matches("<path").count() > 80);
        let px = fig.to_pixels().unwrap();
        assert!(px.rgba.chunks(4).filter(|p| p[0] < 200).count() > 2000);
    }

    #[test]
    fn bspline_ends_at_its_ends() {
        let p = [(0.0, 0.0), (1.0, 2.0), (3.0, 2.0), (4.0, 0.0)];
        let s = super::bspline(&p, 8);
        assert_eq!(s.first(), Some(&(0.0, 0.0)));
        assert_eq!(s.last(), Some(&(4.0, 0.0)));
    }
}
