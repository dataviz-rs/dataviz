//! Distribution plots: strip (jittered) and swarm (beeswarm) plots,
//! ridgelines and Q-Q plots.

use super::{short, Context, Extent, Hit, IntoValues, Rect, Series};
use crate::plot::axes::Axes;
use crate::plot::axis::AxisMap;
use crate::plot::color::Color;
use crate::plot::scene::{Node, Stroke};
use crate::plot::stats;
use crate::plot::style::Marker;
use std::any::Any;

/// Points of several groups along a category axis, spread so they can be
/// seen: randomly within a band (strip plot) or side by side without
/// overlap (swarm plot). Created by [`Axes::strip`] and [`Axes::swarm`].
#[derive(Clone, Debug)]
pub struct Strip {
    pub(crate) pos: Vec<f64>,
    pub(crate) names: Vec<String>,
    pub(crate) data: Vec<Vec<f64>>,
    pub(crate) colors: Vec<Color>,
    pub(crate) size: f64,
    pub(crate) spread: f64,
    pub(crate) swarm: bool,
    pub(crate) horizontal: bool,
    pub(crate) opacity: f32,
    pub(crate) marker: Marker,
    pub(crate) edge: Option<Stroke>,
}

impl Strip {
    /// Marker diameter in pixels (default 5).
    pub fn size(&mut self, px: f64) -> &mut Self {
        self.size = px.max(0.5);
        self
    }
    /// Half width of the band in category units (default 0.3 for strips,
    /// 0.45 for swarms).
    pub fn spread(&mut self, w: f64) -> &mut Self {
        self.spread = w.max(0.0);
        self
    }
    /// Colors per group.
    pub fn colors<C: Into<Color>>(&mut self, colors: impl IntoIterator<Item = C>) -> &mut Self {
        self.colors = colors.into_iter().map(Into::into).collect();
        self
    }
    /// One color for all groups.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.colors = vec![c.into()];
        self
    }
    /// Marker opacity (default 0.8 for strips, 1 for swarms).
    pub fn opacity(&mut self, a: f32) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }
    /// Marker shape.
    pub fn marker(&mut self, m: Marker) -> &mut Self {
        self.marker = m;
        self
    }
    /// Outline of the markers.
    pub fn edge(&mut self, color: impl Into<Color>, width: f64) -> &mut Self {
        self.edge = Some(Stroke::new(color.into(), width));
        self
    }

    fn color_of(&self, i: usize) -> Color {
        if self.colors.is_empty() {
            Color::BLUE
        } else {
            self.colors[i % self.colors.len()]
        }
    }

    /// Canvas positions of group `g`: (along the category axis, along the
    /// value axis).
    fn positions(&self, ctx_x: &AxisMap, ctx_y: &AxisMap, g: usize) -> Vec<(f64, f64, usize)> {
        let (cat, val) = if self.horizontal {
            (ctx_y, ctx_x)
        } else {
            (ctx_x, ctx_y)
        };
        let p = self.pos[g];
        let centre = cat.px(p);
        let band = (cat.px(p + self.spread) - centre).abs();
        let vals = &self.data[g];
        if !self.swarm {
            // Deterministic jitter: the same data always looks the same.
            return vals
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let h = (i as u64 ^ (g as u64) << 32).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    let u = (h >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
                    (centre + u * band, val.px(*v), i)
                })
                .collect();
        }
        swarm(vals, |v| val.px(v), centre, band, self.size)
    }
}

/// Beeswarm layout: each point (in value order) takes the free offset from
/// the centre closest to it, so points never overlap until the band is
/// full. Returns (category px, value px, index).
fn swarm(
    vals: &[f64],
    px: impl Fn(f64) -> f64,
    centre: f64,
    band: f64,
    d: f64,
) -> Vec<(f64, f64, usize)> {
    let mut order: Vec<(f64, usize)> = vals
        .iter()
        .enumerate()
        .map(|(i, v)| (px(*v), i))
        .filter(|(p, _)| p.is_finite())
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut placed: Vec<(f64, f64)> = Vec::with_capacity(order.len());
    let mut out = Vec::with_capacity(order.len());
    let mut start = 0;
    let d2 = d * d * 0.98;
    for (v, i) in order {
        // Only points closer than one diameter along the value axis matter.
        while start < placed.len() && (v - placed[start].1).abs() >= d {
            start += 1;
        }
        let near = &placed[start..];
        let free = |o: f64| {
            near.iter()
                .all(|(c, w)| (c - o).powi(2) + (w - v).powi(2) >= d2)
        };
        // Candidate offsets: the centre and the positions touching each
        // neighbour on either side.
        let mut best: Option<f64> = free(0.0).then_some(0.0);
        if best.is_none() {
            let mut cands: Vec<f64> = Vec::with_capacity(near.len() * 2);
            for (c, w) in near {
                let dy = (w - v).abs();
                let dx = (d2 - dy * dy).max(0.0).sqrt() + 1e-6;
                cands.push(c + dx);
                cands.push(c - dx);
            }
            cands.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
            best = cands.into_iter().find(|o| free(*o));
        }
        let o = best.unwrap_or(0.0).clamp(-band, band);
        placed.push((o, v));
        out.push((centre + o, v, i));
    }
    out
}

impl Series for Strip {
    fn extent(&self) -> Option<Extent> {
        let mut e: Option<Extent> = None;
        for (g, vals) in self.data.iter().enumerate() {
            let p = self.pos[g];
            for v in vals.iter().filter(|v| v.is_finite()) {
                let x = Extent::point(p - 0.5, *v).include(p + 0.5, *v);
                e = Some(e.map_or(x, |e| e.union(x)));
            }
        }
        e.map(|e| if self.horizontal { e.transposed() } else { e })
    }

    fn draw(&self, ctx: &mut Context) {
        for g in 0..self.data.len() {
            let color = self.color_of(g);
            let color = color.with_alpha(color.a * self.opacity);
            let Some(template) = self
                .marker
                .node(0.0, 0.0, self.size, color, self.edge.clone())
            else {
                continue;
            };
            let offsets: Vec<(f32, f32)> = self
                .positions(ctx.x, ctx.y, g)
                .into_iter()
                .map(|(c, v, _)| if self.horizontal { (v, c) } else { (c, v) })
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .map(|(x, y)| (x as f32, y as f32))
                .collect();
            ctx.push(Node::Instances {
                template: Box::new(template),
                offsets,
            });
        }
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        for g in 0..self.data.len() {
            for (c, v, i) in self.positions(x, y, g) {
                let (cx, cy) = if self.horizontal { (v, c) } else { (c, v) };
                let d = (cx - px).hypot(cy - py);
                if best.as_ref().is_none_or(|b| d < b.distance) {
                    best = Some(Hit {
                        px: cx,
                        py: cy,
                        label: format!("{}: {}", self.names[g], short(self.data[g][i])),
                        distance: d,
                    });
                }
            }
        }
        best
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Density curves of several groups stacked along the y-axis with some
/// overlap (joy plot). Created by [`Axes::ridgeline`].
#[derive(Clone, Debug)]
pub struct Ridgeline {
    pub(crate) names: Vec<String>,
    pub(crate) curves: Vec<(Vec<f64>, Vec<f64>)>,
    pub(crate) overlap: f64,
    pub(crate) colors: Vec<Color>,
    pub(crate) opacity: f32,
    pub(crate) edge: Option<Color>,
}

impl Ridgeline {
    /// Height of the highest curve in rows (default 1.6; above 1 curves
    /// overlap the row above).
    pub fn overlap(&mut self, rows: f64) -> &mut Self {
        self.overlap = rows.max(0.1);
        self
    }
    /// Colors per group.
    pub fn colors<C: Into<Color>>(&mut self, colors: impl IntoIterator<Item = C>) -> &mut Self {
        self.colors = colors.into_iter().map(Into::into).collect();
        self
    }
    /// One color for all groups.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.colors = vec![c.into()];
        self
    }
    /// Fill opacity (default 0.85).
    pub fn opacity(&mut self, a: f32) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }
    /// Outline color (default: the theme background, separating the
    /// curves); `None` draws the outline in the fill color.
    pub fn edge(&mut self, c: Option<Color>) -> &mut Self {
        self.edge = c;
        self
    }

    fn scale(&self) -> f64 {
        let max = self
            .curves
            .iter()
            .flat_map(|(_, d)| d.iter().copied())
            .fold(0.0, f64::max);
        if max > 0.0 {
            self.overlap / max
        } else {
            0.0
        }
    }
}

impl Series for Ridgeline {
    fn extent(&self) -> Option<Extent> {
        let n = self.curves.len();
        let mut e: Option<Extent> = None;
        for (xs, _) in &self.curves {
            if let (Some(a), Some(b)) = (xs.first(), xs.last()) {
                let x = Extent::point(*a, -0.5).include(*b, -0.5);
                e = Some(e.map_or(x, |e| e.union(x)));
            }
        }
        // The highest curve reaches `overlap` rows above its baseline.
        let mut e = e?.include(e?.x_min, n as f64 - 1.0 + 0.3);
        e = e.include(e.x_min, -self.overlap.max(0.5) - 0.05);
        e.sticky_y_min = true;
        e.sticky_y_max = true;
        Some(e)
    }

    fn draw(&self, ctx: &mut Context) {
        let k = self.scale();
        let edge = self.edge.unwrap_or(ctx.theme.plot_background);
        // Top rows first: each curve overlaps the one above it.
        for (i, (xs, ds)) in self.curves.iter().enumerate() {
            let base = i as f64;
            let fill = self
                .colors
                .get(i % self.colors.len().max(1))
                .copied()
                .unwrap_or(Color::BLUE);
            let mut poly: Vec<(f64, f64)> = xs
                .iter()
                .zip(ds)
                .map(|(x, d)| ctx.point(*x, base - d * k))
                .collect();
            if poly.len() < 2 {
                continue;
            }
            let outline = poly.clone();
            poly.push(ctx.point(xs[xs.len() - 1], base));
            poly.push(ctx.point(xs[0], base));
            ctx.polygon(&poly, Some(fill.with_alpha(fill.a * self.opacity)), None);
            ctx.polyline(&outline, Stroke::new(edge, 1.5));
            ctx.polyline(
                &[ctx.point(xs[0], base), ctx.point(xs[xs.len() - 1], base)],
                Stroke::new(fill.with_alpha(0.6 * fill.a), 1.0),
            );
        }
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let k = self.scale();
        let v = x.value(px);
        let mut best: Option<Hit> = None;
        for (i, (xs, ds)) in self.curves.iter().enumerate() {
            let j = xs
                .partition_point(|a| *a < v)
                .min(xs.len().saturating_sub(1));
            let Some(d) = ds.get(j) else { continue };
            let (cx, cy) = (x.px(xs[j]), y.px(i as f64 - d * k));
            let dist = (cx - px).hypot(cy - py);
            if best.as_ref().is_none_or(|b| dist < b.distance) {
                best = Some(Hit {
                    px: cx,
                    py: cy,
                    label: format!(
                        "{}: density {} at {}",
                        self.names[i],
                        short(*d),
                        short(xs[j])
                    ),
                    distance: dist,
                });
            }
        }
        best
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Inverse of the standard normal CDF (Acklam's rational approximation,
/// relative error < 1.2e-9 after one Halley refinement step).
pub fn normal_quantile(p: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) || p.is_nan() {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let pl = 0.02425;
    let x = if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    // One Halley step with the complementary error function.
    let e = 0.5 * erfc(-x / std::f64::consts::SQRT_2) - p;
    let u = e * (2.0 * std::f64::consts::PI).sqrt() * (x * x / 2.0).exp();
    x - u / (1.0 + x * u / 2.0)
}

/// Complementary error function (W. J. Cody's rational approximations via
/// the Numerical Recipes Chebyshev fit; relative error < 1.2e-7, enough for
/// one refinement step).
fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let r = t
        * (-z * z - 1.265_512_23
            + t * (1.000_023_68
                + t * (0.374_091_96
                    + t * (0.096_784_18
                        + t * (-0.186_288_06
                            + t * (0.278_868_07
                                + t * (-1.135_203_98
                                    + t * (1.488_515_87
                                        + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
            .exp();
    if x >= 0.0 {
        r
    } else {
        2.0 - r
    }
}

impl Axes {
    fn strip_like<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
        swarm: bool,
        horizontal: bool,
    ) -> &mut Strip {
        let (pos, names, data) = self.groups(groups, horizontal);
        let colors = (0..data.len()).map(|_| self.next_color()).collect();
        self.add(Strip {
            pos,
            names,
            data,
            colors,
            size: 5.0,
            spread: if swarm { 0.45 } else { 0.3 },
            swarm,
            horizontal,
            opacity: if swarm { 1.0 } else { 0.8 },
            marker: Marker::Circle,
            edge: None,
        })
    }

    /// Strip plot: the points of each `(name, values)` group, jittered
    /// across the category so that they can be seen.
    pub fn strip<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
    ) -> &mut Strip {
        self.strip_like(groups, false, false)
    }

    /// Swarm (beeswarm) plot: like [`Axes::strip`], with points placed
    /// side by side without overlapping, so the spread shows the density.
    pub fn swarm<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
    ) -> &mut Strip {
        self.strip_like(groups, true, false)
    }

    /// Horizontal swarm plot (categories on the y-axis).
    pub fn swarm_horizontal<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
    ) -> &mut Strip {
        self.strip_like(groups, true, true)
    }

    /// Ridgeline (joy) plot: the density of each `(name, values)` group as
    /// a filled curve on its own row, overlapping the row above.
    pub fn ridgeline<L: ToString, V: IntoValues>(
        &mut self,
        groups: impl IntoIterator<Item = (L, V)>,
    ) -> &mut Ridgeline {
        let (_, names, data) = self.groups(groups, true);
        let colors = (0..data.len()).map(|_| self.next_color()).collect();
        // One bandwidth and range for all, so the curves compare.
        let curves = data.iter().map(|d| stats::kde(d, None, 200)).collect();
        self.y.grid = Some(false);
        self.add(Ridgeline {
            names,
            curves,
            overlap: 1.6,
            colors,
            opacity: 0.85,
            edge: None,
        })
    }

    /// Q-Q plot of `values` against the normal distribution: sample
    /// quantiles over theoretical ones, with a reference line through the
    /// quartiles. Points on the line mean the data is normal; returns the
    /// points (a scatter series).
    pub fn qq(&mut self, values: impl IntoValues) -> &mut super::scatter::Scatter {
        let mut s: Vec<f64> = values
            .into_values()
            .into_iter()
            .filter(|v| v.is_finite())
            .collect();
        s.sort_by(f64::total_cmp);
        let n = s.len();
        // Blom's plotting positions.
        let z: Vec<f64> = (0..n)
            .map(|i| normal_quantile((i as f64 + 1.0 - 0.375) / (n as f64 + 0.25)))
            .collect();
        let color = self.next_color();
        if n >= 2 {
            let (q1, q3) = (
                stats::quantile_sorted(&s, 0.25),
                stats::quantile_sorted(&s, 0.75),
            );
            let (z1, z3) = (normal_quantile(0.25), normal_quantile(0.75));
            let slope = (q3 - q1) / (z3 - z1);
            let icpt = q1 - slope * z1;
            let (a, b) = (z[0], z[n - 1]);
            self.line([a, b], [icpt + slope * a, icpt + slope * b])
                .color(Color::rgb(214, 39, 40))
                .label("normal");
        }
        if self.x.label.is_none() {
            self.x.label = Some("Theoretical quantiles".into());
        }
        if self.y.label.is_none() {
            self.y.label = Some("Sample quantiles".into());
        }
        let pts = self.scatter(z, s);
        pts.color(color);
        pts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_quantile_is_accurate() {
        for (p, z) in [
            (0.5, 0.0),
            (0.975, 1.959_963_984_540_054),
            (0.025, -1.959_963_984_540_054),
            (0.841_344_746_068_543, 1.0),
            (1e-6, -4.753_424_308_822_899),
        ] {
            assert!(
                (normal_quantile(p) - z).abs() < 1e-6,
                "{p}: {}",
                normal_quantile(p)
            );
        }
    }

    #[test]
    fn swarm_points_do_not_overlap() {
        let vals: Vec<f64> = (0..300).map(|i| ((i * 37) % 100) as f64 / 10.0).collect();
        let pts = swarm(&vals, |v| v * 20.0, 100.0, 1000.0, 6.0);
        assert_eq!(pts.len(), 300);
        for (i, a) in pts.iter().enumerate() {
            for b in &pts[i + 1..] {
                let d = (a.0 - b.0).hypot(a.1 - b.1);
                assert!(d >= 6.0 * 0.98f64.sqrt() - 1e-6, "{a:?} {b:?} {d}");
            }
        }
        // Points stay as close to the centre as they can.
        let spread = pts.iter().map(|p| (p.0 - 100.0).abs()).fold(0.0, f64::max);
        assert!(spread < 60.0, "{spread}");
    }

    #[test]
    fn plots_render() {
        let mut fig = crate::plot::Figure::new(400, 300);
        fig.grid(2, 2);
        let a: Vec<f64> = (0..200).map(|i| (i as f64 * 0.37).sin() * 3.0).collect();
        let b: Vec<f64> = (0..200)
            .map(|i| (i as f64 * 0.11).cos() * 2.0 + 1.0)
            .collect();
        fig.at(0, 0).strip([("a", &a), ("b", &b)]);
        fig.at(0, 1).swarm([("a", &a[..60]), ("b", &b[..60])]);
        fig.at(1, 0).ridgeline([("a", &a), ("b", &b)]);
        fig.at(1, 1).qq(&a);
        let px = fig.to_pixels().unwrap();
        assert_eq!(px.width, 400);
        let svg = fig.to_svg();
        assert!(svg.contains("Theoretical quantiles"));
    }
}
