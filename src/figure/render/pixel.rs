//! Pixel chart frame: background, title, grid, axes, ticks, labels, legend,
//! plus patterned polylines, markers and hover tooltips.

use ab_glyph::{FontArc, PxScale};
use imageproc::drawing::text_size;

use super::{font, AxesSpec, Frame, LegendEntry, Mapper};
use crate::figure::{
    canvas::pixelcanvas::{Pattern, PixelCanvas},
    configuration::figureconfig::FigureConfig,
    utilities::{linetype::LineType, scatterdottype::ScatterDotType},
};

pub(crate) fn frame(canvas: &PixelCanvas) -> Frame {
    Frame::new(canvas.width, canvas.height, canvas.margin)
}

/// Fonts for one drawing pass (loaded once instead of once per text).
pub(crate) struct Fonts {
    pub label: Option<FontArc>,
    pub title: Option<FontArc>,
}

impl Fonts {
    pub fn new(cfg: &FigureConfig) -> Self {
        Self {
            label: font::load(cfg.font_label.as_deref()),
            title: font::load(cfg.font_title.as_deref().or(cfg.font_label.as_deref())),
        }
    }
}

/// Horizontal anchor of a text.
#[derive(Clone, Copy)]
pub(crate) enum HAlign {
    Left,
    Center,
    Right,
}

/// Vertical anchor of a text.
#[derive(Clone, Copy)]
pub(crate) enum VAlign {
    Top,
    Middle,
}

/// Size of `text` in pixels, `(0, 0)` without a font.
pub(crate) fn measure(font: Option<&FontArc>, size: f32, text: &str) -> (f64, f64) {
    match font {
        Some(f) if size > 0.0 => {
            let (w, h) = text_size(PxScale::from(size), f, text);
            (w as f64, h as f64)
        }
        _ => (0.0, 0.0),
    }
}

/// Draws `text` anchored at `(x, y)`. Without a font nothing is drawn.
#[allow(clippy::too_many_arguments)]
pub(crate) fn text(
    canvas: &mut PixelCanvas,
    font: Option<&FontArc>,
    size: f32,
    x: f64,
    y: f64,
    text: &str,
    color: [u8; 3],
    align: (HAlign, VAlign),
) {
    let Some(f) = font else { return };
    if text.is_empty() || size <= 0.0 || !x.is_finite() || !y.is_finite() {
        return;
    }
    let (w, h) = measure(font, size, text);
    let x = match align.0 {
        HAlign::Left => x,
        HAlign::Center => x - w / 2.0,
        HAlign::Right => x - w,
    };
    let y = match align.1 {
        VAlign::Top => y,
        VAlign::Middle => y - h / 2.0,
    };
    canvas.draw_text_font(
        x.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32,
        y.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32,
        text,
        color,
        f,
        PxScale::from(size),
    );
}

/// Fills the closed rectangle `(x0, y0)`–`(x1, y1)` (any corner order).
pub(crate) fn fill_rect(
    canvas: &mut PixelCanvas,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    color: [u8; 3],
) {
    if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
        return;
    }
    let (w, h) = (canvas.width as f64, canvas.height as f64);
    let xa = x0.min(x1).round().clamp(0.0, w) as i64;
    let xb = x0.max(x1).round().clamp(-1.0, w - 1.0) as i64;
    let ya = y0.min(y1).round().clamp(0.0, h) as i64;
    let yb = y0.max(y1).round().clamp(-1.0, h - 1.0) as i64;
    for y in ya..=yb {
        for x in xa..=xb {
            canvas.put(x, y, color);
        }
    }
}

/// Outline of the rectangle `(x0, y0)`–`(x1, y1)`.
pub(crate) fn stroke_rect(
    canvas: &mut PixelCanvas,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    color: [u8; 3],
) {
    for (a, b) in [
        ((x0, y0), (x1, y0)),
        ((x1, y0), (x1, y1)),
        ((x1, y1), (x0, y1)),
        ((x0, y1), (x0, y0)),
    ] {
        line(canvas, a, b, color);
    }
}

/// Solid one-pixel line between two canvas points.
pub(crate) fn line(canvas: &mut PixelCanvas, a: (f64, f64), b: (f64, f64), color: [u8; 3]) {
    let mut p = Pattern::new(&LineType::Solid);
    canvas.stroke_segment(a, b, color, &mut p, true);
}

/// A line through all canvas `points`, keeping the dash pattern continuous
/// across vertices. Non-finite points split the line.
pub(crate) fn polyline(
    canvas: &mut PixelCanvas,
    points: &[(f64, f64)],
    color: [u8; 3],
    line_type: &LineType,
) {
    for run in points.split(|(x, y)| !x.is_finite() || !y.is_finite()) {
        let mut pattern = Pattern::new(line_type);
        for (i, w) in run.windows(2).enumerate() {
            canvas.stroke_segment(w[0], w[1], color, &mut pattern, i == 0);
        }
    }
}

/// A data-point marker centred on `(x, y)`.
pub(crate) fn marker(
    canvas: &mut PixelCanvas,
    x: f64,
    y: f64,
    dot: &ScatterDotType,
    color: [u8; 3],
) {
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    let (x, y) = (x.round() as i64, y.round() as i64);
    match *dot {
        ScatterDotType::Circle(radius) => {
            let r = radius as i64;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx * dx + dy * dy <= r * r {
                        canvas.put(x + dx, y + dy, color);
                    }
                }
            }
        }
        ScatterDotType::Square(size) => {
            let h = size as i64 / 2;
            for dy in -h..=h {
                for dx in -h..=h {
                    canvas.put(x + dx, y + dy, color);
                }
            }
        }
        ScatterDotType::Cross(arm) => {
            let a = arm as i64;
            for i in -a..=a {
                canvas.put(x + i, y, color);
                canvas.put(x, y + i, color);
            }
        }
        ScatterDotType::Triangle(base) => {
            let b = base as i64;
            for dy in 0..=b {
                let half = if b == 0 { 0 } else { b - dy };
                for dx in -half..=half {
                    canvas.put(x + dx, y - dy, color);
                }
            }
        }
    }
}

/// Clears the canvas, paints the plot background and the grid, and returns
/// the data → canvas mapping.
pub(crate) fn begin(canvas: &mut PixelCanvas, spec: &AxesSpec) -> Mapper {
    background(canvas, spec.config);
    grid(canvas, spec.config);
    Mapper::new(frame(canvas), spec.bounds)
}

/// Clears the canvas and paints the plot area with `color_background`.
pub(crate) fn background(canvas: &mut PixelCanvas, cfg: &FigureConfig) {
    canvas.clear();
    let f = frame(canvas);
    fill_rect(
        canvas,
        f.left,
        f.top,
        f.right,
        f.bottom,
        cfg.color_background,
    );
}

/// Grid with `num_grid_vertical` vertical and `num_grid_horizontal`
/// horizontal divisions.
pub(crate) fn grid(canvas: &mut PixelCanvas, cfg: &FigureConfig) {
    let f = frame(canvas);
    let nv = cfg.num_grid_vertical.max(1);
    let nh = cfg.num_grid_horizontal.max(1);
    for i in 0..=nv {
        let x = (f.left + f.width() * i as f64 / nv as f64).round();
        line(canvas, (x, f.top), (x, f.bottom), cfg.color_grid);
    }
    for i in 0..=nh {
        let y = (f.top + f.height() * i as f64 / nh as f64).round();
        line(canvas, (f.left, y), (f.right, y), cfg.color_grid);
    }
}

/// Axes, frame, ticks, axis labels, title and legend, on top of the data.
pub(crate) fn finish(canvas: &mut PixelCanvas, spec: &AxesSpec, m: &Mapper) {
    let cfg = spec.config;
    let fonts = Fonts::new(cfg);
    let f = m.frame;
    let axis = cfg.color_axis;

    stroke_rect(canvas, f.left, f.top, f.right, f.bottom, axis);
    let ax_y = m.x_axis_y().round();
    let ax_x = m.y_axis_x().round();
    line(canvas, (f.left, ax_y), (f.right, ax_y), axis);
    line(canvas, (ax_x, f.top), (ax_x, f.bottom), axis);

    let fs = cfg.font_size_axis;
    let font = fonts.label.as_ref();

    let x_ticks = spec.x_ticks.resolve(m.bounds.x_min, m.bounds.x_max);
    let xs: Vec<f64> = x_ticks.iter().map(|t| m.x(t.0).round()).collect();
    let max_w = x_ticks
        .iter()
        .map(|t| measure(font, fs, &t.1).0)
        .fold(0.0, f64::max);
    let stride = super::label_stride(&xs, max_w);
    for (i, ((_, label), &x)) in x_ticks.iter().zip(&xs).enumerate() {
        line(canvas, (x, ax_y - 3.0), (x, ax_y + 3.0), axis);
        if i % stride == 0 {
            let align = (HAlign::Center, VAlign::Top);
            text(canvas, font, fs, x, f.bottom + 5.0, label, axis, align);
        }
    }

    let y_ticks = spec.y_ticks.resolve(m.bounds.y_min, m.bounds.y_max);
    let ys: Vec<f64> = y_ticks.iter().map(|t| m.y(t.0).round()).collect();
    let max_h = y_ticks
        .iter()
        .map(|t| measure(font, fs, &t.1).1)
        .fold(0.0, f64::max);
    let stride = super::label_stride(&ys, max_h);
    for (i, ((_, label), &y)) in y_ticks.iter().zip(&ys).enumerate() {
        line(canvas, (ax_x - 3.0, y), (ax_x + 3.0, y), axis);
        if i % stride == 0 {
            let align = (HAlign::Right, VAlign::Middle);
            text(canvas, font, fs, f.left - 6.0, y, label, axis, align);
        }
    }

    axis_labels(canvas, cfg, &fonts, ax_y, spec.x_label, spec.y_label);
    title(canvas, cfg, &fonts, spec.title);
    legend(canvas, cfg, &fonts, &spec.legend);
}

/// X label to the right of the x-axis, Y label above the y-axis (the pixel
/// renderer cannot rotate text).
pub(crate) fn axis_labels(
    canvas: &mut PixelCanvas,
    cfg: &FigureConfig,
    fonts: &Fonts,
    x_axis_y: f64,
    x_label: &str,
    y_label: &str,
) {
    let f = frame(canvas);
    let size = cfg.font_size_label;
    text(
        canvas,
        fonts.label.as_ref(),
        size,
        f.right + 6.0,
        x_axis_y,
        x_label,
        cfg.color_axis,
        (HAlign::Left, VAlign::Middle),
    );
    text(
        canvas,
        fonts.label.as_ref(),
        size,
        f.left,
        f.top - 6.0 - measure(fonts.label.as_ref(), size, y_label).1,
        y_label,
        cfg.color_axis,
        (HAlign::Center, VAlign::Top),
    );
}

pub(crate) fn title(canvas: &mut PixelCanvas, cfg: &FigureConfig, fonts: &Fonts, title: &str) {
    let f = frame(canvas);
    text(
        canvas,
        fonts.title.as_ref(),
        cfg.font_size_title,
        canvas.width as f64 / 2.0,
        f.margin() / 2.0,
        title,
        cfg.color_title,
        (HAlign::Center, VAlign::Middle),
    );
}

/// Legend below the x tick labels, wrapping onto several rows when needed.
pub(crate) fn legend(
    canvas: &mut PixelCanvas,
    cfg: &FigureConfig,
    fonts: &Fonts,
    entries: &[LegendEntry],
) {
    if entries.is_empty() {
        return;
    }
    let f = frame(canvas);
    let fs = cfg.font_size_legend;
    let font = fonts.label.as_ref();
    let square = (fs as f64).max(4.0);
    let tick_h = measure(font, cfg.font_size_axis, "0")
        .1
        .max(cfg.font_size_axis as f64);
    let row_h = square.max(measure(font, fs, "Ag").1) + 6.0;
    let start_x = f.left;
    let max_x = (canvas.width as f64 - 4.0).max(start_x + 1.0);
    let mut x = start_x;
    let mut y = f.bottom + 5.0 + tick_h + 8.0;

    for (label, color) in entries {
        let w = square + 5.0 + measure(font, fs, label).0 + 12.0;
        if x + w > max_x && x > start_x {
            x = start_x;
            y += row_h;
        }
        fill_rect(canvas, x, y, x + square - 1.0, y + square - 1.0, *color);
        text(
            canvas,
            font,
            fs,
            x + square + 5.0,
            y + square / 2.0,
            label,
            cfg.color_axis,
            (HAlign::Left, VAlign::Middle),
        );
        x += w;
    }
}

/// Copies the canvas, draws a tooltip box with `label` next to the mouse
/// and a red line from `anchor` (if any) to the mouse, and returns the result
/// in the `0RGB` format used by `minifb`.
pub(crate) fn tooltip(
    canvas: &PixelCanvas,
    cfg: &FigureConfig,
    anchor: Option<(f64, f64)>,
    mouse: (u32, u32),
    label: &str,
) -> Vec<u32> {
    let mut img = PixelCanvas {
        width: canvas.width,
        height: canvas.height,
        background_color: canvas.background_color,
        buffer: canvas.buffer.clone(),
        margin: canvas.margin,
    };
    let (mx, my) = (mouse.0 as f64, mouse.1 as f64);
    if let Some(a) = anchor {
        line(&mut img, a, (mx, my), [255, 0, 0]);
    }

    let font = font::load(cfg.font_label.as_deref());
    let size = 12.0;
    let (tw, th) = measure(font.as_ref(), size, label);
    let (bw, bh) = (tw + 20.0, th.max(12.0) + 12.0);
    let (w, h) = (canvas.width as f64, canvas.height as f64);
    let bx = (mx + 10.0).min(w - bw).max(0.0);
    let by = (my - bh - 5.0).min(h - bh).max(0.0);
    fill_rect(
        &mut img,
        bx,
        by,
        bx + bw - 1.0,
        by + bh - 1.0,
        [255, 255, 255],
    );
    stroke_rect(&mut img, bx, by, bx + bw - 1.0, by + bh - 1.0, [0, 0, 0]);
    text(
        &mut img,
        font.as_ref(),
        size,
        bx + 10.0,
        by + bh / 2.0,
        label,
        [0, 0, 0],
        (HAlign::Left, VAlign::Middle),
    );
    to_argb(&img)
}

/// Converts the RGB buffer of a canvas to `0xFFRRGGBB` pixels.
pub(crate) fn to_argb(canvas: &PixelCanvas) -> Vec<u32> {
    canvas
        .buffer
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| 0xFF00_0000 | (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32)
        .collect()
}

/// Nearest point (by canvas distance) among `points`, with its distance.
pub(crate) fn nearest<'a, I>(m: &Mapper, mouse: (u32, u32), points: I) -> Option<((f64, f64), f64)>
where
    I: IntoIterator<Item = &'a (f64, f64)>,
{
    let (mx, my) = (mouse.0 as f64, mouse.1 as f64);
    points
        .into_iter()
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|&p| {
            let (px, py) = m.point(p);
            (p, (px - mx).hypot(py - my))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Canvas coordinates as `u32`, clamped to the canvas.
pub(crate) fn to_u32(canvas: &PixelCanvas, (x, y): (f64, f64)) -> (u32, u32) {
    let cx = if x.is_finite() {
        x.round().clamp(0.0, canvas.width.saturating_sub(1) as f64)
    } else {
        0.0
    };
    let cy = if y.is_finite() {
        y.round().clamp(0.0, canvas.height.saturating_sub(1) as f64)
    } else {
        0.0
    };
    (cx as u32, cy as u32)
}
