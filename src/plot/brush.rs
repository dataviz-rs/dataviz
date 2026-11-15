//! Linked brushing: points selected in one plot are highlighted in every
//! plot whose series have the same number of rows (row `i` of each is the
//! same record), in windows (Ctrl+drag), interactive HTML (Shift+drag) and
//! static output.
//!
//! ```
//! use dataviz::prelude::*;
//! let (a, b, c) = (vec![1.0, 2.0, 3.0, 4.0], vec![4.0, 1.0, 3.0, 2.0], vec![9.0, 7.0, 8.0, 6.0]);
//! let mut fig = Figure::new(800, 400);
//! fig.grid(1, 2).link_brushing(true);
//! fig.at(0, 0).scatter(a.clone(), b);
//! fig.at(0, 1).scatter(a, c);
//! fig.brush(0, (1.5, 3.5), (0.0, 5.0)); // rows 1 and 2
//! assert_eq!(fig.brushed(), &[1, 2]);
//! ```

use super::axes::Layout;
use super::color::Color;
use super::figure::Figure;
use super::scene::{Node, Stroke};

/// Highlight ring of brushed points.
pub(crate) const BRUSH_COLOR: Color = Color::rgb(255, 152, 0);

/// Selected rows: their count per series (`rows`) and the indices.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Brush {
    pub(crate) rows: usize,
    pub(crate) selected: Vec<usize>,
}

impl Figure {
    /// Links selections between plots: a box selected in one plot (Ctrl+drag
    /// in a window, Shift+drag in HTML output, or [`Figure::brush`])
    /// highlights the same rows in every plot. Rows are matched by index
    /// among series with the same number of points (scatter, line, bar and
    /// price series).
    pub fn link_brushing(&mut self, on: bool) -> &mut Self {
        self.brushing = on;
        if !on {
            self.brush = None;
        }
        self
    }

    /// Whether linked brushing is on.
    pub fn brushing(&self) -> bool {
        self.brushing
    }

    /// Selects the rows of plot `plot` inside the data box `x` × `y`: the
    /// points of its first series that takes part, and of later series with
    /// the same number of rows. Returns the number of rows selected.
    pub fn brush(&mut self, plot: usize, x: (f64, f64), y: (f64, f64)) -> usize {
        let (x0, x1) = (x.0.min(x.1), x.0.max(x.1));
        let (y0, y1) = (y.0.min(y.1), y.0.max(y.1));
        let Some(ax) = self.axes_at(plot) else {
            self.brush = None;
            return 0;
        };
        let mut rows = 0;
        let mut hit = Vec::new();
        for (k, s) in ax.series.iter().enumerate() {
            if !ax.is_visible(k) {
                continue;
            }
            let pts = s.row_points();
            if pts.is_empty() || (rows != 0 && pts.len() != rows) {
                continue;
            }
            if rows == 0 {
                rows = pts.len();
                hit = vec![false; rows];
            }
            for (i, (px, py)) in pts.iter().enumerate() {
                if *px >= x0 && *px <= x1 && *py >= y0 && *py <= y1 {
                    hit[i] = true;
                }
            }
        }
        let selected: Vec<usize> = (0..hit.len()).filter(|&i| hit[i]).collect();
        let n = selected.len();
        self.brush = (rows > 0 && n > 0).then_some(Brush { rows, selected });
        n
    }

    /// Selects rows by index (all series with `rows` points take part).
    pub fn brush_rows(&mut self, rows: usize, selected: impl IntoIterator<Item = usize>) {
        let mut selected: Vec<usize> = selected.into_iter().filter(|&i| i < rows).collect();
        selected.sort_unstable();
        selected.dedup();
        self.brush = (!selected.is_empty()).then_some(Brush { rows, selected });
    }

    /// Clears the brushed rows.
    pub fn clear_brush(&mut self) {
        self.brush = None;
    }

    /// Indices of the brushed rows (empty when nothing is selected).
    pub fn brushed(&self) -> &[usize] {
        self.brush.as_ref().map_or(&[], |b| &b.selected)
    }

    /// Highlight of the brushed rows in plot `i` (clipped to its plot area).
    pub(crate) fn brush_nodes(&self, i: usize, lay: &Layout) -> Option<Node> {
        let b = self.brush.as_ref()?;
        let ax = self.axes_at(i)?;
        let mut children = Vec::new();
        let ring = Stroke::new(BRUSH_COLOR, 2.0);
        for (k, s) in ax.series.iter().enumerate() {
            if !ax.is_visible(k) {
                continue;
            }
            let pts = s.row_points();
            if pts.len() != b.rows {
                continue;
            }
            let y = if s.secondary() {
                lay.y2.as_ref().unwrap_or(&lay.y)
            } else {
                &lay.y
            };
            let offsets: Vec<(f32, f32)> = b
                .selected
                .iter()
                .filter_map(|&r| {
                    let (dx, dy) = pts[r];
                    let (px, py) = (lay.x.px(dx), y.px(dy));
                    (px.is_finite() && py.is_finite()).then_some((px as f32, py as f32))
                })
                .collect();
            if offsets.is_empty() {
                continue;
            }
            children.push(Node::Instances {
                template: Box::new(Node::Circle {
                    cx: 0.0,
                    cy: 0.0,
                    r: 5.5,
                    fill: Some(BRUSH_COLOR.with_alpha(0.25)),
                    stroke: Some(ring.clone()),
                }),
                offsets,
            });
        }
        if children.is_empty() {
            return None;
        }
        let p = lay.plot;
        Some(Node::Group {
            children,
            clip: Some((p.x - 6.0, p.y - 6.0, p.w + 12.0, p.h + 12.0)),
            opacity: 1.0,
            tooltip: None,
            id: Some("brush".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn rows_link_across_plots() {
        let x: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|v| v * v).collect();
        let mut fig = Figure::new(600, 300);
        fig.grid(1, 2).link_brushing(true);
        fig.at(0, 0).scatter(x.clone(), y.clone());
        fig.at(0, 1).line(y, x.clone());
        assert_eq!(fig.brush(0, (2.0, 4.0), (0.0, 100.0)), 3);
        assert_eq!(fig.brushed(), &[2, 3, 4]);
        let svg = fig.to_svg();
        assert!(svg.contains(r#"id="p0-brush""#) && svg.contains(r#"id="p1-brush""#));
        fig.clear_brush();
        assert!(fig.brushed().is_empty());
        assert!(!fig.to_svg().contains("brush"));
    }

    #[test]
    fn other_row_counts_are_not_highlighted() {
        let mut fig = Figure::new(600, 300);
        fig.grid(1, 2).link_brushing(true);
        fig.at(0, 0)
            .scatter(vec![0.0, 1.0, 2.0], vec![0.0, 1.0, 2.0]);
        fig.at(0, 1).scatter(vec![0.0, 1.0], vec![0.0, 1.0]);
        fig.brush_rows(3, [0, 2, 7]);
        assert_eq!(fig.brushed(), &[0, 2]);
        let svg = fig.to_svg();
        assert!(svg.contains(r#"id="p0-brush""#) && !svg.contains(r#"id="p1-brush""#));
    }
}
