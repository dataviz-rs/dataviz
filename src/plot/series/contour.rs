//! Contour lines and filled contours (marching squares).

use super::Rect;
use super::{short, ColorScale, Context, Extent, Hit, Series};
use crate::plot::axes::Axes;
use crate::plot::axis::AxisMap;
use crate::plot::color::{Color, Colormap};
use crate::plot::scene::{Anchor, Baseline, Node, PathCmd, RasterImage, Stroke};
use crate::plot::ticks;
use std::any::Any;
use std::sync::Arc;

/// Contours of a field `z[row][col]` over a rectangle (row 0 at the bottom).
/// Created by [`Axes::contour`] / [`Axes::contour_fn`].
#[derive(Clone, Debug)]
pub struct Contour {
    pub(crate) z: Vec<Vec<f64>>,
    pub(crate) x_range: (f64, f64),
    pub(crate) y_range: (f64, f64),
    pub(crate) levels: Vec<f64>,
    pub(crate) filled: bool,
    pub(crate) lines: bool,
    pub(crate) colormap: Colormap,
    pub(crate) line_color: Option<Color>,
    pub(crate) width: f64,
    pub(crate) labels: bool,
    pub(crate) colorbar: Option<String>,
}

impl Contour {
    /// Number of automatically chosen levels.
    pub fn level_count(&mut self, n: usize) -> &mut Self {
        let (lo, hi) = self.z_range();
        self.levels = ticks::linear_ticks(lo, hi, n.max(1)).0;
        self
    }
    /// Explicit levels.
    pub fn levels(&mut self, levels: impl super::IntoValues) -> &mut Self {
        let mut l = levels.into_values();
        l.sort_by(f64::total_cmp);
        self.levels = l;
        self
    }
    /// Filled bands between levels.
    pub fn filled(&mut self, on: bool) -> &mut Self {
        self.filled = on;
        self
    }
    /// Contour lines (default on).
    pub fn lines(&mut self, on: bool) -> &mut Self {
        self.lines = on;
        self
    }
    /// Colormap.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
    /// Single color for all lines (default: colormap, or dark over fills).
    pub fn line_color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.line_color = Some(c.into());
        self
    }
    /// Line width.
    pub fn width(&mut self, w: f64) -> &mut Self {
        self.width = w.max(0.0);
        self
    }
    /// Writes the level value on each contour.
    pub fn labels(&mut self, on: bool) -> &mut Self {
        self.labels = on;
        self
    }
    /// Shows a colorbar with this label.
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

    fn dims(&self) -> (usize, usize) {
        (
            self.z.len(),
            self.z.iter().map(|r| r.len()).min().unwrap_or(0),
        )
    }

    fn at(&self, r: usize, c: usize) -> (f64, f64) {
        let (rows, cols) = self.dims();
        let x = self.x_range.0
            + (self.x_range.1 - self.x_range.0) * c as f64 / (cols.max(2) - 1) as f64;
        let y = self.y_range.0
            + (self.y_range.1 - self.y_range.0) * r as f64 / (rows.max(2) - 1) as f64;
        (x, y)
    }

    /// Line segments (data coordinates) of one level.
    pub fn segments(&self, level: f64) -> Vec<((f64, f64), (f64, f64))> {
        let (rows, cols) = self.dims();
        let mut out = Vec::new();
        if rows < 2 || cols < 2 {
            return out;
        }
        for r in 0..rows - 1 {
            for c in 0..cols - 1 {
                let v = [
                    self.z[r][c],
                    self.z[r][c + 1],
                    self.z[r + 1][c + 1],
                    self.z[r + 1][c],
                ];
                if v.iter().any(|x| !x.is_finite()) {
                    continue;
                }
                let p = [
                    self.at(r, c),
                    self.at(r, c + 1),
                    self.at(r + 1, c + 1),
                    self.at(r + 1, c),
                ];
                let idx = v
                    .iter()
                    .enumerate()
                    .fold(0, |acc, (i, x)| acc | ((*x > level) as usize) << i);
                let edge = |a: usize, b: usize| {
                    let t = (level - v[a]) / (v[b] - v[a]);
                    let t = if t.is_finite() {
                        t.clamp(0.0, 1.0)
                    } else {
                        0.5
                    };
                    (
                        p[a].0 + (p[b].0 - p[a].0) * t,
                        p[a].1 + (p[b].1 - p[a].1) * t,
                    )
                };
                // Edges: 0 = bottom (0-1), 1 = right (1-2), 2 = top (2-3), 3 = left (3-0).
                let e = |k: usize| match k {
                    0 => edge(0, 1),
                    1 => edge(1, 2),
                    2 => edge(2, 3),
                    _ => edge(3, 0),
                };
                let center = (v[0] + v[1] + v[2] + v[3]) / 4.0 > level;
                let pairs: &[(usize, usize)] = match idx {
                    0 | 15 => &[],
                    1 | 14 => &[(3, 0)],
                    2 | 13 => &[(0, 1)],
                    3 | 12 => &[(3, 1)],
                    4 | 11 => &[(1, 2)],
                    5 => {
                        if center {
                            &[(3, 2), (0, 1)]
                        } else {
                            &[(3, 0), (1, 2)]
                        }
                    }
                    6 | 9 => &[(0, 2)],
                    7 | 8 => &[(3, 2)],
                    10 => {
                        if center {
                            &[(3, 0), (1, 2)]
                        } else {
                            &[(3, 2), (0, 1)]
                        }
                    }
                    _ => &[],
                };
                for &(a, b) in pairs {
                    out.push((e(a), e(b)));
                }
            }
        }
        out
    }

    fn scale(&self) -> ColorScale {
        let (lo, hi) = match (self.levels.first(), self.levels.last()) {
            (Some(a), Some(b)) if b > a => (*a, *b),
            _ => self.z_range(),
        };
        ColorScale {
            colormap: self.colormap,
            min: lo,
            max: hi,
            label: self.colorbar.clone(),
        }
    }

    /// Bilinear value at fractional grid position.
    fn sample(&self, fr: f64, fc: f64) -> f64 {
        let (rows, cols) = self.dims();
        let r0 = (fr.floor() as usize).min(rows - 1);
        let c0 = (fc.floor() as usize).min(cols - 1);
        let r1 = (r0 + 1).min(rows - 1);
        let c1 = (c0 + 1).min(cols - 1);
        let (tr, tc) = (fr - r0 as f64, fc - c0 as f64);
        let top = self.z[r0][c0] * (1.0 - tc) + self.z[r0][c1] * tc;
        let bot = self.z[r1][c0] * (1.0 - tc) + self.z[r1][c1] * tc;
        top * (1.0 - tr) + bot * tr
    }
}

impl Series for Contour {
    fn extent(&self) -> Option<Extent> {
        let (rows, cols) = self.dims();
        if rows < 2 || cols < 2 {
            return None;
        }
        let mut e =
            Extent::point(self.x_range.0, self.y_range.0).include(self.x_range.1, self.y_range.1);
        e.sticky_x_min = true;
        e.sticky_x_max = true;
        e.sticky_y_min = true;
        e.sticky_y_max = true;
        Some(e)
    }

    fn draw(&self, ctx: &mut Context) {
        let (rows, cols) = self.dims();
        if rows < 2 || cols < 2 || self.levels.is_empty() {
            return;
        }
        let s = self.scale();
        if self.filled {
            // Banded raster at up to ~2 px per output pixel.
            let (a, b) = ctx.point(self.x_range.0, self.y_range.1);
            let (c, d) = ctx.point(self.x_range.1, self.y_range.0);
            let (w, h) = (
                (c - a).abs().round().clamp(2.0, 1200.0) as u32,
                (d - b).abs().round().clamp(2.0, 1200.0) as u32,
            );
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            let band = |v: f64| -> Option<Color> {
                if !v.is_finite() {
                    return None;
                }
                let i = self.levels.partition_point(|l| *l <= v);
                let (lo, hi) = match i {
                    0 => (self.levels[0], self.levels[0]),
                    i if i >= self.levels.len() => (self.levels[i - 1], self.levels[i - 1]),
                    i => (self.levels[i - 1], self.levels[i]),
                };
                Some(s.color((lo + hi) / 2.0))
            };
            for py in 0..h {
                // Screen top = y_range.1 = last row.
                let fr = (rows - 1) as f64 * (1.0 - py as f64 / (h - 1) as f64);
                for px in 0..w {
                    let fc = (cols - 1) as f64 * px as f64 / (w - 1) as f64;
                    match band(self.sample(fr, fc)) {
                        Some(c) => rgba.extend_from_slice(&[c.r, c.g, c.b, 255]),
                        None => rgba.extend_from_slice(&[0, 0, 0, 0]),
                    }
                }
            }
            ctx.push(Node::Image {
                x: a.min(c),
                y: b.min(d),
                w: (c - a).abs(),
                h: (d - b).abs(),
                image: Arc::new(RasterImage {
                    width: w,
                    height: h,
                    rgba,
                }),
                smooth: false,
            });
        }
        if self.lines {
            let size = ctx.theme.annotation_size;
            let mut placed: Vec<(f64, f64)> = Vec::new();
            for level in &self.levels {
                let segs = self.segments(*level);
                if segs.is_empty() {
                    continue;
                }
                let color = self.line_color.unwrap_or(if self.filled {
                    Color::rgba(0, 0, 0, 0.6)
                } else {
                    s.color(*level)
                });
                let mut cmds = Vec::with_capacity(segs.len() * 2);
                for (p, q) in &segs {
                    let (a, b) = ctx.point(p.0, p.1);
                    let (c, d) = ctx.point(q.0, q.1);
                    cmds.push(PathCmd::MoveTo(a, b));
                    cmds.push(PathCmd::LineTo(c, d));
                }
                ctx.push(Node::Path {
                    cmds,
                    fill: None,
                    stroke: Some(Stroke::new(color, self.width)),
                });
                if self.labels {
                    // The candidate farthest from labels already placed (and
                    // inside the plot, away from its edges).
                    let r = ctx.rect.inset(size * 1.5);
                    let step = (segs.len() / 40).max(1);
                    let best = segs
                        .iter()
                        .step_by(step)
                        .map(|(p, q)| ctx.point((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0))
                        .filter(|(x, y)| r.contains(*x, *y))
                        .max_by(|a, b| {
                            let d = |p: &(f64, f64)| {
                                placed
                                    .iter()
                                    .map(|q| (p.0 - q.0).hypot(p.1 - q.1))
                                    .fold(f64::INFINITY, f64::min)
                            };
                            d(a).total_cmp(&d(b))
                        });
                    let Some((x, y)) = best else { continue };
                    placed.push((x, y));
                    let text = short(*level);
                    let w = ctx.text_width(&text, size);
                    ctx.rect(
                        x - w / 2.0 - 2.0,
                        y - size * 0.6,
                        x + w / 2.0 + 2.0,
                        y + size * 0.6,
                        Some(ctx.theme.plot_background.with_alpha(0.85)),
                        None,
                    );
                    ctx.text(
                        x,
                        y,
                        text,
                        size,
                        ctx.theme.text,
                        Anchor::Middle,
                        Baseline::Middle,
                    );
                }
            }
        }
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let (rows, cols) = self.dims();
        if rows < 2 || cols < 2 {
            return None;
        }
        let (vx, vy) = (x.value(px), y.value(py));
        let fc = (vx - self.x_range.0) / (self.x_range.1 - self.x_range.0) * (cols - 1) as f64;
        let fr = (vy - self.y_range.0) / (self.y_range.1 - self.y_range.0) * (rows - 1) as f64;
        if !(0.0..=(cols - 1) as f64).contains(&fc) || !(0.0..=(rows - 1) as f64).contains(&fr) {
            return None;
        }
        Some(Hit {
            px,
            py,
            label: format!(
                "({}, {}) = {}",
                short(vx),
                short(vy),
                short(self.sample(fr, fc))
            ),
            distance: 0.0,
        })
    }

    fn color_scale(&self) -> Option<ColorScale> {
        (self.filled || self.colorbar.is_some()).then(|| self.scale())
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// Contours of `z[row][col]` sampled on a regular grid over
    /// `x_range × y_range` (row 0 at `y_range.0`).
    pub fn contour(
        &mut self,
        x_range: (f64, f64),
        y_range: (f64, f64),
        z: Vec<Vec<f64>>,
    ) -> &mut Contour {
        let cm = self.defaults.colormap;
        let mut c = Contour {
            z,
            x_range,
            y_range,
            levels: Vec::new(),
            filled: false,
            lines: true,
            colormap: cm,
            line_color: None,
            width: 1.2,
            labels: false,
            colorbar: None,
        };
        c.level_count(10);
        self.add(c)
    }

    /// Contours of `f(x, y)` sampled on an `n × n` grid.
    pub fn contour_fn(
        &mut self,
        f: impl Fn(f64, f64) -> f64,
        x_range: (f64, f64),
        y_range: (f64, f64),
        n: usize,
    ) -> &mut Contour {
        let n = n.clamp(2, 1000);
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
        self.contour(x_range, y_range, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_contour() {
        let n = 41;
        let z: Vec<Vec<f64>> = (0..n)
            .map(|r| {
                (0..n)
                    .map(|c| {
                        let (x, y) = (c as f64 / 20.0 - 1.0, r as f64 / 20.0 - 1.0);
                        x * x + y * y
                    })
                    .collect()
            })
            .collect();
        let c = Contour {
            z,
            x_range: (-1.0, 1.0),
            y_range: (-1.0, 1.0),
            levels: vec![0.25],
            filled: false,
            lines: true,
            colormap: Colormap::Viridis,
            line_color: None,
            width: 1.0,
            labels: false,
            colorbar: None,
        };
        let segs = c.segments(0.25);
        assert!(segs.len() > 20);
        for (p, _) in segs {
            let r = (p.0 * p.0 + p.1 * p.1).sqrt();
            assert!((r - 0.5).abs() < 0.02, "{r}");
        }
    }
}
