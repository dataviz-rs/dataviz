//! Density images of very many points (as Datashader): instead of drawing
//! each point, the points are counted per screen pixel and the counts are
//! colored. Exact at every zoom (binned again for the visible range), with
//! histogram-equalized colors so that dense cores do not hide the
//! structure around them. [`DensityGrid`] counts chunks one after another,
//! so data larger than memory (billions of points) can be shown.

use super::{ColorScale, Context, Extent, Hit, IntoValues, Rect, Series};
use crate::plot::axes::Axes;
use crate::plot::axis::AxisMap;
use crate::plot::color::Colormap;
use crate::plot::scale::Scale;
use crate::plot::scene::{Node, RasterImage};
use std::any::Any;
use std::sync::Arc;

/// How counts become colors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum DensityScale {
    /// Histogram equalization: every color covers about as many pixels
    /// (shows structure across orders of magnitude; Datashader's default).
    #[default]
    EqHist,
    /// Logarithm of the count.
    Log,
    /// The count itself.
    Linear,
}

/// Point counts on a fixed grid over `x` × `y` ranges, filled chunk by
/// chunk (data that does not fit in memory); show it with
/// [`Axes::density_grid`].
#[derive(Clone, Debug)]
pub struct DensityGrid {
    x: (f64, f64),
    y: (f64, f64),
    w: usize,
    h: usize,
    counts: Vec<u32>,
    total: u64,
}

impl DensityGrid {
    /// An empty grid of `w` × `h` cells over the ranges.
    pub fn new(x: (f64, f64), y: (f64, f64), w: usize, h: usize) -> Self {
        let (w, h) = (w.max(1), h.max(1));
        Self {
            x,
            y,
            w,
            h,
            counts: vec![0; w * h],
            total: 0,
        }
    }

    /// A grid over the finite range of the points, counting them all
    /// (ranges scanned and points counted in parallel).
    pub fn fit(x: &[f64], y: &[f64], w: usize, h: usize) -> Self {
        let n = x.len().min(y.len());
        let pad = |r: Option<(f64, f64)>| match r {
            // The largest values must fall inside the last cell.
            Some((a, b)) if b > a => (a, b + (b - a) * 1e-9),
            Some((a, _)) => (a - 0.5, a + 0.5),
            None => (0.0, 1.0),
        };
        let (xr, yr) = (pad(finite_range(&x[..n])), pad(finite_range(&y[..n])));
        let mut g = Self::new(xr, yr, w, h);
        g.add(&x[..n], &y[..n]);
        g
    }

    /// Counts a chunk of points (points outside the ranges are skipped).
    pub fn add(&mut self, x: &[f64], y: &[f64]) -> &mut Self {
        let n = x.len().min(y.len());
        self.total += bin_into(
            &mut self.counts,
            self.w,
            self.h,
            self.x,
            self.y,
            &x[..n],
            &y[..n],
        );
        self
    }

    /// Points counted inside the ranges.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Grid size (columns, rows).
    pub fn size(&self) -> (usize, usize) {
        (self.w, self.h)
    }

    /// Count of a cell (row 0 at the bottom, at the low end of y).
    pub fn count(&self, col: usize, row: usize) -> u32 {
        self.counts.get(row * self.w + col).copied().unwrap_or(0)
    }
}

/// Bins points into `counts` (`w` × `h`, row 0 at low y), in parallel for
/// large chunks; returns the number of points counted.
fn bin_into(
    counts: &mut [u32],
    w: usize,
    h: usize,
    xr: (f64, f64),
    yr: (f64, f64),
    x: &[f64],
    y: &[f64],
) -> u64 {
    let (sx, sy) = (w as f64 / (xr.1 - xr.0), h as f64 / (yr.1 - yr.0));
    if !(sx.is_finite() && sy.is_finite()) {
        return 0;
    }
    let one = |counts: &mut [u32], x: &[f64], y: &[f64]| {
        let mut n = 0u64;
        for (&a, &b) in x.iter().zip(y) {
            let (fx, fy) = ((a - xr.0) * sx, (b - yr.0) * sy);
            // NaN fails both comparisons.
            if fx >= 0.0 && fy >= 0.0 && fx < w as f64 && fy < h as f64 {
                let i = fy as usize * w + fx as usize;
                counts[i] = counts[i].saturating_add(1);
                n += 1;
            }
        }
        n
    };
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(16);
    // Threads pay off when each has many points per cell of its own grid.
    let per = x.len() / threads.max(1);
    if threads < 2 || per < 250_000 || per < counts.len() {
        return one(counts, x, y);
    }
    let parts: Vec<(Vec<u32>, u64)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|t| {
                let (a, b) = (
                    t * per,
                    if t + 1 == threads {
                        x.len()
                    } else {
                        (t + 1) * per
                    },
                );
                let (xs, ys) = (&x[a..b], &y[a..b]);
                let len = counts.len();
                s.spawn(move || {
                    let mut c = vec![0u32; len];
                    let n = one(&mut c, xs, ys);
                    (c, n)
                })
            })
            .collect();
        hs.into_iter().filter_map(|h| h.join().ok()).collect()
    });
    let mut n = 0;
    for (c, k) in parts {
        n += k;
        for (d, v) in counts.iter_mut().zip(c) {
            *d = d.saturating_add(v);
        }
    }
    n
}

/// Smallest and largest finite value, scanned in parallel for large data.
fn finite_range(v: &[f64]) -> Option<(f64, f64)> {
    let scan = |v: &[f64]| {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for &x in v {
            // NaN and ±∞ fail the finiteness test; branch-free min/max.
            if x.is_finite() {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
        (lo, hi)
    };
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(16);
    let (lo, hi) = if threads > 1 && v.len() > 2_000_000 {
        let per = v.len().div_ceil(threads);
        std::thread::scope(|s| {
            let hs: Vec<_> = v.chunks(per).map(|c| s.spawn(move || scan(c))).collect();
            hs.into_iter()
                .filter_map(|h| h.join().ok())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |a, b| {
                    (a.0.min(b.0), a.1.max(b.1))
                })
        })
    } else {
        scan(v)
    };
    (lo <= hi).then_some((lo, hi))
}

enum Source {
    /// Raw points: binned again for every view.
    Points {
        x: Arc<Vec<f64>>,
        y: Arc<Vec<f64>>,
        extent: Option<(f64, f64, f64, f64)>,
    },
    /// Counts prepared by the caller.
    Grid(DensityGrid),
}

/// A density image (see [`Axes::density`]).
pub struct Density {
    source: Source,
    colormap: Colormap,
    scale: DensityScale,
    colorbar: Option<String>,
    /// Pixels per bin (1 = one bin per canvas pixel).
    pixel: f64,
}

impl std::fmt::Debug for Density {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Density").finish_non_exhaustive()
    }
}

impl Density {
    /// Colormap (default viridis; low counts dark).
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
    /// How counts become colors (default histogram equalization).
    pub fn scale(&mut self, s: DensityScale) -> &mut Self {
        self.scale = s;
        self
    }
    /// Shows a colorbar of counts with this label.
    pub fn colorbar(&mut self, label: impl Into<String>) -> &mut Self {
        self.colorbar = Some(label.into());
        self
    }
    /// Size of a bin in canvas pixels (default 1; larger bins for sparse
    /// data).
    pub fn bin_pixels(&mut self, px: f64) -> &mut Self {
        self.pixel = px.clamp(1.0, 64.0);
        self
    }

    /// Counts for the plot area of `ctx`: (grid, columns, rows).
    fn counts_for(&self, x: &AxisMap, y: &AxisMap, rect: Rect) -> Option<(Vec<u32>, usize, usize)> {
        match &self.source {
            Source::Grid(g) => Some((g.counts.clone(), g.w, g.h)),
            Source::Points { x: xs, y: ys, .. } => {
                let w = ((rect.w / self.pixel).round() as usize).max(1);
                let h = ((rect.h / self.pixel).round() as usize).max(1);
                let mut c = vec![0u32; w * h];
                let lin = |a: &AxisMap| matches!(a.scale, Scale::Linear | Scale::Time);
                if lin(x) && lin(y) && (x.p1 > x.p0) && (y.p0 > y.p1) {
                    bin_into(&mut c, w, h, (x.min, x.max), (y.min, y.max), xs, ys);
                } else {
                    // Log axes, inverted axes: through the axis mapping.
                    let (px, py): (Vec<f64>, Vec<f64>) = xs
                        .iter()
                        .zip(ys.iter())
                        .map(|(&a, &b)| {
                            (
                                (x.px(a) - rect.x) / rect.w,
                                (rect.y + rect.h - y.px(b)) / rect.h,
                            )
                        })
                        .unzip();
                    bin_into(&mut c, w, h, (0.0, 1.0), (0.0, 1.0), &px, &py);
                }
                Some((c, w, h))
            }
        }
    }

    /// Color position (0–1) of every count (NaN for empty cells).
    fn normalize(&self, counts: &[u32]) -> (Vec<f32>, u32) {
        let max = counts.iter().copied().max().unwrap_or(0);
        let mut t = vec![f32::NAN; counts.len()];
        if max == 0 {
            return (t, 0);
        }
        match self.scale {
            DensityScale::Linear => {
                for (o, &c) in t.iter_mut().zip(counts) {
                    if c > 0 {
                        *o = c as f32 / max as f32;
                    }
                }
            }
            DensityScale::Log => {
                let l = (max as f32).ln_1p();
                for (o, &c) in t.iter_mut().zip(counts) {
                    if c > 0 {
                        *o = (c as f32).ln_1p() / l;
                    }
                }
            }
            DensityScale::EqHist => {
                // Rank of each distinct count among the non-empty cells.
                let mut nz: Vec<u32> = counts.iter().copied().filter(|&c| c > 0).collect();
                nz.sort_unstable();
                let n = nz.len() as f32;
                let mut levels: Vec<(u32, f32)> = Vec::new();
                let mut i = 0;
                while i < nz.len() {
                    let v = nz[i];
                    let mut j = i;
                    while j < nz.len() && nz[j] == v {
                        j += 1;
                    }
                    levels.push((v, j as f32 / n));
                    i = j;
                }
                let lo = levels.first().map_or(0.0, |l| l.1);
                let span = (1.0 - lo).max(1e-6);
                for (o, &c) in t.iter_mut().zip(counts) {
                    if c > 0 {
                        let k = levels.partition_point(|l| l.0 < c);
                        let r = levels.get(k).map_or(1.0, |l| l.1);
                        // The emptiest cells start a little above the bottom
                        // of the colormap so that they stay visible.
                        *o = 0.08 + 0.92 * ((r - lo) / span).clamp(0.0, 1.0);
                    }
                }
            }
        }
        (t, max)
    }
}

impl Series for Density {
    fn extent(&self) -> Option<Extent> {
        match &self.source {
            Source::Points { extent, .. } => {
                let (a, b, c, d) = (*extent)?;
                Some(Extent::point(a, c).include(b, d))
            }
            Source::Grid(g) => Some(Extent::point(g.x.0, g.y.0).include(g.x.1, g.y.1)),
        }
    }

    fn draw(&self, ctx: &mut Context) {
        let Some((counts, w, h)) = self.counts_for(ctx.x, ctx.y, ctx.rect) else {
            return;
        };
        let (t, _) = self.normalize(&counts);
        let lut: Vec<[u8; 3]> = (0..256)
            .map(|i| {
                let c = self.colormap.at(i as f64 / 255.0);
                [c.r, c.g, c.b]
            })
            .collect();
        let mut rgba = vec![0u8; w * h * 4];
        // Image rows go down, grid rows up.
        for row in 0..h {
            let src = &t[(h - 1 - row) * w..(h - row) * w];
            let dst = &mut rgba[row * w * 4..(row + 1) * w * 4];
            for (o, &v) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src) {
                if v.is_finite() {
                    let c = lut[(v * 255.0) as usize];
                    o.copy_from_slice(&[c[0], c[1], c[2], 255]);
                }
            }
        }
        let (x0, x1, y0, y1) = match &self.source {
            Source::Grid(g) => {
                let (a, b) = (ctx.x.px(g.x.0), ctx.x.px(g.x.1));
                let (c, d) = (ctx.y.px(g.y.0), ctx.y.px(g.y.1));
                (a.min(b), a.max(b), c.min(d), c.max(d))
            }
            Source::Points { .. } => (
                ctx.rect.x,
                ctx.rect.x + ctx.rect.w,
                ctx.rect.y,
                ctx.rect.y + ctx.rect.h,
            ),
        };
        if !(x0.is_finite() && x1 > x0 && y1 > y0) {
            return;
        }
        let image = Node::Image {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
            image: Arc::new(RasterImage {
                width: w as u32,
                height: h as u32,
                rgba,
            }),
            smooth: false,
        };
        let r = ctx.rect;
        ctx.push(Node::Group {
            children: vec![image],
            clip: Some((r.x, r.y, r.w, r.h)),
            opacity: 1.0,
            tooltip: None,
            id: None,
        });
    }

    fn color_scale(&self) -> Option<ColorScale> {
        let label = self.colorbar.clone()?;
        let max = match &self.source {
            Source::Grid(g) => g.counts.iter().copied().max().unwrap_or(1) as f64,
            Source::Points { x, .. } => x.len() as f64,
        };
        Some(ColorScale {
            colormap: self.colormap,
            min: 0.0,
            max: match self.scale {
                DensityScale::Linear => max,
                _ => 1.0,
            },
            label: Some(match self.scale {
                DensityScale::Linear => label,
                DensityScale::Log => format!("{label} (log)"),
                DensityScale::EqHist => format!("{label} (rank)"),
            }),
        })
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        if !rect.contains(px, py) {
            return None;
        }
        let (counts, w, h) = self.counts_for(x, y, rect)?;
        let (col, row) = match &self.source {
            Source::Grid(g) => (
                ((x.value(px) - g.x.0) / (g.x.1 - g.x.0) * w as f64).floor(),
                ((y.value(py) - g.y.0) / (g.y.1 - g.y.0) * h as f64).floor(),
            ),
            Source::Points { .. } => (
                ((px - rect.x) / rect.w * w as f64).floor(),
                ((rect.y + rect.h - py) / rect.h * h as f64).floor(),
            ),
        };
        if col < 0.0 || row < 0.0 || col >= w as f64 || row >= h as f64 {
            return None;
        }
        let c = counts[row as usize * w + col as usize];
        Some(Hit {
            px,
            py,
            label: format!("{c} points"),
            distance: 0.0,
        })
    }

    fn kind(&self) -> &'static str {
        "density"
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// A density image of points (millions to billions): counted per
    /// screen pixel at every zoom and colored by a colormap — the whole
    /// distribution, where a scatter plot would be a solid blob.
    pub fn density(&mut self, x: impl IntoValues, y: impl IntoValues) -> &mut Density {
        let (x, y) = (x.into_values(), y.into_values());
        let range = |v: &[f64]| finite_range(v);
        let extent = match (range(&x), range(&y)) {
            (Some(a), Some(b)) => Some((a.0, a.1, b.0, b.1)),
            _ => None,
        };
        self.add(Density {
            source: Source::Points {
                x: Arc::new(x),
                y: Arc::new(y),
                extent,
            },
            colormap: Colormap::Viridis,
            scale: DensityScale::EqHist,
            colorbar: None,
            pixel: 1.0,
        })
    }

    /// A density image from counts prepared chunk by chunk (data larger
    /// than memory).
    pub fn density_grid(&mut self, grid: DensityGrid) -> &mut Density {
        self.add(Density {
            source: Source::Grid(grid),
            colormap: Colormap::Viridis,
            scale: DensityScale::EqHist,
            colorbar: None,
            pixel: 1.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn grid_counts_chunks_and_skips_outside() {
        let mut g = DensityGrid::new((0.0, 10.0), (0.0, 10.0), 10, 10);
        g.add(&[0.5, 0.5, 9.5, 11.0, f64::NAN], &[0.5, 0.5, 9.5, 1.0, 1.0]);
        g.add(&[5.0], &[5.0]);
        assert_eq!(g.total(), 4);
        assert_eq!((g.count(0, 0), g.count(9, 9), g.count(5, 5)), (2, 1, 1));
        // Parallel and serial binning agree.
        let n = 3_000_000;
        let x: Vec<f64> = (0..n)
            .map(|i| (i as f64 * 0.618_034).fract() * 10.0)
            .collect();
        let y: Vec<f64> = (0..n)
            .map(|i| (i as f64 * 0.414_214).fract() * 10.0)
            .collect();
        let mut a = DensityGrid::new((0.0, 10.0), (0.0, 10.0), 50, 40);
        a.add(&x, &y);
        let mut b = DensityGrid::new((0.0, 10.0), (0.0, 10.0), 50, 40);
        for (cx, cy) in x.chunks(1000).zip(y.chunks(1000)) {
            b.add(cx, cy);
        }
        assert_eq!(a.counts, b.counts);
        assert_eq!(a.total(), n as u64);
    }

    #[test]
    fn density_draws_and_follows_zoom() {
        let n = 200_000;
        let x: Vec<f64> = (0..n)
            .map(|i| ((i as f64) * 0.001).sin() * (i % 97) as f64)
            .collect();
        let y: Vec<f64> = (0..n)
            .map(|i| ((i as f64) * 0.0013).cos() * (i % 89) as f64)
            .collect();
        let mut fig = Figure::new(400, 300);
        fig.axes().density(&x, &y).colorbar("points");
        let px = fig.to_pixels().unwrap();
        let colored = px
            .rgba
            .chunks(4)
            .filter(|p| p[0] != p[1] || p[1] != p[2])
            .count();
        assert!(colored > 5000, "{colored}");
        // Zoomed in: binned again for the view, still covered.
        fig.axes().x_limits(-5.0, 5.0).y_limits(-5.0, 5.0);
        assert!(fig.to_pixels().is_ok());
        for s in [DensityScale::Log, DensityScale::Linear] {
            let mut f = Figure::new(200, 150);
            f.axes().density(&x, &y).scale(s);
            assert!(f.to_svg().contains("<image"));
        }
    }
}
