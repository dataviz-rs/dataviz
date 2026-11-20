//! Line and marker styles.

use super::color::Color;
use super::scene::{Node, PathCmd, Stroke};

/// Line dash style. Patterns scale with the line width.
#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub enum Dash {
    /// Continuous line.
    #[default]
    Solid,
    /// Long dashes.
    Dashed,
    /// Round dots.
    Dotted,
    /// Dash, dot, dash, …
    DashDot,
    /// Long dash, short gap.
    LongDash,
    /// Custom pattern in pixels (on, off, on, off, …).
    Custom(Vec<f64>),
}

impl Dash {
    /// Dash array for a stroke of the given width (empty = solid).
    pub(crate) fn pattern(&self, width: f64) -> Vec<f64> {
        let w = width.max(0.5);
        match self {
            Dash::Solid => Vec::new(),
            Dash::Dashed => vec![4.0 * w, 2.5 * w],
            Dash::Dotted => vec![0.01, 2.0 * w],
            Dash::DashDot => vec![4.0 * w, 2.0 * w, 0.01, 2.0 * w],
            Dash::LongDash => vec![8.0 * w, 3.0 * w],
            Dash::Custom(v) => v.iter().copied().filter(|x| *x >= 0.0).collect(),
        }
    }

    /// Stroke with this dash; dotted lines get round caps.
    pub(crate) fn stroke(&self, color: Color, width: f64) -> Stroke {
        let mut s = Stroke::new(color, width).dashed(self.pattern(width));
        if matches!(self, Dash::Dotted | Dash::DashDot) {
            s.cap = super::scene::LineCap::Round;
        }
        s
    }
}

/// Marker shapes for scatter plots and line markers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Marker {
    /// No marker.
    None,
    /// ●
    #[default]
    Circle,
    /// ■
    Square,
    /// ◆
    Diamond,
    /// ▲
    TriangleUp,
    /// ▼
    TriangleDown,
    /// ✚ (outline only)
    Plus,
    /// ✖ (outline only)
    Cross,
    /// ★
    Star,
    /// ⬢
    Hexagon,
    /// ⬟
    Pentagon,
}

impl Marker {
    /// All visible markers (useful for cycling).
    pub const ALL: [Marker; 10] = [
        Marker::Circle,
        Marker::Square,
        Marker::Diamond,
        Marker::TriangleUp,
        Marker::TriangleDown,
        Marker::Plus,
        Marker::Cross,
        Marker::Star,
        Marker::Hexagon,
        Marker::Pentagon,
    ];

    fn is_line_only(self) -> bool {
        matches!(self, Marker::Plus | Marker::Cross)
    }

    /// Draws the marker centred on `(x, y)`; `size` is the diameter in pixels.
    pub(crate) fn node(
        self,
        x: f64,
        y: f64,
        size: f64,
        fill: Color,
        edge: Option<Stroke>,
    ) -> Option<Node> {
        if self == Marker::None || !x.is_finite() || !y.is_finite() || size <= 0.0 {
            return None;
        }
        let r = size / 2.0;
        if self.is_line_only() {
            let s = edge
                .filter(|e| e.width > 0.0)
                .unwrap_or_else(|| Stroke::new(fill, (size / 5.0).max(1.0)));
            let d = if self == Marker::Cross { r * 0.8 } else { r };
            let cmds = if self == Marker::Plus {
                vec![
                    PathCmd::MoveTo(x - d, y),
                    PathCmd::LineTo(x + d, y),
                    PathCmd::MoveTo(x, y - d),
                    PathCmd::LineTo(x, y + d),
                ]
            } else {
                vec![
                    PathCmd::MoveTo(x - d, y - d),
                    PathCmd::LineTo(x + d, y + d),
                    PathCmd::MoveTo(x - d, y + d),
                    PathCmd::LineTo(x + d, y - d),
                ]
            };
            let stroke = Stroke {
                color: if s.color.is_visible() { s.color } else { fill },
                ..s
            };
            return Some(Node::Path {
                cmds,
                fill: None,
                stroke: Some(stroke),
            });
        }
        if self == Marker::Circle {
            return Some(Node::Circle {
                cx: x,
                cy: y,
                r,
                fill: Some(fill),
                stroke: edge,
            });
        }
        if self == Marker::Square {
            let h = r * 0.886; // same area as the circle
            return Some(Node::Rect {
                x: x - h,
                y: y - h,
                w: 2.0 * h,
                h: 2.0 * h,
                rx: 0.0,
                fill: Some(fill),
                stroke: edge,
            });
        }
        let pts: Vec<(f64, f64)> = match self {
            Marker::Diamond => vec![(0.0, -1.2), (0.85, 0.0), (0.0, 1.2), (-0.85, 0.0)],
            Marker::TriangleUp => vec![(0.0, -1.2), (1.05, 0.6), (-1.05, 0.6)],
            Marker::TriangleDown => vec![(0.0, 1.2), (1.05, -0.6), (-1.05, -0.6)],
            Marker::Star => (0..10)
                .map(|i| {
                    let a = -std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::PI / 5.0;
                    let rr = if i % 2 == 0 { 1.3 } else { 0.55 };
                    (rr * a.cos(), rr * a.sin())
                })
                .collect(),
            Marker::Hexagon => (0..6)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::PI / 3.0 + std::f64::consts::PI / 6.0;
                    (1.05 * a.cos(), 1.05 * a.sin())
                })
                .collect(),
            Marker::Pentagon => (0..5)
                .map(|i| {
                    let a =
                        -std::f64::consts::FRAC_PI_2 + i as f64 * 2.0 * std::f64::consts::PI / 5.0;
                    (1.1 * a.cos(), 1.1 * a.sin())
                })
                .collect(),
            _ => return None,
        };
        let mut cmds: Vec<PathCmd> = pts
            .iter()
            .enumerate()
            .map(|(i, (px, py))| {
                let (px, py) = (x + px * r, y + py * r);
                if i == 0 {
                    PathCmd::MoveTo(px, py)
                } else {
                    PathCmd::LineTo(px, py)
                }
            })
            .collect();
        cmds.push(PathCmd::Close);
        Some(Node::Path {
            cmds,
            fill: Some(fill),
            stroke: edge,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_marker_draws() {
        for m in Marker::ALL {
            assert!(m.node(5.0, 5.0, 8.0, Color::RED, None).is_some(), "{m:?}");
        }
        assert!(Marker::None.node(5.0, 5.0, 8.0, Color::RED, None).is_none());
        assert!(Marker::Circle
            .node(f64::NAN, 5.0, 8.0, Color::RED, None)
            .is_none());
    }

    #[test]
    fn dash_patterns_scale_with_width() {
        assert!(Dash::Solid.pattern(2.0).is_empty());
        assert_eq!(Dash::Dashed.pattern(2.0), vec![8.0, 5.0]);
        assert_eq!(
            Dash::Custom(vec![3.0, -1.0, 2.0]).pattern(1.0),
            vec![3.0, 2.0]
        );
    }
}
