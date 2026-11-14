//! Charts for data science and machine learning: ROC and precision-recall
//! curves, confusion matrices, regression with a confidence band, LOESS,
//! 2D density contours and dendrograms.

use super::contour::Contour;
use super::grid::Heatmap;
use super::line::Line;
use super::scatter::Scatter;
use super::{short, Context, Extent, Hit, IntoLabels, IntoValues, Rect, Series};
use crate::plot::axes::Axes;
use crate::plot::axis::AxisMap;
use crate::plot::color::Color;
use crate::plot::scene::Stroke;
use crate::plot::style::Dash;
use std::any::Any;

/// ROC curve of binary `labels` (true = positive) by `scores` (higher =
/// more likely positive): (false positive rates, true positive rates, AUC).
pub fn roc(labels: &[bool], scores: &[f64]) -> (Vec<f64>, Vec<f64>, f64) {
    let mut idx: Vec<usize> = (0..labels.len().min(scores.len()))
        .filter(|&i| scores[i].is_finite())
        .collect();
    idx.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    let pos = idx.iter().filter(|&&i| labels[i]).count() as f64;
    let neg = idx.len() as f64 - pos;
    let (mut fpr, mut tpr) = (vec![0.0], vec![0.0]);
    let (mut tp, mut fp) = (0.0, 0.0);
    for (k, &i) in idx.iter().enumerate() {
        if labels[i] {
            tp += 1.0;
        } else {
            fp += 1.0;
        }
        // One point per distinct score (ties move diagonally).
        if k + 1 == idx.len() || scores[idx[k + 1]] != scores[i] {
            fpr.push(if neg > 0.0 { fp / neg } else { 0.0 });
            tpr.push(if pos > 0.0 { tp / pos } else { 0.0 });
        }
    }
    let auc = fpr
        .windows(2)
        .zip(tpr.windows(2))
        .map(|(f, t)| (f[1] - f[0]) * (t[1] + t[0]) / 2.0)
        .sum();
    (fpr, tpr, auc)
}

/// Precision-recall curve: (recalls, precisions, average precision).
pub fn precision_recall(labels: &[bool], scores: &[f64]) -> (Vec<f64>, Vec<f64>, f64) {
    let mut idx: Vec<usize> = (0..labels.len().min(scores.len()))
        .filter(|&i| scores[i].is_finite())
        .collect();
    idx.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    let pos = idx.iter().filter(|&&i| labels[i]).count() as f64;
    let (mut rec, mut prec) = (vec![0.0], vec![1.0]);
    let (mut tp, mut ap, mut last_r) = (0.0, 0.0, 0.0);
    for (k, &i) in idx.iter().enumerate() {
        if labels[i] {
            tp += 1.0;
        }
        if k + 1 == idx.len() || scores[idx[k + 1]] != scores[i] {
            let r = if pos > 0.0 { tp / pos } else { 0.0 };
            let p = tp / (k + 1) as f64;
            ap += (r - last_r) * p;
            last_r = r;
            rec.push(r);
            prec.push(p);
        }
    }
    (rec, prec, ap)
}

/// Confusion matrix `m[actual][predicted]` of class indices below `k`.
pub fn confusion(actual: &[usize], predicted: &[usize], k: usize) -> Vec<Vec<f64>> {
    let mut m = vec![vec![0.0; k]; k];
    for (&a, &p) in actual.iter().zip(predicted) {
        if a < k && p < k {
            m[a][p] += 1.0;
        }
    }
    m
}

/// LOESS (locally weighted linear regression, tricube weights) of `y` on
/// `x`, using the nearest `frac` of the points for each estimate; evaluated
/// at `points` x positions over the data range.
pub fn loess(x: &[f64], y: &[f64], frac: f64, points: usize) -> (Vec<f64>, Vec<f64>) {
    let mut p: Vec<(f64, f64)> = x
        .iter()
        .zip(y)
        .map(|(a, b)| (*a, *b))
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .collect();
    p.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = p.len();
    if n < 3 {
        return (
            p.iter().map(|q| q.0).collect(),
            p.iter().map(|q| q.1).collect(),
        );
    }
    let k = ((frac.clamp(0.05, 1.0) * n as f64).ceil() as usize).clamp(3, n);
    let (lo, hi) = (p[0].0, p[n - 1].0);
    let m = points.max(2);
    let mut xs = Vec::with_capacity(m);
    let mut ys = Vec::with_capacity(m);
    for j in 0..m {
        let x0 = lo + (hi - lo) * j as f64 / (m - 1) as f64;
        // The k nearest points: a window around x0 in the sorted data.
        let mut a = p.partition_point(|q| q.0 < x0).min(n - 1);
        let mut b = a;
        while b - a < k {
            let left = if a > 0 {
                x0 - p[a - 1].0
            } else {
                f64::INFINITY
            };
            let right = if b < n { p[b].0 - x0 } else { f64::INFINITY };
            if left <= right {
                a -= 1;
            } else {
                b += 1;
            }
        }
        let h = p[a..b]
            .iter()
            .map(|q| (q.0 - x0).abs())
            .fold(0.0, f64::max)
            .max(1e-12)
            * 1.0001;
        let (mut sw, mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for q in &p[a..b] {
            let u = (q.0 - x0).abs() / h;
            let w = (1.0 - u * u * u).powi(3);
            sw += w;
            sx += w * q.0;
            sy += w * q.1;
            sxx += w * q.0 * q.0;
            sxy += w * q.0 * q.1;
        }
        let d = sw * sxx - sx * sx;
        let v = if d.abs() > 1e-12 * sw * sxx.max(1e-300) {
            let slope = (sw * sxy - sx * sy) / d;
            (sy - slope * sx) / sw + slope * x0
        } else {
            sy / sw
        };
        xs.push(x0);
        ys.push(v);
    }
    (xs, ys)
}

/// A dendrogram of hierarchical clustering. Created by
/// [`Axes::dendrogram`].
#[derive(Clone, Debug)]
pub struct Dendrogram {
    /// Links: (x of the left child, height of the left child, x of the right
    /// child, height of the right child, merge height).
    pub(crate) links: Vec<(f64, f64, f64, f64, f64)>,
    pub(crate) color: Color,
    pub(crate) width: f64,
    /// Leaf indices in drawing order.
    pub order: Vec<usize>,
    /// Leaves along y (from the top), heights along x.
    pub(crate) horizontal: bool,
}

impl Dendrogram {
    /// Line color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
    /// Line width.
    pub fn width(&mut self, w: f64) -> &mut Self {
        self.width = w.max(0.1);
        self
    }
}

impl Dendrogram {
    /// Canvas position of (leaf position, height): leaves along x, or for a
    /// horizontal dendrogram along y from the top (like heatmap rows).
    fn at(&self, x: &AxisMap, y: &AxisMap, leaf: f64, height: f64) -> (f64, f64) {
        if self.horizontal {
            let n = self.order.len() as f64;
            (x.px_clamped(height), y.px_clamped(n - 1.0 - leaf))
        } else {
            (x.px_clamped(leaf), y.px_clamped(height))
        }
    }
}

impl Series for Dendrogram {
    fn extent(&self) -> Option<Extent> {
        let top = self.links.iter().map(|l| l.4).fold(0.0, f64::max);
        let n = self.order.len() as f64;
        let mut e = Extent::point(-0.5, 0.0).include(n - 0.5, top * 1.05);
        e.sticky_y_min = true;
        e.sticky_x_min = true;
        e.sticky_x_max = true;
        Some(if self.horizontal { e.transposed() } else { e })
    }

    fn draw(&self, ctx: &mut Context) {
        let s = Stroke::new(self.color, self.width);
        for &(x1, h1, x2, h2, h) in &self.links {
            let pts = [
                self.at(ctx.x, ctx.y, x1, h1),
                self.at(ctx.x, ctx.y, x1, h),
                self.at(ctx.x, ctx.y, x2, h),
                self.at(ctx.x, ctx.y, x2, h2),
            ];
            ctx.polyline(&pts, s.clone());
        }
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        self.links
            .iter()
            .map(|&(x1, _, x2, _, h)| {
                let (cx, cy) = self.at(x, y, (x1 + x2) / 2.0, h);
                Hit {
                    px: cx,
                    py: cy,
                    label: format!("merge at {}", short(h)),
                    distance: (cx - px).hypot(cy - py),
                }
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Average-linkage hierarchical clustering of the rows of `data`
/// (Euclidean distances): links and the leaf order.
#[allow(clippy::type_complexity)]
pub(crate) fn cluster(data: &[Vec<f64>]) -> (Vec<(f64, f64, f64, f64, f64)>, Vec<usize>) {
    let n = data.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    let dist = |a: &[f64], b: &[f64]| -> f64 {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            .sqrt()
    };
    // Clusters: members, height; distances between live clusters.
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut height = vec![0.0; n];
    let mut alive: Vec<bool> = vec![true; n];
    let mut d: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| dist(&data[i], &data[j])).collect())
        .collect();
    let mut merges: Vec<(usize, usize, f64)> = Vec::new();
    for _ in 1..n {
        let (mut bi, mut bj, mut bd) = (0, 0, f64::INFINITY);
        for i in 0..d.len() {
            if !alive[i] {
                continue;
            }
            for j in i + 1..d.len() {
                if alive[j] && d[i][j] < bd {
                    (bi, bj, bd) = (i, j, d[i][j]);
                }
            }
        }
        let new = d.len();
        let (si, sj) = (members[bi].len() as f64, members[bj].len() as f64);
        let mut row: Vec<f64> = (0..new)
            .map(|k| {
                if alive[k] && k != bi && k != bj {
                    (d[bi][k] * si + d[bj][k] * sj) / (si + sj)
                } else {
                    f64::INFINITY
                }
            })
            .collect();
        for (k, r) in d.iter_mut().enumerate() {
            r.push(row[k]);
        }
        row.push(f64::INFINITY);
        d.push(row);
        let m: Vec<usize> = members[bi].iter().chain(&members[bj]).copied().collect();
        members.push(m);
        height.push(bd);
        alive[bi] = false;
        alive[bj] = false;
        alive.push(true);
        merges.push((bi, bj, bd));
    }
    // Leaf order: depth-first through the merges from the root.
    let root = members.len() - 1;
    let mut order = Vec::with_capacity(n);
    let mut stack = vec![root];
    while let Some(c) = stack.pop() {
        if c < n {
            order.push(c);
        } else {
            let (a, b, _) = merges[c - n];
            stack.push(b);
            stack.push(a);
        }
    }
    let mut pos = vec![0.0; members.len()];
    for (k, &leaf) in order.iter().enumerate() {
        pos[leaf] = k as f64;
    }
    let mut links = Vec::with_capacity(merges.len());
    for (m, &(a, b, h)) in merges.iter().enumerate() {
        pos[n + m] = (pos[a] + pos[b]) / 2.0;
        links.push((pos[a], height[a], pos[b], height[b], h));
    }
    (links, order)
}

impl Axes {
    /// ROC curve of a classifier named `name` (its AUC is added to the
    /// legend), with the chance diagonal; call it once per model.
    pub fn roc_curve(&mut self, name: &str, labels: &[bool], scores: impl IntoValues) -> &mut Line {
        let (fpr, tpr, auc) = roc(labels, &scores.into_values());
        if self.series_count() == 0 {
            self.line([0.0, 1.0], [0.0, 1.0])
                .color(Color::rgb(160, 160, 160))
                .dash(Dash::Dashed)
                .width(1.0);
        }
        self.x
            .label
            .get_or_insert_with(|| "False positive rate".into());
        self.y
            .label
            .get_or_insert_with(|| "True positive rate".into());
        self.x_limits(0.0, 1.0).y_limits(0.0, 1.02);
        let l = self.line(fpr, tpr);
        l.label(format!("{name} (AUC = {auc:.3})")).width(2.0);
        l.decimate = false;
        l
    }

    /// Precision-recall curve of a classifier named `name` (its average
    /// precision is added to the legend), with the share of positives as
    /// the baseline.
    pub fn pr_curve(&mut self, name: &str, labels: &[bool], scores: impl IntoValues) -> &mut Line {
        let base = labels.iter().filter(|b| **b).count() as f64 / labels.len().max(1) as f64;
        let (r, p, ap) = precision_recall(labels, &scores.into_values());
        if self.series_count() == 0 {
            self.hline(base).color(Color::rgb(160, 160, 160));
        }
        self.x.label.get_or_insert_with(|| "Recall".into());
        self.y.label.get_or_insert_with(|| "Precision".into());
        self.x_limits(0.0, 1.0).y_limits(0.0, 1.02);
        let l = self.line(r, p);
        l.label(format!("{name} (AP = {ap:.3})"))
            .step(super::line::Step::Post)
            .width(2.0);
        l.decimate = false;
        l
    }

    /// Confusion matrix of class indices (rows: actual, columns: predicted),
    /// annotated; `normalize` shows each row as fractions.
    pub fn confusion_matrix(
        &mut self,
        classes: impl IntoLabels,
        actual: &[usize],
        predicted: &[usize],
        normalize: bool,
    ) -> &mut Heatmap {
        let names = classes.into_labels();
        let mut m = confusion(actual, predicted, names.len());
        if normalize {
            for row in &mut m {
                let s: f64 = row.iter().sum();
                if s > 0.0 {
                    row.iter_mut().for_each(|v| *v /= s);
                }
            }
        }
        self.x.label.get_or_insert_with(|| "Predicted".into());
        self.y.label.get_or_insert_with(|| "Actual".into());
        let h = self.heatmap_labeled(names.clone(), names, m);
        h.annotate(true)
            .colormap(crate::plot::color::Colormap::Blues)
            .decimals(if normalize { 2 } else { 0 });
        h
    }

    /// Scatter plot with a least-squares line and its 95 % confidence band
    /// (seaborn's `regplot`). Returns the points.
    pub fn regplot(&mut self, x: impl IntoValues, y: impl IntoValues) -> &mut Scatter {
        let (x, y) = (x.into_values(), y.into_values());
        let pts: Vec<(f64, f64)> = x
            .iter()
            .zip(&y)
            .map(|(a, b)| (*a, *b))
            .filter(|(a, b)| a.is_finite() && b.is_finite())
            .collect();
        let color = self.next_color();
        let n = pts.len() as f64;
        if n >= 3.0 {
            let mx = pts.iter().map(|p| p.0).sum::<f64>() / n;
            let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
            let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
            let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
            let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
            let icpt = my - slope * mx;
            let sse: f64 = pts.iter().map(|p| (p.1 - icpt - slope * p.0).powi(2)).sum();
            let se = (sse / (n - 2.0)).sqrt();
            let t = t_975(n - 2.0);
            let (lo, hi) = pts
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
                    (a.min(p.0), b.max(p.0))
                });
            let xs: Vec<f64> = (0..=100)
                .map(|i| lo + (hi - lo) * i as f64 / 100.0)
                .collect();
            let fit: Vec<f64> = xs.iter().map(|v| icpt + slope * v).collect();
            let half: Vec<f64> = xs
                .iter()
                .map(|v| t * se * (1.0 / n + (v - mx).powi(2) / sxx.max(f64::MIN_POSITIVE)).sqrt())
                .collect();
            let low: Vec<f64> = fit.iter().zip(&half).map(|(f, h)| f - h).collect();
            let high: Vec<f64> = fit.iter().zip(&half).map(|(f, h)| f + h).collect();
            self.fill_between(&xs, low, high).color(color).opacity(0.18);
            let ss_tot: f64 = pts.iter().map(|p| (p.1 - my).powi(2)).sum();
            let r2 = if ss_tot > 0.0 {
                1.0 - sse / ss_tot
            } else {
                1.0
            };
            self.line(&xs, fit).color(color).width(2.0).label(format!(
                "y = {}x + {} (R² = {r2:.3})",
                short(slope),
                short(icpt)
            ));
        }
        let s = self.scatter(x, y);
        s.color(color).opacity(0.7);
        s
    }

    /// LOESS smoothing curve (local linear regression over the nearest
    /// `frac` of the points; 0.3 is a common choice).
    pub fn loess(&mut self, x: impl IntoValues, y: impl IntoValues, frac: f64) -> &mut Line {
        let (xs, ys) = loess(&x.into_values(), &y.into_values(), frac, 200);
        let l = self.line(xs, ys);
        l.label(format!("LOESS ({frac})")).width(2.0);
        l.decimate = false;
        l
    }

    /// Contours of the 2D kernel density of the points (where they
    /// concentrate), filled; any number of points (binned on a grid).
    pub fn density_contour(&mut self, x: impl IntoValues, y: impl IntoValues) -> &mut Contour {
        let (x, y) = (x.into_values(), y.into_values());
        let (z, xr, yr) = kde2d(&x, &y, 96);
        let top = z.iter().flatten().copied().fold(0.0, f64::max);
        self.x.grid = Some(false);
        self.y.grid = Some(false);
        let c = self.contour(xr, yr, z);
        // From 5 % of the peak: the faint tails stay empty instead of tracing
        // noise.
        c.levels((0..8).map(|k| top * (0.05 + 0.9 * k as f64 / 7.0)))
            .filled(true)
            .colormap(crate::plot::color::Colormap::Blues)
            .line_color(Color::rgba(20, 40, 80, 0.35))
            .width(0.8);
        c.clear_below = true;
        c
    }

    /// Dendrogram of average-linkage hierarchical clustering of the rows of
    /// `data` (one feature vector per item); the x-axis shows the item
    /// names in clustered order (`Dendrogram::order`).
    pub fn dendrogram(&mut self, names: impl IntoLabels, data: &[Vec<f64>]) -> &mut Dendrogram {
        let names = names.into_labels();
        let (links, order) = cluster(data);
        let labels: Vec<String> = order
            .iter()
            .map(|&i| names.get(i).cloned().unwrap_or_else(|| i.to_string()))
            .collect();
        self.x.categories(labels);
        self.x.grid = Some(false);
        self.y.label.get_or_insert_with(|| "distance".into());
        let color = self.next_color();
        self.add(Dendrogram {
            links,
            color,
            width: 1.5,
            order,
            horizontal: false,
        })
    }
}

/// Two-sided 97.5 % quantile of Student's t with `df` degrees of freedom
/// (Cornish-Fisher expansion; exact enough for confidence bands).
fn t_975(df: f64) -> f64 {
    let z: f64 = 1.959_963_984_540_054;
    if df < 1.0 {
        return 12.706;
    }
    let (z3, z5, z7) = (z.powi(3), z.powi(5), z.powi(7));
    z + (z3 + z) / (4.0 * df)
        + (5.0 * z5 + 16.0 * z3 + 3.0 * z) / (96.0 * df * df)
        + (3.0 * z7 + 19.0 * z5 + 17.0 * z3 - 15.0 * z) / (384.0 * df.powi(3))
}

/// 2D Gaussian KDE on a `g × g` grid: linear binning, then a separable
/// blur (Scott's bandwidth per axis). Returns (z[row][col], x range, y range).
#[allow(clippy::type_complexity)]
fn kde2d(x: &[f64], y: &[f64], g: usize) -> (Vec<Vec<f64>>, (f64, f64), (f64, f64)) {
    let pts: Vec<(f64, f64)> = x
        .iter()
        .zip(y)
        .map(|(a, b)| (*a, *b))
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .collect();
    let n = pts.len().max(1) as f64;
    let sd = |v: Vec<f64>| -> (f64, f64, f64, f64) {
        let m = v.iter().sum::<f64>() / n;
        let s = (v.iter().map(|a| (a - m).powi(2)).sum::<f64>() / n).sqrt();
        let (lo, hi) = v
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                (a.min(*x), b.max(*x))
            });
        (m, s.max(1e-12), lo, hi)
    };
    let (_, sx, xlo, xhi) = sd(pts.iter().map(|p| p.0).collect());
    let (_, sy, ylo, yhi) = sd(pts.iter().map(|p| p.1).collect());
    let f = n.powf(-1.0 / 6.0);
    let (hx, hy) = (sx * f, sy * f);
    let xr = (xlo - 3.0 * hx, xhi + 3.0 * hx);
    let yr = (ylo - 3.0 * hy, yhi + 3.0 * hy);
    if pts.is_empty() || xr.1 <= xr.0 || yr.1 <= yr.0 {
        return (vec![vec![0.0; 2]; 2], (0.0, 1.0), (0.0, 1.0));
    }
    let (dx, dy) = (
        (xr.1 - xr.0) / (g - 1) as f64,
        (yr.1 - yr.0) / (g - 1) as f64,
    );
    let mut grid = vec![vec![0.0f64; g]; g];
    for &(a, b) in &pts {
        let (tx, ty) = ((a - xr.0) / dx, (b - yr.0) / dy);
        let (i, j) = (
            (tx.floor() as usize).min(g - 2),
            (ty.floor() as usize).min(g - 2),
        );
        let (fx, fy) = (tx - i as f64, ty - j as f64);
        grid[j][i] += (1.0 - fx) * (1.0 - fy);
        grid[j][i + 1] += fx * (1.0 - fy);
        grid[j + 1][i] += (1.0 - fx) * fy;
        grid[j + 1][i + 1] += fx * fy;
    }
    let kernel = |h: f64, d: f64| -> Vec<f64> {
        let r = ((3.0 * h / d).ceil() as usize).max(1);
        let k: Vec<f64> = (0..=2 * r)
            .map(|i| (-0.5 * ((i as f64 - r as f64) * d / h).powi(2)).exp())
            .collect();
        let s: f64 = k.iter().sum();
        k.into_iter().map(|v| v / s).collect()
    };
    let blur = |rows: Vec<Vec<f64>>, k: &[f64]| -> Vec<Vec<f64>> {
        let r = k.len() / 2;
        rows.iter()
            .map(|row| {
                (0..row.len())
                    .map(|i| {
                        k.iter()
                            .enumerate()
                            .filter_map(|(t, w)| {
                                (i + t)
                                    .checked_sub(r)
                                    .and_then(|j| row.get(j))
                                    .map(|v| v * w)
                            })
                            .sum()
                    })
                    .collect()
            })
            .collect()
    };
    let transpose = |m: Vec<Vec<f64>>| -> Vec<Vec<f64>> {
        (0..m[0].len())
            .map(|c| m.iter().map(|r| r[c]).collect())
            .collect()
    };
    let z = blur(grid, &kernel(hx, dx));
    let z = transpose(blur(transpose(z), &kernel(hy, dy)));
    let norm = 1.0 / (n * dx * dy);
    (
        z.into_iter()
            .map(|r| r.into_iter().map(|v| v * norm).collect())
            .collect(),
        xr,
        yr,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roc_and_pr() {
        let labels = [true, true, false, true, false, false];
        let scores = [0.9, 0.8, 0.7, 0.6, 0.3, 0.1];
        let (_, _, auc) = roc(&labels, &scores);
        assert!((auc - 8.0 / 9.0).abs() < 1e-12, "{auc}");
        let (_, _, auc) = roc(&[true, false], &[0.9, 0.1]);
        assert_eq!(auc, 1.0);
        let (r, p, ap) = precision_recall(&labels, &scores);
        assert_eq!(*r.last().unwrap(), 1.0);
        assert!(p[1] == 1.0 && ap > 0.8 && ap <= 1.0, "{ap}");
    }

    #[test]
    fn loess_follows_a_line() {
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|v| 2.0 * v + 1.0).collect();
        let (xs, ys) = loess(&x, &y, 0.3, 20);
        for (a, b) in xs.iter().zip(&ys) {
            assert!((b - (2.0 * a + 1.0)).abs() < 1e-6);
        }
    }

    #[test]
    fn clustering_groups_neighbours() {
        let data = vec![vec![0.0], vec![10.0], vec![0.5], vec![10.4]];
        let (links, order) = cluster(&data);
        assert_eq!(links.len(), 3);
        let p = |i: usize| order.iter().position(|&o| o == i).unwrap() as i64;
        assert_eq!((p(0) - p(2)).abs(), 1);
        assert_eq!((p(1) - p(3)).abs(), 1);
        assert!(links[2].4 > links[0].4);
        assert!((t_975(10.0) - 2.228).abs() < 0.01);
    }

    #[test]
    fn charts_render() {
        let mut fig = crate::plot::Figure::new(600, 600);
        fig.grid(2, 2);
        let labels: Vec<bool> = (0..200).map(|i| i % 3 == 0).collect();
        let scores: Vec<f64> = (0..200)
            .map(|i| if i % 3 == 0 { 0.6 } else { 0.4 } + ((i * 37) % 17) as f64 / 40.0)
            .collect();
        fig.at(0, 0).roc_curve("model", &labels, &scores);
        fig.at(0, 1)
            .confusion_matrix(["a", "b"], &[0, 0, 1, 1], &[0, 1, 1, 1], true);
        let x: Vec<f64> = (0..300).map(|i| (i as f64 * 0.37).sin()).collect();
        let y: Vec<f64> = (0..300).map(|i| (i as f64 * 0.11).cos()).collect();
        fig.at(1, 0).density_contour(&x, &y);
        fig.at(1, 1).regplot(&x, &y);
        fig.to_pixels().unwrap();
        assert!(fig.to_svg().contains("AUC"));
    }
}
