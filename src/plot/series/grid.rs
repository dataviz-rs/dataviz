//! Grid-based charts: heatmaps, images, 2D histograms, hexbin.

use super::Rect;
use super::{short, ColorScale, Context, Extent, Hit, IntoLabels, IntoValues, Series};
use crate::plot::axes::Axes;
use crate::plot::axis::AxisMap;
use crate::plot::color::{Color, Colormap};
use crate::plot::scene::{Anchor, Baseline, Node, RasterImage, Stroke};
use std::any::Any;
use std::sync::Arc;

/// A matrix of values drawn as colored cells. Created by [`Axes::heatmap`]
/// and [`Axes::hist2d`].
#[derive(Clone, Debug)]
pub struct Heatmap {
    pub(crate) z: Vec<Vec<f64>>,
    pub(crate) x_range: (f64, f64),
    pub(crate) y_range: (f64, f64),
    /// Row 0 at the top (matrix layout) instead of the bottom.
    pub(crate) top_down: bool,
    pub(crate) colormap: Colormap,
    pub(crate) range: Option<(f64, f64)>,
    pub(crate) annotate: bool,
    pub(crate) decimals: Option<usize>,
    pub(crate) colorbar: Option<String>,
    pub(crate) gaps: f64,
    pub(crate) smooth: bool,
}

impl Heatmap {
    /// Colormap.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
    /// Fixed value range for the colors.
    pub fn range(&mut self, min: f64, max: f64) -> &mut Self {
        self.range = Some((min, max));
        self
    }
    /// Writes each value in its cell.
    pub fn annotate(&mut self, on: bool) -> &mut Self {
        self.annotate = on;
        self
    }
    /// Decimals of annotations.
    pub fn decimals(&mut self, d: usize) -> &mut Self {
        self.decimals = Some(d);
        self
    }
    /// Colorbar label (a colorbar is shown by default).
    pub fn colorbar(&mut self, label: impl Into<String>) -> &mut Self {
        self.colorbar = Some(label.into());
        self
    }
    /// Gap between cells in pixels (draws cells as rectangles).
    pub fn gaps(&mut self, px: f64) -> &mut Self {
        self.gaps = px.max(0.0);
        self
    }
    /// Interpolates between cells (for images of continuous fields).
    pub fn smooth(&mut self, on: bool) -> &mut Self {
        self.smooth = on;
        self
    }

    fn dims(&self) -> (usize, usize) {
        let rows = self.z.len();
        let cols = self.z.iter().map(|r| r.len()).max().unwrap_or(0);
        (rows, cols)
    }

    pub(crate) fn scale(&self) -> ColorScale {
        let (min, max) = self.range.unwrap_or_else(|| {
            self.z
                .iter()
                .flatten()
                .copied()
                .filter(|v| v.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                    (a.min(v), b.max(v))
                })
        });
        let (min, max) = if min.is_finite() {
            (min, max)
        } else {
            (0.0, 1.0)
        };
        ColorScale {
            colormap: self.colormap,
            min,
            max,
            label: self.colorbar.clone(),
        }
    }

    /// Data rectangle of cell (row, col).
    fn cell(&self, r: usize, c: usize) -> (f64, f64, f64, f64) {
        let (rows, cols) = self.dims();
        let cw = (self.x_range.1 - self.x_range.0) / cols.max(1) as f64;
        let ch = (self.y_range.1 - self.y_range.0) / rows.max(1) as f64;
        let x0 = self.x_range.0 + c as f64 * cw;
        let ri = if self.top_down { r } else { rows - 1 - r };
        let y0 = self.y_range.0 + (rows - 1 - ri) as f64 * ch;
        (x0, y0, x0 + cw, y0 + ch)
    }
}

impl Series for Heatmap {
    fn extent(&self) -> Option<Extent> {
        let (rows, cols) = self.dims();
        if rows == 0 || cols == 0 {
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
        if rows == 0 || cols == 0 {
            return;
        }
        let s = self.scale();
        let missing = ctx.theme.plot_background;
        if self.gaps > 0.0 || ctx.x.scale.is_log() || ctx.y.scale.is_log() {
            for r in 0..rows {
                for c in 0..cols {
                    let v = self.z[r].get(c).copied().unwrap_or(f64::NAN);
                    let (x0, y0, x1, y1) = self.cell(r, c);
                    let (a, b) = ctx.point(x0, y0);
                    let (cx, d) = ctx.point(x1, y1);
                    let g = self.gaps / 2.0;
                    let fill = if v.is_finite() { s.color(v) } else { missing };
                    ctx.rect(
                        a.min(cx) + g,
                        b.min(d) + g,
                        a.max(cx) - g,
                        b.max(d) - g,
                        Some(fill),
                        None,
                    );
                }
            }
        } else {
            let mut rgba = Vec::with_capacity(rows * cols * 4);
            // Image rows go top to bottom on screen.
            let (top_y, bottom_y) = (ctx.y.px(self.y_range.1), ctx.y.px(self.y_range.0));
            let screen_top_down = top_y <= bottom_y;
            let order: Vec<usize> = if self.top_down == screen_top_down {
                (0..rows).collect()
            } else {
                (0..rows).rev().collect()
            };
            let (left, right) = (ctx.x.px(self.x_range.0), ctx.x.px(self.x_range.1));
            for r in order {
                let row: Vec<usize> = if left <= right {
                    (0..cols).collect()
                } else {
                    (0..cols).rev().collect()
                };
                for c in row {
                    let v = self.z[r].get(c).copied().unwrap_or(f64::NAN);
                    let col = if v.is_finite() {
                        s.color(v)
                    } else {
                        Color::TRANSPARENT
                    };
                    rgba.extend_from_slice(&[col.r, col.g, col.b, (col.a * 255.0) as u8]);
                }
            }
            ctx.push(Node::Image {
                x: left.min(right),
                y: top_y.min(bottom_y),
                w: (right - left).abs(),
                h: (bottom_y - top_y).abs(),
                image: Arc::new(RasterImage {
                    width: cols as u32,
                    height: rows as u32,
                    rgba,
                }),
                smooth: self.smooth,
            });
        }
        if self.annotate {
            let size = ctx.theme.annotation_size;
            for r in 0..rows {
                for c in 0..cols {
                    let v = self.z[r].get(c).copied().unwrap_or(f64::NAN);
                    if !v.is_finite() {
                        continue;
                    }
                    let (x0, y0, x1, y1) = self.cell(r, c);
                    let (px, py) = ctx.point((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                    let text = match self.decimals {
                        Some(d) => format!("{v:.d$}"),
                        None => short(v),
                    };
                    let cell_w = (ctx.x.px(x1) - ctx.x.px(x0)).abs();
                    if ctx.text_width(&text, size) > cell_w - 2.0 {
                        continue;
                    }
                    let tc = s.color(v).contrasting_text();
                    ctx.text(px, py, text, size, tc, Anchor::Middle, Baseline::Middle);
                }
            }
        }
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let (rows, cols) = self.dims();
        let (vx, vy) = (x.value(px), y.value(py));
        for r in 0..rows {
            for c in 0..cols {
                let (x0, y0, x1, y1) = self.cell(r, c);
                if vx >= x0 && vx <= x1 && vy >= y0 && vy <= y1 {
                    let v = self.z[r].get(c).copied().unwrap_or(f64::NAN);
                    return Some(Hit {
                        px: x.px((x0 + x1) / 2.0),
                        py: y.px((y0 + y1) / 2.0),
                        label: format!("[{r}, {c}] = {}", short(v)),
                        distance: 0.0,
                    });
                }
            }
        }
        None
    }

    fn color_scale(&self) -> Option<ColorScale> {
        Some(self.scale())
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// An RGBA image placed in data coordinates. Created by [`Axes::image`].
#[derive(Clone, Debug)]
pub struct Image {
    pub(crate) image: Arc<RasterImage>,
    pub(crate) extent: (f64, f64, f64, f64),
    pub(crate) smooth: bool,
}

impl Image {
    /// Data rectangle covered (x0, x1, y0, y1); default `0..width × 0..height`.
    pub fn extent(&mut self, x0: f64, x1: f64, y0: f64, y1: f64) -> &mut Self {
        self.extent = (x0, x1, y0, y1);
        self
    }
    /// Interpolated scaling (default: crisp pixels).
    pub fn smooth(&mut self, on: bool) -> &mut Self {
        self.smooth = on;
        self
    }
}

impl Series for Image {
    fn extent(&self) -> Option<Extent> {
        let (x0, x1, y0, y1) = self.extent;
        let mut e = Extent::point(x0, y0).include(x1, y1);
        e.sticky_x_min = true;
        e.sticky_x_max = true;
        e.sticky_y_min = true;
        e.sticky_y_max = true;
        Some(e)
    }

    fn draw(&self, ctx: &mut Context) {
        let (x0, x1, y0, y1) = self.extent;
        let (a, b) = ctx.point(x0, y1);
        let (c, d) = ctx.point(x1, y0);
        ctx.push(Node::Image {
            x: a.min(c),
            y: b.min(d),
            w: (c - a).abs(),
            h: (d - b).abs(),
            image: self.image.clone(),
            smooth: self.smooth,
        });
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Hexagonal binning of points. Created by [`Axes::hexbin`].
#[derive(Clone, Debug)]
pub struct Hexbin {
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) gridsize: usize,
    pub(crate) colormap: Colormap,
    pub(crate) colorbar: Option<String>,
    pub(crate) log: bool,
}

impl Hexbin {
    /// Number of hexagons across the x range (default 30).
    pub fn gridsize(&mut self, n: usize) -> &mut Self {
        self.gridsize = n.max(1);
        self
    }
    /// Colormap.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
    /// Colorbar label.
    pub fn colorbar(&mut self, label: impl Into<String>) -> &mut Self {
        self.colorbar = Some(label.into());
        self
    }
    /// Log-scaled colors (`log10(1 + count)`).
    pub fn log(&mut self, on: bool) -> &mut Self {
        self.log = on;
        self
    }

    /// Hexagon centres (data units), cell size and counts (matplotlib-style
    /// lattice in data space, so counts do not depend on the canvas).
    #[allow(clippy::type_complexity)]
    fn bin(&self) -> Option<((f64, f64), Vec<((f64, f64), usize)>)> {
        let fin = |v: &[f64]| {
            v.iter()
                .copied()
                .filter(|t| t.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), t| {
                    (a.min(t), b.max(t))
                })
        };
        let (x0, x1) = fin(&self.x);
        let (y0, y1) = fin(&self.y);
        if !x0.is_finite() || !y0.is_finite() {
            return None;
        }
        let nx = self.gridsize as f64;
        let ny = (nx / 3f64.sqrt()).round().max(1.0);
        let sx = if x1 > x0 { (x1 - x0) / nx } else { 1.0 };
        let sy = if y1 > y0 { (y1 - y0) / ny } else { 1.0 };
        let mut map: std::collections::BTreeMap<(i64, i64, bool), usize> = Default::default();
        for (a, b) in self.x.iter().zip(&self.y) {
            if !a.is_finite() || !b.is_finite() {
                continue;
            }
            let (u, v) = ((a - x0) / sx, (b - y0) / sy);
            let (i1, j1) = (u.round(), v.round());
            let (i2, j2) = ((u - 0.5).floor() + 0.5, (v - 0.5).floor() + 0.5);
            let d1 = (u - i1).powi(2) + 3.0 * (v - j1).powi(2);
            let d2 = (u - i2).powi(2) + 3.0 * (v - j2).powi(2);
            let key = if d1 <= d2 {
                (i1 as i64, j1 as i64, false)
            } else {
                ((i2 - 0.5) as i64, (j2 - 0.5) as i64, true)
            };
            *map.entry(key).or_default() += 1;
        }
        let cells = map
            .into_iter()
            .map(|((i, j, odd), n)| {
                let o = if odd { 0.5 } else { 0.0 };
                ((x0 + (i as f64 + o) * sx, y0 + (j as f64 + o) * sy), n)
            })
            .collect();
        Some(((sx, sy), cells))
    }

    fn scale_for(&self, max: usize) -> ColorScale {
        let max = if self.log {
            (1.0 + max as f64).log10()
        } else {
            max as f64
        };
        ColorScale {
            colormap: self.colormap,
            min: 0.0,
            max: max.max(1.0),
            label: self.colorbar.clone().or(Some(if self.log {
                "log10(count)".into()
            } else {
                "count".into()
            })),
        }
    }
}

impl Series for Hexbin {
    fn extent(&self) -> Option<Extent> {
        Extent::of_points(self.x.iter().copied().zip(self.y.iter().copied()))
    }

    fn draw(&self, ctx: &mut Context) {
        let Some(((sx, sy), cells)) = self.bin() else {
            return;
        };
        let max = cells.iter().map(|c| c.1).max().unwrap_or(1);
        let s = self.scale_for(max);
        const HEX: [(f64, f64); 6] = [
            (0.5, -0.5),
            (0.5, 0.5),
            (0.0, 1.0),
            (-0.5, 0.5),
            (-0.5, -0.5),
            (0.0, -1.0),
        ];
        for ((cx, cy), n) in cells {
            let v = if self.log {
                (1.0 + n as f64).log10()
            } else {
                n as f64
            };
            let pts: Vec<(f64, f64)> = HEX
                .iter()
                .map(|(a, b)| ctx.point(cx + a * sx, cy + b * sy / 3.0))
                .collect();
            let c = s.color(v);
            ctx.polygon(&pts, Some(c), Some(Stroke::new(c, 0.5)));
        }
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let (_, cells) = self.bin()?;
        cells
            .into_iter()
            .map(|((cx, cy), n)| {
                let (a, b) = (x.px(cx), y.px(cy));
                Hit {
                    px: a,
                    py: b,
                    label: format!("count: {n}"),
                    distance: (a - px).hypot(b - py),
                }
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn color_scale(&self) -> Option<ColorScale> {
        let (_, cells) = self.bin()?;
        Some(self.scale_for(cells.iter().map(|c| c.1).max().unwrap_or(1)))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// Heatmap of a matrix (`z[row][col]`), row 0 at the top. Rows and
    /// columns are labelled 0, 1, … unless set with
    /// [`Axes::heatmap_labeled`].
    pub fn heatmap(&mut self, z: Vec<Vec<f64>>) -> &mut Heatmap {
        let rows = z.len();
        let cols = z.iter().map(|r| r.len()).max().unwrap_or(0);
        let xl: Vec<String> = (0..cols).map(|i| i.to_string()).collect();
        let yl: Vec<String> = (0..rows).map(|i| i.to_string()).collect();
        self.heatmap_labeled(xl, yl, z)
    }

    /// Heatmap with column and row labels (e.g. a correlation matrix).
    pub fn heatmap_labeled(
        &mut self,
        x_labels: impl IntoLabels,
        y_labels: impl IntoLabels,
        z: Vec<Vec<f64>>,
    ) -> &mut Heatmap {
        let (xl, yl) = (x_labels.into_labels(), y_labels.into_labels());
        let rows = z.len();
        let cols = z.iter().map(|r| r.len()).max().unwrap_or(0);
        self.x.categories(xl);
        // y categories bottom-up so that row 0 (top) gets the first label.
        self.y.categories(yl.into_iter().rev().collect::<Vec<_>>());
        self.x.grid = Some(false);
        self.y.grid = Some(false);
        let cm = self.defaults.colormap;
        self.add(Heatmap {
            z,
            x_range: (-0.5, cols as f64 - 0.5),
            y_range: (-0.5, rows as f64 - 0.5),
            top_down: true,
            colormap: cm,
            range: None,
            annotate: false,
            decimals: None,
            colorbar: None,
            gaps: 0.0,
            smooth: false,
        })
    }

    /// A field `z[row][col]` over a numeric rectangle (row 0 at `y_range.0`),
    /// e.g. the values of `f(x, y)` on a grid.
    pub fn field(
        &mut self,
        x_range: (f64, f64),
        y_range: (f64, f64),
        z: Vec<Vec<f64>>,
    ) -> &mut Heatmap {
        let cm = self.defaults.colormap;
        self.add(Heatmap {
            z,
            x_range,
            y_range,
            top_down: false,
            colormap: cm,
            range: None,
            annotate: false,
            decimals: None,
            colorbar: None,
            gaps: 0.0,
            smooth: false,
        })
    }

    /// 2D histogram of points with `nx × ny` bins.
    pub fn hist2d(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        nx: usize,
        ny: usize,
    ) -> &mut Heatmap {
        let (x, y) = (x.into_values(), y.into_values());
        let (nx, ny) = (nx.max(1), ny.max(1));
        let range = |v: &[f64]| {
            let (a, b) = v
                .iter()
                .copied()
                .filter(|t| t.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), t| {
                    (a.min(t), b.max(t))
                });
            if !a.is_finite() {
                (0.0, 1.0)
            } else if a == b {
                (a - 0.5, b + 0.5)
            } else {
                (a, b)
            }
        };
        let (xr, yr) = (range(&x), range(&y));
        let mut z = vec![vec![0.0; nx]; ny];
        for (a, b) in x.iter().zip(&y) {
            if !a.is_finite() || !b.is_finite() {
                continue;
            }
            let i = (((a - xr.0) / (xr.1 - xr.0) * nx as f64) as usize).min(nx - 1);
            let j = (((b - yr.0) / (yr.1 - yr.0) * ny as f64) as usize).min(ny - 1);
            z[j][i] += 1.0;
        }
        let h = self.field(xr, yr, z);
        h.colorbar = Some("count".into());
        h
    }

    /// Hexagonal binning of points.
    pub fn hexbin(&mut self, x: impl IntoValues, y: impl IntoValues) -> &mut Hexbin {
        let cm = self.defaults.colormap;
        self.add(Hexbin {
            x: x.into_values(),
            y: y.into_values(),
            gridsize: 30,
            colormap: cm,
            colorbar: None,
            log: false,
        })
    }

    /// Shows an RGBA image (`width * height * 4` bytes, row 0 at the top)
    /// over `0..width × 0..height`.
    pub fn image(&mut self, width: u32, height: u32, rgba: Vec<u8>) -> &mut Image {
        let mut rgba = rgba;
        rgba.resize(width as usize * height as usize * 4, 0);
        self.x.grid = Some(false);
        self.y.grid = Some(false);
        self.y.inverted = true;
        self.equal_aspect = true;
        self.add(Image {
            image: Arc::new(RasterImage {
                width,
                height,
                rgba,
            }),
            extent: (0.0, width as f64, 0.0, height as f64),
            smooth: false,
        })
    }
}
