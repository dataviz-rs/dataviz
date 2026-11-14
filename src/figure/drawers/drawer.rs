use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    render::{
        font,
        pixel::{self, HAlign, VAlign},
        svg,
    },
    utilities::{axistype::AxisType, linetype::LineType},
};

use std::any::Any;

/// A trait for rendering charts and graphs, supporting multiple output formats.
///
/// Text on a [`PixelCanvas`] is rendered with the fonts in
/// [`FigureConfig::font_label`] / [`FigureConfig::font_title`]. When they are
/// not set (or cannot be loaded) a sans-serif system font is used; if no
/// font is available at all, text is skipped.
pub trait Drawer: Any {
    fn as_any(&mut self) -> &mut (dyn Any + 'static);
    fn get_figure_config(&self) -> &FigureConfig;

    /// Draws the main content of the plot on a `PixelCanvas`.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the plot on.
    fn draw(&mut self, canvas: &mut PixelCanvas);

    /// Draws the legend for the plot on a `PixelCanvas`.
    /// [`Drawer::draw`] already draws it; calling this again is harmless.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the legend on.
    fn draw_legend(&self, canvas: &mut PixelCanvas);

    /// Draws the plot content on an `SvgCanvas`.
    ///
    /// # Parameters
    /// - `svg_canvas`: The `SvgCanvas` to render the plot on.
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas);

    /// Converts RGB color array to SVG color string format.
    ///
    /// # Parameters
    /// - `color`: RGB color as `[u8; 3]`.
    ///
    /// # Returns
    /// A string in format `"rgb(r,g,b)"`.
    fn rgb_to_svg_color(&self, color: [u8; 3]) -> String {
        svg::rgb(color)
    }

    /// Fills the SVG chart background area (inside margins) with the background color.
    ///
    /// # Parameters
    /// - `svg_canvas`: The `SvgCanvas` to draw on.
    /// - `config`: The `FigureConfig` containing the background color.
    fn fill_svg_background(&self, svg_canvas: &mut SvgCanvas, config: &FigureConfig) {
        let f = svg::frame(svg_canvas);
        svg_canvas.draw_rect(
            f.left,
            f.top,
            f.width(),
            f.height(),
            &svg::rgb(config.color_background),
            "none",
            0.0,
            1.0,
        );
    }

    /// Fills the chart background area (inside margins) with the background color.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to fill.
    /// - `config`: The `FigureConfig` containing the background color.
    fn fill_background(&self, canvas: &mut PixelCanvas, config: &FigureConfig) {
        let f = pixel::frame(canvas);
        pixel::fill_rect(
            canvas,
            f.left,
            f.top,
            f.right,
            f.bottom,
            config.color_background,
        );
    }

    /// Draws the grid for the plot: `num_grid_vertical` vertical and
    /// `num_grid_horizontal` horizontal divisions of the plot area.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the grid on.
    /// - `config`: The `FigureConfig` containing grid appearance settings.
    fn draw_grid(&self, canvas: &mut PixelCanvas, config: &FigureConfig) {
        pixel::grid(canvas, config);
    }

    /// Draws an axis line on the canvas.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the axis on.
    /// - `config`: The `FigureConfig` containing axis appearance settings.
    /// - `x1`, `y1`: The starting coordinates of the axis.
    /// - `x2`, `y2`: The ending coordinates of the axis.
    fn draw_axis(
        &self,
        canvas: &mut PixelCanvas,
        config: &FigureConfig,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    ) {
        canvas.draw_line(x1, y1, x2, y2, config.color_axis, LineType::Solid);
    }

    /// Draws a text label on the canvas.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the label on.
    /// - `config`: The `FigureConfig` containing label appearance settings.
    /// - `x`, `y`: The position to draw the label, centered on `(x, y)`.
    /// - `text`: The label text.
    fn draw_label(
        &self,
        canvas: &mut PixelCanvas,
        config: &FigureConfig,
        x: u32,
        y: u32,
        text: &str,
    ) {
        let font = font::load(config.font_label.as_deref());
        pixel::text(
            canvas,
            font.as_ref(),
            config.font_size_label,
            x as f64,
            y as f64,
            text,
            config.color_axis,
            (HAlign::Center, VAlign::Middle),
        );
    }

    /// Draws the plot title on the canvas.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the title on.
    /// - `config`: The `FigureConfig` containing title appearance settings.
    /// - `x`, `y`: The position to draw the title, centered on `(x, y)`.
    /// - `text`: The title text.
    fn draw_title(
        &self,
        canvas: &mut PixelCanvas,
        config: &FigureConfig,
        x: u32,
        y: u32,
        text: &str,
    ) {
        let font = font::load(
            config
                .font_title
                .as_deref()
                .or(config.font_label.as_deref()),
        );
        pixel::text(
            canvas,
            font.as_ref(),
            config.font_size_title,
            x as f64,
            y as f64,
            text,
            config.color_title,
            (HAlign::Center, VAlign::Middle),
        );
    }

    /// Draws a value on the axis (tick label) based on its type.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the axis value on.
    /// - `config`: The `FigureConfig` containing axis value appearance settings.
    /// - `x`, `y`: The position to draw the value.
    /// - `text`: The text of the axis value.
    /// - `axis`: The type of axis (`AxisType::AxisX`: centered below `(x, y)`;
    ///   `AxisType::AxisY`: right-aligned and vertically centered on `(x, y)`).
    fn draw_axis_value(
        &self,
        canvas: &mut PixelCanvas,
        config: &FigureConfig,
        x: u32,
        y: u32,
        text: &str,
        axis: AxisType,
    ) {
        let font = font::load(config.font_label.as_deref());
        let (h, v) = match axis {
            AxisType::AxisX => (HAlign::Center, VAlign::Top),
            AxisType::AxisY => (HAlign::Right, VAlign::Middle),
        };
        let y = match axis {
            AxisType::AxisX => {
                y as f64 + pixel::measure(font.as_ref(), config.font_size_axis, text).1
            }
            AxisType::AxisY => y as f64,
        };
        pixel::text(
            canvas,
            font.as_ref(),
            config.font_size_axis,
            x as f64,
            y,
            text,
            config.color_axis,
            (h, v),
        );
    }
}
