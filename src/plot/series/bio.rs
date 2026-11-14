//! Charts for biology, genomics and medicine: sequence logos, gene tracks,
//! Manhattan and p-value Q-Q plots, volcano and MA plots, clustermaps,
//! single-cell embeddings, dose-response curves, Kaplan-Meier survival
//! curves and forest plots.

use super::distribution::erfc;
use super::line::{Line, Step};
use super::ml::{cluster, Dendrogram};
use super::scatter::Scatter;
use super::{short, Context, Extent, Hit, IntoLabels, IntoValues, Rect, Series};
use crate::plot::axes::Axes;
use crate::plot::axis::AxisMap;
use crate::plot::color::{Color, Colormap};
use crate::plot::figure::Figure;
use crate::plot::scale::Scale;
use crate::plot::scene::{Anchor, Baseline, Node, PathCmd, Stroke};
use crate::plot::style::Marker;
use crate::plot::TickSpec;
use ab_glyph::{Font, OutlineCurve};
use std::any::Any;

const NS: Color = Color::rgb(190, 190, 190);
const UP: Color = Color::rgb(214, 39, 40);
const DOWN: Color = Color::rgb(31, 119, 180);

// ---------------------------------------------------------------- logos

/// Letter colors of a sequence logo: nucleotides, or amino acids by
/// chemistry (acidic red, basic blue, polar green, hydrophobic black).
fn letter_color(c: char) -> Color {
    match c.to_ascii_uppercase() {
        'A' => Color::rgb(16, 150, 72),
        'C' => Color::rgb(37, 92, 153),
        'G' => Color::rgb(247, 179, 43),
        'T' | 'U' => Color::rgb(214, 40, 57),
        _ => Color::rgb(60, 60, 60),
    }
}

fn amino_color(c: char) -> Color {
    match c.to_ascii_uppercase() {
        'D' | 'E' => Color::rgb(214, 40, 57),
        'K' | 'R' | 'H' => Color::rgb(37, 92, 153),
        'S' | 'T' | 'Y' | 'C' | 'N' | 'Q' | 'G' => Color::rgb(16, 150, 72),
        _ => Color::rgb(40, 40, 40),
    }
}

/// A sequence logo: at each position the letters stacked by frequency,
/// the stack height the information content in bits. Created by
/// [`Axes::sequence_logo`].
#[derive(Clone, Debug)]
pub struct SequenceLogo {
    /// Per position: (letter, height in bits), smallest first.
    pub(crate) columns: Vec<Vec<(char, f64)>>,
    pub(crate) protein: bool,
}

/// Letter heights of a sequence logo from aligned sequences of equal length
/// (gaps `-` and `.` ignored), with the small-sample correction.
pub fn logo_heights(sequences: &[&str], protein: bool) -> Vec<Vec<(char, f64)>> {
    let len = sequences
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0);
    let alphabet: f64 = if protein { 20.0 } else { 4.0 };
    let max_bits = alphabet.log2();
    (0..len)
        .map(|i| {
            let mut counts: Vec<(char, f64)> = Vec::new();
            for s in sequences {
                if let Some(c) = s.chars().nth(i) {
                    let c = c.to_ascii_uppercase();
                    if c == '-' || c == '.' || c.is_whitespace() {
                        continue;
                    }
                    match counts.iter_mut().find(|(k, _)| *k == c) {
                        Some(e) => e.1 += 1.0,
                        None => counts.push((c, 1.0)),
                    }
                }
            }
            let n: f64 = counts.iter().map(|c| c.1).sum();
            if n == 0.0 {
                return Vec::new();
            }
            let entropy: f64 = counts.iter().map(|(_, k)| -(k / n) * (k / n).log2()).sum();
            let correction = (alphabet - 1.0) / (2.0 * std::f64::consts::LN_2 * n);
            let info = (max_bits - (entropy + correction)).max(0.0);
            let mut col: Vec<(char, f64)> =
                counts.iter().map(|(c, k)| (*c, k / n * info)).collect();
            col.sort_by(|a, b| a.1.total_cmp(&b.1));
            col
        })
        .collect()
}

impl Series for SequenceLogo {
    fn extent(&self) -> Option<Extent> {
        let top = if self.protein { 20f64.log2() } else { 2.0 };
        let mut e = Extent::point(0.5, 0.0).include(self.columns.len() as f64 + 0.5, top);
        e.sticky_y_min = true;
        e.sticky_y_max = true;
        e.sticky_x_min = true;
        e.sticky_x_max = true;
        Some(e)
    }

    fn draw(&self, ctx: &mut Context) {
        let Some(font) = ctx.fonts.font.clone() else {
            return;
        };
        for (i, col) in self.columns.iter().enumerate() {
            let x = i as f64 + 1.0;
            let mut y = 0.0;
            for &(c, h) in col {
                if h <= 0.0 {
                    continue;
                }
                let (l, t) = ctx.point(x - 0.45, y + h);
                let (r, b) = ctx.point(x + 0.45, y);
                y += h;
                if (b - t).abs() < 0.3 {
                    continue;
                }
                let color = if self.protein {
                    amino_color(c)
                } else {
                    letter_color(c)
                };
                if let Some(cmds) = stretched_glyph(&font, c, l, t, r - l, b - t) {
                    ctx.push(Node::Path {
                        cmds,
                        fill: Some(color),
                        stroke: None,
                    });
                }
            }
        }
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let i = (x.value(px).round() as usize).checked_sub(1)?;
        let col = self.columns.get(i)?;
        let total: f64 = col.iter().map(|c| c.1).sum();
        let text = col
            .iter()
            .rev()
            .map(|(c, h)| format!("{c} {:.0} %", h / total.max(1e-12) * 100.0))
            .collect::<Vec<_>>()
            .join(", ");
        Some(Hit {
            px: x.px(i as f64 + 1.0),
            py: y.px(total),
            label: format!("position {}: {} bits ({text})", i + 1, short(total)),
            distance: (py - y.px(total / 2.0)).abs() * 0.0,
        })
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The outline of `c` stretched to fill the rectangle (x, y, w, h) exactly
/// (logo letters), as a path.
fn stretched_glyph(
    font: &ab_glyph::FontArc,
    c: char,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Option<Vec<PathCmd>> {
    let outline = font.outline(font.glyph_id(c))?;
    let b = outline.bounds;
    let (x0, x1) = (b.min.x.min(b.max.x) as f64, b.min.x.max(b.max.x) as f64);
    let (y0, y1) = (b.min.y.min(b.max.y) as f64, b.min.y.max(b.max.y) as f64);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let map = |p: ab_glyph::Point| {
        (
            x + (p.x as f64 - x0) / (x1 - x0) * w,
            y + (y1 - p.y as f64) / (y1 - y0) * h,
        )
    };
    let mut cmds = Vec::with_capacity(outline.curves.len() + 8);
    let mut last: Option<ab_glyph::Point> = None;
    for curve in &outline.curves {
        let (start, end) = match curve {
            OutlineCurve::Line(a, b) => (*a, *b),
            OutlineCurve::Quad(a, _, c) => (*a, *c),
            OutlineCurve::Cubic(a, _, _, d) => (*a, *d),
        };
        if last.is_none_or(|l| (l.x - start.x).abs() > 1e-3 || (l.y - start.y).abs() > 1e-3) {
            if last.is_some() {
                cmds.push(PathCmd::Close);
            }
            let (sx, sy) = map(start);
            cmds.push(PathCmd::MoveTo(sx, sy));
        }
        match curve {
            OutlineCurve::Line(_, b) => {
                let (bx, by) = map(*b);
                cmds.push(PathCmd::LineTo(bx, by));
            }
            OutlineCurve::Quad(a, b, c) => {
                // Quadratic → cubic: control points 2/3 of the way.
                let (p0, p1, p2) = (map(*a), map(*b), map(*c));
                let c1 = (
                    p0.0 + 2.0 / 3.0 * (p1.0 - p0.0),
                    p0.1 + 2.0 / 3.0 * (p1.1 - p0.1),
                );
                let c2 = (
                    p2.0 + 2.0 / 3.0 * (p1.0 - p2.0),
                    p2.1 + 2.0 / 3.0 * (p1.1 - p2.1),
                );
                cmds.push(PathCmd::CubicTo(c1.0, c1.1, c2.0, c2.1, p2.0, p2.1));
            }
            OutlineCurve::Cubic(_, b, c, d) => {
                let (b, c, d) = (map(*b), map(*c), map(*d));
                cmds.push(PathCmd::CubicTo(b.0, b.1, c.0, c.1, d.0, d.1));
            }
        }
        last = Some(end);
    }
    if last.is_some() {
        cmds.push(PathCmd::Close);
    }
    Some(cmds)
}

// ----------------------------------------------------------- gene track

/// A gene (or transcript) on a genome track.
#[derive(Clone, Debug, PartialEq)]
pub struct Gene {
    /// Name shown above it.
    pub name: String,
    /// Start coordinate.
    pub start: f64,
    /// End coordinate.
    pub end: f64,
    /// `+` strand (true) or `-` strand.
    pub forward: bool,
    /// Exons as (start, end).
    pub exons: Vec<(f64, f64)>,
}

/// Genes as exon boxes joined by intron lines with strand chevrons, one row
/// each. Created by [`Axes::gene_track`].
#[derive(Clone, Debug)]
pub struct GeneTrack {
    pub(crate) genes: Vec<Gene>,
    pub(crate) color: Color,
}

impl GeneTrack {
    /// Color of the exons and lines.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
}

impl Series for GeneTrack {
    fn extent(&self) -> Option<Extent> {
        let (lo, hi) = self
            .genes
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), g| {
                (a.min(g.start), b.max(g.end))
            });
        lo.is_finite()
            .then(|| Extent::point(lo, -0.6).include(hi, self.genes.len() as f64 - 0.4))
    }

    fn draw(&self, ctx: &mut Context) {
        let line = Stroke::new(self.color, 1.4);
        let size = ctx.theme.annotation_size;
        for (row, g) in self.genes.iter().enumerate() {
            let y = row as f64;
            let (x0, yc) = ctx.point(g.start, y);
            let (x1, _) = ctx.point(g.end, y);
            ctx.polyline(&[(x0, yc), (x1, yc)], line.clone());
            // Strand chevrons every ~40 px along the gene.
            let n = (((x1 - x0).abs() / 40.0).floor() as usize).max(1);
            let d = if g.forward { 1.0 } else { -1.0 } * 3.5;
            for k in 0..n {
                let cx = x0 + (x1 - x0) * (k as f64 + 0.5) / n as f64;
                ctx.polyline(
                    &[(cx - d, yc - 3.5), (cx + d, yc), (cx - d, yc + 3.5)],
                    Stroke::new(self.color, 1.2),
                );
            }
            let half = (ctx.y.px(y - 0.18) - ctx.y.px(y + 0.18)).abs() / 2.0;
            for &(a, b) in &g.exons {
                let (ea, _) = ctx.point(a, y);
                let (eb, _) = ctx.point(b, y);
                ctx.rect(
                    ea.min(eb),
                    yc - half,
                    ea.max(eb),
                    yc + half,
                    Some(self.color),
                    None,
                );
            }
            ctx.text(
                (x0 + x1) / 2.0,
                yc - half - 3.0,
                format!("{} ({})", g.name, if g.forward { "+" } else { "−" }),
                size,
                ctx.theme.text,
                Anchor::Middle,
                Baseline::Bottom,
            );
        }
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let row = y.value(py).round();
        let g = self.genes.get(row as usize).filter(|_| row >= 0.0)?;
        let v = x.value(px);
        (v >= g.start && v <= g.end).then(|| Hit {
            px,
            py: y.px(row),
            label: format!(
                "{}: {}–{} ({} exons)",
                g.name,
                short(g.start),
                short(g.end),
                g.exons.len()
            ),
            distance: 0.0,
        })
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// -------------------------------------------------------- statistics

fn neg_log10(p: f64) -> f64 {
    -(p.max(1e-300)).log10()
}

/// Kaplan-Meier estimate of survival times with event flags (false =
/// censored): (times, survival, lower 95 %, upper 95 %) as steps starting at
/// (0, 1), and the censoring times with their survival.
#[allow(clippy::type_complexity)]
pub fn kaplan_meier(
    times: &[f64],
    events: &[bool],
) -> ((Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>), Vec<(f64, f64)>) {
    let mut d: Vec<(f64, bool)> = times
        .iter()
        .zip(events)
        .map(|(t, e)| (*t, *e))
        .filter(|(t, _)| t.is_finite())
        .collect();
    d.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    let (mut t, mut s, mut lo, mut hi) = (vec![0.0], vec![1.0], vec![1.0], vec![1.0]);
    let mut censored = Vec::new();
    let (mut surv, mut green) = (1.0f64, 0.0f64);
    let mut at_risk = d.len() as f64;
    let mut i = 0;
    while i < d.len() {
        let time = d[i].0;
        let (mut deaths, mut cens) = (0.0, 0.0);
        while i < d.len() && d[i].0 == time {
            if d[i].1 {
                deaths += 1.0;
            } else {
                cens += 1.0;
            }
            i += 1;
        }
        if deaths > 0.0 {
            surv *= 1.0 - deaths / at_risk;
            if at_risk > deaths {
                green += deaths / (at_risk * (at_risk - deaths));
            }
            // Log-log transformed interval (stays within 0..1).
            let (l, h) = if surv > 0.0 && surv < 1.0 {
                let se = green.sqrt() / surv.ln().abs();
                let c = 1.96 * se;
                (surv.powf(c.exp()), surv.powf((-c).exp()))
            } else {
                (surv, surv)
            };
            t.push(time);
            s.push(surv);
            lo.push(l);
            hi.push(h);
        }
        if cens > 0.0 {
            censored.push((time, surv));
        }
        at_risk -= deaths + cens;
    }
    ((t, s, lo, hi), censored)
}

/// Log-rank test of two survival samples: (χ², p).
pub fn logrank(a: (&[f64], &[bool]), b: (&[f64], &[bool])) -> (f64, f64) {
    let mut all: Vec<(f64, bool, usize)> =
        a.0.iter()
            .zip(a.1)
            .map(|(t, e)| (*t, *e, 0))
            .chain(b.0.iter().zip(b.1).map(|(t, e)| (*t, *e, 1)))
            .filter(|x| x.0.is_finite())
            .collect();
    all.sort_by(|x, y| x.0.total_cmp(&y.0));
    let (mut na, mut nb) = (a.0.len() as f64, b.0.len() as f64);
    let (mut o_minus_e, mut var) = (0.0, 0.0);
    let mut i = 0;
    while i < all.len() {
        let time = all[i].0;
        let (mut da, mut db, mut ca, mut cb) = (0.0, 0.0, 0.0, 0.0);
        while i < all.len() && all[i].0 == time {
            match (all[i].1, all[i].2) {
                (true, 0) => da += 1.0,
                (true, _) => db += 1.0,
                (false, 0) => ca += 1.0,
                (false, _) => cb += 1.0,
            }
            i += 1;
        }
        let (d, n) = (da + db, na + nb);
        if d > 0.0 && n > 1.0 {
            o_minus_e += da - d * na / n;
            var += d * (na / n) * (nb / n) * (n - d) / (n - 1.0);
        }
        na -= da + ca;
        nb -= db + cb;
    }
    let chi2 = if var > 0.0 {
        o_minus_e * o_minus_e / var
    } else {
        0.0
    };
    (chi2, erfc((chi2 / 2.0).sqrt()))
}

/// Four-parameter logistic fit of a dose-response curve:
/// response = bottom + (top − bottom) / (1 + (x / ic50)^hill), fitted by
/// Levenberg-Marquardt. Returns (bottom, top, ic50, hill).
pub fn fit_4pl(dose: &[f64], response: &[f64]) -> Option<(f64, f64, f64, f64)> {
    let pts: Vec<(f64, f64)> = dose
        .iter()
        .zip(response)
        .map(|(d, r)| (*d, *r))
        .filter(|(d, r)| *d > 0.0 && d.is_finite() && r.is_finite())
        .collect();
    if pts.len() < 4 {
        return None;
    }
    let (lo, hi) = pts
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
            (a.min(p.1), b.max(p.1))
        });
    let mid = pts
        .iter()
        .min_by(|a, b| {
            (a.1 - (lo + hi) / 2.0)
                .abs()
                .total_cmp(&(b.1 - (lo + hi) / 2.0).abs())
        })
        .map_or(1.0, |p| p.0);
    // Parameters: bottom, top, log10(ic50), hill.
    let increasing = {
        let first = pts.iter().min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
        let last = pts.iter().max_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
        last.1 > first.1
    };
    let mut p = [lo, hi, mid.log10(), if increasing { -1.0 } else { 1.0 }];
    let f =
        |p: &[f64; 4], x: f64| p[0] + (p[1] - p[0]) / (1.0 + 10f64.powf((x.log10() - p[2]) * p[3]));
    let sse = |p: &[f64; 4]| pts.iter().map(|(x, y)| (y - f(p, *x)).powi(2)).sum::<f64>();
    let mut lambda = 1e-2;
    let mut cost = sse(&p);
    for _ in 0..200 {
        // Jacobian by finite differences; normal equations (4 × 4).
        let mut jtj = [[0.0; 4]; 4];
        let mut jtr = [0.0; 4];
        for (x, y) in &pts {
            let r = y - f(&p, *x);
            let mut g = [0.0; 4];
            for (k, gk) in g.iter_mut().enumerate() {
                let mut q = p;
                let h = 1e-6 * p[k].abs().max(1e-3);
                q[k] += h;
                *gk = (f(&q, *x) - f(&p, *x)) / h;
            }
            for a in 0..4 {
                jtr[a] += g[a] * r;
                for b in 0..4 {
                    jtj[a][b] += g[a] * g[b];
                }
            }
        }
        let mut m = jtj;
        for (a, row) in m.iter_mut().enumerate() {
            row[a] *= 1.0 + lambda;
        }
        let Some(step) = solve4(m, jtr) else { break };
        let mut q = p;
        for k in 0..4 {
            q[k] += step[k];
        }
        let c = sse(&q);
        if c < cost {
            let done = (cost - c) < 1e-12 * cost.max(1e-300);
            p = q;
            cost = c;
            lambda = (lambda / 3.0).max(1e-9);
            if done {
                break;
            }
        } else {
            lambda *= 4.0;
        }
    }
    Some((p[0], p[1], 10f64.powf(p[2]), p[3]))
}

fn solve4(mut a: [[f64; 4]; 4], mut b: [f64; 4]) -> Option<[f64; 4]> {
    for c in 0..4 {
        let piv = (c..4).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[piv][c].abs() < 1e-300 {
            return None;
        }
        a.swap(c, piv);
        b.swap(c, piv);
        for r in c + 1..4 {
            let f = a[r][c] / a[c][c];
            for k in c..4 {
                a[r][k] -= f * a[c][k];
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = [0.0; 4];
    for r in (0..4).rev() {
        x[r] = (b[r] - (r + 1..4).map(|k| a[r][k] * x[k]).sum::<f64>()) / a[r][r];
    }
    Some(x)
}

// ---------------------------------------------------------- forest plot

/// Effect estimates with confidence intervals per study and a reference
/// line. Created by [`Axes::forest_plot`].
#[derive(Clone, Debug)]
pub struct ForestPlot {
    pub(crate) est: Vec<f64>,
    pub(crate) lo: Vec<f64>,
    pub(crate) hi: Vec<f64>,
    pub(crate) names: Vec<String>,
    pub(crate) null: f64,
    pub(crate) color: Color,
    pub(crate) summary: bool,
}

impl ForestPlot {
    /// Draws the last row as a pooled estimate (diamond).
    pub fn summary_last(&mut self, on: bool) -> &mut Self {
        self.summary = on;
        self
    }
    /// Marker and line color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
}

impl Series for ForestPlot {
    fn extent(&self) -> Option<Extent> {
        let (lo, hi) = self
            .lo
            .iter()
            .chain(&self.hi)
            .chain(std::iter::once(&self.null))
            .filter(|v| v.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        lo.is_finite()
            .then(|| Extent::point(lo, -0.5).include(hi, self.est.len() as f64 - 0.5))
    }

    fn draw(&self, ctx: &mut Context) {
        let n = self.est.len();
        let (top, bottom) = (ctx.rect.y, ctx.rect.bottom());
        let (nx, _) = ctx.point(self.null, 0.0);
        ctx.polyline(
            &[(nx, top), (nx, bottom)],
            Stroke::new(ctx.theme.text.with_alpha(0.6), 1.0).dashed(vec![4.0, 3.0]),
        );
        for i in 0..n {
            let y = i as f64;
            let (a, yc) = ctx.point(self.lo[i], y);
            let (b, _) = ctx.point(self.hi[i], y);
            let (e, _) = ctx.point(self.est[i], y);
            if !(a.is_finite() && b.is_finite() && e.is_finite()) {
                continue;
            }
            if self.summary && i + 1 == n {
                let h = 7.0;
                ctx.polygon(
                    &[(a, yc), (e, yc - h), (b, yc), (e, yc + h)],
                    Some(self.color),
                    None,
                );
                continue;
            }
            ctx.polyline(&[(a, yc), (b, yc)], Stroke::new(self.color, 1.5));
            for x in [a, b] {
                ctx.polyline(
                    &[(x, yc - 4.0), (x, yc + 4.0)],
                    Stroke::new(self.color, 1.5),
                );
            }
            if let Some(m) = Marker::Square.node(e, yc, 9.0, self.color, None) {
                ctx.push(m);
            }
        }
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let i = y.value(py).round();
        let k = i as usize;
        (i >= 0.0 && k < self.est.len()).then(|| Hit {
            px: x.px(self.est[k]),
            py: y.px(i),
            label: format!(
                "{}: {} [{}, {}]",
                self.names[k],
                short(self.est[k]),
                short(self.lo[k]),
                short(self.hi[k])
            ),
            distance: (px - x.px(self.est[k])).abs() * 0.0,
        })
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// --------------------------------------------------------------- Axes

impl Axes {
    /// Sequence logo of aligned sequences (DNA/RNA, or protein when any
    /// letter is not ACGTUN): letters stacked by frequency at each position,
    /// stack height = information content in bits.
    pub fn sequence_logo(&mut self, sequences: &[&str]) -> &mut SequenceLogo {
        let protein = sequences
            .iter()
            .flat_map(|s| s.chars())
            .any(|c| !"ACGTUN-. ".contains(c.to_ascii_uppercase()));
        let columns = logo_heights(sequences, protein);
        let len = columns.len();
        self.x.label.get_or_insert_with(|| "position".into());
        self.y.label.get_or_insert_with(|| "bits".into());
        self.x.grid = Some(false);
        let step = (len / 20).max(1);
        self.x.ticks = TickSpec::At(
            (1..=len)
                .filter(|i| (i - 1) % step == 0)
                .map(|i| i as f64)
                .collect(),
        );
        self.add(SequenceLogo { columns, protein })
    }

    /// Gene track: each gene on its own row with exon boxes, intron lines,
    /// strand chevrons and its name.
    pub fn gene_track(&mut self, genes: &[Gene]) -> &mut GeneTrack {
        self.y.hidden();
        self.y.grid = Some(false);
        self.y.inverted = true;
        self.x.label.get_or_insert_with(|| "position [bp]".into());
        self.add(GeneTrack {
            genes: genes.to_vec(),
            color: Color::rgb(36, 82, 140),
        })
    }

    /// Manhattan plot of a genome-wide association study: −log10 p of each
    /// variant along the genome, chromosomes in alternating colors, with the
    /// genome-wide (5 × 10⁻⁸) and suggestive (10⁻⁵) thresholds.
    pub fn manhattan<C: ToString>(
        &mut self,
        chromosomes: impl IntoIterator<Item = C>,
        positions: impl IntoValues,
        p_values: impl IntoValues,
    ) -> &mut Self {
        let chrom: Vec<String> = chromosomes.into_iter().map(|c| c.to_string()).collect();
        let (pos, p) = (positions.into_values(), p_values.into_values());
        let n = chrom.len().min(pos.len()).min(p.len());
        // Chromosome order of first appearance; each starts after the last.
        let mut order: Vec<String> = Vec::new();
        let mut span: Vec<f64> = Vec::new();
        for i in 0..n {
            match order.iter().position(|c| *c == chrom[i]) {
                Some(k) => span[k] = span[k].max(pos[i]),
                None => {
                    order.push(chrom[i].clone());
                    span.push(pos[i]);
                }
            }
        }
        let mut offset = vec![0.0; order.len()];
        for k in 1..order.len() {
            offset[k] = offset[k - 1] + span[k - 1];
        }
        let colors = [Color::rgb(36, 82, 140), Color::rgb(122, 168, 214)];
        let mut groups: [(Vec<f64>, Vec<f64>); 2] = Default::default();
        for i in 0..n {
            let k = order.iter().position(|c| *c == chrom[i]).unwrap_or(0);
            groups[k % 2].0.push(offset[k] + pos[i]);
            groups[k % 2].1.push(neg_log10(p[i]));
        }
        for (g, c) in groups.into_iter().zip(colors) {
            if !g.0.is_empty() {
                self.scatter(g.0, g.1).color(c).size(4.0);
            }
        }
        self.hline(neg_log10(5e-8)).color(UP).width(1.2);
        self.hline(5.0).color(Color::rgb(120, 120, 120)).width(1.0);
        // Many chromosomes: from the 10th on, every other one is labelled.
        let crowded = order.len() > 14;
        let ticks: Vec<(f64, String)> = order
            .iter()
            .enumerate()
            .filter(|(k, _)| !crowded || *k < 9 || k % 2 == 0)
            .map(|(k, c)| (offset[k] + span[k] / 2.0, c.clone()))
            .collect();
        self.x.tick_rotation = Some(0.0);
        self.x.ticks = TickSpec::Labeled(ticks);
        self.x.grid = Some(false);
        self.x.label.get_or_insert_with(|| "chromosome".into());
        self.y
            .label
            .get_or_insert_with(|| "$-\\log_{10}(p)$".into());
        self
    }

    /// Q-Q plot of p-values against the uniform distribution (−log10
    /// scale) with the genomic inflation factor λ in the legend.
    pub fn qq_pvalues(&mut self, p_values: impl IntoValues) -> &mut Scatter {
        let mut p: Vec<f64> = p_values
            .into_values()
            .into_iter()
            .filter(|v| *v > 0.0 && *v <= 1.0)
            .collect();
        p.sort_by(f64::total_cmp);
        let n = p.len();
        let expected: Vec<f64> = (0..n)
            .map(|i| neg_log10((i as f64 + 0.5) / n as f64))
            .collect();
        let observed: Vec<f64> = p.iter().map(|v| neg_log10(*v)).collect();
        // λ: median χ²(1) of the p-values over its expected median 0.455.
        let lambda = if n > 0 {
            let med = p[n / 2];
            let z = crate::plot::series::distribution::normal_quantile(1.0 - med / 2.0);
            z * z / 0.454_936_4
        } else {
            f64::NAN
        };
        let top = expected.first().copied().unwrap_or(1.0);
        self.line([0.0, top], [0.0, top]).color(UP).width(1.2);
        self.x
            .label
            .get_or_insert_with(|| "expected $-\\log_{10}(p)$".into());
        self.y
            .label
            .get_or_insert_with(|| "observed $-\\log_{10}(p)$".into());
        let s = self.scatter(expected, observed);
        s.color(Color::rgb(36, 82, 140))
            .size(4.0)
            .label(format!("λ = {lambda:.3}"));
        s
    }

    /// Volcano plot of differential expression: log2 fold change against
    /// −log10 p, significant up (red) and down (blue) genes beyond the
    /// thresholds, and the names of the `label_top` most significant ones.
    pub fn volcano(
        &mut self,
        names: impl IntoLabels,
        log2_fc: impl IntoValues,
        p_values: impl IntoValues,
        fc_threshold: f64,
        p_threshold: f64,
        label_top: usize,
    ) -> &mut Self {
        let (names, fc, p) = (
            names.into_labels(),
            log2_fc.into_values(),
            p_values.into_values(),
        );
        let n = fc.len().min(p.len());
        let mut groups: [(Vec<f64>, Vec<f64>); 3] = Default::default();
        let mut sig: Vec<usize> = Vec::new();
        for i in 0..n {
            let y = neg_log10(p[i]);
            let g = if p[i] < p_threshold && fc[i] >= fc_threshold {
                sig.push(i);
                1
            } else if p[i] < p_threshold && fc[i] <= -fc_threshold {
                sig.push(i);
                2
            } else {
                0
            };
            groups[g].0.push(fc[i]);
            groups[g].1.push(y);
        }
        let labels = ["not significant", "up", "down"];
        for (k, (g, c)) in groups.into_iter().zip([NS, UP, DOWN]).enumerate() {
            if !g.0.is_empty() {
                let count = g.0.len();
                self.scatter(g.0, g.1)
                    .color(c)
                    .size(5.0)
                    .opacity(if k == 0 { 0.5 } else { 0.85 })
                    .label(format!("{} ({count})", labels[k]));
            }
        }
        let guide = Color::rgb(130, 130, 130);
        self.hline(neg_log10(p_threshold)).color(guide).width(1.0);
        self.vline(fc_threshold).color(guide).width(1.0);
        self.vline(-fc_threshold).color(guide).width(1.0);
        sig.sort_by(|a, b| p[*a].total_cmp(&p[*b]));
        // Labels beside their points, pushed apart vertically when they
        // would overlap (per side), with room for them at the edges.
        let ymax = (0..n).map(|i| neg_log10(p[i])).fold(0.0, f64::max).max(1.0);
        let gap = ymax * 0.045;
        let mut placed: Vec<(bool, f64)> = Vec::new();
        for &i in sig.iter().take(label_top) {
            let Some(name) = names.get(i) else { continue };
            let right = fc[i] >= 0.0;
            let mut y = neg_log10(p[i]);
            while placed
                .iter()
                .any(|(r, py)| *r == right && (py - y).abs() < gap)
            {
                y -= gap;
            }
            placed.push((right, y));
            self.text(fc[i], y, format!(" {name} "))
                .size(10.0)
                .anchor(if right { Anchor::Start } else { Anchor::End })
                .background(Color::rgba(255, 255, 255, 0.75));
        }
        if label_top > 0 {
            self.x.margin = 0.14;
        }
        self.x
            .label
            .get_or_insert_with(|| "$\\log_2$ fold change".into());
        self.y
            .label
            .get_or_insert_with(|| "$-\\log_{10}(p)$".into());
        self
    }

    /// MA plot: mean expression (A, log scale) against log2 fold change (M),
    /// significant genes colored.
    pub fn ma_plot(
        &mut self,
        mean_expression: impl IntoValues,
        log2_fc: impl IntoValues,
        significant: &[bool],
    ) -> &mut Self {
        let (a, m) = (mean_expression.into_values(), log2_fc.into_values());
        let mut groups: [(Vec<f64>, Vec<f64>); 2] = Default::default();
        for i in 0..a.len().min(m.len()) {
            let k = usize::from(significant.get(i).copied().unwrap_or(false));
            groups[k].0.push(a[i]);
            groups[k].1.push(m[i]);
        }
        for (k, (g, c)) in groups.into_iter().zip([NS, UP]).enumerate() {
            if !g.0.is_empty() {
                self.scatter(g.0, g.1)
                    .color(c)
                    .size(4.0)
                    .opacity(if k == 0 { 0.5 } else { 0.85 })
                    .label(if k == 0 {
                        "not significant"
                    } else {
                        "significant"
                    });
            }
        }
        self.hline(0.0).color(Color::rgb(90, 90, 90)).width(1.0);
        self.x.scale = Scale::Log(10.0);
        self.x.label.get_or_insert_with(|| "mean expression".into());
        self.y
            .label
            .get_or_insert_with(|| "$\\log_2$ fold change".into());
        self
    }

    /// A 2D embedding (UMAP, t-SNE, PCA) of cells colored by cluster, each
    /// cluster named at its centre.
    pub fn embedding(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        clusters: impl IntoLabels,
    ) -> &mut Self {
        let (x, y, c) = (x.into_values(), y.into_values(), clusters.into_labels());
        let mut names: Vec<String> = Vec::new();
        let mut groups: Vec<(Vec<f64>, Vec<f64>)> = Vec::new();
        for i in 0..x.len().min(y.len()).min(c.len()) {
            let k = match names.iter().position(|n| *n == c[i]) {
                Some(k) => k,
                None => {
                    names.push(c[i].clone());
                    groups.push(Default::default());
                    names.len() - 1
                }
            };
            groups[k].0.push(x[i]);
            groups[k].1.push(y[i]);
        }
        let palette = if names.len() > 10 {
            crate::plot::color::Palette::Tab20.colors()
        } else {
            crate::plot::color::Palette::Tab10.colors()
        };
        let size = if x.len() > 20_000 {
            2.0
        } else if x.len() > 2_000 {
            3.0
        } else {
            5.0
        };
        for (k, (gx, gy)) in groups.iter().enumerate() {
            let color = palette[k % palette.len()];
            self.scatter(gx.clone(), gy.clone())
                .color(color)
                .size(size)
                .opacity(0.8);
        }
        for (k, (gx, gy)) in groups.iter().enumerate() {
            // Label at the median, which lies inside the cluster.
            let med = |v: &[f64]| {
                let mut s = v.to_vec();
                s.sort_by(f64::total_cmp);
                s[s.len() / 2]
            };
            self.text(med(gx), med(gy), names[k].clone())
                .size(12.0)
                .anchor(Anchor::Middle)
                .bold(true)
                .background(Color::rgba(255, 255, 255, 0.8));
        }
        self.x.label.get_or_insert_with(|| "UMAP 1".into());
        self.y.label.get_or_insert_with(|| "UMAP 2".into());
        self.x.tick_labels(false);
        self.y.tick_labels(false);
        self.legend(crate::plot::LegendPosition::Hidden);
        self
    }

    /// Dose-response curve: the measurements and a four-parameter logistic
    /// fit on a log dose axis, with the IC50 (EC50) marked and in the legend.
    pub fn dose_response(&mut self, dose: impl IntoValues, response: impl IntoValues) -> &mut Line {
        let (d, r) = (dose.into_values(), response.into_values());
        let color = self.next_color();
        self.x.scale = Scale::Log(10.0);
        self.x.label.get_or_insert_with(|| "concentration".into());
        self.y.label.get_or_insert_with(|| "response".into());
        self.scatter(d.clone(), r.clone()).color(color).size(7.0);
        let fit = fit_4pl(&d, &r);
        let (lo, hi) = d
            .iter()
            .filter(|v| **v > 0.0)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        let (xs, ys, label) = match fit {
            Some((bottom, top, ic50, hill)) if lo.is_finite() => {
                let xs: Vec<f64> = (0..=200)
                    .map(|i| lo * (hi / lo).powf(i as f64 / 200.0))
                    .collect();
                let ys = xs
                    .iter()
                    .map(|x| bottom + (top - bottom) / (1.0 + (x / ic50).powf(hill)))
                    .collect();
                self.vline(ic50).color(color.with_alpha(0.6)).width(1.0);
                (
                    xs,
                    ys,
                    format!("IC50 = {}, Hill = {:.2}", short(ic50), hill.abs()),
                )
            }
            _ => (Vec::new(), Vec::new(), "no fit".to_string()),
        };
        let l = self.line(xs, ys);
        l.color(color).width(2.0).label(label);
        l.decimate = false;
        l
    }

    /// Kaplan-Meier survival curves of `(name, times, events)` groups (event
    /// false = censored): steps with 95 % bands and censoring marks; with
    /// two groups the log-rank p-value is added to the legend.
    pub fn kaplan_meier<L: ToString>(
        &mut self,
        groups: impl IntoIterator<Item = (L, Vec<f64>, Vec<bool>)>,
    ) -> &mut Self {
        let groups: Vec<(String, Vec<f64>, Vec<bool>)> = groups
            .into_iter()
            .map(|(n, t, e)| (n.to_string(), t, e))
            .collect();
        for (name, t, e) in &groups {
            let color = self.next_color();
            let ((ts, s, lo, hi), cens) = kaplan_meier(t, e);
            self.fill_between(&ts, lo, hi)
                .color(color)
                .opacity(0.15)
                .step(Step::Post);
            let l = self.line(&ts, &s);
            l.color(color)
                .width(2.0)
                .step(Step::Post)
                .label(format!("{name} (n = {})", t.len()));
            l.decimate = false;
            if !cens.is_empty() {
                let (cx, cy): (Vec<f64>, Vec<f64>) = cens.into_iter().unzip();
                self.scatter(cx, cy)
                    .marker(Marker::Plus)
                    .color(color)
                    .size(7.0);
            }
        }
        if groups.len() == 2 {
            let (_, p) = logrank((&groups[0].1, &groups[0].2), (&groups[1].1, &groups[1].2));
            let p = if p < 1e-4 {
                format!("{p:.1e}")
            } else {
                format!("{p:.4}")
            };
            self.text(0.0, 0.05, format!("  log-rank p = {p}"))
                .size(11.0);
        }
        self.y_limits(0.0, 1.02);
        self.x.label.get_or_insert_with(|| "time".into());
        self.y
            .label
            .get_or_insert_with(|| "survival probability".into());
        self
    }

    /// Forest plot: one row per study with its estimate and confidence
    /// interval, and a dashed line at the null value (0 for differences, 1
    /// for ratios; use a log x-axis for ratios).
    pub fn forest_plot(
        &mut self,
        names: impl IntoLabels,
        estimate: impl IntoValues,
        low: impl IntoValues,
        high: impl IntoValues,
        null: f64,
    ) -> &mut ForestPlot {
        let names = names.into_labels();
        self.y.categories(names.clone());
        self.y.inverted = true;
        self.y.grid = Some(false);
        let color = Color::rgb(36, 82, 140);
        self.add(ForestPlot {
            est: estimate.into_values(),
            lo: low.into_values(),
            hi: high.into_values(),
            names,
            null,
            color,
            summary: false,
        })
    }
}

impl Figure {
    /// Clustermap: a heatmap of `data[row][col]` with rows and columns
    /// reordered by hierarchical clustering and their dendrograms on the
    /// left and on top (gene expression, correlations).
    pub fn clustermap(
        &mut self,
        row_names: impl IntoLabels,
        col_names: impl IntoLabels,
        data: &[Vec<f64>],
    ) -> &mut Self {
        let (rn, cn) = (row_names.into_labels(), col_names.into_labels());
        let cols = data.iter().map(|r| r.len()).max().unwrap_or(0);
        let columns: Vec<Vec<f64>> = (0..cols)
            .map(|c| {
                data.iter()
                    .map(|r| r.get(c).copied().unwrap_or(0.0))
                    .collect()
            })
            .collect();
        let (rlinks, rorder) = cluster(data);
        let (clinks, corder) = cluster(&columns);
        let z: Vec<Vec<f64>> = rorder
            .iter()
            .map(|&r| {
                corder
                    .iter()
                    .map(|&c| data[r].get(c).copied().unwrap_or(f64::NAN))
                    .collect()
            })
            .collect();
        let name = |v: &[String], i: usize| v.get(i).cloned().unwrap_or_else(|| i.to_string());
        self.grid(6, 6).spacing(4.0);
        let line = Color::rgb(70, 70, 70);
        let ax = self.span(0, 1, 1, 5);
        ax.x.hidden();
        ax.y.hidden();
        ax.x.grid = Some(false);
        ax.y.grid = Some(false);
        ax.frame = false;
        ax.add(Dendrogram {
            links: clinks,
            color: line,
            width: 1.2,
            order: corder.clone(),
            horizontal: false,
        });
        let ax = self.span(1, 0, 5, 1);
        ax.x.hidden();
        ax.y.hidden();
        ax.x.grid = Some(false);
        ax.y.grid = Some(false);
        ax.x.inverted = true;
        ax.frame = false;
        ax.add(Dendrogram {
            links: rlinks,
            color: line,
            width: 1.2,
            order: rorder.clone(),
            horizontal: true,
        });
        let ax = self.span(1, 1, 5, 5);
        let h = ax.heatmap_labeled(
            corder.iter().map(|&c| name(&cn, c)).collect::<Vec<_>>(),
            rorder.iter().map(|&r| name(&rn, r)).collect::<Vec<_>>(),
            z,
        );
        // Diverging colors centred on 0.
        let m = data
            .iter()
            .flatten()
            .filter(|v| v.is_finite())
            .fold(0.0f64, |a, v| a.max(v.abs()));
        h.colormap(Colormap::RdBu).colorbar("value");
        if m > 0.0 {
            h.range(-m, m);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_information_content() {
        let h = logo_heights(&["ACGT", "ACGA", "ACTT", "ACGT"], false);
        let total = |c: &Vec<(char, f64)>| c.iter().map(|x| x.1).sum::<f64>();
        // Fully conserved columns approach 2 bits (minus the small-sample
        // correction); mixed ones are lower.
        // 4 sequences: correction (4 − 1) / (2 ln 2 · 4) = 0.541 bits.
        assert!((total(&h[0]) - (2.0 - 3.0 / (8.0 * std::f64::consts::LN_2))).abs() < 1e-9);
        assert!(total(&h[2]) < total(&h[0]));
        assert_eq!(h[0].len(), 1);
    }

    #[test]
    fn survival_and_logrank() {
        let ((t, s, lo, hi), cens) = kaplan_meier(
            &[1.0, 2.0, 2.0, 3.0, 4.0],
            &[true, true, false, true, false],
        );
        assert_eq!(t, vec![0.0, 1.0, 2.0, 3.0]);
        assert!(
            (s[1] - 0.8).abs() < 1e-12 && (s[2] - 0.6).abs() < 1e-12 && (s[3] - 0.3).abs() < 1e-12
        );
        assert!(lo[2] < s[2] && hi[2] > s[2]);
        assert_eq!(cens.len(), 2);
        assert!((cens[0].1 - 0.6).abs() < 1e-12 && (cens[1].1 - 0.3).abs() < 1e-12);
        let a: Vec<f64> = (1..=20).map(|v| v as f64).collect();
        let b: Vec<f64> = (1..=20).map(|v| v as f64 * 3.0).collect();
        let e = vec![true; 20];
        let (chi2, p) = logrank((&a, &e), (&b, &e));
        assert!(chi2 > 5.0 && p < 0.05, "{chi2} {p}");
        let (_, p) = logrank((&a, &e), (&a, &e));
        assert!(p > 0.9);
    }

    #[test]
    fn four_parameter_fit() {
        let d: Vec<f64> = (0..12).map(|i| 10f64.powf(-3.0 + i as f64 * 0.5)).collect();
        let r: Vec<f64> = d
            .iter()
            .map(|x| 5.0 + 95.0 / (1.0 + (x / 0.2f64).powf(1.3)))
            .collect();
        let (bottom, top, ic50, hill) = fit_4pl(&d, &r).unwrap();
        assert!(
            (bottom - 5.0).abs() < 0.5 && (top - 100.0).abs() < 0.5,
            "{bottom} {top}"
        );
        assert!(
            (ic50 / 0.2 - 1.0).abs() < 0.02 && (hill - 1.3).abs() < 0.05,
            "{ic50} {hill}"
        );
    }

    #[test]
    fn charts_render() {
        let mut fig = Figure::new(800, 800);
        fig.grid(3, 3);
        fig.at(0, 0)
            .sequence_logo(&["TATAAT", "TATGAT", "TACAAT", "TATAAT"]);
        fig.at(0, 1).gene_track(&[Gene {
            name: "BRCA1".into(),
            start: 0.0,
            end: 100.0,
            forward: false,
            exons: vec![(0.0, 10.0), (40.0, 55.0), (90.0, 100.0)],
        }]);
        fig.at(0, 2)
            .manhattan(["1", "1", "2"], [10.0, 20.0, 5.0], [0.5, 1e-9, 1e-3]);
        fig.at(1, 0).volcano(
            ["a", "b", "c"],
            [2.0, -3.0, 0.1],
            [1e-6, 1e-4, 0.5],
            1.0,
            0.01,
            2,
        );
        fig.at(1, 1).kaplan_meier([
            ("A", vec![1.0, 2.0, 3.0], vec![true, false, true]),
            ("B", vec![2.0, 4.0, 6.0], vec![true, true, false]),
        ]);
        fig.at(1, 2)
            .forest_plot(
                ["s1", "s2", "pooled"],
                [0.8, 1.2, 0.95],
                [0.6, 0.9, 0.85],
                [1.1, 1.6, 1.05],
                1.0,
            )
            .summary_last(true);
        fig.at(2, 0)
            .dose_response([0.01, 0.1, 1.0, 10.0, 100.0], [98.0, 90.0, 50.0, 12.0, 4.0]);
        fig.at(2, 1).embedding(
            [0.0, 0.1, 5.0, 5.1],
            [0.0, 0.2, 5.0, 5.2],
            ["T", "T", "B", "B"],
        );
        fig.at(2, 2).qq_pvalues([0.5, 0.1, 0.01, 0.9, 0.3]);
        fig.to_pixels().unwrap();
        let mut c = Figure::new(600, 600);
        c.clustermap(
            ["g1", "g2", "g3"],
            ["s1", "s2"],
            &[vec![1.0, 2.0], vec![1.1, 2.1], vec![-3.0, 0.0]],
        );
        c.to_pixels().unwrap();
    }
}
