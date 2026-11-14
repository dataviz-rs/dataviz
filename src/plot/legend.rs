//! Legend and colorbar drawing.

use super::scene::{Anchor, Baseline, Node, PathCmd, RasterImage, Stroke, TextNode};
use super::series::{ColorScale, LegendEntry, LegendSymbol, Rect};
use super::text::FontSet;
use super::theme::Theme;
use super::ticks;
use std::sync::Arc;

/// Legend placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum LegendPosition {
    /// The inside corner covering the least data (default).
    #[default]
    Best,
    /// Inside, top right.
    UpperRight,
    /// Inside, top left.
    UpperLeft,
    /// Inside, bottom left.
    LowerLeft,
    /// Inside, bottom right.
    LowerRight,
    /// Inside, top centre.
    UpperCenter,
    /// Inside, bottom centre.
    LowerCenter,
    /// Outside, right of the plot.
    OutsideRight,
    /// Outside, below the plot.
    OutsideBottom,
    /// Outside, above the plot.
    OutsideTop,
    /// No legend.
    Hidden,
}

pub(crate) struct LegendLayout {
    pub w: f64,
    pub h: f64,
    rows: Vec<Vec<(usize, f64)>>, // entry index, column x offset
    row_h: f64,
    sym_w: f64,
}

const PAD: f64 = 8.0;
const GAP: f64 = 6.0;

pub(crate) fn layout(
    entries: &[LegendEntry],
    theme: &Theme,
    fonts: &FontSet,
    columns: usize,
    max_width: f64,
) -> LegendLayout {
    let size = theme.legend_size;
    let sym_w = size * 2.0;
    let row_h = (size * 1.45).max(theme.marker_size + 2.0);
    let item_w: Vec<f64> = entries
        .iter()
        .map(|e| sym_w + GAP + fonts.width(&e.label, size))
        .collect();
    let cols = if columns == 0 {
        // Automatic: as many columns as fit (for horizontal legends).
        let mut cols = entries.len().max(1);
        while cols > 1 {
            let w = column_widths(&item_w, cols).iter().sum::<f64>()
                + (cols - 1) as f64 * 16.0
                + 2.0 * PAD;
            if w <= max_width {
                break;
            }
            cols -= 1;
        }
        cols
    } else {
        columns.max(1)
    };
    let widths = column_widths(&item_w, cols);
    let mut offsets = Vec::with_capacity(cols);
    let mut acc = PAD;
    for w in &widths {
        offsets.push(acc);
        acc += w + 16.0;
    }
    let rows: Vec<Vec<(usize, f64)>> = (0..entries.len())
        .collect::<Vec<_>>()
        .chunks(cols)
        .map(|c| {
            c.iter()
                .enumerate()
                .map(|(k, &i)| (i, offsets[k]))
                .collect()
        })
        .collect();
    LegendLayout {
        w: acc - 16.0 + PAD,
        h: rows.len() as f64 * row_h + 2.0 * PAD - 4.0,
        rows,
        row_h,
        sym_w,
    }
}

fn column_widths(item_w: &[f64], cols: usize) -> Vec<f64> {
    let mut w = vec![0.0f64; cols];
    for (i, iw) in item_w.iter().enumerate() {
        w[i % cols] = w[i % cols].max(*iw);
    }
    w
}

/// Canvas rectangle of every legend row (entry index) for a legend drawn
/// at (x, y).
pub(crate) fn item_rects(l: &LegendLayout, x: f64, y: f64) -> Vec<(Rect, usize)> {
    let mut v = Vec::new();
    for (r, row) in l.rows.iter().enumerate() {
        let top = y + PAD - 2.0 + l.row_h * r as f64;
        for (k, &(i, off)) in row.iter().enumerate() {
            let right = row
                .get(k + 1)
                .map_or(x + l.w - PAD / 2.0, |n| x + n.1 - 8.0);
            v.push((
                Rect {
                    x: x + off - PAD / 2.0,
                    y: top,
                    w: right - (x + off - PAD / 2.0),
                    h: l.row_h,
                },
                i,
            ));
        }
    }
    v
}

/// Draws a legend; rows with `dim[i]` (hidden series) are faded.
pub(crate) fn draw(
    out: &mut Vec<Node>,
    entries: &[LegendEntry],
    dim: &[bool],
    l: &LegendLayout,
    x: f64,
    y: f64,
    theme: &Theme,
) {
    out.push(Node::Rect {
        x,
        y,
        w: l.w,
        h: l.h,
        rx: 3.0,
        fill: Some(theme.legend_background),
        stroke: theme
            .legend_border
            .is_visible()
            .then(|| Stroke::new(theme.legend_border, 1.0)),
    });
    let size = theme.legend_size;
    for (r, row) in l.rows.iter().enumerate() {
        let cy = y + PAD - 2.0 + l.row_h * (r as f64 + 0.5);
        for &(i, off) in row {
            let e = &entries[i];
            let sx = x + off;
            let faded = dim.get(i).copied().unwrap_or(false);
            let start = out.len();
            symbol(out, &e.symbol, sx, cy, l.sym_w, size, theme);

            out.push(Node::Text(TextNode {
                x: sx + l.sym_w + GAP,
                y: cy,
                text: e.label.clone(),
                size,
                color: theme.text,
                anchor: Anchor::Start,
                baseline: Baseline::Middle,
                rotate: 0.0,
                bold: false,
                italic: false,
            }));
            if faded {
                let children = out.split_off(start);
                out.push(Node::Group {
                    children,
                    clip: None,
                    opacity: 0.3,
                    tooltip: None,
                });
            }
        }
    }
}

fn symbol(
    out: &mut Vec<Node>,
    s: &LegendSymbol,
    x: f64,
    cy: f64,
    w: f64,
    size: f64,
    theme: &Theme,
) {
    match s {
        LegendSymbol::Line {
            color,
            width,
            dash,
            marker,
        } => {
            if *width > 0.0 {
                out.push(Node::Path {
                    cmds: vec![PathCmd::MoveTo(x, cy), PathCmd::LineTo(x + w, cy)],
                    fill: None,
                    stroke: Some(dash.stroke(*color, width.min(4.0))),
                });
            }
            if let Some((m, ms)) = marker {
                if let Some(n) = m.node(x + w / 2.0, cy, ms.min(size), *color, None) {
                    out.push(n);
                }
            }
        }
        LegendSymbol::Patch { fill, edge } => out.push(Node::Rect {
            x: x + w * 0.15,
            y: cy - size * 0.4,
            w: w * 0.7,
            h: size * 0.8,
            rx: 1.5,
            fill: Some(*fill),
            stroke: edge.map(|c| Stroke::new(c, 1.0)),
        }),
        LegendSymbol::Marker {
            marker,
            size: ms,
            color,
        } => {
            if let Some(n) = marker.node(x + w / 2.0, cy, ms.min(size).max(4.0), *color, None) {
                out.push(n);
            }
        }
    }
    let _ = theme;
}

/// Width needed by a vertical colorbar with its labels.
pub(crate) fn colorbar_width(scale: &ColorScale, theme: &Theme, fonts: &FontSet) -> f64 {
    let t = ticks::ticks(&super::scale::Scale::Linear, scale.min, scale.max, 6);
    let lw = t
        .major
        .iter()
        .map(|v| fonts.width(&cb_label(*v, t.step, scale), theme.tick_size))
        .fold(0.0, f64::max);
    let label = if scale.label.is_some() {
        theme.label_size * 1.4
    } else {
        0.0
    };
    16.0 + 14.0 + 6.0 + lw + label
}

fn cb_label(v: f64, step: f64, s: &ColorScale) -> String {
    ticks::format_value(
        v,
        &ticks::TickFormat::Auto,
        &super::scale::Scale::Linear,
        step,
        s.min.abs().max(s.max.abs()),
        None,
    )
}

/// Draws a vertical colorbar in `r` (left edge = bar).
pub(crate) fn draw_colorbar(out: &mut Vec<Node>, s: &ColorScale, r: Rect, theme: &Theme) {
    let bar_w = 14.0;
    let x = r.x + 16.0;
    let n = 256u32;
    let mut rgba = Vec::with_capacity(n as usize * 4);
    for i in 0..n {
        let c = s.colormap.at(1.0 - i as f64 / (n - 1) as f64);
        rgba.extend_from_slice(&[c.r, c.g, c.b, 255]);
    }
    out.push(Node::Image {
        x,
        y: r.y,
        w: bar_w,
        h: r.h,
        image: Arc::new(RasterImage {
            width: 1,
            height: n,
            rgba,
        }),
        smooth: true,
    });
    out.push(Node::Rect {
        x,
        y: r.y,
        w: bar_w,
        h: r.h,
        rx: 0.0,
        fill: None,
        stroke: Some(Stroke::new(theme.axis, 1.0)),
    });
    let t = ticks::ticks(&super::scale::Scale::Linear, s.min, s.max, 6);
    let span = s.max - s.min;
    let mut maxw: f64 = 0.0;
    for v in &t.major {
        let f = if span == 0.0 { 0.5 } else { (v - s.min) / span };
        let y = r.bottom() - f * r.h;
        out.push(Node::Path {
            cmds: vec![
                PathCmd::MoveTo(x + bar_w, y),
                PathCmd::LineTo(x + bar_w + 4.0, y),
            ],
            fill: None,
            stroke: Some(Stroke::new(theme.axis, 1.0)),
        });
        let label = cb_label(*v, t.step, s);
        maxw = maxw.max(label.chars().count() as f64 * theme.tick_size * 0.6);
        out.push(Node::Text(TextNode {
            x: x + bar_w + 6.0,
            y,
            text: label,
            size: theme.tick_size,
            color: theme.text,
            anchor: Anchor::Start,
            baseline: Baseline::Middle,
            rotate: 0.0,
            bold: false,
            italic: false,
        }));
    }
    if let Some(label) = &s.label {
        let lx = r.right() - theme.label_size * 0.5;
        out.push(Node::Text(TextNode {
            x: lx,
            y: r.y + r.h / 2.0,
            text: label.clone(),
            size: theme.label_size,
            color: theme.text,
            anchor: Anchor::Middle,
            baseline: Baseline::Middle,
            rotate: 90.0,
            bold: false,
            italic: false,
        }));
    }
}
