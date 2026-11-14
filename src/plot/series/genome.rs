//! Genomics charts: Circos plot, Hi-C contact map, coverage track,
//! phylogenetic tree (Newick), Ramachandran plot, oncoprint and mutation
//! lollipop plot.

use super::general::Shapes;
use super::{short, Context, Extent, Hit, IntoValues, LegendEntry, LegendSymbol, Rect, Series};
use crate::plot::axes::{Axes, AxesKind};
use crate::plot::axis::{AxisMap, TickSpec};
use crate::plot::color::{Color, Colormap, Palette};
use crate::plot::scene::{Anchor, Baseline, LineJoin, Node, Stroke, TextNode};
use crate::plot::style::Marker;
use std::any::Any;
use std::f64::consts::{FRAC_PI_2, TAU};

// ----- Circos ------------------------------------------------------------------------

/// Circos plot: chromosomes around a circle with a data track and links
/// between positions (rearrangements, interactions). Created by
/// [`Axes::circos`].
#[derive(Clone, Debug)]
pub struct Circos {
    pub(crate) chroms: Vec<(String, f64)>,
    pub(crate) track: Vec<(usize, f64, f64)>,
    pub(crate) links: Vec<(usize, f64, usize, f64)>,
    pub(crate) colors: Vec<Color>,
    pub(crate) gap: f64,
    pub(crate) track_color: Color,
}

impl Circos {
    fn index(&self, name: &str) -> Option<usize> {
        self.chroms.iter().position(|c| c.0 == name)
    }
    /// A ring of bars: `value` at `pos` on chromosome `chrom` (e.g. copy
    /// number, mutation density).
    pub fn track<S: AsRef<str>>(
        &mut self,
        chrom: &[S],
        pos: impl IntoValues,
        value: impl IntoValues,
    ) -> &mut Self {
        let (pos, value) = (pos.into_values(), value.into_values());
        for ((c, p), v) in chrom.iter().zip(pos).zip(value) {
            if let Some(i) = self.index(c.as_ref()) {
                self.track.push((i, p, v));
            }
        }
        self
    }
    /// Links between `(chrom a, pos a, chrom b, pos b)`.
    pub fn links<S: AsRef<str>>(&mut self, links: &[(S, f64, S, f64)]) -> &mut Self {
        for (a, pa, b, pb) in links {
            if let (Some(i), Some(j)) = (self.index(a.as_ref()), self.index(b.as_ref())) {
                self.links.push((i, *pa, j, *pb));
            }
        }
        self
    }
    /// Color of the data track.
    pub fn track_color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.track_color = c.into();
        self
    }
}

impl Series for Circos {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(-1.0, -1.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let n = self.chroms.len();
        if n == 0 {
            return;
        }
        let total: f64 = self
            .chroms
            .iter()
            .map(|c| c.1.max(0.0))
            .sum::<f64>()
            .max(1e-300);
        let gap = self.gap.to_radians();
        let span = TAU - gap * n as f64;
        let (cx, cy) = (ctx.rect.x + ctx.rect.w / 2.0, ctx.rect.y + ctx.rect.h / 2.0);
        let r = ctx.rect.w.min(ctx.rect.h) / 2.0 - ctx.theme.tick_size * 2.6;
        let mut start = Vec::with_capacity(n);
        let mut a = -FRAC_PI_2;
        for c in &self.chroms {
            start.push(a);
            a += c.1.max(0.0) / total * span + gap;
        }
        let angle = |i: usize, pos: f64| start[i] + pos.clamp(0.0, self.chroms[i].1) / total * span;
        let at = |t: f64, rr: f64| (cx + rr * t.cos(), cy + rr * t.sin());
        let arc = |t0: f64, t1: f64, rr: f64, out: &mut Vec<(f64, f64)>| {
            let steps = (((t1 - t0).abs() * rr / 3.0) as usize).clamp(2, 256);
            for s in 0..=steps {
                out.push(at(t0 + (t1 - t0) * s as f64 / steps as f64, rr));
            }
        };
        let (r_out, r_in) = (r, r * 0.93);
        // Ideogram ring.
        let ts = ctx.theme.tick_size;
        for (i, c) in self.chroms.iter().enumerate() {
            let (t0, t1) = (start[i], angle(i, c.1));
            let mut pts = Vec::new();
            arc(t0, t1, r_out, &mut pts);
            arc(t1, t0, r_in, &mut pts);
            ctx.polygon(&pts, Some(self.colors[i % self.colors.len()]), None);
            let m = (t0 + t1) / 2.0;
            let (lx, ly) = at(m, r_out + ts * 0.9);
            // Labels upright, along the circle.
            let mut rot = m.to_degrees() + 90.0;
            if m.sin() > 0.0 {
                rot += 180.0;
            }
            ctx.push(Node::Text(TextNode {
                x: lx,
                y: ly,
                text: c.0.clone(),
                size: ts,
                color: ctx.theme.text,
                anchor: Anchor::Middle,
                baseline: Baseline::Middle,
                rotate: if (t1 - t0) * r < ts * 2.5 { rot } else { 0.0 },
                bold: false,
                italic: false,
            }));
        }
        // Data track as radial bars between r_in·0.97 and 0.78 r.
        let (t_lo, t_hi) = (r * 0.76, r * 0.9);
        if !self.track.is_empty() {
            let vmax = self
                .track
                .iter()
                .map(|t| t.2.abs())
                .fold(0.0, f64::max)
                .max(1e-300);
            let mut bg = Shapes::default();
            for i in 0..n {
                let mut pts = Vec::new();
                arc(start[i], angle(i, self.chroms[i].1), t_hi, &mut pts);
                arc(angle(i, self.chroms[i].1), start[i], t_lo, &mut pts);
                bg.poly(&pts);
            }
            bg.fill(ctx, ctx.theme.text.with_alpha(0.05));
            let mut bars = Shapes::default();
            for &(i, p, v) in &self.track {
                let t = angle(i, p);
                let rr = t_lo + (t_hi - t_lo) * (v.abs() / vmax);
                bars.line(&[at(t, t_lo), at(t, rr)]);
            }
            bars.stroke(ctx, Stroke::new(self.track_color, 1.6));
        }
        // Links as quadratic curves through the centre, flattened.
        let rl = if self.track.is_empty() {
            r_in * 0.97
        } else {
            t_lo * 0.97
        };
        for &(i, pa, j, pb) in &self.links {
            let (a, b) = (at(angle(i, pa), rl), at(angle(j, pb), rl));
            let mut pts = Vec::with_capacity(24);
            for s in 0..=24 {
                let t = s as f64 / 24.0;
                let u = 1.0 - t;
                pts.push((
                    u * u * a.0 + 2.0 * u * t * cx + t * t * b.0,
                    u * u * a.1 + 2.0 * u * t * cy + t * t * b.1,
                ));
            }
            let c = self.colors[i % self.colors.len()].with_alpha(0.55);
            ctx.polyline(&pts, Stroke::new(c, 1.4));
        }
    }
    fn kind(&self) -> &'static str {
        "circos"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- Hi-C -------------------------------------------------------------------------

/// Hi-C contact map as a triangle above the genome axis (log colors), with
/// optional domains (TADs) outlined. Created by [`Axes::hic`].
#[derive(Clone, Debug)]
pub struct HiC {
    pub(crate) m: Vec<Vec<f64>>,
    pub(crate) bin: f64,
    pub(crate) start: f64,
    pub(crate) colormap: Colormap,
    pub(crate) domains: Vec<(usize, usize)>,
    pub(crate) max_dist: usize,
}

impl HiC {
    /// Domains (TADs) as bin ranges `[start, end)` outlined on the map.
    pub fn domains(&mut self, d: &[(usize, usize)]) -> &mut Self {
        self.domains = d.to_vec();
        self
    }
    /// Colormap (default Reds).
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
    /// Largest distance (in bins) shown, cutting the triangle's top.
    pub fn max_distance(&mut self, bins: usize) -> &mut Self {
        self.max_dist = bins.max(1);
        self
    }
}

impl Series for HiC {
    fn extent(&self) -> Option<Extent> {
        let n = self.m.len() as f64;
        Some(Extent::point(self.start, 0.0).include(
            self.start + n * self.bin,
            self.max_dist.min(self.m.len()) as f64 * self.bin / 2.0,
        ))
    }
    fn draw(&self, ctx: &mut Context) {
        let n = self.m.len();
        // Cell (i, j), i <= j, is a diamond centred at ((i + j + 1) / 2, (j - i) / 2) bins.
        let mut vals: Vec<f64> = self
            .m
            .iter()
            .flatten()
            .copied()
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| v.ln_1p())
            .collect();
        vals.sort_by(f64::total_cmp);
        // Colors from the 5th percentile to the maximum (log scale).
        let vmin = if vals.is_empty() {
            0.0
        } else {
            crate::plot::stats::quantile_sorted(&vals, 0.05)
        };
        let vmax = vals.last().copied().unwrap_or(1.0).max(vmin + 1e-12);
        const LEVELS: usize = 40;
        let mut groups: Vec<Shapes> = (0..LEVELS).map(|_| Shapes::default()).collect();
        let (xm, ym) = (ctx.x, ctx.y);
        let p = move |x: f64, y: f64| {
            (
                xm.px_clamped(self.start + x * self.bin),
                ym.px_clamped(y * self.bin),
            )
        };
        for i in 0..n {
            for j in i..n.min(i + self.max_dist) {
                let v = self.m[i].get(j).copied().unwrap_or(0.0);
                if !(v.is_finite() && v > 0.0) {
                    continue;
                }
                let k = (((v.ln_1p() - vmin) / (vmax - vmin)).clamp(0.0, 1.0) * (LEVELS - 1) as f64)
                    .round() as usize;
                let (cxx, cyy) = ((i + j + 1) as f64 / 2.0, (j - i) as f64 / 2.0);
                groups[k.min(LEVELS - 1)].poly(&[
                    p(cxx - 0.5, cyy),
                    p(cxx, cyy + 0.5),
                    p(cxx + 0.5, cyy),
                    p(cxx, cyy - 0.5),
                ]);
            }
        }
        for (k, g) in groups.into_iter().enumerate() {
            g.fill(
                ctx,
                self.colormap
                    .at(0.04 + 0.96 * k as f64 / (LEVELS - 1) as f64),
            );
        }
        let mut tads = Shapes::default();
        for &(a, b) in &self.domains {
            let (a, b) = (a as f64, b as f64);
            tads.line(&[p(a, 0.0), p((a + b) / 2.0, (b - a) / 2.0), p(b, 0.0)]);
        }
        tads.stroke(
            ctx,
            Stroke::new(Color::rgb(30, 30, 160), 1.6).join(LineJoin::Round),
        );
    }
    fn hit(&self, x: &AxisMap, y: &AxisMap, _r: Rect, px: f64, py: f64) -> Option<Hit> {
        let (u, v) = (
            (x.value(px) - self.start) / self.bin,
            y.value(py) / self.bin,
        );
        let (i, j) = ((u - v - 0.5).round(), (u + v - 0.5).round());
        if i < 0.0 || j < i || j as usize >= self.m.len() {
            return None;
        }
        let (i, j) = (i as usize, j as usize);
        Some(Hit {
            px,
            py,
            label: format!("bins {i}–{j}: {}", short(self.m[i][j])),
            distance: 0.0,
        })
    }
    fn kind(&self) -> &'static str {
        "Hi-C map"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- coverage ------------------------------------------------------------------------

/// Read coverage (depth) along a region as a filled step track, with an
/// optional minimum-depth line. Created by [`Axes::coverage`].
#[derive(Clone, Debug)]
pub struct Coverage {
    pub(crate) pos: Vec<f64>,
    pub(crate) depth: Vec<f64>,
    pub(crate) color: Color,
    pub(crate) min_depth: Option<f64>,
}

impl Coverage {
    /// Marks positions below this depth in red and draws the line.
    pub fn min_depth(&mut self, d: f64) -> &mut Self {
        self.min_depth = Some(d);
        self
    }
    /// Fill color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
}

impl Series for Coverage {
    fn extent(&self) -> Option<Extent> {
        let (x0, x1) = self
            .pos
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        let hi = self
            .depth
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(0.0, f64::max);
        x0.is_finite()
            .then(|| Extent::point(x0, 0.0).include(x1, hi))
    }
    fn draw(&self, ctx: &mut Context) {
        let n = self.pos.len().min(self.depth.len());
        if n < 2 {
            return;
        }
        // At most one step per pixel column: max depth per column.
        let (x0, x1) = (ctx.x.px(self.pos[0]), ctx.x.px(self.pos[n - 1]));
        let cols = ((x1 - x0).abs().ceil() as usize).clamp(1, n);
        let mut top = vec![0.0f64; cols];
        let mut low = vec![f64::INFINITY; cols];
        for i in 0..n {
            let c = (((ctx.x.px(self.pos[i]) - x0) / (x1 - x0).max(1e-9)) * (cols - 1) as f64)
                .round() as usize;
            let c = c.min(cols - 1);
            top[c] = top[c].max(self.depth[i]);
            low[c] = low[c].min(self.depth[i]);
        }
        let (_, y0) = ctx.point(self.pos[0], 0.0);
        let mut pts = vec![(x0, y0)];
        let step = (x1 - x0) / cols as f64;
        for (c, &d) in top.iter().enumerate() {
            let y = ctx.y.px(d);
            pts.push((x0 + c as f64 * step, y));
            pts.push((x0 + (c + 1) as f64 * step, y));
        }
        pts.push((x1, y0));
        ctx.polygon(&pts, Some(self.color), None);
        if let Some(m) = self.min_depth {
            let mut bad = Shapes::default();
            for (c, &d) in low.iter().enumerate() {
                if d < m {
                    bad.rect(
                        x0 + c as f64 * step,
                        y0,
                        x0 + (c + 1) as f64 * step,
                        y0 + 5.0,
                    );
                }
            }
            bad.fill(ctx, Color::rgb(214, 39, 40));
            let y = ctx.y.px(m);
            ctx.polyline(
                &[(ctx.rect.x, y), (ctx.rect.right(), y)],
                Stroke::new(Color::rgb(214, 39, 40), 1.0).dashed(vec![5.0, 3.0]),
            );
        }
    }
    fn kind(&self) -> &'static str {
        "coverage"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- phylogenetic tree -----------------------------------------------------------------

/// A node of a parsed Newick tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeNode {
    /// Label (leaves and named clades).
    pub name: String,
    /// Branch length to the parent.
    pub length: f64,
    /// Children.
    pub children: Vec<TreeNode>,
}

/// Parses a Newick tree (`((A:0.1,B:0.2)AB:0.3,C:0.4);`). Missing branch
/// lengths count as 1.
pub fn parse_newick(s: &str) -> Option<TreeNode> {
    let b = s.trim().trim_end_matches(';').as_bytes();
    let mut i = 0;
    let node = newick_node(b, &mut i)?;
    Some(node)
}

fn newick_node(b: &[u8], i: &mut usize) -> Option<TreeNode> {
    let mut node = TreeNode {
        length: f64::NAN,
        ..Default::default()
    };
    skip_ws(b, i);
    if b.get(*i) == Some(&b'(') {
        *i += 1;
        loop {
            node.children.push(newick_node(b, i)?);
            skip_ws(b, i);
            match b.get(*i) {
                Some(b',') => *i += 1,
                Some(b')') => {
                    *i += 1;
                    break;
                }
                _ => return None,
            }
        }
    }
    skip_ws(b, i);
    // Name (quoted or bare).
    if b.get(*i) == Some(&b'\'') {
        *i += 1;
        let s = *i;
        while *i < b.len() && b[*i] != b'\'' {
            *i += 1;
        }
        node.name = String::from_utf8_lossy(&b[s..*i]).into_owned();
        *i += 1;
    } else {
        let s = *i;
        while *i < b.len() && !b"(),:;".contains(&b[*i]) {
            *i += 1;
        }
        node.name = String::from_utf8_lossy(&b[s..*i]).trim().replace('_', " ");
    }
    skip_ws(b, i);
    if b.get(*i) == Some(&b':') {
        *i += 1;
        let s = *i;
        while *i < b.len() && !b"(),;".contains(&b[*i]) {
            *i += 1;
        }
        node.length = std::str::from_utf8(&b[s..*i]).ok()?.trim().parse().ok()?;
    }
    Some(node)
}

fn skip_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && b[*i].is_ascii_whitespace() {
        *i += 1;
    }
}

/// Phylogenetic tree with branch lengths, rectangular or circular. Created
/// by [`Axes::phylo_tree`].
#[derive(Clone, Debug)]
pub struct PhyloTree {
    /// Flattened nodes: (parent, depth, name, is leaf).
    pub(crate) nodes: Vec<(usize, f64, String, bool)>,
    /// Leaf order position (y) of each node.
    pub(crate) pos: Vec<f64>,
    pub(crate) leaves: usize,
    pub(crate) circular: bool,
    pub(crate) color: Color,
    pub(crate) leaf_colors: Vec<(String, Color)>,
}

impl PhyloTree {
    /// Colors of leaves whose name starts with a prefix (e.g. a genus).
    pub fn color_leaves(&mut self, prefix: &str, c: impl Into<Color>) -> &mut Self {
        self.leaf_colors.push((prefix.into(), c.into()));
        self
    }
    fn max_depth(&self) -> f64 {
        self.nodes
            .iter()
            .map(|n| n.1)
            .fold(0.0, f64::max)
            .max(1e-12)
    }
    fn leaf_color(&self, name: &str) -> Color {
        self.leaf_colors
            .iter()
            .rev()
            .find(|(p, _)| name.starts_with(p.as_str()))
            .map_or(self.color, |c| c.1)
    }
}

fn flatten(
    n: &TreeNode,
    parent: usize,
    depth: f64,
    out: &mut Vec<(usize, f64, String, bool)>,
    kids: &mut Vec<Vec<usize>>,
) -> usize {
    let len = if n.length.is_finite() {
        n.length
    } else if parent == usize::MAX {
        0.0
    } else {
        1.0
    };
    let id = out.len();
    out.push((parent, depth + len, n.name.clone(), n.children.is_empty()));
    kids.push(Vec::new());
    for c in &n.children {
        let k = flatten(c, id, depth + len, out, kids);
        kids[id].push(k);
    }
    id
}

impl Series for PhyloTree {
    fn extent(&self) -> Option<Extent> {
        if self.circular {
            return Some(Extent::point(-1.0, -1.0).include(1.0, 1.0));
        }
        // Room right of the deepest leaf for the labels.
        Some(Extent::point(0.0, -0.5).include(self.max_depth() * 1.32, self.leaves as f64 - 0.5))
    }
    fn draw(&self, ctx: &mut Context) {
        let ts = ctx.theme.tick_size;
        let stroke = Stroke::new(self.color, 1.3).join(LineJoin::Round);
        let mut lines = Shapes::default();
        if !self.circular {
            for (k, n) in self.nodes.iter().enumerate() {
                if n.0 == usize::MAX {
                    continue;
                }
                let par = &self.nodes[n.0];
                // Elbow: vertical at the parent's depth, then horizontal.
                let a = ctx.point(par.1, self.pos[n.0]);
                let b = ctx.point(par.1, self.pos[k]);
                let c = ctx.point(n.1, self.pos[k]);
                lines.line(&[a, b, c]);
            }
            lines.stroke(ctx, stroke);
            for (k, n) in self.nodes.iter().enumerate().filter(|(_, n)| n.3) {
                let (x, y) = ctx.point(n.1, self.pos[k]);
                let c = self.leaf_color(&n.2);
                ctx.push(Node::Circle {
                    cx: x,
                    cy: y,
                    r: 2.5,
                    fill: Some(c),
                    stroke: None,
                });
                ctx.text(
                    x + 6.0,
                    y,
                    n.2.clone(),
                    ts,
                    c,
                    Anchor::Start,
                    Baseline::Middle,
                );
            }
            return;
        }
        let (cx, cy) = (ctx.rect.x + ctx.rect.w / 2.0, ctx.rect.y + ctx.rect.h / 2.0);
        let longest = self
            .nodes
            .iter()
            .filter(|n| n.3)
            .map(|n| ctx.text_width(&n.2, ts))
            .fold(0.0, f64::max);
        let rmax = (ctx.rect.w.min(ctx.rect.h) / 2.0 - longest - 12.0).max(30.0);
        let k = rmax / self.max_depth();
        let span = TAU * (1.0 - 1.0 / self.leaves.max(2) as f64);
        let ang = |p: f64| -FRAC_PI_2 + p / (self.leaves.max(2) - 1) as f64 * span;
        let at = |t: f64, r: f64| (cx + r * t.cos(), cy + r * t.sin());
        for (id, n) in self.nodes.iter().enumerate() {
            if n.0 == usize::MAX {
                continue;
            }
            let par = &self.nodes[n.0];
            let (tp, tc) = (ang(self.pos[n.0]), ang(self.pos[id]));
            // Arc at the parent's radius from the parent's angle to the
            // child's, then the radial branch.
            let mut pts = Vec::new();
            let steps = (((tc - tp).abs() * par.1 * k / 4.0) as usize).clamp(1, 64);
            for s in 0..=steps {
                pts.push(at(tp + (tc - tp) * s as f64 / steps as f64, par.1 * k));
            }
            pts.push(at(tc, n.1 * k));
            lines.line(&pts);
        }
        lines.stroke(ctx, stroke);
        for (id, n) in self.nodes.iter().enumerate().filter(|(_, n)| n.3) {
            let t = ang(self.pos[id]);
            let (x, y) = at(t, n.1 * k + 6.0);
            let left = t.cos() < 0.0;
            let c = self.leaf_color(&n.2);
            ctx.push(Node::Text(TextNode {
                x,
                y,
                text: n.2.clone(),
                size: ts,
                color: c,
                anchor: if left { Anchor::End } else { Anchor::Start },
                baseline: Baseline::Middle,
                rotate: t.to_degrees() + if left { 180.0 } else { 0.0 },
                bold: false,
                italic: false,
            }));
        }
    }
    fn kind(&self) -> &'static str {
        "phylogenetic tree"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- oncoprint --------------------------------------------------------------------------

/// Kinds of genomic alterations in an oncoprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Alteration {
    /// Copy number gain (full red cell).
    Amplification,
    /// Copy number loss (full blue cell).
    Deletion,
    /// Missense mutation (green bar).
    Missense,
    /// Truncating mutation: nonsense, frameshift, splice (black bar).
    Truncating,
    /// Gene fusion (purple bar).
    Fusion,
}

impl Alteration {
    fn color(self) -> Color {
        match self {
            Alteration::Amplification => Color::rgb(220, 50, 47),
            Alteration::Deletion => Color::rgb(38, 99, 186),
            Alteration::Missense => Color::rgb(46, 160, 67),
            Alteration::Truncating => Color::rgb(25, 25, 25),
            Alteration::Fusion => Color::rgb(148, 87, 205),
        }
    }
    fn full(self) -> bool {
        matches!(self, Alteration::Amplification | Alteration::Deletion)
    }
    fn name(self) -> &'static str {
        match self {
            Alteration::Amplification => "amplification",
            Alteration::Deletion => "deep deletion",
            Alteration::Missense => "missense",
            Alteration::Truncating => "truncating",
            Alteration::Fusion => "fusion",
        }
    }
}

/// Oncoprint: genes × samples grid of alterations, genes by frequency
/// (percent on the left), samples sorted so that altered samples group.
/// Created by [`Axes::oncoprint`].
#[derive(Clone, Debug)]
pub struct Oncoprint {
    pub(crate) genes: Vec<String>,
    pub(crate) samples: usize,
    /// Per (gene, sample) the alterations.
    pub(crate) events: Vec<(usize, usize, Alteration)>,
    /// Sample columns in display order.
    pub(crate) order: Vec<usize>,
}

impl Series for Oncoprint {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let r = ctx.rect;
        let ts = ctx.theme.tick_size;
        let ng = self.genes.len();
        if ng == 0 || self.samples == 0 {
            return;
        }
        let pct_w = ctx.text_width("100 %", ts) + 8.0;
        let name_w = self
            .genes
            .iter()
            .map(|g| ctx.text_width(g, ts))
            .fold(0.0, f64::max)
            + 10.0;
        let left = r.x + name_w + pct_w;
        let cw = (r.right() - left) / self.samples as f64;
        let rh = (r.h / ng as f64).min(ts * 2.4);
        let top = r.y + (r.h - rh * ng as f64) / 2.0;
        let col: Vec<usize> = {
            let mut c = vec![0; self.samples];
            for (k, &s) in self.order.iter().enumerate() {
                c[s] = k;
            }
            c
        };
        let gap = if cw > 3.0 { 1.0 } else { 0.0 };
        let mut bg = Shapes::default();
        for g in 0..ng {
            for s in 0..self.samples {
                let x = left + s as f64 * cw;
                let y = top + g as f64 * rh;
                bg.rect(x + gap / 2.0, y + 1.0, x + cw - gap / 2.0, y + rh - 1.0);
            }
        }
        bg.fill(ctx, ctx.theme.text.mix(ctx.theme.plot_background, 0.86));
        let kinds = [
            Alteration::Amplification,
            Alteration::Deletion,
            Alteration::Fusion,
            Alteration::Missense,
            Alteration::Truncating,
        ];
        for kind in kinds {
            let mut s = Shapes::default();
            for &(g, smp, k) in &self.events {
                if k != kind {
                    continue;
                }
                let x = left + col[smp] as f64 * cw;
                let y = top + g as f64 * rh;
                if k.full() {
                    s.rect(x + gap / 2.0, y + 1.0, x + cw - gap / 2.0, y + rh - 1.0);
                } else {
                    s.rect(
                        x + gap / 2.0,
                        y + rh * 0.33,
                        x + cw - gap / 2.0,
                        y + rh * 0.67,
                    );
                }
            }
            s.fill(ctx, kind.color());
        }
        for (g, name) in self.genes.iter().enumerate() {
            let y = top + (g as f64 + 0.5) * rh;
            let mut altered: Vec<usize> = self
                .events
                .iter()
                .filter(|e| e.0 == g)
                .map(|e| e.1)
                .collect();
            altered.sort_unstable();
            altered.dedup();
            let pct = altered.len() as f64 * 100.0 / self.samples as f64;
            ctx.text(
                r.x + name_w - 6.0,
                y,
                name.clone(),
                ts,
                ctx.theme.text,
                Anchor::End,
                Baseline::Middle,
            );
            ctx.text(
                left - 6.0,
                y,
                format!("{pct:.0} %"),
                ts * 0.9,
                ctx.theme.text,
                Anchor::End,
                Baseline::Middle,
            );
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        let mut kinds: Vec<Alteration> = Vec::new();
        for e in &self.events {
            if !kinds.contains(&e.2) {
                kinds.push(e.2);
            }
        }
        kinds
            .iter()
            .map(|k| LegendEntry {
                label: k.name().into(),
                symbol: LegendSymbol::Patch {
                    fill: k.color(),
                    edge: None,
                },
            })
            .collect()
    }
    fn kind(&self) -> &'static str {
        "oncoprint"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- mutation lollipop -------------------------------------------------------------------

/// Mutations along a protein: the protein as a bar with its domains,
/// a stem per mutated position with height = count, the hotspots named.
/// Created by [`Axes::mutation_lollipop`].
#[derive(Clone, Debug)]
pub struct MutationLollipop {
    pub(crate) length: f64,
    pub(crate) domains: Vec<(String, f64, f64)>,
    pub(crate) muts: Vec<(f64, f64, String, Alteration)>,
    pub(crate) labels: usize,
    pub(crate) colors: Vec<Color>,
}

impl MutationLollipop {
    /// How many of the most frequent mutations are named (default 5).
    pub fn labels(&mut self, n: usize) -> &mut Self {
        self.labels = n;
        self
    }
}

impl Series for MutationLollipop {
    fn extent(&self) -> Option<Extent> {
        let hi = self.muts.iter().map(|m| m.1).fold(1.0, f64::max);
        Some(Extent::point(0.0, -hi * 0.18).include(self.length, hi * 1.15))
    }
    fn draw(&self, ctx: &mut Context) {
        let hi = self.muts.iter().map(|m| m.1).fold(1.0, f64::max);
        let (bar0, bar1) = (-hi * 0.06, -hi * 0.14);
        // Protein backbone and domains.
        let (x0, y0) = ctx.point(0.0, bar0 + (bar1 - bar0) * 0.3);
        let (x1, y1) = ctx.point(self.length, bar0 + (bar1 - bar0) * 0.7);
        ctx.rect(
            x0,
            y0,
            x1,
            y1,
            Some(ctx.theme.text.mix(ctx.theme.plot_background, 0.7)),
            None,
        );
        for (k, (name, a, b)) in self.domains.iter().enumerate() {
            let (xa, ya) = ctx.point(*a, bar0);
            let (xb, yb) = ctx.point(*b, bar1);
            let c = self.colors[k % self.colors.len()];
            ctx.push(Node::Rect {
                x: xa.min(xb),
                y: ya.min(yb),
                w: (xb - xa).abs(),
                h: (yb - ya).abs(),
                rx: 3.0,
                fill: Some(c),
                stroke: None,
            });
            if ctx.text_width(name, ctx.theme.tick_size * 0.85) < (xb - xa).abs() - 4.0 {
                ctx.text(
                    (xa + xb) / 2.0,
                    (ya + yb) / 2.0,
                    name.clone(),
                    ctx.theme.tick_size * 0.85,
                    c.contrasting_text(),
                    Anchor::Middle,
                    Baseline::Middle,
                );
            }
        }
        // Stems in one path, heads by kind.
        let mut stems = Shapes::default();
        for m in &self.muts {
            stems.line(&[ctx.point(m.0, bar0), ctx.point(m.0, m.1)]);
        }
        stems.stroke(ctx, Stroke::new(ctx.theme.text.with_alpha(0.55), 1.0));
        let mut order: Vec<usize> = (0..self.muts.len()).collect();
        order.sort_by(|a, b| self.muts[*a].1.total_cmp(&self.muts[*b].1));
        for &i in &order {
            let m = &self.muts[i];
            let (x, y) = ctx.point(m.0, m.1);
            let r = 3.0 + (m.1 / hi).sqrt() * 4.0;
            ctx.push(Node::Circle {
                cx: x,
                cy: y,
                r,
                fill: Some(m.3.color()),
                stroke: Some(Stroke::new(ctx.theme.plot_background, 1.0)),
            });
        }
        // Labels above the heads, or right of them when that spot is taken;
        // left out when both overlap a placed label.
        let ts = ctx.theme.tick_size * 0.9;
        let mut placed: Vec<(f64, f64, f64, f64)> = Vec::new();
        let free = |b: (f64, f64, f64, f64), placed: &[(f64, f64, f64, f64)]| {
            placed
                .iter()
                .all(|p| b.2 < p.0 || b.0 > p.2 || b.3 < p.1 || b.1 > p.3)
        };
        for &i in order.iter().rev().take(self.labels) {
            let m = &self.muts[i];
            let (x, y) = ctx.point(m.0, m.1);
            let r = 3.0 + (m.1 / hi).sqrt() * 4.0;
            let w = ctx.text_width(&m.2, ts);
            let above = (
                x - w / 2.0 - 2.0,
                y - r - 3.0 - ts * 1.2,
                x + w / 2.0 + 2.0,
                y - r - 2.0,
            );
            let right = (x + r + 2.0, y - ts * 0.6, x + r + 5.0 + w, y + ts * 0.6);
            if free(above, &placed) {
                ctx.text(
                    x,
                    y - r - 3.0,
                    m.2.clone(),
                    ts,
                    ctx.theme.text,
                    Anchor::Middle,
                    Baseline::Bottom,
                );
                placed.push(above);
            } else if free(right, &placed) {
                ctx.text(
                    x + r + 4.0,
                    y,
                    m.2.clone(),
                    ts,
                    ctx.theme.text,
                    Anchor::Start,
                    Baseline::Middle,
                );
                placed.push(right);
            }
        }
    }
    fn hit(&self, x: &AxisMap, y: &AxisMap, _r: Rect, px: f64, py: f64) -> Option<Hit> {
        self.muts
            .iter()
            .map(|m| {
                let (a, b) = (x.px(m.0), y.px(m.1));
                Hit {
                    px: a,
                    py: b,
                    label: format!("{} ({}×, {})", m.2, short(m.1), m.3.name()),
                    distance: (a - px).hypot(b - py),
                }
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn legend(&self) -> Vec<LegendEntry> {
        let mut kinds: Vec<Alteration> = Vec::new();
        for m in &self.muts {
            if !kinds.contains(&m.3) {
                kinds.push(m.3);
            }
        }
        kinds
            .iter()
            .map(|k| LegendEntry {
                label: k.name().into(),
                symbol: LegendSymbol::Marker {
                    marker: Marker::Circle,
                    size: 9.0,
                    color: k.color(),
                },
            })
            .collect()
    }
    fn kind(&self) -> &'static str {
        "mutation lollipop"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// Circos plot of named chromosomes with lengths; add a data ring with
    /// [`Circos::track`] and links with [`Circos::links`].
    pub fn circos<S: AsRef<str>>(&mut self, chromosomes: &[(S, f64)]) -> &mut Circos {
        self.kind = AxesKind::Frameless;
        let n = chromosomes.len();
        let palette = if n > 10 {
            Palette::Tab20.colors()
        } else {
            Palette::Tab10.colors()
        };
        self.add(Circos {
            chroms: chromosomes
                .iter()
                .map(|c| (c.0.as_ref().to_string(), c.1))
                .collect(),
            track: Vec::new(),
            links: Vec::new(),
            colors: palette,
            gap: 1.2,
            track_color: Color::rgb(60, 60, 70),
        })
    }

    /// Hi-C contact map of a symmetric `matrix` of bins of `bin_size` from
    /// `start`, drawn as a triangle above the genomic axis.
    pub fn hic(&mut self, matrix: Vec<Vec<f64>>, start: f64, bin_size: f64) -> &mut HiC {
        self.y.visible = false;
        self.y.grid = Some(false);
        self.x.grid = Some(false);
        self.y.margin = 0.0;
        let n = matrix.len();
        self.add(HiC {
            m: matrix,
            bin: bin_size,
            start,
            colormap: Colormap::Reds,
            domains: Vec::new(),
            max_dist: n,
        })
    }

    /// Coverage (read depth) along positions as a filled step track.
    pub fn coverage(
        &mut self,
        positions: impl IntoValues,
        depth: impl IntoValues,
    ) -> &mut Coverage {
        self.y.label.get_or_insert_with(|| "depth".into());
        self.add(Coverage {
            pos: positions.into_values(),
            depth: depth.into_values(),
            color: Color::rgb(120, 130, 150),
            min_depth: None,
        })
    }

    /// Phylogenetic tree from Newick text (branch lengths as x). Returns
    /// `None` when the text does not parse.
    pub fn phylo_tree(&mut self, newick: &str) -> Option<&mut PhyloTree> {
        let root = parse_newick(newick)?;
        let mut nodes = Vec::new();
        let mut kids = Vec::new();
        flatten(&root, usize::MAX, 0.0, &mut nodes, &mut kids);
        // Leaves in order 0, 1, …; inner nodes at the mean of their children.
        let mut pos = vec![0.0; nodes.len()];
        let mut leaf = 0.0;
        fn place(id: usize, kids: &[Vec<usize>], pos: &mut [f64], leaf: &mut f64) {
            if kids[id].is_empty() {
                pos[id] = *leaf;
                *leaf += 1.0;
                return;
            }
            for &k in &kids[id] {
                place(k, kids, pos, leaf);
            }
            pos[id] = (pos[kids[id][0]] + pos[*kids[id].last().unwrap()]) / 2.0;
        }
        place(0, &kids, &mut pos, &mut leaf);
        self.y.visible = false;
        self.y.grid = Some(false);
        self.y.inverted = true;
        self.x
            .label
            .get_or_insert_with(|| "substitutions per site".into());
        Some(self.add(PhyloTree {
            leaves: leaf as usize,
            nodes,
            pos,
            circular: false,
            color: Color::rgb(50, 55, 70),
            leaf_colors: Vec::new(),
        }))
    }

    /// Phylogenetic tree from Newick text in a circular (radial) layout.
    pub fn phylo_tree_circular(&mut self, newick: &str) -> Option<&mut PhyloTree> {
        self.kind = AxesKind::Frameless;
        let t = self.phylo_tree(newick)?;
        t.circular = true;
        Some(t)
    }

    /// Ramachandran plot of backbone dihedral angles φ, ψ (degrees) over
    /// the regions of α-helices, β-sheets and left-handed helices.
    pub fn ramachandran(
        &mut self,
        phi: impl IntoValues,
        psi: impl IntoValues,
    ) -> &mut super::scatter::Scatter {
        let (phi, psi) = (phi.into_values(), psi.into_values());
        // Favoured regions (approximate, Lovell et al. 2003) as shaded shapes.
        let region = |cx: f64, cy: f64, rx: f64, ry: f64| -> (Vec<f64>, Vec<f64>) {
            (0..=48)
                .map(|k| {
                    let t = k as f64 / 48.0 * TAU;
                    (cx + rx * t.cos(), cy + ry * t.sin())
                })
                .unzip()
        };
        let shade = Color::rgb(70, 130, 180);
        for (cx, cy, rx, ry, name) in [
            (-120.0, 135.0, 55.0, 45.0, "β"),
            (-65.0, -40.0, 35.0, 30.0, "α"),
            (65.0, 40.0, 22.0, 25.0, "Lα"),
        ] {
            let (x, y) = region(cx, cy, rx, ry);
            self.fill_between(&x, &y, vec![cy; x.len()])
                .color(shade)
                .opacity(0.14);
            self.text(cx, cy + ry + 6.0, name)
                .size(13.0)
                .bold(true)
                .color(shade.darken(0.2))
                .anchor(Anchor::Middle);
        }
        let ticks: Vec<f64> = (-3..=3).map(|k| k as f64 * 60.0).collect();
        self.x.ticks = TickSpec::At(ticks.clone());
        self.y.ticks = TickSpec::At(ticks);
        self.x_limits(-180.0, 180.0).y_limits(-180.0, 180.0);
        self.x.margin = 0.0;
        self.y.margin = 0.0;
        self.equal_aspect = true;
        self.x.label.get_or_insert_with(|| "φ (degrees)".into());
        self.y.label.get_or_insert_with(|| "ψ (degrees)".into());
        let c = Color::rgb(30, 30, 40);
        self.scatter(phi, psi).color(c).size(5.0)
    }

    /// Oncoprint of `(gene index, sample index, alteration)` events over
    /// named genes and `samples` samples.
    pub fn oncoprint<S: AsRef<str>>(
        &mut self,
        genes: &[S],
        samples: usize,
        events: &[(usize, usize, Alteration)],
    ) -> &mut Oncoprint {
        self.kind = AxesKind::Frameless;
        let ng = genes.len();
        // Genes by frequency, most altered on top.
        let freq = |g: usize| {
            let mut s: Vec<usize> = events.iter().filter(|e| e.0 == g).map(|e| e.1).collect();
            s.sort_unstable();
            s.dedup();
            s.len()
        };
        let mut gorder: Vec<usize> = (0..ng).collect();
        gorder.sort_by_key(|g| std::cmp::Reverse(freq(*g)));
        let rank: Vec<usize> = {
            let mut r = vec![0; ng];
            for (k, &g) in gorder.iter().enumerate() {
                r[g] = k;
            }
            r
        };
        let events: Vec<(usize, usize, Alteration)> = events
            .iter()
            .filter(|e| e.0 < ng && e.1 < samples)
            .map(|e| (rank[e.0], e.1, e.2))
            .collect();
        // Samples sorted by their alteration pattern in gene order (mutual
        // exclusivity shows as a staircase).
        let mut key = vec![0u128; samples];
        for e in &events {
            if e.0 < 128 {
                key[e.1] |= 1u128 << (127 - e.0);
            }
        }
        let mut order: Vec<usize> = (0..samples).collect();
        order.sort_by(|a, b| key[*b].cmp(&key[*a]));
        self.legend(crate::plot::LegendPosition::OutsideBottom);
        self.add(Oncoprint {
            genes: gorder
                .iter()
                .map(|&g| genes[g].as_ref().to_string())
                .collect(),
            samples,
            events,
            order,
        })
    }

    /// Mutation lollipop plot of a protein of `length` amino acids with
    /// `(name, start, end)` domains and `(position, count, label, kind)`
    /// mutations.
    pub fn mutation_lollipop<S: AsRef<str>, L: AsRef<str>>(
        &mut self,
        length: f64,
        domains: &[(S, f64, f64)],
        mutations: &[(f64, f64, L, Alteration)],
    ) -> &mut MutationLollipop {
        self.y.label.get_or_insert_with(|| "mutations".into());
        self.x.label.get_or_insert_with(|| "amino acid".into());
        // The protein bar sits below 0: no tick labels there.
        self.y.format = crate::plot::TickFormat::Custom(std::sync::Arc::new(|v| {
            if v < -1e-9 {
                String::new()
            } else {
                short(v)
            }
        }));
        self.x.grid = Some(false);
        let colors = Palette::Set2.colors();
        self.add(MutationLollipop {
            length,
            domains: domains
                .iter()
                .map(|d| (d.0.as_ref().to_string(), d.1, d.2))
                .collect(),
            muts: mutations
                .iter()
                .map(|m| (m.0, m.1, m.2.as_ref().to_string(), m.3))
                .collect(),
            labels: 5,
            colors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn newick() {
        let t = parse_newick("((A:0.1,B:0.2)AB:0.3,'C c':0.4);").unwrap();
        assert_eq!(t.children.len(), 2);
        assert_eq!(t.children[0].name, "AB");
        assert_eq!(t.children[0].children[1].name, "B");
        assert!((t.children[0].children[1].length - 0.2).abs() < 1e-12);
        assert_eq!(t.children[1].name, "C c");
        assert!(parse_newick("((A,B").is_none());
    }

    #[test]
    fn all_render() {
        let mut fig = Figure::new(1400, 800);
        fig.grid(2, 4);
        fig.at(0, 0)
            .circos(&[("chr1", 248.0), ("chr2", 242.0), ("chr3", 198.0)])
            .track(&["chr1", "chr2"], [10.0, 50.0], [1.0, 3.0])
            .links(&[("chr1", 20.0, "chr3", 100.0)]);
        let m: Vec<Vec<f64>> = (0..30)
            .map(|i| {
                (0..30)
                    .map(|j| 100.0 / (1.0 + (i as f64 - j as f64).abs()))
                    .collect()
            })
            .collect();
        fig.at(0, 1)
            .hic(m, 0.0, 10_000.0)
            .domains(&[(0, 10), (10, 30)]);
        fig.at(0, 2)
            .coverage(
                (0..500).map(|i| i as f64),
                (0..500).map(|i| 30.0 + (i as f64 / 20.0).sin() * 20.0),
            )
            .min_depth(15.0);
        fig.at(0, 3)
            .phylo_tree("((A:0.1,B:0.2):0.3,(C:0.2,(D:0.1,E:0.15):0.1):0.2);")
            .unwrap();
        fig.at(1, 0)
            .phylo_tree_circular("((A:0.1,B:0.2):0.3,(C:0.2,(D:0.1,E:0.15):0.1):0.2);")
            .unwrap();
        fig.at(1, 1).ramachandran([-60.0, -120.0], [-45.0, 130.0]);
        fig.at(1, 2).oncoprint(
            &["TP53", "KRAS"],
            10,
            &[
                (0, 1, Alteration::Missense),
                (1, 2, Alteration::Amplification),
            ],
        );
        fig.at(1, 3).mutation_lollipop(
            393.0,
            &[("DBD", 100.0, 290.0)],
            &[(175.0, 30.0, "R175H", Alteration::Missense)],
        );
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
    }
}
