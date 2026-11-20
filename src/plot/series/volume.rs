//! Volume rendering of a 3D scalar field (densities, CT/MRI scans,
//! simulation fields): rays cast through the grid for every pixel, colored
//! by a colormap and made transparent where values are low.

use super::three_d::{Bounds3, Scene3d};
use super::{ColorScale, Context, Extent, LegendEntry, LegendSymbol, Series};
use crate::plot::axes::Axes;
use crate::plot::color::Colormap;
use crate::plot::scene::{Node, RasterImage};
use std::any::Any;
use std::sync::Arc;

/// How the samples along a ray become a pixel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VolumeMode {
    /// Emission and absorption front to back: a translucent cloud.
    #[default]
    Composite,
    /// The largest value along the ray (maximum intensity projection, as
    /// for angiography).
    MaxIntensity,
}

/// A volume rendering (see [`Axes::volume`]).
#[derive(Clone, Debug)]
pub struct Volume {
    values: Arc<Vec<f32>>,
    dims: [usize; 3],
    ranges: Bounds3,
    colormap: Colormap,
    range: Option<(f64, f64)>,
    opacity: f64,
    threshold: f64,
    mode: VolumeMode,
    shading: bool,
    colorbar: Option<String>,
    label: Option<String>,
}

impl Volume {
    /// Data coordinates of the grid along x, y and z (default the indices).
    pub fn extent(&mut self, x: (f64, f64), y: (f64, f64), z: (f64, f64)) -> &mut Self {
        self.ranges = [x, y, z];
        self
    }
    /// Colormap (default inferno).
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
    /// Values mapped to the ends of the colormap (default the data range).
    pub fn range(&mut self, min: f64, max: f64) -> &mut Self {
        self.range = Some((min, max));
        self
    }
    /// Opacity of the densest values per grid cell crossed (default 0.25);
    /// higher looks more solid.
    pub fn opacity(&mut self, a: f64) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }
    /// Values below this fraction of the range are fully transparent
    /// (default 0.1).
    pub fn threshold(&mut self, t: f64) -> &mut Self {
        self.threshold = t.clamp(0.0, 0.99);
        self
    }
    /// Composite (default) or maximum intensity projection.
    pub fn mode(&mut self, m: VolumeMode) -> &mut Self {
        self.mode = m;
        self
    }
    /// Shows a colorbar with this label.
    pub fn colorbar(&mut self, label: impl Into<String>) -> &mut Self {
        self.colorbar = Some(label.into());
        self
    }
    /// Lighting from the value gradient (default on): surfaces inside the
    /// volume get light and shade from the viewer's upper left, which shows
    /// their shape. Off: pure emission and absorption.
    pub fn shading(&mut self, on: bool) -> &mut Self {
        self.shading = on;
        self
    }
    /// Legend label.
    pub fn label(&mut self, l: impl Into<String>) -> &mut Self {
        self.label = Some(l.into());
        self
    }

    fn value_range(&self) -> (f64, f64) {
        self.range.unwrap_or_else(|| {
            let (lo, hi) = self
                .values
                .iter()
                .filter(|v| v.is_finite())
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &v| {
                    (a.min(v), b.max(v))
                });
            if lo.is_finite() && hi > lo {
                (lo as f64, hi as f64)
            } else {
                (0.0, 1.0)
            }
        })
    }

    /// Trilinear sample at fractional grid coordinates (inside the grid).
    #[inline]
    fn sample(&self, g: [f64; 3]) -> f32 {
        let [nx, ny, nz] = self.dims;
        let (x, y, z) = (
            g[0].clamp(0.0, (nx - 1) as f64),
            g[1].clamp(0.0, (ny - 1) as f64),
            g[2].clamp(0.0, (nz - 1) as f64),
        );
        let (x0, y0, z0) = (x as usize, y as usize, z as usize);
        let (x1, y1, z1) = (
            (x0 + 1).min(nx - 1),
            (y0 + 1).min(ny - 1),
            (z0 + 1).min(nz - 1),
        );
        let (fx, fy, fz) = (
            (x - x0 as f64) as f32,
            (y - y0 as f64) as f32,
            (z - z0 as f64) as f32,
        );
        let v = &self.values;
        let at = |i: usize, j: usize, k: usize| v[i + nx * (j + ny * k)];
        let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let c00 = l(at(x0, y0, z0), at(x1, y0, z0), fx);
        let c10 = l(at(x0, y1, z0), at(x1, y1, z0), fx);
        let c01 = l(at(x0, y0, z1), at(x1, y0, z1), fx);
        let c11 = l(at(x0, y1, z1), at(x1, y1, z1), fx);
        l(l(c00, c10, fy), l(c01, c11, fy), fz)
    }
}

/// Colormap lookup table (256 entries, straight RGB 0–1).
fn lut(cm: Colormap) -> Vec<[f32; 3]> {
    (0..256)
        .map(|i| {
            let c = cm.at(i as f64 / 255.0);
            [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]
        })
        .collect()
}

impl Series for Volume {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }

    fn draw(&self, _ctx: &mut Context) {}

    fn bounds3d(&self) -> Option<Bounds3> {
        Some(self.ranges)
    }

    fn draw3d(&self, scene: &mut Scene3d) {
        if !scene.drawing() {
            // Neither a GPU mesh nor part of the HTML scene.
            scene.needs_cpu();
            return;
        }
        let [nx, ny, nz] = self.dims;
        if nx < 2 || ny < 2 || nz < 2 || self.values.len() < nx * ny * nz {
            return;
        }
        let p = scene.projection();
        let m = p.cam.m;
        // The grid's box in normalized coordinates.
        let lo: [f64; 3] = std::array::from_fn(|i| (self.ranges[i].0 - p.mid[i]) * p.k[i]);
        let hi: [f64; 3] = std::array::from_fn(|i| (self.ranges[i].1 - p.mid[i]) * p.k[i]);
        // Its screen rectangle.
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for c in 0..8 {
            let n = [
                if c & 1 == 0 { lo[0] } else { hi[0] },
                if c & 2 == 0 { lo[1] } else { hi[1] },
                if c & 4 == 0 { lo[2] } else { hi[2] },
            ];
            let v = [
                m[0][0] * n[0] + m[0][1] * n[1] + m[0][2] * n[2],
                m[1][0] * n[0] + m[1][1] * n[1] + m[1][2] * n[2],
                m[2][0] * n[0] + m[2][1] * n[1] + m[2][2] * n[2],
            ];
            let f = if p.dist > 0.0 {
                p.dist / (p.dist - v[2]).max(p.radius * 0.05)
            } else {
                1.0
            };
            let (sx, sy) = (
                p.center.0 + v[0] * p.scale * f,
                p.center.1 - v[1] * p.scale * f,
            );
            (x0, y0, x1, y1) = (x0.min(sx), y0.min(sy), x1.max(sx), y1.max(sy));
        }
        if !(x0.is_finite() && x1 > x0 && y1 > y0) {
            return;
        }
        let (x0, y0) = (x0.floor(), y0.floor());
        let (w, h) = ((x1.ceil() - x0) as usize, (y1.ceil() - y0) as usize);
        // Rays: two per grid cell across the screen is all the detail the
        // grid has (the image is scaled up smoothly); at most one per pixel
        // and 400 000 in all.
        let cells = self.dims.iter().copied().max().unwrap_or(2) as f64;
        let q = (w.max(h) as f64 / cells / 2.0)
            .max(((w * h) as f64 / 400_000.0).sqrt())
            .max(1.0);
        let (iw, ih) = (
            ((w as f64 / q).ceil() as usize).max(1),
            ((h as f64 / q).ceil() as usize).max(1),
        );
        let (vmin, vmax) = self.value_range();
        let span = (vmax - vmin).max(1e-12);
        let lut = lut(self.colormap);
        let size: [f64; 3] = std::array::from_fn(|i| hi[i] - lo[i]);
        // Steps of half a grid cell (normalized units).
        let cell = (0..3)
            .map(|i| size[i].abs() / (self.dims[i] - 1) as f64)
            .fold(f64::INFINITY, f64::min);
        let step = (cell * 0.5).max(1e-9);
        let (thr, op, mode) = (self.threshold, self.opacity, self.mode);
        // Largest value per brick of 8³ cells (and its neighbours' corner
        // samples): bricks entirely below the threshold are skipped.
        const B: usize = 8;
        let bd = [nx.div_ceil(B), ny.div_ceil(B), nz.div_ceil(B)];
        let mut bmax = vec![f32::NEG_INFINITY; bd[0] * bd[1] * bd[2]];
        for kz in 0..nz {
            for jy in 0..ny {
                let row = &self.values[nx * (jy + ny * kz)..][..nx];
                for (ix, &v) in row.iter().enumerate() {
                    // A cell's samples reach one cell into the next brick.
                    for bz in [kz / B, kz.saturating_sub(1) / B] {
                        for by in [jy / B, jy.saturating_sub(1) / B] {
                            for bx in [ix / B, ix.saturating_sub(1) / B] {
                                let b = &mut bmax[bx + bd[0] * (by + bd[1] * bz)];
                                if v > *b {
                                    *b = v;
                                }
                            }
                        }
                    }
                }
            }
        }
        let cut = (vmin + thr * span) as f32;
        let empty = |g: [f64; 3]| {
            let b = |i: usize| ((g[i].max(0.0) as usize) / B).min(bd[i] - 1);
            bmax[b(0) + bd[0] * (b(1) + bd[1] * b(2))] <= cut
        };
        let alpha_of = |t: f64| {
            let r = ((t - thr) / (1.0 - thr)).clamp(0.0, 1.0);
            r * r.sqrt()
        };
        // Opacity per step from the opacity per cell.
        let per_step = |a: f64| 1.0 - (1.0 - (a * op).min(0.999)).powf(step / cell);
        let dist = p.dist;
        let (scale, center) = (p.scale, p.center);
        // Light from the viewer's upper left, in grid coordinates (the
        // gradient is taken per grid cell; `gs` turns it into normalized
        // units so anisotropic voxels shade correctly).
        let shading = self.shading;
        let lv = {
            let l = [-0.35, 0.45, 0.82];
            let v: [f64; 3] =
                std::array::from_fn(|i| m[0][i] * l[0] + m[1][i] * l[1] + m[2][i] * l[2]);
            let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
            v.map(|c| c / n)
        };
        let gs: [f64; 3] = std::array::from_fn(|i| (self.dims[i] - 1) as f64 / size[i]);
        let render_row = |row: usize, out: &mut [u8]| {
            let sy = y0 + (row as f64 + 0.5) * h as f64 / ih as f64;
            for col in 0..iw {
                let sx = x0 + (col as f64 + 0.5) * w as f64 / iw as f64;
                // View-space ray v(z) = a + z·b (perspective-correct).
                let (px, py) = ((sx - center.0) / scale, -(sy - center.1) / scale);
                let (a, b) = if dist > 0.0 {
                    ([px, py, 0.0], [-px / dist, -py / dist, 1.0])
                } else {
                    ([px, py, 0.0], [0.0, 0.0, 1.0])
                };
                // To normalized box coordinates (transpose of the rotation).
                let un = |v: [f64; 3]| -> [f64; 3] {
                    std::array::from_fn(|i| m[0][i] * v[0] + m[1][i] * v[1] + m[2][i] * v[2])
                };
                let (na, nb) = (un(a), un(b));
                // Slab intersection with the grid's box.
                let (mut t0, mut t1) = (f64::NEG_INFINITY, f64::INFINITY);
                for i in 0..3 {
                    let (l, u) = (lo[i].min(hi[i]), lo[i].max(hi[i]));
                    if nb[i].abs() < 1e-12 {
                        if na[i] < l || na[i] > u {
                            t1 = t0 - 1.0;
                        }
                        continue;
                    }
                    let (ta, tb) = ((l - na[i]) / nb[i], (u - na[i]) / nb[i]);
                    t0 = t0.max(ta.min(tb));
                    t1 = t1.min(ta.max(tb));
                }
                let o = &mut out[col * 4..col * 4 + 4];
                if t1 <= t0 {
                    o.copy_from_slice(&[0; 4]);
                    continue;
                }
                // Nearer is larger z: march from t1 down to t0.
                let len = (nb[0] * nb[0] + nb[1] * nb[1] + nb[2] * nb[2]).sqrt();
                let dt = step / len.max(1e-12);
                let n = (((t1 - t0) / dt) as usize).clamp(1, 2048);
                let (mut acc, mut alpha, mut best) = ([0.0f64; 3], 0.0f64, f64::NEG_INFINITY);
                for s in 0..n {
                    let t = t1 - (s as f64 + 0.5) * dt;
                    let g: [f64; 3] = std::array::from_fn(|i| {
                        (na[i] + nb[i] * t - lo[i]) / size[i] * (self.dims[i] - 1) as f64
                    });
                    if mode == VolumeMode::Composite && empty(g) {
                        continue;
                    }
                    let v = self.sample(g);
                    if !v.is_finite() {
                        continue;
                    }
                    let tv = ((v as f64 - vmin) / span).clamp(0.0, 1.0);
                    match mode {
                        VolumeMode::MaxIntensity => best = best.max(tv),
                        VolumeMode::Composite => {
                            let a = per_step(alpha_of(tv));
                            if a <= 0.0 {
                                continue;
                            }
                            let c = lut[(tv * 255.0) as usize];
                            let k = (1.0 - alpha) * a;
                            // Gradient lighting: central differences give
                            // the surface normal (values rise inwards).
                            let (lit, spec) = if shading && k > 0.002 {
                                let d = |i: usize| {
                                    let (mut ga, mut gb) = (g, g);
                                    ga[i] += 1.0;
                                    gb[i] -= 1.0;
                                    (self.sample(gb) - self.sample(ga)) as f64 * 0.5 * gs[i]
                                };
                                let nv = [d(0), d(1), d(2)];
                                let len = (nv[0] * nv[0] + nv[1] * nv[1] + nv[2] * nv[2]).sqrt();
                                // Flat regions keep their color unshaded.
                                let w = (len / span / gs.iter().fold(0.0f64, |a, b| a.max(*b))
                                    * 6.0)
                                    .min(1.0);
                                if len > 1e-12 {
                                    let nd = (nv[0] * lv[0] + nv[1] * lv[1] + nv[2] * lv[2]) / len;
                                    let diffuse = 0.4 + 0.6 * nd.abs();
                                    // Half vector of light and view (view along +z).
                                    let hv = [lv[0], lv[1], lv[2] + 1.0];
                                    let hn = (hv[0] * hv[0] + hv[1] * hv[1] + hv[2] * hv[2]).sqrt();
                                    let vz: [f64; 3] = std::array::from_fn(|i| m[2][i]);
                                    let h: [f64; 3] =
                                        std::array::from_fn(|i| (lv[i] + vz[i]) / hn.max(1e-12));
                                    let sh =
                                        ((nv[0] * h[0] + nv[1] * h[1] + nv[2] * h[2]) / len).abs();
                                    let sp = sh.powi(24) * 0.35;
                                    (1.0 + (diffuse - 1.0) * w, sp * w)
                                } else {
                                    (1.0, 0.0)
                                }
                            } else {
                                (1.0, 0.0)
                            };
                            for j in 0..3 {
                                acc[j] += k * (c[j] as f64 * lit + spec).min(1.0);
                            }
                            alpha += k;
                            if alpha > 0.995 {
                                break;
                            }
                        }
                    }
                }
                if mode == VolumeMode::MaxIntensity && best.is_finite() {
                    let c = lut[(best * 255.0) as usize];
                    alpha = alpha_of(best).sqrt();
                    acc = std::array::from_fn(|j| c[j] as f64 * alpha);
                }
                // Straight alpha.
                let inv = if alpha > 1e-6 { 1.0 / alpha } else { 0.0 };
                for j in 0..3 {
                    o[j] = (acc[j] * inv * 255.0).round().clamp(0.0, 255.0) as u8;
                }
                o[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        };
        let mut rgba = vec![0u8; iw * ih * 4];
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .min(8);
        if threads > 1 && iw * ih > 20_000 {
            let rows_per = ih.div_ceil(threads);
            std::thread::scope(|sc| {
                for (c, chunk) in rgba.chunks_mut(rows_per * iw * 4).enumerate() {
                    let render_row = &render_row;
                    sc.spawn(move || {
                        for (r, out) in chunk.chunks_mut(iw * 4).enumerate() {
                            render_row(c * rows_per + r, out);
                        }
                    });
                }
            });
        } else {
            for (r, out) in rgba.chunks_mut(iw * 4).enumerate() {
                render_row(r, out);
            }
        }
        scene.backdrop(Node::Image {
            x: x0,
            y: y0,
            w: w as f64,
            h: h as f64,
            image: Arc::new(RasterImage {
                width: iw as u32,
                height: ih as u32,
                rgba,
            }),
            smooth: true,
        });
    }

    fn color_scale(&self) -> Option<ColorScale> {
        let (min, max) = self.value_range();
        self.colorbar.as_ref().map(|l| ColorScale {
            colormap: self.colormap,
            min,
            max,
            label: Some(l.clone()),
        })
    }

    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.colormap.at(0.8),
                    edge: None,
                },
            })
            .collect()
    }

    fn kind(&self) -> &'static str {
        "volume"
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// Volume rendering of a 3D grid of values: `values[x + nx·(y + ny·z)]`
    /// (x fastest, as a NumPy array of shape `(nz, ny, nx)`), `dims` =
    /// `[nx, ny, nz]`. Low values are transparent ([`Volume::threshold`]),
    /// higher ones glow in the colormap; drawn behind the other 3D series.
    pub fn volume(&mut self, values: impl super::IntoValues, dims: [usize; 3]) -> &mut Volume {
        let values: Vec<f32> = values.into_values().into_iter().map(|v| v as f32).collect();
        self.volume_f32(values, dims)
    }

    /// [`Axes::volume`] from `f32` values, without a conversion (half the
    /// memory of `f64`; how scans and simulations usually store them).
    pub fn volume_f32(&mut self, values: Vec<f32>, dims: [usize; 3]) -> &mut Volume {
        self.use_3d();
        let ranges = dims.map(|n| (0.0, n.saturating_sub(1).max(1) as f64));
        self.add(Volume {
            values: Arc::new(values),
            dims,
            ranges,
            colormap: Colormap::Inferno,
            range: None,
            opacity: 0.25,
            threshold: 0.1,
            mode: VolumeMode::Composite,
            shading: true,
            colorbar: None,
            label: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    fn blob(n: usize) -> Vec<f64> {
        let mut v = Vec::with_capacity(n * n * n);
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    let c = |a: usize| a as f64 / (n - 1) as f64 - 0.5;
                    let r2 = c(i) * c(i) + c(j) * c(j) + c(k) * c(k);
                    v.push((-r2 * 12.0).exp());
                }
            }
        }
        v
    }

    #[test]
    fn volume_renders_a_blob_in_the_middle() {
        let mut fig = Figure::new(400, 400);
        fig.axes().volume(blob(24), [24, 24, 24]).opacity(0.6);
        let px = fig.to_pixels().unwrap();
        let at = |x: u32, y: u32| {
            let i = ((y * px.width + x) * 4) as usize;
            px.rgba[i] as u32 + px.rgba[i + 1] as u32 + px.rgba[i + 2] as u32
        };
        // The centre glows (not the white background), a corner does not.
        let (cx, cy) = (px.width / 2, px.height / 2);
        assert!(at(cx, cy) < 700, "centre {}", at(cx, cy));
        assert!(at(4, 4) > 740);
        // Maximum intensity mode draws as well.
        let mut fig = Figure::new(300, 300);
        fig.axes()
            .volume(blob(16), [16, 16, 16])
            .mode(VolumeMode::MaxIntensity);
        assert!(fig.to_svg().contains("<image"));
    }

    #[test]
    fn volume_turns_off_the_gpu_mesh() {
        let mut fig = Figure::new(300, 300);
        fig.axes().volume(blob(8), [8, 8, 8]);
        fig.axes().rotate(Axis3::Z, 20.0);
        assert!(fig.to_pixels().is_ok());
    }
}
