//! Model evaluation and explanation: SHAP summary (beeswarm), calibration
//! (reliability) curve, learning curve, cumulative gain and lift charts,
//! raincloud plot and UpSet plot.

use super::general::Shapes;
use super::line::Line;
use super::{ColorScale, Context, Extent, IntoValues, Series};
use crate::plot::axes::{Axes, AxesKind};
use crate::plot::color::{Color, Colormap};
use crate::plot::scene::{Anchor, Baseline, Node, Stroke};
use crate::plot::stats;
use crate::plot::style::{Dash, Marker};
use std::any::Any;

// ----- SHAP summary -----------------------------------------------------------

/// SHAP summary (beeswarm): per feature the SHAP value of every sample,
/// colored by the feature's value. Created by [`Axes::shap_summary`].
#[derive(Clone, Debug)]
pub struct ShapSummary {
    /// Rows from top to bottom: (feature index, x, color 0..1, y offset).
    pub(crate) points: Vec<(u32, f32, f32, f32)>,
    pub(crate) rows: usize,
    pub(crate) colormap: Colormap,
    pub(crate) size: f64,
    pub(crate) range: (f64, f64),
}

impl ShapSummary {
    /// Point size in pixels.
    pub fn size(&mut self, s: f64) -> &mut Self {
        self.size = s.max(1.0);
        self
    }
    /// Colormap from low to high feature value.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
}

impl Series for ShapSummary {
    fn extent(&self) -> Option<Extent> {
        let (a, b) = self.range;
        let pad = (b - a).abs().max(1e-12) * 0.04;
        a.is_finite()
            .then(|| Extent::point(a - pad, -0.5).include(b + pad, self.rows as f64 - 0.5))
    }
    fn draw(&self, ctx: &mut Context) {
        let (zx, _) = ctx.point(0.0, 0.0);
        ctx.polyline(
            &[(zx, ctx.rect.y), (zx, ctx.rect.bottom())],
            Stroke::new(ctx.theme.text.with_alpha(0.45), 1.0),
        );
        // Points grouped by 32 color levels: one sprite-drawn node each.
        const LEVELS: usize = 32;
        let mut groups: Vec<Vec<(f32, f32)>> = vec![Vec::new(); LEVELS];
        let (_, y0) = ctx.point(0.0, 0.0);
        let (_, y1) = ctx.point(0.0, 1.0);
        let row_h = (y1 - y0).abs();
        for &(row, x, c, dy) in &self.points {
            let (px, py) = ctx.point(x as f64, row as f64);
            let k = ((c * (LEVELS - 1) as f32).round() as usize).min(LEVELS - 1);
            groups[k].push((px as f32, (py + dy as f64 * row_h * 0.42) as f32));
        }
        for (k, offsets) in groups.into_iter().enumerate() {
            if offsets.is_empty() {
                continue;
            }
            let color = self.colormap.at(k as f64 / (LEVELS - 1) as f64);
            ctx.push(Node::Instances {
                template: Box::new(Node::Circle {
                    cx: 0.0,
                    cy: 0.0,
                    r: self.size / 2.0,
                    fill: Some(color),
                    stroke: None,
                }),
                offsets,
            });
        }
    }
    fn color_scale(&self) -> Option<ColorScale> {
        Some(ColorScale {
            colormap: self.colormap,
            min: 0.0,
            max: 1.0,
            label: Some("feature value (low → high)".into()),
        })
    }
    fn kind(&self) -> &'static str {
        "SHAP summary"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Vertical offsets (−1..1) that spread points with close x values, like
/// a beeswarm: points in the same x bin alternate up and down by their
/// rank in the bin, scaled by the bin's crowding.
fn swarm_offsets(x: &[f64], bins: usize) -> Vec<f32> {
    let (lo, hi) = x
        .iter()
        .filter(|v| v.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(*v), b.max(*v))
        });
    if !lo.is_finite() || hi <= lo {
        return vec![0.0; x.len()];
    }
    let bin = |v: f64| (((v - lo) / (hi - lo) * bins as f64) as usize).min(bins - 1);
    let mut count = vec![0u32; bins];
    for &v in x.iter().filter(|v| v.is_finite()) {
        count[bin(v)] += 1;
    }
    let most = (*count.iter().max().unwrap_or(&1)).max(1) as f32;
    let mut seen = vec![0u32; bins];
    x.iter()
        .map(|&v| {
            if !v.is_finite() {
                return 0.0;
            }
            let b = bin(v);
            let k = seen[b];
            seen[b] += 1;
            let spread = count[b] as f32 / most;
            // 0, +1, −1, +2, −2, … scaled into the bin's spread.
            let step = (k as f32 + 1.0) / 2.0;
            let side = if k.is_multiple_of(2) { 1.0 } else { -1.0 };
            (side * step.floor() / (count[b] as f32 / 2.0).max(1.0) * spread).clamp(-1.0, 1.0)
        })
        .collect()
}

// ----- raincloud ---------------------------------------------------------------

/// Raincloud plot: per group a half violin (the cloud), a box plot and
/// the raw points (the rain). Created by [`Axes::raincloud`].
#[derive(Clone, Debug)]
pub struct Raincloud {
    pub(crate) groups: Vec<Vec<f64>>,
    pub(crate) curves: Vec<(Vec<f64>, Vec<f64>)>,
    pub(crate) colors: Vec<Color>,
    pub(crate) jitter: Vec<Vec<f32>>,
}

impl Series for Raincloud {
    fn extent(&self) -> Option<Extent> {
        let (lo, hi) = self
            .curves
            .iter()
            .flat_map(|c| c.0.iter())
            .chain(self.groups.iter().flatten())
            .filter(|v| v.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        lo.is_finite()
            .then(|| Extent::point(lo, -0.5).include(hi, self.groups.len() as f64 - 0.5))
    }
    fn draw(&self, ctx: &mut Context) {
        for (g, values) in self.groups.iter().enumerate() {
            // Each cloud scaled to its own peak (shapes compare, heights not).
            let peak = self.curves[g]
                .1
                .iter()
                .copied()
                .fold(0.0, f64::max)
                .max(1e-300);
            let c = self.colors[g % self.colors.len()];
            let y = g as f64;
            // Cloud above the row centre (the axis is inverted: smaller y
            // is higher), scaled to the largest density.
            let (xs, ds) = &self.curves[g];
            let mut pts: Vec<(f64, f64)> = xs
                .iter()
                .zip(ds)
                .map(|(x, d)| ctx.point(*x, y - 0.05 - d / peak * 0.5))
                .collect();
            if let (Some(a), Some(b)) = (xs.first(), xs.last()) {
                pts.push(ctx.point(*b, y - 0.05));
                pts.push(ctx.point(*a, y - 0.05));
            }
            ctx.polygon(
                &pts,
                Some(c.with_alpha(0.75)),
                Some(Stroke::new(c.darken(0.25), 1.0)),
            );
            // Box plot on the row centre.
            let mut s: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
            s.sort_by(f64::total_cmp);
            if s.is_empty() {
                continue;
            }
            let q = |p: f64| stats::quantile_sorted(&s, p);
            let (q1, med, q3) = (q(0.25), q(0.5), q(0.75));
            let iqr = q3 - q1;
            let lo = s
                .iter()
                .copied()
                .find(|v| *v >= q1 - 1.5 * iqr)
                .unwrap_or(q1);
            let hi = s
                .iter()
                .rev()
                .copied()
                .find(|v| *v <= q3 + 1.5 * iqr)
                .unwrap_or(q3);
            let (a, y0) = ctx.point(q1, y + 0.02);
            let (b, y1) = ctx.point(q3, y + 0.1);
            let (wl, ym) = ctx.point(lo, y + 0.06);
            let (wh, _) = ctx.point(hi, y + 0.06);
            let line = Stroke::new(ctx.theme.text, 1.2);
            ctx.polyline(&[(wl, ym), (a, ym)], line.clone());
            ctx.polyline(&[(b, ym), (wh, ym)], line.clone());
            ctx.rect(
                a,
                y0,
                b,
                y1,
                Some(ctx.theme.plot_background),
                Some(line.clone()),
            );
            let (m, _) = ctx.point(med, y);
            ctx.polyline(&[(m, y0), (m, y1)], Stroke::new(ctx.theme.text, 2.0));
            // Rain below the box.
            let offsets: Vec<(f32, f32)> = values
                .iter()
                .zip(&self.jitter[g])
                .filter(|(v, _)| v.is_finite())
                .map(|(v, j)| {
                    let (px, py) = ctx.point(*v, y + 0.2 + *j as f64 * 0.22);
                    (px as f32, py as f32)
                })
                .collect();
            ctx.push(Node::Instances {
                template: Box::new(Node::Circle {
                    cx: 0.0,
                    cy: 0.0,
                    r: 2.2,
                    fill: Some(c.with_alpha(0.7)),
                    stroke: None,
                }),
                offsets,
            });
        }
    }
    fn kind(&self) -> &'static str {
        "raincloud"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- UpSet -------------------------------------------------------------------

/// UpSet plot: sizes of set intersections as bars over a dot matrix of the
/// sets involved, with set sizes on the left. Created by [`Axes::upset`].
#[derive(Clone, Debug)]
pub struct UpSet {
    pub(crate) sets: Vec<String>,
    pub(crate) set_sizes: Vec<usize>,
    /// (membership bit mask, count), largest first.
    pub(crate) intersections: Vec<(u64, usize)>,
    pub(crate) color: Color,
}

impl UpSet {
    /// Color of the bars and dots.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
}

impl Series for UpSet {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let r = ctx.rect;
        let (ns, ni) = (self.sets.len(), self.intersections.len());
        if ns == 0 || ni == 0 {
            return;
        }
        let ts = ctx.theme.tick_size;
        let name_w = self
            .sets
            .iter()
            .map(|s| ctx.text_width(s, ts))
            .fold(0.0, f64::max)
            + 10.0;
        let size_w = (r.w * 0.18).clamp(60.0, 160.0);
        let left = r.x + size_w + name_w;
        let col = (r.right() - left) / ni as f64;
        let row = (ts * 1.9).min(r.h * 0.45 / ns as f64);
        let matrix_top = r.bottom() - row * ns as f64;
        let bars_top = r.y + ts * 1.5;
        let bars_bottom = matrix_top - 6.0;
        let max_i = self
            .intersections
            .iter()
            .map(|i| i.1)
            .max()
            .unwrap_or(1)
            .max(1) as f64;
        let dark = self.color;
        let light = ctx.theme.text.mix(ctx.theme.plot_background, 0.85);
        // Row stripes.
        let mut stripes = Shapes::default();
        for s in (0..ns).step_by(2) {
            let y = matrix_top + s as f64 * row;
            stripes.rect(r.x + size_w, y, r.right(), y + row);
        }
        stripes.fill(ctx, ctx.theme.text.with_alpha(0.05));
        // Intersection bars with counts.
        let mut bars = Shapes::default();
        for (k, &(_, n)) in self.intersections.iter().enumerate() {
            let x = left + (k as f64 + 0.5) * col;
            let h = (bars_bottom - bars_top) * n as f64 / max_i;
            let w = (col * 0.6).min(28.0);
            bars.rect(x - w / 2.0, bars_bottom - h, x + w / 2.0, bars_bottom);
            ctx.text(
                x,
                bars_bottom - h - 3.0,
                n.to_string(),
                ts * 0.85,
                ctx.theme.text,
                Anchor::Middle,
                Baseline::Bottom,
            );
        }
        bars.fill(ctx, dark);
        // Dot matrix: members dark and joined, others light.
        let rad = (row * 0.32).min(col * 0.32).min(7.0);
        let mut joins = Shapes::default();
        let (mut on, mut off) = (Vec::new(), Vec::new());
        for (k, &(mask, _)) in self.intersections.iter().enumerate() {
            let x = left + (k as f64 + 0.5) * col;
            let members: Vec<usize> = (0..ns).filter(|s| mask >> s & 1 == 1).collect();
            for s in 0..ns {
                let y = matrix_top + (s as f64 + 0.5) * row;
                if mask >> s & 1 == 1 {
                    on.push((x as f32, y as f32))
                } else {
                    off.push((x as f32, y as f32))
                }
            }
            if let (Some(a), Some(b)) = (members.first(), members.last()) {
                if a != b {
                    let ya = matrix_top + (*a as f64 + 0.5) * row;
                    let yb = matrix_top + (*b as f64 + 0.5) * row;
                    joins.rect(x - 1.5, ya, x + 1.5, yb);
                }
            }
        }
        for (pts, c) in [(off, light), (on, dark)] {
            if !pts.is_empty() {
                ctx.push(Node::Instances {
                    template: Box::new(Node::Circle {
                        cx: 0.0,
                        cy: 0.0,
                        r: rad,
                        fill: Some(c),
                        stroke: None,
                    }),
                    offsets: pts,
                });
            }
        }
        joins.fill(ctx, dark);
        // Set names and set size bars (growing to the left).
        let max_s = self.set_sizes.iter().copied().max().unwrap_or(1).max(1) as f64;
        let mut sbars = Shapes::default();
        for (s, name) in self.sets.iter().enumerate() {
            let y = matrix_top + (s as f64 + 0.5) * row;
            ctx.text(
                left - 6.0,
                y,
                name.clone(),
                ts,
                ctx.theme.text,
                Anchor::End,
                Baseline::Middle,
            );
            let w = (size_w - 8.0) * self.set_sizes[s] as f64 / max_s;
            let x1 = r.x + size_w;
            sbars.rect(x1 - w, y - row * 0.3, x1, y + row * 0.3);
            ctx.text(
                x1 - w - 3.0,
                y,
                self.set_sizes[s].to_string(),
                ts * 0.8,
                ctx.theme.text,
                Anchor::End,
                Baseline::Middle,
            );
        }
        sbars.fill(ctx, dark.mix(ctx.theme.plot_background, 0.35));
        ctx.text(
            left + (r.right() - left) / 2.0,
            r.y,
            "intersection size",
            ts,
            ctx.theme.text,
            Anchor::Middle,
            Baseline::Top,
        );
        ctx.text(
            r.x + size_w / 2.0,
            matrix_top - 6.0,
            "set size",
            ts,
            ctx.theme.text,
            Anchor::Middle,
            Baseline::Bottom,
        );
    }
    fn kind(&self) -> &'static str {
        "UpSet"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Exclusive intersections of named sets of items: (membership mask,
/// count), largest first, and the set sizes.
pub fn intersections<N: AsRef<str>, I: AsRef<str>>(
    sets: &[(N, Vec<I>)],
) -> (Vec<(u64, usize)>, Vec<usize>) {
    let mut mask: crate::plot::raster::HashMap<&str, u64> = Default::default();
    for (k, (_, items)) in sets.iter().enumerate().take(64) {
        for it in items {
            *mask.entry(it.as_ref()).or_default() |= 1 << k;
        }
    }
    let mut count: crate::plot::raster::HashMap<u64, usize> = Default::default();
    for m in mask.values() {
        *count.entry(*m).or_default() += 1;
    }
    let mut v: Vec<(u64, usize)> = count.into_iter().collect();
    v.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then(a.0.count_ones().cmp(&b.0.count_ones()))
            .then(a.0.cmp(&b.0))
    });
    let sizes = sets
        .iter()
        .take(64)
        .map(|(_, items)| {
            let mut u: Vec<&str> = items.iter().map(|s| s.as_ref()).collect();
            u.sort_unstable();
            u.dedup();
            u.len()
        })
        .collect();
    (v, sizes)
}

/// Reliability (calibration) curve: per bin of predicted probability the
/// mean prediction and the observed fraction of positives, and the Brier
/// score.
pub fn calibration(labels: &[bool], probs: &[f64], bins: usize) -> (Vec<f64>, Vec<f64>, f64) {
    let bins = bins.max(1);
    let (mut sp, mut sy, mut n) = (vec![0.0; bins], vec![0.0; bins], vec![0usize; bins]);
    let mut brier = 0.0;
    let mut total = 0;
    for (&l, &p) in labels.iter().zip(probs) {
        if !p.is_finite() {
            continue;
        }
        let p = p.clamp(0.0, 1.0);
        let b = ((p * bins as f64) as usize).min(bins - 1);
        let y = if l { 1.0 } else { 0.0 };
        sp[b] += p;
        sy[b] += y;
        n[b] += 1;
        brier += (p - y) * (p - y);
        total += 1;
    }
    let (mut mx, mut my) = (Vec::new(), Vec::new());
    for b in 0..bins {
        if n[b] > 0 {
            mx.push(sp[b] / n[b] as f64);
            my.push(sy[b] / n[b] as f64);
        }
    }
    (mx, my, brier / total.max(1) as f64)
}

/// Cumulative gain: (fraction of the population contacted, fraction of
/// all positives found) when going down the scores from the highest.
pub fn gain(labels: &[bool], scores: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut idx: Vec<usize> = (0..labels.len().min(scores.len()))
        .filter(|&i| scores[i].is_finite())
        .collect();
    idx.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    let n = idx.len().max(1) as f64;
    let pos = idx.iter().filter(|&&i| labels[i]).count().max(1) as f64;
    // At most ~200 points (the curve is smooth).
    let step = (idx.len() / 200).max(1);
    let (mut x, mut y) = (vec![0.0], vec![0.0]);
    let mut found = 0.0;
    for (k, &i) in idx.iter().enumerate() {
        if labels[i] {
            found += 1.0;
        }
        if (k + 1) % step == 0 || k + 1 == idx.len() {
            x.push((k + 1) as f64 / n);
            y.push(found / pos);
        }
    }
    (x, y)
}

impl Axes {
    /// SHAP summary (beeswarm) plot: `shap[f][i]` is the SHAP value of
    /// feature `f` for sample `i`, `values[f][i]` the feature's value
    /// (colors from low to high). Features are sorted by mean |SHAP|, the
    /// most important on top; at most `max_features` are shown.
    pub fn shap_summary<S: AsRef<str>>(
        &mut self,
        features: &[S],
        shap: &[Vec<f64>],
        values: &[Vec<f64>],
        max_features: usize,
    ) -> &mut ShapSummary {
        let mut order: Vec<usize> = (0..features.len().min(shap.len())).collect();
        let importance = |f: usize| {
            let v = &shap[f];
            v.iter()
                .filter(|x| x.is_finite())
                .map(|x| x.abs())
                .sum::<f64>()
                / v.len().max(1) as f64
        };
        order.sort_by(|a, b| importance(*b).total_cmp(&importance(*a)));
        order.truncate(max_features.max(1));
        let mut points = Vec::new();
        let mut range = (f64::INFINITY, f64::NEG_INFINITY);
        for (row, &f) in order.iter().enumerate() {
            let sv = &shap[f];
            let fv = values.get(f).cloned().unwrap_or_default();
            // Colors by rank (robust to outliers), 0..1.
            let mut sorted: Vec<f64> = fv.iter().copied().filter(|v| v.is_finite()).collect();
            sorted.sort_by(f64::total_cmp);
            let rank = |v: f64| -> f32 {
                if !v.is_finite() || sorted.is_empty() {
                    return 0.5;
                }
                (sorted.partition_point(|x| *x < v) as f32 / sorted.len().max(1) as f32)
                    .clamp(0.0, 1.0)
            };
            let dy = swarm_offsets(sv, 120);
            for (i, &x) in sv.iter().enumerate() {
                if x.is_finite() {
                    range = (range.0.min(x), range.1.max(x));
                    points.push((
                        row as u32,
                        x as f32,
                        rank(fv.get(i).copied().unwrap_or(f64::NAN)),
                        dy[i],
                    ));
                }
            }
        }
        let names: Vec<String> = order
            .iter()
            .map(|&f| features[f].as_ref().to_string())
            .collect();
        self.y.categories(names);
        self.y.inverted = true;
        self.y.grid = Some(false);
        self.x
            .label
            .get_or_insert_with(|| "SHAP value (impact on model output)".into());
        self.add(ShapSummary {
            points,
            rows: order.len(),
            colormap: Colormap::Coolwarm,
            size: 4.0,
            range,
        })
    }

    /// Calibration (reliability) curve of predicted probabilities against
    /// the observed fraction of positives, with the diagonal of perfect
    /// calibration; the legend shows the Brier score.
    pub fn calibration_curve(
        &mut self,
        name: &str,
        labels: &[bool],
        probs: impl IntoValues,
        bins: usize,
    ) -> &mut Line {
        let (x, y, brier) = calibration(labels, &probs.into_values(), bins);
        if self.series.is_empty() {
            self.line([0.0, 1.0], [0.0, 1.0])
                .color(Color::rgb(140, 140, 140))
                .dash(Dash::Dashed)
                .label("perfectly calibrated");
            self.x
                .label
                .get_or_insert_with(|| "mean predicted probability".into());
            self.y
                .label
                .get_or_insert_with(|| "fraction of positives".into());
            self.x_limits(0.0, 1.0).y_limits(0.0, 1.0);
        }
        self.line(x, y)
            .marker(Marker::Circle)
            .width(2.0)
            .label(format!("{name} (Brier {brier:.3})"))
    }

    /// Learning curve: mean training and validation scores (with ±1 standard
    /// deviation bands) against the training set size; `train[k]` and
    /// `validation[k]` hold the scores of the folds at `sizes[k]`.
    pub fn learning_curve(
        &mut self,
        sizes: impl IntoValues,
        train: &[Vec<f64>],
        validation: &[Vec<f64>],
    ) -> &mut Self {
        let sizes = sizes.into_values();
        let ms = |v: &[f64]| {
            let n = v.len().max(1) as f64;
            let m = v.iter().sum::<f64>() / n;
            (
                m,
                (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / n).sqrt(),
            )
        };
        for (name, data, color) in [
            ("training score", train, Color::rgb(31, 119, 180)),
            ("validation score", validation, Color::rgb(255, 127, 14)),
        ] {
            let st: Vec<(f64, f64)> = data.iter().map(|v| ms(v)).collect();
            let m: Vec<f64> = st.iter().map(|s| s.0).collect();
            self.fill_between(
                &sizes,
                st.iter().map(|s| s.0 - s.1),
                st.iter().map(|s| s.0 + s.1),
            )
            .color(color)
            .opacity(0.18);
            self.line(&sizes, m)
                .color(color)
                .marker(Marker::Circle)
                .width(2.0)
                .label(name);
        }
        self.x
            .label
            .get_or_insert_with(|| "training examples".into());
        self.y.label.get_or_insert_with(|| "score".into());
        self
    }

    /// Cumulative gain chart: share of all positives found against the
    /// share of the population taken in score order, with the random and
    /// the perfect model.
    pub fn gain_curve(
        &mut self,
        name: &str,
        labels: &[bool],
        scores: impl IntoValues,
    ) -> &mut Line {
        let (x, y) = gain(labels, &scores.into_values());
        if self.series.is_empty() {
            let rate = labels.iter().filter(|l| **l).count() as f64 / labels.len().max(1) as f64;
            self.line([0.0, rate.max(1e-9), 1.0], [0.0, 1.0, 1.0])
                .color(Color::rgb(150, 150, 150))
                .dash(Dash::Dotted)
                .label("perfect model");
            self.line([0.0, 1.0], [0.0, 1.0])
                .color(Color::rgb(140, 140, 140))
                .dash(Dash::Dashed)
                .label("random");
            self.x
                .label
                .get_or_insert_with(|| "share of population".into());
            self.y
                .label
                .get_or_insert_with(|| "share of positives found".into());
            self.x_limits(0.0, 1.0).y_limits(0.0, 1.02);
        }
        self.line(x, y).width(2.2).label(name)
    }

    /// Lift chart: how many times more positives than random the top of the
    /// score ranking holds.
    pub fn lift_curve(
        &mut self,
        name: &str,
        labels: &[bool],
        scores: impl IntoValues,
    ) -> &mut Line {
        let (x, y) = gain(labels, &scores.into_values());
        let (lx, ly): (Vec<f64>, Vec<f64>) = x
            .iter()
            .zip(&y)
            .filter(|(a, _)| **a >= 0.01)
            .map(|(a, b)| (*a, b / a))
            .unzip();
        if self.series.is_empty() {
            self.line([0.0, 1.0], [1.0, 1.0])
                .color(Color::rgb(140, 140, 140))
                .dash(Dash::Dashed)
                .label("random");
            self.x
                .label
                .get_or_insert_with(|| "share of population".into());
            self.y.label.get_or_insert_with(|| "lift".into());
        }
        self.line(lx, ly).width(2.2).label(name)
    }

    /// Raincloud plot of `(name, values)` groups: a half violin, a box plot
    /// and the points of each group on its own row.
    pub fn raincloud<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
    ) -> &mut Raincloud {
        let (names, data): (Vec<String>, Vec<Vec<f64>>) = groups
            .into_iter()
            .map(|(l, v)| (l.to_string(), v.into_values()))
            .unzip();
        let curves = data.iter().map(|d| stats::kde(d, None, 160)).collect();
        let colors = (0..data.len()).map(|_| self.next_color()).collect();
        let jitter = data
            .iter()
            .map(|d| {
                (0..d.len())
                    .map(|i| (((i as f64 * 0.618_034).fract()) * 2.0 - 1.0) as f32 * 0.5)
                    .collect()
            })
            .collect();
        self.y.categories(names);
        self.y.inverted = true;
        self.y.grid = Some(false);
        self.add(Raincloud {
            groups: data,
            curves,
            colors,
            jitter,
        })
    }

    /// UpSet plot of named sets of items: the sizes of their exclusive
    /// intersections (largest first, at most `max_intersections`) over a dot
    /// matrix of the sets, and the set sizes.
    pub fn upset<N: AsRef<str>, I: AsRef<str>>(
        &mut self,
        sets: &[(N, Vec<I>)],
        max_intersections: usize,
    ) -> &mut UpSet {
        self.kind = AxesKind::Frameless;
        let (mut inter, sizes) = intersections(sets);
        inter.truncate(max_intersections.max(1));
        let color = Color::rgb(45, 45, 60);
        self.add(UpSet {
            sets: sets
                .iter()
                .take(64)
                .map(|s| s.0.as_ref().to_string())
                .collect(),
            set_sizes: sizes,
            intersections: inter,
            color,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn calibration_and_gain() {
        let labels = [false, false, true, true];
        let (x, y, b) = calibration(&labels, &[0.1, 0.2, 0.8, 0.9], 2);
        assert_eq!(x.len(), 2);
        assert!((y[0] - 0.0).abs() < 1e-12 && (y[1] - 1.0).abs() < 1e-12);
        assert!((b - (0.01 + 0.04 + 0.04 + 0.01) / 4.0).abs() < 1e-12);
        let (gx, gy) = gain(&labels, &[0.1, 0.2, 0.8, 0.9]);
        assert!((gx[2] - 0.5).abs() < 1e-12 && (gy[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn upset_intersections() {
        let (v, sizes) = intersections(&[
            ("A", vec!["1", "2", "3"]),
            ("B", vec!["2", "3", "4"]),
            ("C", vec!["3"]),
        ]);
        assert_eq!(sizes, vec![3, 3, 1]);
        // {1}:A, {2}:AB, {3}:ABC, {4}:B
        assert_eq!(v.iter().map(|x| x.1).sum::<usize>(), 4);
        assert!(v.contains(&(0b111, 1)) && v.contains(&(0b011, 1)));
    }

    #[test]
    fn all_render() {
        let mut fig = Figure::new(1200, 800);
        fig.grid(2, 3);
        let n = 300;
        let shap: Vec<Vec<f64>> = (0..4)
            .map(|f| {
                (0..n)
                    .map(|i| ((i * (f + 3)) as f64 * 0.37).sin() * (f + 1) as f64)
                    .collect()
            })
            .collect();
        let vals: Vec<Vec<f64>> = (0..4)
            .map(|f| (0..n).map(|i| ((i + f) as f64 * 0.11).cos()).collect())
            .collect();
        fig.at(0, 0)
            .shap_summary(&["a", "b", "c", "d"], &shap, &vals, 10);
        let labels: Vec<bool> = (0..n).map(|i| i % 3 == 0).collect();
        let p: Vec<f64> = (0..n)
            .map(|i| if i % 3 == 0 { 0.7 } else { 0.2 } + (i as f64 * 0.7).sin() * 0.2)
            .collect();
        fig.at(0, 1).calibration_curve("m", &labels, &p, 10);
        fig.at(0, 2).learning_curve(
            [10.0, 20.0, 40.0],
            &[vec![0.9, 0.95], vec![0.9, 0.92], vec![0.88, 0.9]],
            &[vec![0.6, 0.7], vec![0.7, 0.75], vec![0.8, 0.82]],
        );
        fig.at(1, 0).gain_curve("m", &labels, &p);
        fig.at(1, 1).raincloud([
            ("a", vec![1.0, 2.0, 2.5, 3.0]),
            ("b", vec![2.0, 3.0, 3.5, 5.0]),
        ]);
        fig.at(1, 2).upset(
            &[("A", vec!["1", "2", "3"]), ("B", vec!["2", "3", "4"])],
            10,
        );
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
    }
}
