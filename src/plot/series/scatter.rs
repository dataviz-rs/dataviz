//! Scatter and bubble plots.

use super::Rect;
use super::{
    nearest_point, short, ColorScale, Context, Extent, Hit, LegendEntry, LegendSymbol, Series,
};
use crate::plot::axis::AxisMap;
use crate::plot::color::{Color, Colormap};
use crate::plot::scene::{Node, Stroke};
use crate::plot::style::Marker;
use std::any::Any;

/// Points at `(x, y)`, optionally sized and colored per point. Created by
/// [`Axes::scatter`](crate::plot::Axes::scatter).
#[derive(Clone, Debug)]
pub struct Scatter {
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) label: Option<String>,
    pub(crate) color: Color,
    pub(crate) marker: Marker,
    pub(crate) size: f64,
    pub(crate) sizes: Option<Vec<f64>>,
    pub(crate) values: Option<Vec<f64>>,
    pub(crate) colormap: Colormap,
    pub(crate) value_range: Option<(f64, f64)>,
    pub(crate) colorbar: Option<String>,
    pub(crate) edge: Option<Stroke>,
    pub(crate) opacity: f32,
    pub(crate) names: Option<Vec<String>>,
    pub(crate) secondary: bool,
}

impl Scatter {
    pub(crate) fn new(
        x: Vec<f64>,
        y: Vec<f64>,
        color: Color,
        size: f64,
        colormap: Colormap,
    ) -> Self {
        let n = x.len().min(y.len());
        Self {
            x: super::truncated(x, n),
            y: super::truncated(y, n),
            label: None,
            color,
            marker: Marker::Circle,
            size,
            sizes: None,
            values: None,
            colormap,
            value_range: None,
            colorbar: None,
            edge: None,
            opacity: 1.0,
            names: None,
            secondary: false,
        }
    }

    /// Replaces the data (per-point sizes/values/names are kept by index).
    pub fn set_data(&mut self, x: impl super::IntoValues, y: impl super::IntoValues) -> &mut Self {
        let (x, y) = (x.into_values(), y.into_values());
        let n = x.len().min(y.len());
        self.x = super::truncated(x, n);
        self.y = super::truncated(y, n);
        self
    }
    /// Appends a point.
    pub fn push(&mut self, x: f64, y: f64) -> &mut Self {
        self.x.push(x);
        self.y.push(y);
        self
    }
    /// Keeps only the last `n` points.
    pub fn keep_last(&mut self, n: usize) -> &mut Self {
        if self.x.len() > n {
            let k = self.x.len() - n;
            self.x.drain(..k);
            self.y.drain(..k);
            for v in [&mut self.sizes, &mut self.values].into_iter().flatten() {
                if v.len() >= k {
                    v.drain(..k);
                }
            }
        }
        self
    }

    /// Legend label.
    pub fn label(&mut self, label: impl Into<String>) -> &mut Self {
        self.label = Some(label.into());
        self
    }
    /// Uniform color.
    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.color = color.into();
        self
    }
    /// Marker shape.
    pub fn marker(&mut self, marker: Marker) -> &mut Self {
        self.marker = marker;
        self
    }
    /// Uniform marker diameter in pixels.
    pub fn size(&mut self, size: f64) -> &mut Self {
        self.size = size.max(0.0);
        self
    }
    /// Per-point marker diameters in pixels.
    pub fn sizes(&mut self, sizes: impl super::IntoValues) -> &mut Self {
        self.sizes = Some(sizes.into_values());
        self
    }
    /// Bubble chart: per-point values mapped to diameters between `min_px`
    /// and `max_px` (area proportional to the value).
    pub fn bubble(
        &mut self,
        values: impl super::IntoValues,
        min_px: f64,
        max_px: f64,
    ) -> &mut Self {
        let v = values.into_values();
        let finite = v.iter().copied().filter(|x| x.is_finite() && *x >= 0.0);
        let max = finite.fold(0.0f64, f64::max);
        self.sizes = Some(
            v.iter()
                .map(|x| {
                    if max > 0.0 && x.is_finite() && *x >= 0.0 {
                        min_px + (max_px - min_px) * (x / max).sqrt()
                    } else {
                        min_px
                    }
                })
                .collect(),
        );
        if self.opacity == 1.0 {
            self.opacity = 0.7;
        }
        self
    }
    /// Per-point values colored with a colormap.
    pub fn values(&mut self, values: impl super::IntoValues) -> &mut Self {
        self.values = Some(values.into_values());
        self
    }
    /// Colormap for [`Scatter::values`].
    pub fn colormap(&mut self, colormap: Colormap) -> &mut Self {
        self.colormap = colormap;
        self
    }
    /// Fixed value range for the colormap (default: data range).
    pub fn value_range(&mut self, min: f64, max: f64) -> &mut Self {
        self.value_range = Some((min, max));
        self
    }
    /// Shows a colorbar with this label (requires [`Scatter::values`]).
    pub fn colorbar(&mut self, label: impl Into<String>) -> &mut Self {
        self.colorbar = Some(label.into());
        self
    }
    /// Marker outline.
    pub fn edge(&mut self, color: impl Into<Color>, width: f64) -> &mut Self {
        self.edge = Some(Stroke::new(color.into(), width));
        self
    }
    /// Opacity (`0..1`), useful for dense data.
    pub fn opacity(&mut self, opacity: f32) -> &mut Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
    /// Per-point names shown in tooltips (SVG `<title>` and window hover).
    pub fn names(&mut self, names: impl super::IntoLabels) -> &mut Self {
        self.names = Some(names.into_labels());
        self
    }
    /// Plots against the secondary y-axis.
    pub fn secondary(&mut self, on: bool) -> &mut Self {
        self.secondary = on;
        self
    }

    fn scale(&self) -> Option<ColorScale> {
        let v = self.values.as_ref()?;
        let (min, max) = self.value_range.unwrap_or_else(|| {
            v.iter()
                .copied()
                .filter(|x| x.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                    (a.min(x), b.max(x))
                })
        });
        min.is_finite().then(|| ColorScale {
            colormap: self.colormap,
            min,
            max,
            label: self.colorbar.clone(),
        })
    }
}

impl Series for Scatter {
    fn y_range_in(&self, x0: f64, x1: f64) -> Option<Option<(f64, f64)>> {
        Some(super::y_range_where(&self.x, x0, x1, |i| {
            (self.y[i], self.y[i])
        }))
    }
    fn extent(&self) -> Option<Extent> {
        Extent::of_points(self.x.iter().copied().zip(self.y.iter().copied()))
    }

    fn draw(&self, ctx: &mut Context) {
        let scale = self.scale();
        // Many identical opaque markers: a marker in the same quarter-unit
        // cell as an earlier one paints the same pixels (the CPU rasterizer
        // places markers on a quarter-pixel grid; exact at scale 1, within
        // a quarter pixel otherwise), and markers far outside the clipped
        // plot area are invisible; skip both.
        let base = self.color.with_alpha(self.color.a * self.opacity);
        let dedupe = self.x.len() > 20_000
            && self.sizes.is_none()
            && self.values.is_none()
            && self.names.is_none()
            && base.a >= 1.0;
        let margin = self.size * 2.0 + self.edge.as_ref().map_or(0.0, |e| e.width) + 4.0;
        // Grid aligned to whole units, so its cells are the rasterizer's
        // quarter-pixel cells (at scale 1).
        let (gx, gy) = ((ctx.rect.x - margin).floor(), (ctx.rect.y - margin).floor());
        let gw = ((ctx.rect.w + 2.0 * margin) * 4.0).ceil().max(1.0) as usize;
        let gh = ((ctx.rect.h + 2.0 * margin) * 4.0).ceil().max(1.0) as usize;
        let mut seen = if dedupe {
            vec![0u64; (gw * gh).div_ceil(64)]
        } else {
            Vec::new()
        };
        // Identical markers: one template and a compact list of positions.
        let uniform = self.sizes.is_none() && self.values.is_none() && self.names.is_none();
        let template = (uniform && self.x.len() >= 64)
            .then(|| {
                self.marker
                    .node(0.0, 0.0, self.size, base, self.edge.clone())
            })
            .flatten();
        // Opaque identical markers: only the set of occupied cells matters
        // (drawing order cannot change the result), so the positions are
        // kept as the occupancy bitset itself (size independent of n).
        if dedupe {
            if let Some(template) = template {
                for i in 0..self.x.len() {
                    let (px, py) = ctx.point(self.x[i], self.y[i]);
                    let (qx, qy) = (((px - gx) * 4.0).floor(), ((py - gy) * 4.0).floor());
                    if qx >= 0.0 && qy >= 0.0 && (qx as usize) < gw && (qy as usize) < gh {
                        let k = qy as usize * gw + qx as usize;
                        seen[k / 64] |= 1 << (k % 64);
                    }
                }
                ctx.push(Node::MarkerGrid {
                    template: Box::new(template),
                    x: gx,
                    y: gy,
                    step: 0.25,
                    cols: gw,
                    bits: seen,
                });
                return;
            }
        }
        let mut offsets: Vec<(f32, f32)> = Vec::new();
        for i in 0..self.x.len() {
            let (px, py) = ctx.point(self.x[i], self.y[i]);
            if dedupe {
                let (qx, qy) = (((px - gx) * 4.0).floor(), ((py - gy) * 4.0).floor());
                if !(qx >= 0.0 && qy >= 0.0 && (qx as usize) < gw && (qy as usize) < gh) {
                    continue;
                }
                let k = qy as usize * gw + qx as usize;
                if seen[k / 64] & (1 << (k % 64)) != 0 {
                    continue;
                }
                seen[k / 64] |= 1 << (k % 64);
            }
            if template.is_some() {
                if px.is_finite() && py.is_finite() {
                    offsets.push((px as f32, py as f32));
                }
                continue;
            }
            let size = self
                .sizes
                .as_ref()
                .and_then(|s| s.get(i).copied())
                .unwrap_or(self.size);
            let color = match (&scale, self.values.as_ref().and_then(|v| v.get(i))) {
                (Some(s), Some(v)) if v.is_finite() => s.color(*v),
                _ => self.color,
            };
            let color = color.with_alpha(color.a * self.opacity);
            let Some(node) = self.marker.node(px, py, size, color, self.edge.clone()) else {
                continue;
            };
            match self.names.as_ref().and_then(|n| n.get(i)) {
                Some(name) => ctx.push(Node::Group {
                    children: vec![node],
                    clip: None,
                    opacity: 1.0,
                    tooltip: Some(format!(
                        "{name} ({}, {})",
                        short(self.x[i]),
                        short(self.y[i])
                    )),
                    id: None,
                }),
                None => ctx.push(node),
            }
        }
        if let Some(template) = template {
            ctx.push(Node::Instances {
                template: Box::new(template),
                offsets,
            });
        }
    }

    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Marker {
                    marker: self.marker,
                    size: self.size.clamp(5.0, 12.0),
                    color: self.color,
                },
            })
            .collect()
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let pts = self.x.iter().copied().zip(self.y.iter().copied());
        let ((dx, dy), (cx, cy), d) = nearest_point(pts, x, y, px, py)?;
        let i = self
            .x
            .iter()
            .zip(&self.y)
            .position(|(a, b)| *a == dx && *b == dy);
        let name = i
            .and_then(|i| self.names.as_ref().and_then(|n| n.get(i)).cloned())
            .or_else(|| self.label.clone())
            .map(|n| format!("{n}: "))
            .unwrap_or_default();
        let value = i
            .and_then(|i| self.values.as_ref().and_then(|v| v.get(i)))
            .map(|v| format!(" = {}", short(*v)))
            .unwrap_or_default();
        Some(Hit {
            px: cx,
            py: cy,
            label: format!("{name}({}, {}){value}", short(dx), short(dy)),
            distance: d,
        })
    }

    fn secondary(&self) -> bool {
        self.secondary
    }

    fn color_scale(&self) -> Option<ColorScale> {
        self.colorbar.as_ref()?;
        self.scale()
    }

    fn data_points(&self, max: usize) -> Vec<(f64, f64)> {
        super::spread_points(&self.x, &self.y, max)
    }

    fn row_points(&self) -> Vec<(f64, f64)> {
        super::row_pairs(&self.x, &self.y)
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod dedupe_tests {
    use crate::prelude::*;

    /// Skipping markers that land on an already drawn position must not
    /// change the image.
    #[test]
    fn dense_scatter_dedupe_is_invisible() {
        let n = 60_000;
        let x: Vec<f64> = (0..n)
            .map(|i| ((i * 7919) % 1000) as f64 / 10.0 + (i % 3) as f64 * 0.001)
            .collect();
        let y: Vec<f64> = (0..n)
            .map(|i| ((i * 104_729) % 997) as f64 / 10.0)
            .collect();
        let render = |x: &[f64], y: &[f64]| {
            let mut fig = Figure::new(300, 200);
            fig.axes()
                .scatter(x, y)
                .size(3.0)
                .color(Color::rgb(30, 90, 200));
            fig.axes().x_limits(0.0, 100.0).y_limits(0.0, 100.0);
            fig.backend(Backend::Cpu { threads: 1 })
                .to_pixels()
                .unwrap()
        };
        // Deduplicated (n > 20 000) vs drawn in 4 batches below the threshold.
        let full = render(&x, &y);
        let mut fig = Figure::new(300, 200);
        for k in 0..4 {
            let r = k * n / 4..(k + 1) * n / 4;
            fig.axes()
                .scatter(&x[r.clone()], &y[r])
                .size(3.0)
                .color(Color::rgb(30, 90, 200));
        }
        fig.axes().x_limits(0.0, 100.0).y_limits(0.0, 100.0);
        let parts = fig
            .backend(Backend::Cpu { threads: 1 })
            .to_pixels()
            .unwrap();
        let worst = full
            .rgba
            .iter()
            .zip(&parts.rgba)
            .map(|(a, b)| a.abs_diff(*b))
            .max();
        // Same marker set; only 8-bit rounding of anti-aliased edges can
        // depend on drawing order.
        assert!(worst <= Some(2), "max difference {worst:?}");
    }
}
