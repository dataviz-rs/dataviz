//! SVG chart frame: background, title, grid, axes, ticks, labels, legend.
//! Every size and colour comes from `FigureConfig`.

use super::{AxesSpec, Frame, LegendEntry, Mapper};
use crate::figure::{
    canvas::svgcanvas::{escape_xml, SvgCanvas},
    configuration::figureconfig::FigureConfig,
    utilities::scatterdottype::ScatterDotType,
};

/// `rgb(r,g,b)` colour string.
pub(crate) fn rgb(c: [u8; 3]) -> String {
    format!("rgb({},{},{})", c[0], c[1], c[2])
}

pub(crate) fn frame(svg: &SvgCanvas) -> Frame {
    Frame::new(svg.width, svg.height, svg.margin)
}

/// Canvas background (`SvgCanvas::background_color`) and plot background
/// (`FigureConfig::color_background`).
pub(crate) fn background(svg: &mut SvgCanvas, cfg: &FigureConfig) {
    let f = frame(svg);
    let (w, h) = (svg.width as f64, svg.height as f64);
    let outer = svg.background_color.clone();
    svg.draw_rect(0.0, 0.0, w, h, &outer, "black", 1.0, 1.0);
    svg.draw_rect(
        f.left,
        f.top,
        f.width(),
        f.height(),
        &rgb(cfg.color_background),
        "none",
        0.0,
        1.0,
    );
}

pub(crate) fn title(svg: &mut SvgCanvas, cfg: &FigureConfig, text: &str) {
    let f = frame(svg);
    let size = cfg.font_size_title as f64;
    let y = (f.margin() / 2.0 + size * 0.35).max(size);
    svg.draw_text(svg.width as f64 / 2.0, y, text, size, &rgb(cfg.color_title));
}

/// Grid with `num_grid_vertical` vertical and `num_grid_horizontal`
/// horizontal divisions.
pub(crate) fn grid(svg: &mut SvgCanvas, cfg: &FigureConfig) {
    let f = frame(svg);
    svg.draw_grid(
        f.left,
        f.right,
        f.top,
        f.bottom,
        cfg.num_grid_vertical,
        cfg.num_grid_horizontal,
        &rgb(cfg.color_grid),
    );
}

/// Background, title and grid; returns the data → canvas mapping.
pub(crate) fn begin(svg: &mut SvgCanvas, spec: &AxesSpec) -> Mapper {
    background(svg, spec.config);
    grid(svg, spec.config);
    let m = Mapper::new(frame(svg), spec.bounds);
    // Axis lines go below the data so data on an axis stays visible.
    let f = m.frame;
    let axis = rgb(spec.config.color_axis);
    let (ax_y, ax_x) = (m.x_axis_y(), m.y_axis_x());
    svg.draw_line(f.left, ax_y, f.right, ax_y, &axis, 2.0);
    svg.draw_line(ax_x, f.top, ax_x, f.bottom, &axis, 2.0);
    m
}

/// Ticks, axis labels, title and legend, drawn on top of the data.
pub(crate) fn finish(svg: &mut SvgCanvas, spec: &AxesSpec, m: &Mapper) {
    let cfg = spec.config;
    let f = m.frame;
    let axis = rgb(cfg.color_axis);
    let fs = cfg.font_size_axis as f64;
    let ax_y = m.x_axis_y();
    let ax_x = m.y_axis_x();

    let mut marks = String::new();
    let mut labels = String::new();
    // SVG has no text metrics; estimate the width of a label.
    let width = |l: &str| l.chars().count() as f64 * fs * 0.6;

    let x_ticks = spec.x_ticks.resolve(m.bounds.x_min, m.bounds.x_max);
    let xs: Vec<f64> = x_ticks.iter().map(|t| m.x(t.0)).collect();
    let max_w = x_ticks.iter().map(|t| width(&t.1)).fold(0.0, f64::max);
    let stride = super::label_stride(&xs, max_w);
    for (i, ((_, label), x)) in x_ticks.iter().zip(&xs).enumerate() {
        marks.push_str(&format!(
            "M {x:.2},{:.2} L {x:.2},{:.2} ",
            ax_y - 4.0,
            ax_y + 4.0
        ));
        if i % stride == 0 {
            labels.push_str(&format!(
                r#"<text x="{x:.2}" y="{:.2}" font-size="{fs:.2}" font-family="sans-serif" text-anchor="middle" fill="{axis}">{}</text>"#,
                f.bottom + 4.0 + fs * 1.1,
                escape_xml(label)
            ));
        }
    }

    let y_ticks = spec.y_ticks.resolve(m.bounds.y_min, m.bounds.y_max);
    let ys: Vec<f64> = y_ticks.iter().map(|t| m.y(t.0)).collect();
    let stride = super::label_stride(&ys, fs);
    for (i, ((_, label), y)) in y_ticks.iter().zip(&ys).enumerate() {
        marks.push_str(&format!(
            "M {:.2},{y:.2} L {:.2},{y:.2} ",
            ax_x - 4.0,
            ax_x + 4.0
        ));
        if i % stride == 0 {
            labels.push_str(&format!(
                r#"<text x="{:.2}" y="{:.2}" font-size="{fs:.2}" font-family="sans-serif" text-anchor="end" fill="{axis}">{}</text>"#,
                f.left - 6.0,
                y + fs * 0.35,
                escape_xml(label)
            ));
        }
    }
    if !marks.is_empty() {
        svg.elements.push(format!(
            r#"<path d="{}" stroke="{axis}" stroke-width="1" fill="none"/>"#,
            marks.trim_end()
        ));
    }
    if !labels.is_empty() {
        svg.elements.push(labels);
    }

    axis_labels(svg, cfg, spec.x_label, spec.y_label);
    title(svg, cfg, spec.title);
    legend(svg, cfg, &spec.legend);
}

/// Baseline of the x-axis label (below the tick labels).
fn x_label_baseline(f: &Frame, cfg: &FigureConfig) -> f64 {
    f.bottom + 4.0 + cfg.font_size_axis as f64 * 1.1 + 6.0 + cfg.font_size_label as f64
}

pub(crate) fn axis_labels(svg: &mut SvgCanvas, cfg: &FigureConfig, x_label: &str, y_label: &str) {
    let f = frame(svg);
    let color = rgb(cfg.color_axis);
    let size = cfg.font_size_label as f64;
    if !x_label.is_empty() {
        svg.draw_text(
            (f.left + f.right) / 2.0,
            x_label_baseline(&f, cfg),
            x_label,
            size,
            &color,
        );
    }
    if !y_label.is_empty() {
        let x = (f.margin() / 4.0).max(size * 0.8);
        let y = (f.top + f.bottom) / 2.0;
        svg.elements.push(format!(
            r#"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" font-family="sans-serif" text-anchor="middle" fill="{color}" transform="rotate(-90 {x:.2} {y:.2})">{}</text>"#,
            escape_xml(y_label)
        ));
    }
}

/// Legend below the x-axis label, wrapping onto several rows when needed.
pub(crate) fn legend(svg: &mut SvgCanvas, cfg: &FigureConfig, entries: &[LegendEntry]) {
    if entries.is_empty() {
        return;
    }
    let f = frame(svg);
    let fs = cfg.font_size_legend as f64;
    let row_height = fs + 8.0;
    let start_x = f.left + 10.0;
    let max_x = (svg.width as f64 - 10.0).max(start_x + 1.0);
    let top = x_label_baseline(&f, cfg) + 8.0;

    let mut items = String::new();
    let (mut x, mut row) = (start_x, 0usize);
    let mut widest: f64 = 0.0;
    for (label, color) in entries {
        // Rough text width; SVG has no text metrics without a renderer.
        let width = fs * 1.4 + label.chars().count() as f64 * fs * 0.6 + fs * 1.5;
        if x + width > max_x && x > start_x {
            x = start_x;
            row += 1;
        }
        let y = top + row as f64 * row_height;
        items.push_str(&format!(
            r#"<rect x="{x:.2}" y="{y:.2}" width="{fs:.2}" height="{fs:.2}" fill="{}"/>"#,
            rgb(*color)
        ));
        items.push_str(&format!(
            r#"<text x="{:.2}" y="{:.2}" font-size="{fs:.2}" font-family="sans-serif" fill="{}">{}</text>"#,
            x + fs * 1.4,
            y + fs * 0.85,
            rgb(cfg.color_axis),
            escape_xml(label)
        ));
        x += width;
        widest = widest.max(x - start_x);
    }
    let bg = svg.background_color.clone();
    svg.draw_rect(
        start_x - 5.0,
        top - 4.0,
        widest + 5.0,
        (row + 1) as f64 * row_height,
        &bg,
        &rgb(cfg.color_axis),
        0.5,
        0.5,
    );
    svg.elements.push(items);
}

/// A data-point marker in the dataset colour.
pub(crate) fn marker(svg: &mut SvgCanvas, x: f64, y: f64, dot: &ScatterDotType, color: [u8; 3]) {
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    let fill = rgb(color);
    match *dot {
        ScatterDotType::Circle(radius) => svg.draw_circle(x, y, radius as f64, &fill),
        ScatterDotType::Square(side) => {
            let half = side as f64 / 2.0;
            svg.draw_rect(
                x - half,
                y - half,
                side as f64,
                side as f64,
                &fill,
                "none",
                0.0,
                1.0,
            )
        }
        ScatterDotType::Cross(arm) => {
            let a = arm as f64;
            svg.draw_line(x - a, y, x + a, y, &fill, 2.0);
            svg.draw_line(x, y - a, x, y + a, &fill, 2.0);
        }
        ScatterDotType::Triangle(base) => {
            let half = base as f64 / 2.0;
            let h = base as f64 * 0.866;
            svg.elements.push(format!(
                r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{fill}"/>"#,
                x,
                y - h / 2.0,
                x - half,
                y + h / 2.0,
                x + half,
                y + h / 2.0,
            ));
        }
    }
}
