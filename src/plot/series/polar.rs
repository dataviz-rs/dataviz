//! Polar plots, radar charts and polar bar (rose) charts.
//!
//! On polar axes, the x mapping turns an angle (radians) into a screen
//! angle and the y mapping turns a radius into pixels from the centre.

use super::{
    short, Context, Extent, Hit, IntoLabels, IntoValues, LegendEntry, LegendSymbol, Rect, Series,
};
use crate::plot::axes::{Axes, AxesKind, Layout};
use crate::plot::axis::AxisMap;
use crate::plot::color::Color;
use crate::plot::scale::Scale;
use crate::plot::scene::{Anchor, Baseline, Node, PathCmd, Stroke};
use crate::plot::style::{Dash, Marker};
use crate::plot::text::FontSet;
use crate::plot::theme::Theme;
use crate::plot::ticks;
use std::any::Any;
use std::f64::consts::{PI, TAU};

/// Configuration of the polar frame.
#[derive(Clone, Debug)]
pub struct PolarFrame {
    /// Spoke labels for radar charts (evenly spaced).
    pub(crate) categories: Vec<String>,
    /// Fixed radius range.
    pub(crate) r_limits: Option<(f64, f64)>,
    /// Screen angle of θ = 0, degrees counter-clockwise from east.
    pub(crate) start_deg: f64,
    /// θ grows clockwise.
    pub(crate) clockwise: bool,
    /// Straight (polygon) rings instead of circles.
    pub(crate) polygon_rings: bool,
}

impl Default for PolarFrame {
    fn default() -> Self {
        Self {
            categories: Vec::new(),
            r_limits: None,
            start_deg: 0.0,
            clockwise: false,
            polygon_rings: false,
        }
    }
}

/// Canvas position of polar coordinates using the polar maps.
pub(crate) fn polar_point(
    ctx_x: &AxisMap,
    ctx_y: &AxisMap,
    center: (f64, f64),
    theta: f64,
    r: f64,
) -> (f64, f64) {
    let a = ctx_x.px(theta);
    let rad = ctx_y.px(r).max(0.0);
    (center.0 + rad * a.cos(), center.1 - rad * a.sin())
}

impl PolarFrame {
    /// Square plot rect, theta map and radius map for a polar axes.
    pub(crate) fn maps(&self, axes: &Axes, r: Rect, theme: &Theme) -> (Rect, AxisMap, AxisMap) {
        let label_room = theme.tick_size * 2.6;
        let radius = ((r.w.min(r.h)) / 2.0 - label_room).max(5.0);
        let (cx, cy) = r.center();
        let plot = Rect {
            x: cx - radius,
            y: cy - radius,
            w: 2.0 * radius,
            h: 2.0 * radius,
        };
        let (r0, r1) = self.r_limits.unwrap_or_else(|| {
            let hi = axes
                .series
                .iter()
                .filter_map(|s| s.extent())
                .map(|e| e.y_max)
                .fold(0.0, f64::max);
            let hi = if hi > 0.0 { hi } else { 1.0 };
            let step = ticks::nice_step(hi / 4.0);
            (0.0, (hi / step).ceil() * step)
        });
        let a0 = self.start_deg.to_radians();
        let a1 = if self.clockwise { a0 - TAU } else { a0 + TAU };
        (
            plot,
            AxisMap::new(Scale::Linear, 0.0, TAU, a0, a1),
            AxisMap::new(Scale::Linear, r0, r1, 0.0, radius),
        )
    }

    pub(crate) fn draw_frame(
        &self,
        _axes: &Axes,
        lay: &Layout,
        theme: &Theme,
        fonts: &FontSet,
        out: &mut Vec<Node>,
        below: bool,
    ) {
        let center = lay.plot.center();
        let (r0, r1) = (lay.y.min, lay.y.max);
        let grid = Stroke::new(theme.grid, theme.grid_width);
        let spokes: Vec<(f64, String)> = if self.categories.is_empty() {
            (0..12)
                .map(|i| (i as f64 * PI / 6.0, format!("{}°", i * 30)))
                .collect()
        } else {
            let n = self.categories.len();
            self.categories
                .iter()
                .enumerate()
                .map(|(i, c)| (i as f64 * TAU / n as f64, c.clone()))
                .collect()
        };
        let rings = ticks::linear_ticks(r0, r1, 4).0;
        if below {
            // Background.
            let radius = lay.y.px(r1);
            if self.polygon_rings && !spokes.is_empty() {
                let pts: Vec<(f64, f64)> = spokes
                    .iter()
                    .map(|(t, _)| polar_point(&lay.x, &lay.y, center, *t, r1))
                    .collect();
                out.push(polygon(&pts, Some(theme.plot_background), None));
            } else {
                out.push(Node::Circle {
                    cx: center.0,
                    cy: center.1,
                    r: radius,
                    fill: Some(theme.plot_background),
                    stroke: None,
                });
            }
            for r in &rings {
                if *r <= r0 {
                    continue;
                }
                if self.polygon_rings && !spokes.is_empty() {
                    let pts: Vec<(f64, f64)> = spokes
                        .iter()
                        .map(|(t, _)| polar_point(&lay.x, &lay.y, center, *t, *r))
                        .collect();
                    out.push(polygon(&pts, None, Some(grid.clone())));
                } else {
                    out.push(Node::Circle {
                        cx: center.0,
                        cy: center.1,
                        r: lay.y.px(*r),
                        fill: None,
                        stroke: Some(grid.clone()),
                    });
                }
            }
            for (t, _) in &spokes {
                let p = polar_point(&lay.x, &lay.y, center, *t, r1);
                out.push(Node::Path {
                    cmds: vec![
                        PathCmd::MoveTo(center.0, center.1),
                        PathCmd::LineTo(p.0, p.1),
                    ],
                    fill: None,
                    stroke: Some(grid.clone()),
                });
            }
            return;
        }
        // Outer ring, spoke labels and radius labels on top of the data.
        let outer = Stroke::new(theme.axis, theme.axis_width);
        if self.polygon_rings && !spokes.is_empty() {
            let pts: Vec<(f64, f64)> = spokes
                .iter()
                .map(|(t, _)| polar_point(&lay.x, &lay.y, center, *t, r1))
                .collect();
            out.push(polygon(&pts, None, Some(outer)));
        } else {
            out.push(Node::Circle {
                cx: center.0,
                cy: center.1,
                r: lay.y.px(r1),
                fill: None,
                stroke: Some(outer),
            });
        }
        let ts = theme.tick_size;
        for (t, label) in &spokes {
            let a = lay.x.px(*t);
            let rad = lay.y.px(r1) + ts * 0.9;
            let (x, y) = (center.0 + rad * a.cos(), center.1 - rad * a.sin());
            let anchor = if a.cos() > 0.3 {
                Anchor::Start
            } else if a.cos() < -0.3 {
                Anchor::End
            } else {
                Anchor::Middle
            };
            out.push(crate::plot::axes::text(
                x,
                y,
                label,
                ts,
                theme.text,
                anchor,
                Baseline::Middle,
                false,
            ));
        }
        let step = rings
            .get(1)
            .zip(rings.first())
            .map(|(b, a)| b - a)
            .unwrap_or(1.0);
        // Radius labels along the bisector of the first two spokes.
        let label_theta = spokes.get(1).map_or(PI / 12.0, |s| s.0 / 2.0);
        for r in rings.iter().filter(|r| **r > r0) {
            let p = polar_point(&lay.x, &lay.y, center, label_theta, *r);
            let p = (p.0 + 2.0, p.1);
            let label =
                ticks::format_value(*r, &ticks::TickFormat::Auto, &Scale::Linear, step, r1, None);
            out.push(crate::plot::axes::text(
                p.0,
                p.1,
                &label,
                ts * 0.9,
                theme.text.with_alpha(0.75),
                Anchor::Start,
                Baseline::Bottom,
                false,
            ));
        }
        let _ = fonts;
    }
}

fn polygon(pts: &[(f64, f64)], fill: Option<Color>, stroke: Option<Stroke>) -> Node {
    let mut cmds: Vec<PathCmd> = pts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == 0 {
                PathCmd::MoveTo(p.0, p.1)
            } else {
                PathCmd::LineTo(p.0, p.1)
            }
        })
        .collect();
    cmds.push(PathCmd::Close);
    Node::Path { cmds, fill, stroke }
}

/// A line/area in polar coordinates (θ in radians). Created by
/// [`Axes::polar_line`] and [`Axes::radar`].
#[derive(Clone, Debug)]
pub struct PolarLine {
    pub(crate) theta: Vec<f64>,
    pub(crate) r: Vec<f64>,
    pub(crate) color: Color,
    pub(crate) width: f64,
    pub(crate) dash: Dash,
    pub(crate) closed: bool,
    pub(crate) fill: Option<f32>,
    pub(crate) marker: Marker,
    pub(crate) marker_size: f64,
    pub(crate) label: Option<String>,
}

impl PolarLine {
    /// Legend label.
    pub fn label(&mut self, l: impl Into<String>) -> &mut Self {
        self.label = Some(l.into());
        self
    }
    /// Color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
    /// Line width (0 = markers only).
    pub fn width(&mut self, w: f64) -> &mut Self {
        self.width = w.max(0.0);
        self
    }
    /// Dash style.
    pub fn dash(&mut self, d: Dash) -> &mut Self {
        self.dash = d;
        self
    }
    /// Connects the last point back to the first.
    pub fn closed(&mut self, on: bool) -> &mut Self {
        self.closed = on;
        self
    }
    /// Fills the enclosed area with this opacity.
    pub fn fill(&mut self, opacity: f32) -> &mut Self {
        self.fill = Some(opacity.clamp(0.0, 1.0));
        self
    }
    /// Markers.
    pub fn marker(&mut self, m: Marker) -> &mut Self {
        self.marker = m;
        self
    }
}

impl Series for PolarLine {
    fn extent(&self) -> Option<Extent> {
        let max = self
            .r
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(f64::NEG_INFINITY, f64::max);
        max.is_finite()
            .then(|| Extent::point(0.0, 0.0).include(TAU, max))
    }
    fn draw(&self, ctx: &mut Context) {
        let c = ctx.rect.center();
        let mut pts: Vec<(f64, f64)> = self
            .theta
            .iter()
            .zip(&self.r)
            .map(|(t, r)| polar_point(ctx.x, ctx.y, c, *t, *r))
            .collect();
        if let Some(op) = self.fill {
            ctx.polygon(&pts, Some(self.color.with_alpha(op)), None);
        }
        if self.closed && pts.len() > 2 {
            pts.push(pts[0]);
        }
        if self.width > 0.0 {
            ctx.polyline(&pts, self.dash.stroke(self.color, self.width));
        }
        for p in pts.iter().take(self.theta.len()) {
            if let Some(n) = self
                .marker
                .node(p.0, p.1, self.marker_size, self.color, None)
            {
                ctx.push(n);
            }
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: match self.fill {
                    Some(op) if self.width == 0.0 => LegendSymbol::Patch {
                        fill: self.color.with_alpha(op),
                        edge: None,
                    },
                    _ => LegendSymbol::Line {
                        color: self.color,
                        width: self.width,
                        dash: self.dash.clone(),
                        marker: (self.marker != Marker::None)
                            .then_some((self.marker, self.marker_size)),
                    },
                },
            })
            .collect()
    }
    fn hit(&self, x: &AxisMap, y: &AxisMap, rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let c = rect.center();
        self.theta
            .iter()
            .zip(&self.r)
            .filter(|(t, r)| t.is_finite() && r.is_finite())
            .map(|(t, r)| {
                let (a, b) = polar_point(x, y, c, *t, *r);
                let name = self
                    .label
                    .as_deref()
                    .map(|l| format!("{l}: "))
                    .unwrap_or_default();
                Hit {
                    px: a,
                    py: b,
                    label: format!("{name}θ={}°, r={}", short(t.to_degrees()), short(*r)),
                    distance: (a - px).hypot(b - py),
                }
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Bars in polar coordinates (rose / wind-rose / polar bar chart).
/// Created by [`Axes::polar_bar`] and [`Axes::rose`].
#[derive(Clone, Debug)]
pub struct PolarBars {
    pub(crate) theta: Vec<f64>,
    pub(crate) r: Vec<f64>,
    pub(crate) width: f64,
    pub(crate) colors: Vec<Color>,
    pub(crate) names: Vec<String>,
    pub(crate) opacity: f32,
}

impl PolarBars {
    /// Angular width in radians (default: spacing × 0.9).
    pub fn width(&mut self, radians: f64) -> &mut Self {
        self.width = radians.max(0.0);
        self
    }
    /// Colors per bar.
    pub fn colors<C: Into<Color>>(&mut self, colors: impl IntoIterator<Item = C>) -> &mut Self {
        self.colors = colors.into_iter().map(Into::into).collect();
        self
    }
    /// Fill opacity.
    pub fn opacity(&mut self, o: f32) -> &mut Self {
        self.opacity = o.clamp(0.0, 1.0);
        self
    }
}

impl Series for PolarBars {
    fn extent(&self) -> Option<Extent> {
        let max = self
            .r
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(f64::NEG_INFINITY, f64::max);
        max.is_finite()
            .then(|| Extent::point(0.0, 0.0).include(TAU, max))
    }
    fn draw(&self, ctx: &mut Context) {
        let c = ctx.rect.center();
        for (i, (t, r)) in self.theta.iter().zip(&self.r).enumerate() {
            if !r.is_finite() || *r <= 0.0 {
                continue;
            }
            let color = self.colors[i % self.colors.len().max(1)].with_alpha(self.opacity);
            let steps = 24;
            let mut pts = vec![c];
            for k in 0..=steps {
                let a = t - self.width / 2.0 + self.width * k as f64 / steps as f64;
                pts.push(polar_point(ctx.x, ctx.y, c, a, *r));
            }
            ctx.polygon(
                &pts,
                Some(color),
                Some(Stroke::new(ctx.theme.plot_background, 1.0)),
            );
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        if self.names.is_empty() || self.colors.len() < 2 {
            return Vec::new();
        }
        self.names
            .iter()
            .enumerate()
            .map(|(i, n)| LegendEntry {
                label: n.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.colors[i % self.colors.len()],
                    edge: None,
                },
            })
            .collect()
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    fn make_polar(&mut self) {
        self.kind = AxesKind::Polar;
    }

    /// Line in polar coordinates (θ in radians, counter-clockwise from east).
    pub fn polar_line(&mut self, theta: impl IntoValues, r: impl IntoValues) -> &mut PolarLine {
        self.make_polar();
        let color = self.next_color();
        let (w, ms) = (self.defaults.line_width, self.defaults.marker_size);
        self.add(PolarLine {
            theta: theta.into_values(),
            r: r.into_values(),
            color,
            width: w,
            dash: Dash::Solid,
            closed: false,
            fill: None,
            marker: Marker::None,
            marker_size: ms,
            label: None,
        })
    }

    /// Scatter in polar coordinates.
    pub fn polar_scatter(&mut self, theta: impl IntoValues, r: impl IntoValues) -> &mut PolarLine {
        let l = self.polar_line(theta, r);
        l.width = 0.0;
        l.marker = Marker::Circle;
        l
    }

    /// Radar (spider) chart: one closed, filled polygon per `(label, values)`
    /// over the given axes (`categories`).
    pub fn radar<L: Into<String>>(
        &mut self,
        categories: impl IntoLabels,
        series: impl IntoIterator<Item = (L, Vec<f64>)>,
    ) -> Vec<&mut PolarLine> {
        self.make_polar();
        let cats = categories.into_labels();
        let n = cats.len().max(1);
        self.polar.categories = cats;
        self.polar.start_deg = 90.0;
        self.polar.clockwise = true;
        self.polar.polygon_rings = true;
        let mut count = 0;
        for (label, values) in series {
            let theta: Vec<f64> = (0..values.len().min(n))
                .map(|i| i as f64 * TAU / n as f64)
                .collect();
            let r = values[..values.len().min(n)].to_vec();
            let l = self.polar_line(theta, r);
            l.closed = true;
            l.fill = Some(0.25);
            l.marker = Marker::Circle;
            l.marker_size = 5.0;
            l.label = Some(label.into());
            count += 1;
        }
        self.last_n(count)
    }

    /// Fixed radius range of polar axes.
    pub fn r_limits(&mut self, min: f64, max: f64) -> &mut Self {
        self.polar.r_limits = Some((min, max));
        self
    }

    /// Angle of θ = 0 (degrees counter-clockwise from east) and direction.
    pub fn polar_orientation(&mut self, start_deg: f64, clockwise: bool) -> &mut Self {
        self.polar.start_deg = start_deg;
        self.polar.clockwise = clockwise;
        self
    }

    /// Bars at angles θ (radians) with lengths `r`.
    pub fn polar_bar(&mut self, theta: impl IntoValues, r: impl IntoValues) -> &mut PolarBars {
        self.make_polar();
        let theta = theta.into_values();
        let mut sorted = theta.clone();
        sorted.sort_by(f64::total_cmp);
        let spacing = sorted
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|d| *d > 0.0)
            .fold(TAU, f64::min);
        let color = self.next_color();
        self.add(PolarBars {
            theta,
            r: r.into_values(),
            width: spacing * 0.9,
            colors: vec![color],
            names: Vec::new(),
            opacity: 0.85,
        })
    }

    /// Rose (Nightingale) chart: one wedge per category, clockwise from north.
    pub fn rose(&mut self, categories: impl IntoLabels, values: impl IntoValues) -> &mut PolarBars {
        let cats = categories.into_labels();
        let n = cats.len().max(1);
        let theta: Vec<f64> = (0..cats.len()).map(|i| i as f64 * TAU / n as f64).collect();
        self.polar.categories = cats.clone();
        self.polar.start_deg = 90.0;
        self.polar.clockwise = true;
        let colors: Vec<Color> = (0..cats.len()).map(|_| self.next_color()).collect();
        let b = self.polar_bar(theta, values);
        b.colors = colors;
        b.names = cats;
        b.width = TAU / n as f64 * 0.95;
        b
    }
}
