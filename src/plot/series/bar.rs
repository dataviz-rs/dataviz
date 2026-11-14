//! Bar charts: vertical/horizontal, grouped, stacked, percent, value labels.

use super::Rect;
use super::{short, Context, Extent, Hit, LegendEntry, LegendSymbol, Series};
use crate::plot::axis::AxisMap;
use crate::plot::color::Color;
use crate::plot::scene::{Anchor, Baseline, Node, Stroke};
use std::any::Any;

/// Where value labels go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LabelPosition {
    /// Just outside the end of the bar.
    Outside,
    /// Centred inside the bar.
    Inside,
}

/// A set of bars. Created by [`Axes::bar`](crate::plot::Axes::bar) and
/// friends; grouped and stacked charts are several `Bars`.
#[derive(Clone, Debug)]
pub struct Bars {
    pub(crate) pos: Vec<f64>,
    pub(crate) heights: Vec<f64>,
    pub(crate) bottoms: Vec<f64>,
    pub(crate) width: f64,
    pub(crate) offset: f64,
    pub(crate) horizontal: bool,
    pub(crate) color: Color,
    pub(crate) colors: Option<Vec<Color>>,
    pub(crate) edge: Option<Stroke>,
    pub(crate) label: Option<String>,
    pub(crate) value_labels: Option<LabelPosition>,
    pub(crate) label_values: Option<Vec<f64>>,
    pub(crate) label_format: Option<fn(f64) -> String>,
    pub(crate) names: Option<Vec<String>>,
    pub(crate) corner_radius: f64,
    pub(crate) secondary: bool,
}

impl Bars {
    pub(crate) fn new(pos: Vec<f64>, heights: Vec<f64>, color: Color) -> Self {
        let n = pos.len().min(heights.len());
        let pos = super::truncated(pos, n);
        // Default width: 80 % of the smallest spacing.
        let mut sorted: Vec<f64> = pos.iter().copied().filter(|v| v.is_finite()).collect();
        sorted.sort_by(f64::total_cmp);
        sorted.dedup();
        let spacing = sorted
            .windows(2)
            .map(|w| w[1] - w[0])
            .fold(f64::INFINITY, f64::min);
        let width = if spacing.is_finite() {
            spacing * 0.8
        } else {
            0.8
        };
        Self {
            pos,
            heights: super::truncated(heights, n),
            bottoms: vec![0.0; n],
            width,
            offset: 0.0,
            horizontal: false,
            color,
            colors: None,
            edge: None,
            label: None,
            value_labels: None,
            label_values: None,
            label_format: None,
            names: None,
            corner_radius: 0.0,
            secondary: false,
        }
    }

    /// Legend label.
    pub fn label(&mut self, label: impl Into<String>) -> &mut Self {
        self.label = Some(label.into());
        self
    }
    /// Bar color.
    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.color = color.into();
        self
    }
    /// One color per bar.
    pub fn colors<C: Into<Color>>(&mut self, colors: impl IntoIterator<Item = C>) -> &mut Self {
        self.colors = Some(colors.into_iter().map(Into::into).collect());
        self
    }
    /// Bar width in data units (category spacing is 1).
    pub fn width(&mut self, width: f64) -> &mut Self {
        self.width = width.max(0.0);
        self
    }
    /// Bar outline.
    pub fn edge(&mut self, color: impl Into<Color>, width: f64) -> &mut Self {
        self.edge = Some(Stroke::new(color.into(), width));
        self
    }
    /// Rounded bar ends (pixels).
    pub fn corner_radius(&mut self, radius: f64) -> &mut Self {
        self.corner_radius = radius.max(0.0);
        self
    }
    /// Starts each bar at these values instead of 0 (for stacking).
    pub fn bottoms(&mut self, bottoms: impl super::IntoValues) -> &mut Self {
        let mut b = bottoms.into_values();
        b.resize(self.pos.len(), 0.0);
        self.bottoms = b;
        self
    }
    /// Shows each bar's value.
    pub fn value_labels(&mut self, position: LabelPosition) -> &mut Self {
        self.value_labels = Some(position);
        self
    }
    /// Formatter for value labels.
    pub fn label_format(&mut self, f: fn(f64) -> String) -> &mut Self {
        self.label_format = Some(f);
        self
    }
    /// Plots against the secondary y-axis.
    pub fn secondary(&mut self, on: bool) -> &mut Self {
        self.secondary = on;
        self
    }

    /// (start, end, base, top) per bar along (position axis, value axis).
    fn geometry(&self) -> impl Iterator<Item = (usize, f64, f64, f64, f64)> + '_ {
        self.geometry_in(0..self.pos.len())
    }

    /// [`Bars::geometry`] of the bars with indices in `r`.
    fn geometry_in(
        &self,
        r: std::ops::Range<usize>,
    ) -> impl Iterator<Item = (usize, f64, f64, f64, f64)> + '_ {
        r.filter_map(move |i| {
            let (p, h, b) = (self.pos[i], self.heights[i], self.bottoms[i]);
            if !p.is_finite() || !h.is_finite() || !b.is_finite() {
                return None;
            }
            let c = p + self.offset;
            Some((i, c - self.width / 2.0, c + self.width / 2.0, b, b + h))
        })
    }

    fn color_of(&self, i: usize) -> Color {
        self.colors
            .as_ref()
            .and_then(|c| c.get(i % c.len().max(1)).copied())
            .unwrap_or(self.color)
    }

    fn fmt(&self, v: f64) -> String {
        match self.label_format {
            Some(f) => f(v),
            None => short(v),
        }
    }
}

impl Series for Bars {
    fn y_range_in(&self, x0: f64, x1: f64) -> Option<Option<(f64, f64)>> {
        if self.horizontal {
            return None;
        }
        let x: Vec<f64> = self.pos.iter().map(|p| p + self.offset).collect();
        Some(super::y_range_where(&x, x0, x1, |i| {
            let (b, h) = (self.bottoms.get(i).copied().unwrap_or(0.0), self.heights[i]);
            (b.min(b + h), b.max(b + h))
        }))
    }
    fn extent(&self) -> Option<Extent> {
        let mut e: Option<Extent> = None;
        for (_, a, b, lo, hi) in self.geometry() {
            let ext = Extent::point(a, lo).include(b, hi);
            e = Some(match e {
                None => ext,
                Some(e) => e.union(ext),
            });
        }
        let e = e?;
        let e = e.sticky_y_at(0.0);
        Some(if self.horizontal { e.transposed() } else { e })
    }

    fn draw(&self, ctx: &mut Context) {
        let ts = ctx.theme.annotation_size;
        for (i, a, b, lo, hi) in self.geometry() {
            let (x0, y0, x1, y1) = if self.horizontal {
                let (p0, q0) = ctx.point(lo, a);
                let (p1, q1) = ctx.point(hi, b);
                (p0, q0, p1, q1)
            } else {
                let (p0, q0) = ctx.point(a, lo);
                let (p1, q1) = ctx.point(b, hi);
                (p0, q0, p1, q1)
            };
            if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
                continue;
            }
            let color = self.color_of(i);
            let tooltip = self
                .names
                .as_ref()
                .and_then(|n| n.get(i))
                .map(|n| format!("{n}: {}", self.fmt(self.heights[i])));
            let rect = Node::Rect {
                x: x0.min(x1),
                y: y0.min(y1),
                w: (x1 - x0).abs(),
                h: (y1 - y0).abs(),
                rx: self.corner_radius,
                fill: Some(color),
                stroke: self.edge.clone(),
            };
            ctx.push(match tooltip {
                Some(t) => Node::Group {
                    children: vec![rect],
                    clip: None,
                    opacity: 1.0,
                    tooltip: Some(t),
                },
                None => rect,
            });

            if let Some(position) = self.value_labels {
                let value = self
                    .label_values
                    .as_ref()
                    .and_then(|v| v.get(i))
                    .copied()
                    .unwrap_or(self.heights[i]);
                let text = self.fmt(value);
                let negative = hi < lo;
                let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                let (tx, ty, anchor, baseline, tcolor) = match (position, self.horizontal) {
                    (LabelPosition::Inside, _) => (
                        cx,
                        cy,
                        Anchor::Middle,
                        Baseline::Middle,
                        color.contrasting_text(),
                    ),
                    (LabelPosition::Outside, false) => {
                        let y = if negative { y1 + 3.0 } else { y1 - 3.0 };
                        let b = if negative {
                            Baseline::Top
                        } else {
                            Baseline::Bottom
                        };
                        (cx, y, Anchor::Middle, b, ctx.theme.text)
                    }
                    (LabelPosition::Outside, true) => {
                        let (x, a) = if negative {
                            (x1 - 4.0, Anchor::End)
                        } else {
                            (x1 + 4.0, Anchor::Start)
                        };
                        (x, cy, a, Baseline::Middle, ctx.theme.text)
                    }
                };
                // Skip inside labels that do not fit.
                let fits = position == LabelPosition::Outside
                    || (ctx.text_width(&text, ts) < (x1 - x0).abs() - 2.0 && ts < (y1 - y0).abs());
                if fits {
                    ctx.text(tx, ty, text, ts, tcolor, anchor, baseline);
                }
            }
        }
    }

    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.color,
                    edge: self.edge.as_ref().map(|e| e.color),
                },
            })
            .collect()
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        let n = self.pos.len();
        let near = if self.horizontal {
            None
        } else {
            let r = 40.0 + (x.px(self.width) - x.px(0.0)).abs();
            super::x_window(&self.pos, self.offset, x, px, r)
        }
        .unwrap_or(0..n);
        for (i, a, b, lo, hi) in self.geometry_in(near) {
            let (x0, y0, x1, y1) = if self.horizontal {
                (x.px(lo), y.px(a), x.px(hi), y.px(b))
            } else {
                (x.px(a), y.px(lo), x.px(b), y.px(hi))
            };
            let inside =
                px >= x0.min(x1) && px <= x0.max(x1) && py >= y0.min(y1) && py <= y0.max(y1);
            let (tx, ty) = if self.horizontal {
                (x1, (y0 + y1) / 2.0)
            } else {
                ((x0 + x1) / 2.0, y1)
            };
            let d = if inside {
                0.0
            } else {
                (tx - px).hypot(ty - py)
            };
            if !d.is_finite() || best.as_ref().is_some_and(|h| h.distance <= d) {
                continue;
            }
            let name = self
                .names
                .as_ref()
                .and_then(|n| n.get(i).cloned())
                .unwrap_or_else(|| short(self.pos[i]));
            let series = self
                .label
                .as_deref()
                .map(|l| format!(" ({l})"))
                .unwrap_or_default();
            best = Some(Hit {
                px: tx,
                py: ty,
                label: format!("{name}{series}: {}", self.fmt(self.heights[i])),
                distance: d,
            });
        }
        best
    }

    fn secondary(&self) -> bool {
        self.secondary
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Bar layout for several series per category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BarMode {
    /// Side by side.
    Grouped,
    /// On top of each other (negative values stack downwards).
    Stacked,
    /// Stacked and normalized to 100 % per category.
    Percent,
    /// On top of each other at the same position, overlapping.
    Overlay,
}
