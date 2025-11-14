use ab_glyph::{FontRef, PxScale};
use imageproc::drawing::text_size;

use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    utilities::{axistype::AxisType, linetype::LineType},
};

use std::any::Any;

/// A trait for rendering charts and graphs, supporting multiple output formats.
pub trait Drawer: Any {
    fn as_any(&mut self) -> &mut (dyn Any + 'static);
    fn get_figure_config(&self) -> &FigureConfig;

    /// Draws the main content of the plot on a `PixelCanvas`.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the plot on.
    fn draw(&mut self, canvas: &mut PixelCanvas);

    /// Draws the legend for the plot on a `PixelCanvas`.
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
        format!("rgb({},{},{})", color[0], color[1], color[2])
    }

    /// Fills the SVG chart background area (inside margins) with the background color.
    ///
    /// # Parameters
    /// - `svg_canvas`: The `SvgCanvas` to draw on.
    /// - `config`: The `FigureConfig` containing the background color.
    fn fill_svg_background(&self, svg_canvas: &mut SvgCanvas, config: &FigureConfig) {
        let margin = svg_canvas.margin as f64;
        let width = svg_canvas.width as f64;
        let height = svg_canvas.height as f64;
        let bg_color = self.rgb_to_svg_color(config.color_background);

        svg_canvas.draw_rect(
            margin,
            margin,
            width - 2.0 * margin,
            height - 2.0 * margin,
            &bg_color,
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
        for y in canvas.margin..canvas.height - canvas.margin {
            for x in canvas.margin..canvas.width - canvas.margin {
                canvas.draw_pixel(x, y, config.color_background);
            }
        }
    }

    /// Draws the grid for the plot based on the provided configuration.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the grid on.
    /// - `config`: The `FigureConfig` containing grid appearance settings.
    fn draw_grid(&self, canvas: &mut PixelCanvas, config: &FigureConfig) {
        canvas.draw_grid(
            &[config.num_grid_horizontal, config.num_grid_vertical],
            config.color_grid,
        );
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
        let font_path = config.font_label.as_ref().expect("Font path is not set");
        let font_bytes = std::fs::read(font_path).expect("Failed to read font file");
        let font = FontRef::try_from_slice(&font_bytes).unwrap();
        let scale = ab_glyph::PxScale {
            x: config.font_size_label,
            y: config.font_size_label,
        };

        let (w, h) = text_size(scale, &font, text);

        canvas.draw_text(
            x.saturating_sub(w / 2),
            y.saturating_sub(h / 2),
            text,
            config.color_axis,
            &font,
            scale,
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
        let font_path = config.font_title.as_ref().expect("Font path is not set");
        let font_bytes = std::fs::read(font_path).expect("Failed to read font file");
        let font = FontRef::try_from_slice(&font_bytes).unwrap();
        let scale = PxScale {
            x: config.font_size_title,
            y: config.font_size_title,
        };

        let (w, h) = text_size(scale, &font, text);

        canvas.draw_text(
            x.saturating_sub(w / 2),
            y.saturating_sub(h / 2),
            text,
            config.color_title,
            &font,
            scale,
        );
    }

    /// Draws a value on the axis (tick label) based on its type.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the axis value on.
    /// - `config`: The `FigureConfig` containing axis value appearance settings.
    /// - `x`, `y`: The position to draw the value.
    /// - `text`: The text of the axis value.
    /// - `axis`: The type of axis (`AxisType::AxisX` or `AxisType::AxisY`).
    fn draw_axis_value(
        &self,
        canvas: &mut PixelCanvas,
        config: &FigureConfig,
        x: u32,
        y: u32,
        text: &str,
        axis: AxisType,
    ) {
        let font_path = config.font_label.as_ref().expect("Font path is not set");
        let font_bytes = std::fs::read(font_path).expect("Failed to read font file");
        let font = FontRef::try_from_slice(&font_bytes).unwrap();
        let scale = ab_glyph::PxScale {
            x: config.font_size_axis,
            y: config.font_size_axis,
        };

        let (w, h) = text_size(scale, &font, text);
        let mut x = x;
        let mut y = y;
        match axis {
            AxisType::AxisX => {
                x = x.saturating_sub(w / 2);
                y = y.saturating_add(h);
            }
            AxisType::AxisY => {
                x = x.saturating_sub(w);
                y = y.saturating_sub(h / 2);
            }
        }

        canvas.draw_text(x, y, text, config.color_axis, &font, scale);
    }
}
