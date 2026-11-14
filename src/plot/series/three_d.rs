//! 3D charts (orthographic projection): surface, wireframe, scatter, line.

use super::{short, ColorScale, Context, Extent, Hit, IntoValues, Rect, Series};
use crate::plot::axes::{Axes, AxesKind};
use crate::plot::axis::AxisMap;
use crate::plot::color::{Color, Colormap};
use crate::plot::scene::{Anchor, Baseline, Node, Stroke};
use crate::plot::ticks;
use std::any::Any;

/// Viewing angles and data box of a 3D chart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Rotation around the vertical axis in degrees.
    pub azimuth: f64,
    /// Angle above the x/y plane in degrees.
    pub elevation: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            azimuth: -60.0,
            elevation: 30.0,
        }
    }
}

type Bounds3 = [(f64, f64); 3];

struct Projector {
    bounds: Bounds3,
    cam: Camera,
    scale: f64,
    center: (f64, f64),
}

impl Projector {
    fn new(bounds: Bounds3, cam: Camera, r: Rect) -> Self {
        let mut p = Self {
            bounds,
            cam,
            scale: 1.0,
            center: (0.0, 0.0),
        };
        // Fit the projected unit cube into the rect.
        let corners: Vec<(f64, f64)> = (0..8)
            .map(|i| {
                let c = [
                    (i & 1) as f64 * 2.0 - 1.0,
                    ((i >> 1) & 1) as f64 * 2.0 - 1.0,
                    ((i >> 2) & 1) as f64 * 2.0 - 1.0,
                ];
                let (x, y, _) = p.rotate(c);
                (x, y)
            })
            .collect();
        let (x0, x1) = corners
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), c| {
                (a.min(c.0), b.max(c.0))
            });
        let (y0, y1) = corners
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), c| {
                (a.min(c.1), b.max(c.1))
            });
        let margin = 0.82;
        p.scale = (r.w / (x1 - x0)).min(r.h / (y1 - y0)) * margin;
        let (cx, cy) = r.center();
        p.center = (
            cx - (x0 + x1) / 2.0 * p.scale,
            cy + (y0 + y1) / 2.0 * p.scale,
        );
        p
    }

    fn norm(&self, v: [f64; 3]) -> [f64; 3] {
        let mut o = [0.0; 3];
        for i in 0..3 {
            let (a, b) = self.bounds[i];
            o[i] = if b > a {
                (v[i] - a) / (b - a) * 2.0 - 1.0
            } else {
                0.0
            };
        }
        o[2] *= 0.75; // flatter box like matplotlib
        o
    }

    /// Rotated coordinates (screen x, screen y up, depth towards viewer).
    fn rotate(&self, c: [f64; 3]) -> (f64, f64, f64) {
        let (az, el) = (
            self.cam.azimuth.to_radians(),
            self.cam.elevation.to_radians(),
        );
        let x1 = c[0] * az.cos() - c[1] * az.sin();
        let y1 = c[0] * az.sin() + c[1] * az.cos();
        let sy = c[2] * el.cos() + y1 * el.sin();
        let depth = -y1 * el.cos() + c[2] * el.sin();
        (x1, sy, depth)
    }

    fn project(&self, v: [f64; 3]) -> (f64, f64, f64) {
        let (x, y, d) = self.rotate(self.norm(v));
        (
            self.center.0 + x * self.scale,
            self.center.1 - y * self.scale,
            d,
        )
    }

    fn project_unit(&self, c: [f64; 3]) -> (f64, f64, f64) {
        let c = [c[0], c[1], c[2] * 0.75];
        let (x, y, d) = self.rotate(c);
        (
            self.center.0 + x * self.scale,
            self.center.1 - y * self.scale,
            d,
        )
    }
}

/// Draws the back panes, grid and axis labels of the 3D box.
fn draw_box(ctx: &mut Context, p: &Projector, labels: &[Option<String>; 3]) {
    let grid = Stroke::new(ctx.theme.grid, 1.0);
    let axis = Stroke::new(ctx.theme.axis, 1.0);
    // Back panes: for each axis pick the side facing away from the viewer.
    let side = |i: usize| -> f64 {
        let mut a = [0.0; 3];
        a[i] = 1.0;
        let (_, _, d1) = p.project_unit(a);
        let (_, _, d0) = p.project_unit([0.0; 3]);
        if d1 > d0 {
            -1.0
        } else {
            1.0
        }
    };
    let (sx, sy, sz) = (side(0), side(1), -1.0);
    let panes = [
        // (fixed axis, fixed value)
        (0usize, sx),
        (1usize, sy),
        (2usize, sz),
    ];
    for (axis_i, v) in panes {
        let (u, w) = match axis_i {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let corner = |a: f64, b: f64| {
            let mut c = [0.0; 3];
            c[axis_i] = v;
            c[u] = a;
            c[w] = b;
            let (x, y, _) = p.project_unit(c);
            (x, y)
        };
        let pts = [
            corner(-1.0, -1.0),
            corner(1.0, -1.0),
            corner(1.0, 1.0),
            corner(-1.0, 1.0),
        ];
        ctx.polygon(
            &pts,
            Some(ctx.theme.plot_background.darken(0.03)),
            Some(grid.clone()),
        );
        for k in 1..4 {
            let t = -1.0 + k as f64 * 0.5;
            ctx.polyline(&[corner(t, -1.0), corner(t, 1.0)], grid.clone());
            ctx.polyline(&[corner(-1.0, t), corner(1.0, t)], grid.clone());
        }
    }
    // Tick labels along three front edges.
    let ts = ctx.theme.tick_size * 0.85;
    for i in 0..3 {
        let (lo, hi) = p.bounds[i];
        let t = ticks::linear_ticks(lo, hi, 4);
        let mut base = [0.0; 3];
        match i {
            0 => base = [0.0, -sy, -1.0],
            1 => base = [-sx, 0.0, -1.0],
            _ => base[0] = sx.min(0.0).abs() * 2.0 - 1.0,
        }
        if i == 2 {
            base = [-sx, sy, 0.0];
        }
        let mut endpoints = Vec::new();
        for v in &t.0 {
            let f = if hi > lo {
                (v - lo) / (hi - lo) * 2.0 - 1.0
            } else {
                0.0
            };
            let mut c = base;
            c[i] = f;
            let (x, y, _) = p.project_unit(c);
            let label = ticks::format_value(
                *v,
                &ticks::TickFormat::Auto,
                &crate::plot::Scale::Linear,
                t.1,
                lo.abs().max(hi.abs()),
                None,
            );
            let anchor = if i == 2 { Anchor::End } else { Anchor::Middle };
            let dx = if i == 2 { -6.0 } else { 0.0 };
            ctx.text(
                x + dx,
                y + if i == 2 { 0.0 } else { 10.0 },
                label,
                ts,
                ctx.theme.text,
                anchor,
                Baseline::Middle,
            );
            endpoints.push((x, y));
        }
        if let (Some(a), Some(b)) = (endpoints.first(), endpoints.last()) {
            ctx.polyline(&[*a, *b], axis.clone());
            if let Some(l) = &labels[i] {
                let (mx, my) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let (x, y) = if i == 2 {
                    (mx - 40.0, my)
                } else {
                    (mx, my + 28.0)
                };
                ctx.text(
                    x,
                    y,
                    l.clone(),
                    ctx.theme.label_size,
                    ctx.theme.text,
                    Anchor::Middle,
                    Baseline::Middle,
                );
            }
        }
    }
}

/// Surface or wireframe `z[row][col]` over `x_range × y_range` (row 0 at
/// `y_range.0`). Created by [`Axes::surface`] / [`Axes::surface_fn`].
#[derive(Clone, Debug)]
pub struct Surface {
    pub(crate) z: Vec<Vec<f64>>,
    pub(crate) x_range: (f64, f64),
    pub(crate) y_range: (f64, f64),
    pub(crate) colormap: Colormap,
    pub(crate) wireframe: bool,
    pub(crate) camera: Camera,
    pub(crate) labels: [Option<String>; 3],
    pub(crate) colorbar: Option<String>,
}

impl Surface {
    /// Viewing angles in degrees.
    pub fn camera(&mut self, azimuth: f64, elevation: f64) -> &mut Self {
        self.camera = Camera {
            azimuth,
            elevation: elevation.clamp(-90.0, 90.0),
        };
        self
    }
    /// Lines only.
    pub fn wireframe(&mut self, on: bool) -> &mut Self {
        self.wireframe = on;
        self
    }
    /// Colormap.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
    /// Axis labels.
    pub fn labels(&mut self, x: &str, y: &str, z: &str) -> &mut Self {
        self.labels = [Some(x.into()), Some(y.into()), Some(z.into())];
        self
    }
    /// Shows a colorbar.
    pub fn colorbar(&mut self, label: impl Into<String>) -> &mut Self {
        self.colorbar = Some(label.into());
        self
    }
    fn z_range(&self) -> (f64, f64) {
        let (a, b) = self
            .z
            .iter()
            .flatten()
            .copied()
            .filter(|v| v.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(v), b.max(v))
            });
        if a.is_finite() {
            (a, b)
        } else {
            (0.0, 1.0)
        }
    }
    fn grid_point(&self, r: usize, c: usize) -> [f64; 3] {
        let rows = self.z.len();
        let cols = self.z.iter().map(|r| r.len()).min().unwrap_or(0);
        let x = self.x_range.0
            + (self.x_range.1 - self.x_range.0) * c as f64 / (cols.max(2) - 1) as f64;
        let y = self.y_range.0
            + (self.y_range.1 - self.y_range.0) * r as f64 / (rows.max(2) - 1) as f64;
        [x, y, self.z[r][c]]
    }
}

impl Series for Surface {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let rows = self.z.len();
        let cols = self.z.iter().map(|r| r.len()).min().unwrap_or(0);
        if rows < 2 || cols < 2 {
            return;
        }
        let zr = self.z_range();
        let p = Projector::new([self.x_range, self.y_range, zr], self.camera, ctx.rect);
        draw_box(ctx, &p, &self.labels);
        let cs = ColorScale {
            colormap: self.colormap,
            min: zr.0,
            max: zr.1,
            label: None,
        };
        // Painter's algorithm: far quads first.
        let mut quads = Vec::with_capacity(rows * cols);
        for r in 0..rows - 1 {
            for c in 0..cols - 1 {
                let v = [
                    self.grid_point(r, c),
                    self.grid_point(r, c + 1),
                    self.grid_point(r + 1, c + 1),
                    self.grid_point(r + 1, c),
                ];
                if v.iter().any(|q| !q[2].is_finite()) {
                    continue;
                }
                let pr: Vec<(f64, f64, f64)> = v.iter().map(|q| p.project(*q)).collect();
                let depth = pr.iter().map(|q| q.2).sum::<f64>() / 4.0;
                let zm = v.iter().map(|q| q[2]).sum::<f64>() / 4.0;
                quads.push((depth, pr, zm));
            }
        }
        quads.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, pr, zm) in quads {
            let pts: Vec<(f64, f64)> = pr.iter().map(|q| (q.0, q.1)).collect();
            let c = cs.color(zm);
            if self.wireframe {
                let mut closed = pts.clone();
                closed.push(pts[0]);
                ctx.polyline(&closed, Stroke::new(c, 0.8));
            } else {
                ctx.polygon(&pts, Some(c), Some(Stroke::new(c.darken(0.25), 0.4)));
            }
        }
    }
    fn color_scale(&self) -> Option<ColorScale> {
        let (a, b) = self.z_range();
        self.colorbar.as_ref().map(|l| ColorScale {
            colormap: self.colormap,
            min: a,
            max: b,
            label: Some(l.clone()),
        })
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// 3D points or line. Created by [`Axes::scatter3d`] / [`Axes::line3d`].
#[derive(Clone, Debug)]
pub struct Points3d {
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) z: Vec<f64>,
    pub(crate) color: Color,
    pub(crate) colormap: Option<Colormap>,
    pub(crate) line: bool,
    pub(crate) size: f64,
    pub(crate) camera: Camera,
    pub(crate) labels: [Option<String>; 3],
}

impl Points3d {
    /// Viewing angles in degrees.
    pub fn camera(&mut self, azimuth: f64, elevation: f64) -> &mut Self {
        self.camera = Camera {
            azimuth,
            elevation: elevation.clamp(-90.0, 90.0),
        };
        self
    }
    /// Colors points by z with a colormap.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = Some(cm);
        self
    }
    /// Uniform color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
    /// Marker size / line width.
    pub fn size(&mut self, s: f64) -> &mut Self {
        self.size = s.max(0.5);
        self
    }
    /// Axis labels.
    pub fn labels(&mut self, x: &str, y: &str, z: &str) -> &mut Self {
        self.labels = [Some(x.into()), Some(y.into()), Some(z.into())];
        self
    }
    fn bounds(&self) -> Bounds3 {
        let r = |v: &[f64]| {
            let (a, b) = v
                .iter()
                .copied()
                .filter(|t| t.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), t| {
                    (a.min(t), b.max(t))
                });
            if a.is_finite() {
                if b > a {
                    (a, b)
                } else {
                    (a - 0.5, a + 0.5)
                }
            } else {
                (0.0, 1.0)
            }
        };
        [r(&self.x), r(&self.y), r(&self.z)]
    }
}

impl Series for Points3d {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, ctx: &mut Context) {
        let b = self.bounds();
        let p = Projector::new(b, self.camera, ctx.rect);
        draw_box(ctx, &p, &self.labels);
        let n = self.x.len().min(self.y.len()).min(self.z.len());
        let pts: Vec<(f64, f64, f64, f64)> = (0..n)
            .map(|i| {
                let (x, y, d) = p.project([self.x[i], self.y[i], self.z[i]]);
                (x, y, d, self.z[i])
            })
            .collect();
        let cs = self.colormap.map(|cm| ColorScale {
            colormap: cm,
            min: b[2].0,
            max: b[2].1,
            label: None,
        });
        if self.line {
            for w in pts.windows(2) {
                let c = cs
                    .as_ref()
                    .map_or(self.color, |s| s.color((w[0].3 + w[1].3) / 2.0));
                ctx.polyline(
                    &[(w[0].0, w[0].1), (w[1].0, w[1].1)],
                    Stroke::new(c, self.size),
                );
            }
        } else {
            let mut order: Vec<usize> = (0..pts.len()).collect();
            order.sort_by(|a, b| pts[*a].2.total_cmp(&pts[*b].2));
            for i in order {
                let (x, y, _, z) = pts[i];
                let c = cs.as_ref().map_or(self.color, |s| s.color(z));
                ctx.push(Node::Circle {
                    cx: x,
                    cy: y,
                    r: self.size / 2.0,
                    fill: Some(c),
                    stroke: Some(Stroke::new(c.darken(0.3), 0.6)),
                });
            }
        }
    }
    fn hit(&self, _x: &AxisMap, _y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let p = Projector::new(self.bounds(), self.camera, rect);
        (0..self.x.len().min(self.y.len()).min(self.z.len()))
            .map(|i| {
                let (x, y, _) = p.project([self.x[i], self.y[i], self.z[i]]);
                Hit {
                    px: x,
                    py: y,
                    label: format!(
                        "({}, {}, {})",
                        short(self.x[i]),
                        short(self.y[i]),
                        short(self.z[i])
                    ),
                    distance: (x - px).hypot(y - py),
                }
            })
            .filter(|h| h.distance.is_finite())
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// 3D surface of `z[row][col]` over `x_range × y_range`.
    pub fn surface(
        &mut self,
        x_range: (f64, f64),
        y_range: (f64, f64),
        z: Vec<Vec<f64>>,
    ) -> &mut Surface {
        self.kind = AxesKind::Frameless;
        let cm = self.defaults.colormap;
        self.add(Surface {
            z,
            x_range,
            y_range,
            colormap: cm,
            wireframe: false,
            camera: Camera::default(),
            labels: [None, None, None],
            colorbar: None,
        })
    }

    /// 3D surface of `f(x, y)` sampled on an `n × n` grid.
    pub fn surface_fn(
        &mut self,
        f: impl Fn(f64, f64) -> f64,
        x_range: (f64, f64),
        y_range: (f64, f64),
        n: usize,
    ) -> &mut Surface {
        let n = n.clamp(2, 200);
        let z = (0..n)
            .map(|r| {
                let y = y_range.0 + (y_range.1 - y_range.0) * r as f64 / (n - 1) as f64;
                (0..n)
                    .map(|c| {
                        f(
                            x_range.0 + (x_range.1 - x_range.0) * c as f64 / (n - 1) as f64,
                            y,
                        )
                    })
                    .collect()
            })
            .collect();
        self.surface(x_range, y_range, z)
    }

    /// 3D scatter plot.
    pub fn scatter3d(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        z: impl IntoValues,
    ) -> &mut Points3d {
        self.kind = AxesKind::Frameless;
        let color = self.next_color();
        self.add(Points3d {
            x: x.into_values(),
            y: y.into_values(),
            z: z.into_values(),
            color,
            colormap: None,
            line: false,
            size: 7.0,
            camera: Camera::default(),
            labels: [None, None, None],
        })
    }

    /// 3D line (e.g. a trajectory).
    pub fn line3d(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        z: impl IntoValues,
    ) -> &mut Points3d {
        let p = self.scatter3d(x, y, z);
        p.line = true;
        p.size = 2.0;
        p
    }
}
