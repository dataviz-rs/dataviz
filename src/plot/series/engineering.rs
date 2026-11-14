//! Engineering and science charts: streamlines, ternary diagrams, 3D
//! waterfall spectra, SPC control charts, Pareto charts, 3D vector fields
//! and 3D bar charts.

use super::general::Shapes;
use super::line::Line;
use super::three_d::{Bounds3, Scene3d};
use super::{short, ColorScale, Context, Extent, Hit, IntoLabels, IntoValues, Rect, Series};
use crate::plot::axes::{Axes, AxesKind};
use crate::plot::axis::AxisMap;
use crate::plot::color::{Color, Colormap};
use crate::plot::scene::{Anchor, Baseline, LineCap, LineJoin, Node, Stroke};
use crate::plot::style::{Dash, Marker};
use std::any::Any;

// ----- streamplot ----------------------------------------------------------------

/// Streamlines: the points of each line and the speed at each point.
pub type StreamLines = Vec<(Vec<(f64, f64)>, Vec<f64>)>;

/// Streamlines of a 2D vector field, evenly spaced, colored by speed, with
/// an arrow on each. Created by [`Axes::streamplot`].
#[derive(Clone, Debug)]
pub struct Streamplot {
    /// Points and speeds of each streamline (data coordinates).
    pub(crate) lines: StreamLines,
    pub(crate) x_range: (f64, f64),
    pub(crate) y_range: (f64, f64),
    pub(crate) colormap: Option<Colormap>,
    pub(crate) color: Color,
    pub(crate) width: f64,
    pub(crate) speed: (f64, f64),
    /// Colors by log10 of the speed (wide speed ranges).
    pub(crate) log: bool,
}

impl Streamplot {
    /// Uniform color instead of colors by speed.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self.colormap = None;
        self
    }
    /// Colormap of the speed.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = Some(cm);
        self
    }
    /// Line width.
    pub fn width(&mut self, w: f64) -> &mut Self {
        self.width = w;
        self
    }
}

/// Evenly spaced streamlines of `f` over a rectangle (Jobard–Lefer style:
/// a line stops where it comes closer than one cell to another line).
pub fn streamlines(
    f: &dyn Fn(f64, f64) -> (f64, f64),
    x_range: (f64, f64),
    y_range: (f64, f64),
    density: usize,
) -> StreamLines {
    let n = density.clamp(4, 200);
    let (w, h) = (x_range.1 - x_range.0, y_range.1 - y_range.0);
    if w <= 0.0 || h <= 0.0 {
        return Vec::new();
    }
    // Occupancy grid in cells; integration in cell units.
    let mut used = vec![u32::MAX; n * n];
    let cell = |x: f64, y: f64| -> Option<usize> {
        let (i, j) = (
            (x - x_range.0) / w * n as f64,
            (y - y_range.0) / h * n as f64,
        );
        (i >= 0.0 && j >= 0.0 && i < n as f64 && j < n as f64).then(|| j as usize * n + i as usize)
    };
    // Velocity in cell units per step (direction only; speed recorded).
    let vel = |x: f64, y: f64| -> Option<(f64, f64, f64)> {
        let (u, v) = f(x, y);
        let (cu, cv) = (u / w * n as f64, v / h * n as f64);
        let s = cu.hypot(cv);
        (s.is_finite() && s > 1e-12).then(|| (cu / s, cv / s, u.hypot(v)))
    };
    let step = 0.25; // cells
    let mut lines = Vec::new();
    // Seeds from the middle outwards, so the first lines are central.
    let mut seeds: Vec<(usize, usize)> = (0..n).flat_map(|j| (0..n).map(move |i| (i, j))).collect();
    let c = (n as f64 - 1.0) / 2.0;
    seeds.sort_by(|a, b| {
        let da = (a.0 as f64 - c).abs().max((a.1 as f64 - c).abs());
        let db = (b.0 as f64 - c).abs().max((b.1 as f64 - c).abs());
        da.total_cmp(&db)
    });
    for (id, (si, sj)) in seeds.into_iter().enumerate() {
        if used[sj * n + si] != u32::MAX {
            continue;
        }
        let id = id as u32;
        let start = (
            x_range.0 + (si as f64 + 0.5) / n as f64 * w,
            y_range.0 + (sj as f64 + 0.5) / n as f64 * h,
        );
        let mut halves: [Vec<(f64, f64, f64)>; 2] = [Vec::new(), Vec::new()];
        for (k, dir) in [1.0, -1.0].into_iter().enumerate() {
            let (mut x, mut y) = start;
            let mut last = usize::MAX;
            let mut own: Vec<usize> = Vec::new();
            for _ in 0..(n * 8) {
                let Some((u1, v1, s)) = vel(x, y) else { break };
                // Midpoint (RK2) step.
                let (mx, my) = (
                    x + dir * u1 * step * 0.5 * w / n as f64,
                    y + dir * v1 * step * 0.5 * h / n as f64,
                );
                let Some((u2, v2, _)) = vel(mx, my) else {
                    break;
                };
                let (nx, ny) = (
                    x + dir * u2 * step * w / n as f64,
                    y + dir * v2 * step * h / n as f64,
                );
                let Some(cn) = cell(nx, ny) else { break };
                if cn != last {
                    if used[cn] != u32::MAX && used[cn] != id {
                        break;
                    }
                    // Loops: stop on re-entering an own cell visited long ago.
                    if own.len() > 8 && own[..own.len() - 4].contains(&cn) {
                        break;
                    }
                    own.push(cn);
                    last = cn;
                }
                halves[k].push((x, y, s));
                x = nx;
                y = ny;
            }
            for &c in &own {
                used[c] = id;
            }
        }
        let mut pts: Vec<(f64, f64, f64)> = halves[1].iter().rev().copied().collect();
        pts.extend(halves[0].iter().skip(1));
        if pts.len() >= 6 {
            lines.push((
                pts.iter().map(|p| (p.0, p.1)).collect(),
                pts.iter().map(|p| p.2).collect(),
            ));
        }
    }
    lines
}

impl Series for Streamplot {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(self.x_range.0, self.y_range.0).include(self.x_range.1, self.y_range.1))
    }
    fn draw(&self, ctx: &mut Context) {
        // Pieces grouped into 24 colors: a few stroked paths in total.
        const LEVELS: usize = 24;
        let mut groups: Vec<Shapes> = (0..LEVELS).map(|_| Shapes::default()).collect();
        let mut heads: Vec<Shapes> = (0..LEVELS).map(|_| Shapes::default()).collect();
        let (s0, s1) = self.speed;
        let level = |s: f64| -> usize {
            if self.colormap.is_none() || s1 <= s0 {
                return 0;
            }
            let s = if self.log { s.max(1e-300).log10() } else { s };
            (((s - s0) / (s1 - s0)).clamp(0.0, 1.0) * (LEVELS - 1) as f64).round() as usize
        };
        for (pts, speed) in &self.lines {
            let px: Vec<(f64, f64)> = pts.iter().map(|p| ctx.point(p.0, p.1)).collect();
            let mut start = 0;
            while start + 1 < px.len() {
                let end = (start + 6).min(px.len() - 1);
                let l = level(speed[(start + end) / 2]);
                groups[l].line(&px[start..=end]);
                start = end;
            }
            // Arrow head in the middle of the line, along its direction.
            let m = px.len() / 2;
            let (a, b) = (px[m.saturating_sub(1)], px[(m + 1).min(px.len() - 1)]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len = dx.hypot(dy);
            if len > 1e-9 {
                let (ux, uy) = (dx / len, dy / len);
                let hs = 3.2 + self.width * 2.0;
                let tip = (px[m].0 + ux * hs * 0.6, px[m].1 + uy * hs * 0.6);
                let base = (tip.0 - ux * hs, tip.1 - uy * hs);
                heads[level(speed[m])].poly(&[
                    tip,
                    (base.0 - uy * hs * 0.5, base.1 + ux * hs * 0.5),
                    (base.0 + uy * hs * 0.5, base.1 - ux * hs * 0.5),
                ]);
            }
        }
        for (l, (g, h)) in groups.into_iter().zip(heads).enumerate() {
            let c = match self.colormap {
                Some(cm) => cm.at(l as f64 / (LEVELS - 1) as f64),
                None => self.color,
            };
            g.stroke(
                ctx,
                Stroke::new(c, self.width)
                    .cap(LineCap::Round)
                    .join(LineJoin::Round),
            );
            h.fill(ctx, c);
        }
    }
    fn color_scale(&self) -> Option<ColorScale> {
        self.colormap.map(|cm| ColorScale {
            colormap: cm,
            min: self.speed.0,
            max: self.speed.1,
            label: Some(if self.log { "log₁₀ speed" } else { "speed" }.into()),
        })
    }
    fn kind(&self) -> &'static str {
        "streamlines"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- ternary -------------------------------------------------------------------

/// Ternary diagram: compositions of three parts as points in a triangle.
/// Created by [`Axes::ternary`].
#[derive(Clone, Debug)]
pub struct Ternary {
    pub(crate) abc: Vec<[f64; 3]>,
    pub(crate) names: [String; 3],
    pub(crate) color: Color,
    pub(crate) values: Option<(Vec<f64>, Colormap)>,
    pub(crate) size: f64,
    pub(crate) label: Option<String>,
}

impl Ternary {
    /// Names of the three components (top, bottom left, bottom right).
    pub fn names(&mut self, a: &str, b: &str, c: &str) -> &mut Self {
        self.names = [a.into(), b.into(), c.into()];
        self
    }
    /// Uniform color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
    /// Colors from values through a colormap.
    pub fn values(&mut self, v: impl IntoValues, cm: Colormap) -> &mut Self {
        self.values = Some((v.into_values(), cm));
        self
    }
    /// Marker size in pixels.
    pub fn size(&mut self, s: f64) -> &mut Self {
        self.size = s;
        self
    }
    /// Legend label.
    pub fn label(&mut self, l: impl Into<String>) -> &mut Self {
        self.label = Some(l.into());
        self
    }
}

/// The triangle (top, bottom left, bottom right) inside a rectangle.
fn triangle(r: Rect, pad: f64) -> [(f64, f64); 3] {
    let side = (r.w - 2.0 * pad).min((r.h - 2.0 * pad) * 2.0 / 3f64.sqrt());
    let h = side * 3f64.sqrt() / 2.0;
    let (cx, top) = (r.x + r.w / 2.0, r.y + (r.h - h) / 2.0);
    [
        (cx, top),
        (cx - side / 2.0, top + h),
        (cx + side / 2.0, top + h),
    ]
}

fn bary(t: &[(f64, f64); 3], a: f64, b: f64, c: f64) -> (f64, f64) {
    (
        a * t[0].0 + b * t[1].0 + c * t[2].0,
        a * t[0].1 + b * t[1].1 + c * t[2].1,
    )
}

impl Series for Ternary {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let ts = ctx.theme.tick_size;
        let t = triangle(ctx.rect, ts * 3.2);
        ctx.polygon(&t, Some(ctx.theme.plot_background), None);
        // Grid every 10 %, labelled every 20 % along the edges.
        let mut grid = Shapes::default();
        for k in 1..10 {
            let f = k as f64 / 10.0;
            grid.line(&[bary(&t, f, 1.0 - f, 0.0), bary(&t, f, 0.0, 1.0 - f)]);
            grid.line(&[bary(&t, 1.0 - f, f, 0.0), bary(&t, 0.0, f, 1.0 - f)]);
            grid.line(&[bary(&t, 1.0 - f, 0.0, f), bary(&t, 0.0, 1.0 - f, f)]);
        }
        grid.stroke(ctx, Stroke::new(ctx.theme.grid, 1.0));
        ctx.polygon(&t, None, Some(Stroke::new(ctx.theme.axis, 1.2)));
        let tc = ctx.theme.text;
        for k in (2..=8).step_by(2) {
            let f = k as f64 / 10.0;
            let lab = format!("{:.0}", f * 100.0);
            // a on the left edge, b on the bottom edge, c on the right edge.
            let p = bary(&t, f, 1.0 - f, 0.0);
            ctx.text(
                p.0 - 6.0,
                p.1,
                lab.clone(),
                ts * 0.85,
                tc,
                Anchor::End,
                Baseline::Middle,
            );
            let p = bary(&t, 0.0, f, 1.0 - f);
            ctx.text(
                p.0,
                p.1 + 6.0,
                lab.clone(),
                ts * 0.85,
                tc,
                Anchor::Middle,
                Baseline::Top,
            );
            let p = bary(&t, 1.0 - f, 0.0, f);
            ctx.text(
                p.0 + 6.0,
                p.1,
                lab,
                ts * 0.85,
                tc,
                Anchor::Start,
                Baseline::Middle,
            );
        }
        let ls = ctx.theme.label_size;
        ctx.text(
            t[0].0,
            t[0].1 - 8.0,
            self.names[0].clone(),
            ls,
            tc,
            Anchor::Middle,
            Baseline::Bottom,
        );
        ctx.text(
            t[1].0 - 4.0,
            t[1].1 + ts * 1.6,
            self.names[1].clone(),
            ls,
            tc,
            Anchor::End,
            Baseline::Top,
        );
        ctx.text(
            t[2].0 + 4.0,
            t[2].1 + ts * 1.6,
            self.names[2].clone(),
            ls,
            tc,
            Anchor::Start,
            Baseline::Top,
        );
        let scale = self.values.as_ref().map(|(v, cm)| {
            let (a, b) = v
                .iter()
                .filter(|x| x.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                    (a.min(*x), b.max(*x))
                });
            (
                v,
                ColorScale {
                    colormap: *cm,
                    min: a,
                    max: b,
                    label: None,
                },
            )
        });
        for (i, p) in self.abc.iter().enumerate() {
            let s = p[0] + p[1] + p[2];
            if !(s.is_finite() && s > 0.0) {
                continue;
            }
            let (x, y) = bary(&t, p[0] / s, p[1] / s, p[2] / s);
            let c = scale.as_ref().map_or(self.color, |(v, cs)| {
                cs.color(v.get(i).copied().unwrap_or(f64::NAN))
            });
            ctx.push(Node::Circle {
                cx: x,
                cy: y,
                r: self.size / 2.0,
                fill: Some(c.with_alpha(0.85)),
                stroke: Some(Stroke::new(c.darken(0.3), 0.6)),
            });
        }
    }
    fn hit(&self, _x: &AxisMap, _y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let t = triangle(rect, 12.0 * 3.2);
        self.abc
            .iter()
            .filter_map(|p| {
                let s = p[0] + p[1] + p[2];
                (s > 0.0).then(|| {
                    let (x, y) = bary(&t, p[0] / s, p[1] / s, p[2] / s);
                    Hit {
                        px: x,
                        py: y,
                        label: format!(
                            "{} {:.0} %, {} {:.0} %, {} {:.0} %",
                            self.names[0],
                            p[0] / s * 100.0,
                            self.names[1],
                            p[1] / s * 100.0,
                            self.names[2],
                            p[2] / s * 100.0
                        ),
                        distance: (x - px).hypot(y - py),
                    }
                })
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn color_scale(&self) -> Option<ColorScale> {
        self.values.as_ref().map(|(v, cm)| {
            let (a, b) = v
                .iter()
                .filter(|x| x.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                    (a.min(*x), b.max(*x))
                });
            ColorScale {
                colormap: *cm,
                min: a,
                max: b,
                label: None,
            }
        })
    }
    fn legend(&self) -> Vec<super::LegendEntry> {
        self.label
            .iter()
            .map(|l| super::LegendEntry {
                label: l.clone(),
                symbol: super::LegendSymbol::Marker {
                    marker: Marker::Circle,
                    size: 8.0,
                    color: self.color,
                },
            })
            .collect()
    }
    fn kind(&self) -> &'static str {
        "ternary"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- 3D waterfall ------------------------------------------------------------

/// Spectra over time as a 3D waterfall: one curve per time, each hiding
/// what is behind it. Created by [`Axes::waterfall3d`].
#[derive(Clone, Debug)]
pub struct Waterfall3d {
    pub(crate) x: Vec<f64>,
    pub(crate) t: Vec<f64>,
    pub(crate) z: Vec<Vec<f64>>,
    pub(crate) colormap: Colormap,
    pub(crate) floor: f64,
}

impl Waterfall3d {
    /// Colormap along time.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
}

impl Series for Waterfall3d {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, _ctx: &mut Context) {}
    fn bounds3d(&self) -> Option<Bounds3> {
        let r = |v: &[f64]| {
            v.iter()
                .filter(|x| x.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                    (a.min(*x), b.max(*x))
                })
        };
        let zmax = self
            .z
            .iter()
            .flatten()
            .filter(|x| x.is_finite())
            .fold(f64::NEG_INFINITY, |a, b| a.max(*b));
        Some([r(&self.x), r(&self.t), (self.floor, zmax)])
    }
    fn draw3d(&self, scene: &mut Scene3d) {
        let n = self.t.len().min(self.z.len());
        for k in 0..n {
            let tt = self.t[k];
            let c = self.colormap.at(if n > 1 {
                k as f64 / (n - 1) as f64
            } else {
                0.5
            });
            let fill = c.mix(Color::WHITE, 0.55);
            let row = &self.z[k];
            let m = self.x.len().min(row.len());
            // Vertical curtain under the curve (hides the curves behind).
            for i in 0..m.saturating_sub(1) {
                if !(row[i].is_finite() && row[i + 1].is_finite()) {
                    continue;
                }
                scene.polygon(
                    &[
                        [self.x[i], tt, self.floor],
                        [self.x[i], tt, row[i]],
                        [self.x[i + 1], tt, row[i + 1]],
                        [self.x[i + 1], tt, self.floor],
                    ],
                    fill,
                    None,
                );
            }
            let pts: Vec<[f64; 3]> = (0..m)
                .filter(|&i| row[i].is_finite())
                .map(|i| [self.x[i], tt, row[i]])
                .collect();
            scene.polyline(&pts, c.darken(0.15), None, 1.4);
        }
    }
    fn kind(&self) -> &'static str {
        "3D waterfall"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- 3D vectors and bars --------------------------------------------------------

/// Arrows in 3D (vector fields, forces, velocities). Created by
/// [`Axes::quiver3d`].
#[derive(Clone, Debug)]
pub struct Quiver3d {
    pub(crate) p: Vec<[f64; 3]>,
    pub(crate) v: Vec<[f64; 3]>,
    pub(crate) scale: f64,
    pub(crate) colormap: Option<Colormap>,
    pub(crate) color: Color,
    pub(crate) width: f64,
}

impl Quiver3d {
    /// Arrow length per unit of vector length (default: longest arrow
    /// about one tenth of the box).
    pub fn scale(&mut self, s: f64) -> &mut Self {
        self.scale = s;
        self
    }
    /// Colors by magnitude.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = Some(cm);
        self
    }
    /// Uniform color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self.colormap = None;
        self
    }
}

impl Series for Quiver3d {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, _ctx: &mut Context) {}
    fn bounds3d(&self) -> Option<Bounds3> {
        let mut b = [(f64::INFINITY, f64::NEG_INFINITY); 3];
        for (p, v) in self.p.iter().zip(&self.v) {
            for k in 0..3 {
                let e = p[k] + v[k] * self.scale;
                b[k] = (b[k].0.min(p[k]).min(e), b[k].1.max(p[k]).max(e));
            }
        }
        b[0].0.is_finite().then_some(b)
    }
    fn draw3d(&self, scene: &mut Scene3d) {
        let mags: Vec<f64> = self
            .v
            .iter()
            .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
            .collect();
        let max = mags
            .iter()
            .copied()
            .filter(|m| m.is_finite())
            .fold(0.0, f64::max)
            .max(1e-300);
        for ((p, v), m) in self.p.iter().zip(&self.v).zip(&mags) {
            let e = [
                p[0] + v[0] * self.scale,
                p[1] + v[1] * self.scale,
                p[2] + v[2] * self.scale,
            ];
            let c = self.colormap.map_or(self.color, |cm| cm.at(m / max));
            scene.arrow(*p, e, c, self.width, 7.0);
        }
    }
    fn color_scale(&self) -> Option<ColorScale> {
        let max = self
            .v
            .iter()
            .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
            .fold(0.0, f64::max);
        self.colormap.map(|cm| ColorScale {
            colormap: cm,
            min: 0.0,
            max,
            label: Some("magnitude".into()),
        })
    }
    fn kind(&self) -> &'static str {
        "3D vectors"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// 3D bar chart: a box per (row, column) cell, height by value. Created by
/// [`Axes::bar3d`].
#[derive(Clone, Debug)]
pub struct Bar3d {
    pub(crate) z: Vec<Vec<f64>>,
    pub(crate) colormap: Option<Colormap>,
    pub(crate) colors: Vec<Color>,
    pub(crate) width: f64,
}

impl Bar3d {
    /// Colors by height through a colormap (default: one color per row).
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = Some(cm);
        self
    }
    /// Width of the bars (0..1 of a cell).
    pub fn width(&mut self, w: f64) -> &mut Self {
        self.width = w.clamp(0.05, 1.0);
        self
    }
}

impl Series for Bar3d {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, _ctx: &mut Context) {}
    fn bounds3d(&self) -> Option<Bounds3> {
        let rows = self.z.len();
        let cols = self.z.iter().map(|r| r.len()).max().unwrap_or(0);
        let hi = self
            .z
            .iter()
            .flatten()
            .filter(|v| v.is_finite())
            .fold(0.0f64, |a, b| a.max(*b));
        let lo = self
            .z
            .iter()
            .flatten()
            .filter(|v| v.is_finite())
            .fold(0.0f64, |a, b| a.min(*b));
        Some([
            (-0.5, cols as f64 - 0.5),
            (-0.5, rows as f64 - 0.5),
            (lo, hi),
        ])
    }
    fn draw3d(&self, scene: &mut Scene3d) {
        let hi = self
            .z
            .iter()
            .flatten()
            .filter(|v| v.is_finite())
            .fold(0.0f64, |a, b| a.max(*b))
            .max(1e-300);
        let h = self.width / 2.0;
        let cam = scene.projection().camera();
        for (r, row) in self.z.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                if !v.is_finite() || v == 0.0 {
                    continue;
                }
                let color = match self.colormap {
                    Some(cm) => cm.at(v / hi),
                    None => self.colors[r % self.colors.len()],
                };
                let (x0, x1, y0, y1, z0, z1) = (
                    c as f64 - h,
                    c as f64 + h,
                    r as f64 - h,
                    r as f64 + h,
                    0.0f64.min(v),
                    0.0f64.max(v),
                );
                // Six faces with outward normals; faces turned away skipped.
                let faces: [([[f64; 3]; 4], [f64; 3]); 6] = [
                    (
                        [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
                        [0.0, 0.0, 1.0],
                    ),
                    (
                        [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]],
                        [0.0, 0.0, -1.0],
                    ),
                    (
                        [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]],
                        [0.0, -1.0, 0.0],
                    ),
                    (
                        [[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]],
                        [0.0, 1.0, 0.0],
                    ),
                    (
                        [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]],
                        [-1.0, 0.0, 0.0],
                    ),
                    (
                        [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]],
                        [1.0, 0.0, 0.0],
                    ),
                ];
                for (q, n) in faces {
                    let nv = cam.view(n);
                    if nv[2] < 0.0 {
                        continue;
                    }
                    scene.face_data(&q, color, Some(nv));
                }
            }
        }
    }
    fn kind(&self) -> &'static str {
        "3D bars"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Western Electric rules 1 (beyond 3σ) and 2 (9 in a row on one side):
/// indices of the points that break them.
pub fn control_violations(values: &[f64], center: f64, sigma: f64) -> Vec<usize> {
    let mut out = Vec::new();
    let mut run = 0i32;
    for (i, &v) in values.iter().enumerate() {
        if !v.is_finite() {
            run = 0;
            continue;
        }
        let side = if v > center {
            1
        } else if v < center {
            -1
        } else {
            0
        };
        run = if side != 0 && run.signum() == side {
            run + side
        } else {
            side
        };
        if (v - center).abs() > 3.0 * sigma || run.abs() >= 9 {
            out.push(i);
        }
    }
    out
}

impl Axes {
    /// Streamlines of the field `f(x, y) = (u, v)` over a rectangle,
    /// colored by speed; `density` sets the spacing (about `density`
    /// lines across, 25 is a good start).
    pub fn streamplot(
        &mut self,
        f: impl Fn(f64, f64) -> (f64, f64),
        x_range: (f64, f64),
        y_range: (f64, f64),
        density: usize,
    ) -> &mut Streamplot {
        let lines = streamlines(&f, x_range, y_range, density);
        // Color range from the 2nd to the 98th percentile of the speed, so
        // singular points (charges, sources) do not wash out the colors.
        let mut all: Vec<f64> = lines
            .iter()
            .flat_map(|l| l.1.iter().copied())
            .filter(|s| s.is_finite())
            .collect();
        all.sort_by(f64::total_cmp);
        let speed = if all.is_empty() {
            (0.0, 1.0)
        } else {
            (
                crate::plot::stats::quantile_sorted(&all, 0.02),
                crate::plot::stats::quantile_sorted(&all, 0.98),
            )
        };
        // Speeds over more than a factor of 20: colors by their logarithm.
        let log = speed.0 > 0.0 && speed.1 / speed.0 > 20.0;
        let speed = if log {
            (speed.0.log10(), speed.1.log10())
        } else {
            speed
        };
        let color = self.next_color();
        self.x.limits(x_range.0, x_range.1);
        self.y.limits(y_range.0, y_range.1);
        self.add(Streamplot {
            lines,
            x_range,
            y_range,
            colormap: Some(Colormap::Viridis),
            color,
            width: 1.4,
            speed,
            log,
        })
    }

    /// Ternary diagram of compositions `(a, b, c)` (normalized to 100 %).
    pub fn ternary(
        &mut self,
        a: impl IntoValues,
        b: impl IntoValues,
        c: impl IntoValues,
    ) -> &mut Ternary {
        self.kind = AxesKind::Frameless;
        let (a, b, c) = (a.into_values(), b.into_values(), c.into_values());
        let abc = a
            .iter()
            .zip(&b)
            .zip(&c)
            .map(|((a, b), c)| [*a, *b, *c])
            .collect();
        let color = self.next_color();
        self.add(Ternary {
            abc,
            names: ["A".into(), "B".into(), "C".into()],
            color,
            values: None,
            size: 7.0,
            label: None,
        })
    }

    /// 3D waterfall of spectra `z[time][i]` over `x` (e.g. frequency) and
    /// `t` (time), each curve hiding the ones behind it.
    pub fn waterfall3d(
        &mut self,
        x: impl IntoValues,
        t: impl IntoValues,
        z: Vec<Vec<f64>>,
    ) -> &mut Waterfall3d {
        self.kind = AxesKind::ThreeD;
        let floor = z
            .iter()
            .flatten()
            .filter(|v| v.is_finite())
            .fold(f64::INFINITY, |a, b| a.min(*b));
        self.camera(-50.0, 25.0);
        self.lighting(false);
        self.add(Waterfall3d {
            x: x.into_values(),
            t: t.into_values(),
            z,
            colormap: Colormap::Viridis,
            floor: if floor.is_finite() { floor } else { 0.0 },
        })
    }

    /// Arrows from points `p` along vectors `v` in 3D, colored by magnitude.
    pub fn quiver3d(&mut self, p: Vec<[f64; 3]>, v: Vec<[f64; 3]>) -> &mut Quiver3d {
        self.kind = AxesKind::ThreeD;
        let ext = (0..3)
            .map(|k| {
                let (a, b) = p
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), q| {
                        (a.min(q[k]), b.max(q[k]))
                    });
                b - a
            })
            .fold(0.0, f64::max);
        // Longest arrow about the typical spacing of the points.
        let longest = v
            .iter()
            .map(|w| (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt())
            .fold(0.0, f64::max);
        let spacing = ext / (p.len().max(1) as f64).cbrt().max(1.0);
        let scale = if longest > 0.0 && ext > 0.0 {
            spacing * 0.9 / longest
        } else {
            1.0
        };
        let color = self.next_color();
        self.add(Quiver3d {
            p,
            v,
            scale,
            colormap: Some(Colormap::Viridis),
            color,
            width: 1.5,
        })
    }

    /// 3D bar chart of `z[row][col]` with named columns (x) and rows (y).
    pub fn bar3d(
        &mut self,
        columns: impl IntoLabels,
        rows: impl IntoLabels,
        z: Vec<Vec<f64>>,
    ) -> &mut Bar3d {
        self.kind = AxesKind::ThreeD;
        self.x.categories = columns.into_labels();
        self.y.categories = rows.into_labels();
        let colors = (0..z.len().max(1))
            .map(|i| self.defaults.palette[i % self.defaults.palette.len()])
            .collect();
        self.add(Bar3d {
            z,
            colormap: None,
            colors,
            width: 0.7,
        })
    }

    /// Statistical process control chart (individuals): the values, centre
    /// line, ±1/2/3σ zones (σ from the average moving range) and the points
    /// breaking the Western Electric rules 1 and 2 marked in red.
    pub fn control_chart(&mut self, values: impl IntoValues) -> &mut Line {
        let v = values.into_values();
        let f: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
        let center = f.iter().sum::<f64>() / f.len().max(1) as f64;
        let mr =
            f.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (f.len().max(2) - 1) as f64;
        let sigma = mr / 1.128;
        let x: Vec<f64> = (1..=v.len()).map(|i| i as f64).collect();
        let red = Color::rgb(214, 39, 40);
        for k in [1.0, 2.0] {
            self.hspan(center - k * sigma, center + k * sigma)
                .color(Color::rgb(44, 160, 44).with_alpha(0.07));
        }
        for (y, name) in [(center + 3.0 * sigma, "UCL"), (center - 3.0 * sigma, "LCL")] {
            self.hline(y)
                .color(red)
                .dash(Dash::Dashed)
                .label(format!("{name} {}", short(y)));
        }
        self.hline(center)
            .color(Color::rgb(60, 60, 60))
            .label(format!("mean {}", short(center)));
        let bad = control_violations(&v, center, sigma);
        let color = self.next_color();
        self.x.label.get_or_insert_with(|| "sample".into());
        if !bad.is_empty() {
            self.scatter(
                bad.iter().map(|&i| x[i]).collect::<Vec<_>>(),
                bad.iter().map(|&i| v[i]).collect::<Vec<_>>(),
            )
            .color(red)
            .size(11.0)
            .label("out of control");
        }
        let line = self
            .line(x, v)
            .color(color)
            .marker(Marker::Circle)
            .marker_size(5.0)
            .label("measurement");
        line
    }

    /// Pareto chart: categories sorted by count as bars, the cumulative
    /// percentage as a line on the right axis, and the 80 % line.
    pub fn pareto(&mut self, categories: impl IntoLabels, counts: impl IntoValues) -> &mut Line {
        let (names, counts) = (categories.into_labels(), counts.into_values());
        let mut idx: Vec<usize> = (0..names.len().min(counts.len())).collect();
        idx.sort_by(|a, b| counts[*b].total_cmp(&counts[*a]));
        let total: f64 = idx
            .iter()
            .map(|&i| counts[i].max(0.0))
            .sum::<f64>()
            .max(1e-300);
        let sorted_names: Vec<String> = idx.iter().map(|&i| names[i].clone()).collect();
        let sorted: Vec<f64> = idx.iter().map(|&i| counts[i]).collect();
        let mut acc = 0.0;
        let cum: Vec<f64> = sorted
            .iter()
            .map(|c| {
                acc += c.max(0.0);
                acc / total * 100.0
            })
            .collect();
        let bar_color = self.next_color();
        self.bar(sorted_names.clone(), sorted).color(bar_color);
        let pos: Vec<f64> = (0..sorted_names.len()).map(|i| i as f64).collect();
        self.y2.limits(0.0, 105.0);
        self.y2.format =
            crate::plot::TickFormat::Custom(std::sync::Arc::new(|v| format!("{v:.0} %")));
        let red = Color::rgb(214, 39, 40);
        self.line(pos.clone(), vec![80.0; pos.len()])
            .secondary(true)
            .color(red.with_alpha(0.5))
            .dash(Dash::Dashed);
        self.y.label.get_or_insert_with(|| "count".into());
        self.y2.label.get_or_insert_with(|| "cumulative %".into());
        self.legend(crate::plot::LegendPosition::Hidden);
        self.line(pos, cum)
            .secondary(true)
            .color(red)
            .marker(Marker::Circle)
            .width(2.0)
            .label("cumulative %")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn streamlines_follow_rotation() {
        let l = streamlines(&|x, y| (-y, x), (-1.0, 1.0), (-1.0, 1.0), 20);
        assert!(!l.is_empty());
        // Points of a rotation field keep their distance from the centre.
        for (pts, _) in l.iter().take(5) {
            let r0 = pts[0].0.hypot(pts[0].1);
            for p in pts {
                assert!((p.0.hypot(p.1) - r0).abs() < 0.05, "{r0} {:?}", p);
            }
        }
    }

    #[test]
    fn control_rules() {
        let mut v = vec![0.0, 1.0, -1.0, 0.5, -0.5, 10.0];
        v.extend([0.2; 9]);
        let bad = control_violations(&v, 0.0, 1.0);
        // Index 5 is beyond 3σ; from index 5 on nine values lie above the
        // centre, so the run rule fires at index 13.
        assert!(bad.contains(&5));
        assert!(bad.contains(&13) && bad.contains(&14));
        assert!(!bad.contains(&12));
    }

    #[test]
    fn all_render() {
        let mut fig = Figure::new(1200, 800);
        fig.grid(2, 4);
        fig.at(0, 0)
            .streamplot(|x, y| (y, -x - 0.3 * y), (-2.0, 2.0), (-2.0, 2.0), 20);
        fig.at(0, 1).ternary([0.2, 0.5], [0.3, 0.2], [0.5, 0.3]);
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let z: Vec<Vec<f64>> = (0..10)
            .map(|t| x.iter().map(|v| (v / 5.0 + t as f64).sin()).collect())
            .collect();
        fig.at(0, 2).waterfall3d(&x, (0..10).map(|t| t as f64), z);
        fig.at(0, 3).control_chart([1.0, 2.0, 1.5, 9.0, 1.2]);
        fig.at(1, 0).pareto(["a", "b", "c"], [5.0, 20.0, 9.0]);
        fig.at(1, 1).quiver3d(
            vec![[0.0; 3], [1.0, 1.0, 1.0]],
            vec![[1.0, 0.0, 0.0], [0.0, 1.0, 1.0]],
        );
        fig.at(1, 2)
            .bar3d(["a", "b"], ["x", "y"], vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
    }
}
