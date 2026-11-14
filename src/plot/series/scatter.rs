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
            x: x[..n].to_vec(),
            y: y[..n].to_vec(),
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
        self.x = x[..n].to_vec();
        self.y = y[..n].to_vec();
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
    fn extent(&self) -> Option<Extent> {
        Extent::of_points(self.x.iter().copied().zip(self.y.iter().copied()))
    }

    fn draw(&self, ctx: &mut Context) {
        let scale = self.scale();
        for i in 0..self.x.len() {
            let (px, py) = ctx.point(self.x[i], self.y[i]);
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
                }),
                None => ctx.push(node),
            }
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

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
