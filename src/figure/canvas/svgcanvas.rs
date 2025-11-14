use crate::figure::utilities::linetype::LineType;
use std::{
    fs::File,
    io::{self, Write},
};

/// A structure for creating and managing an SVG-based drawing canvas.
pub struct SvgCanvas {
    /// Width of the SVG canvas.
    pub width: u32,
    /// Height of the SVG canvas.
    pub height: u32,
    /// A vector storing SVG elements as strings.
    pub elements: Vec<String>,
    /// Margin size for the SVG canvas.
    pub margin: u32,
    /// Background color of the SVG canvas.
    pub background_color: String,
}

impl SvgCanvas {
    /// Creates a new `SvgCanvas` with the specified dimensions, background color, and margin.
    ///
    /// # Parameters
    /// - `width`: The width of the canvas in pixels.
    /// - `height`: The height of the canvas in pixels.
    /// - `background_color`: The background color as a string.
    /// - `margin`: Margin size in pixels.
    ///
    /// # Returns
    /// A new `SvgCanvas` instance.
    pub fn new(width: u32, height: u32, background_color: &str, margin: u32) -> Self {
        Self {
            width,
            height,
            elements: vec![format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}">"#,
                width, height
            )],
            margin,
            background_color: background_color.to_string(),
        }
    }

    /// Clears the SVG canvas by removing all elements and reinitializing.
    pub fn clear(&mut self) {
        // Clear all SVG elements
        self.elements.clear();
    }

    /// Adds a line to the SVG canvas.
    ///
    /// # Parameters
    /// - `x1`, `y1`: Coordinates of the start point.
    /// - `x2`, `y2`: Coordinates of the end point.
    /// - `color`: The stroke color of the line.
    /// - `stroke_width`: The width of the line stroke.
    pub fn draw_line(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: &str,
        stroke_width: f64,
    ) {
        self.elements.push(format!(
            r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{color}" stroke-width="{stroke_width:.2}"/>"#
        ));
    }

    /// Adds a line with RGB color to the SVG canvas.
    ///
    /// # Parameters
    /// - `x1`, `y1`: Coordinates of the start point.
    /// - `x2`, `y2`: Coordinates of the end point.
    /// - `color`: The RGB color of the line.
    /// - `stroke_width`: The width of the line stroke.
    pub fn draw_line_rgb(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: [u8; 3],
        stroke_width: f64,
    ) {
        self.elements.push(format!(
            r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="rgb({},{},{})" stroke-width="{:.2}"/>"#,
            x1, y1, x2, y2, color[0], color[1], color[2], stroke_width
        ));
    }

    /// Adds a line with RGB color and line style (solid, dashed, or dotted) to the SVG canvas.
    ///
    /// # Parameters
    /// - `x1`, `y1`: Coordinates of the start point.
    /// - `x2`, `y2`: Coordinates of the end point.
    /// - `color`: The RGB color of the line.
    /// - `stroke_width`: The width of the line stroke.
    /// - `line_type`: The line style (Solid, Dashed, or Dotted).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_line_rgb_styled(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: [u8; 3],
        stroke_width: f64,
        line_type: LineType,
    ) {
        let stroke_dasharray = match line_type {
            LineType::Solid => String::new(),
            LineType::Dashed(length) => format!("{length},{length}"),
            LineType::Dotted(spacing) => {
                // For dots: small dash (3) followed by gap (spacing)
                format!("3,{spacing}")
            }
        };

        if stroke_dasharray.is_empty() {
            self.elements.push(format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="rgb({},{},{})" stroke-width="{:.2}"/>"#,
                x1, y1, x2, y2, color[0], color[1], color[2], stroke_width
            ));
        } else {
            self.elements.push(format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="rgb({},{},{})" stroke-width="{:.2}" stroke-dasharray="{}"/>"#,
                x1, y1, x2, y2, color[0], color[1], color[2], stroke_width, stroke_dasharray
            ));
        }
    }

    /// Adds a rectangle to the SVG canvas.
    ///
    /// # Parameters
    /// - `x`, `y`: Coordinates of the top-left corner.
    /// - `width`, `height`: Dimensions of the rectangle.
    /// - `fill_color`: Fill color of the rectangle.
    /// - `stroke_color`: Stroke color of the rectangle.
    /// - `stroke_width`: Width of the rectangle's border.
    /// - `opacity`: Opacity of the rectangle fill (0.0 to 1.0).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_rect(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        fill_color: &str,
        stroke_color: &str,
        stroke_width: f64,
        opacity: f64,
    ) {
        self.elements.push(format!(
            r#"<rect x="{x:.2}" y="{y:.2}" width="{width:.2}" height="{height:.2}" fill="{fill_color}" stroke="{stroke_color}" stroke-width="{stroke_width:.2}" fill-opacity="{opacity}"/>"#
        ));
    }

    /// Adds a font style definition to the SVG canvas.
    ///
    /// # Parameters
    /// - `font_url`: URL of the font to be imported.
    /// - `class_name`: CSS class name for applying the font.
    /// - `font_family`: Name of the font family.
    pub fn add_font_style(&mut self, font_url: &str, class_name: &str, font_family: &str) {
        self.elements.push(format!(
            r#"<style>
                @import url('{font_url}');
                .{class_name} {{
                    font-family: '{font_family}', sans-serif;
                }}
            </style>"#
        ));
    }

    /// Draws a circle on the SVG canvas.
    ///
    /// # Parameters
    /// - `cx`, `cy`: Coordinates of the circle's center.
    /// - `r`: Radius of the circle.
    /// - `color`: Fill color of the circle.
    pub fn draw_circle(&mut self, cx: f64, cy: f64, r: f64, color: &str) {
        self.elements.push(format!(
            r#"<circle cx="{cx:.2}" cy="{cy:.2}" r="{r:.2}" fill="{color}"/>"#
        ));
    }

    /// Adds a text element to the SVG canvas.
    ///
    /// # Parameters
    /// - `x`, `y`: Coordinates of the text's position.
    /// - `text`: The text content.
    /// - `font_size`: Font size of the text.
    /// - `color`: Text color.
    pub fn draw_text(&mut self, x: f64, y: f64, text: &str, font_size: f64, color: &str) {
        self.elements.push(format!(
            r#"<text x="{x:.2}" y="{y:.2}" font-size="{font_size:.2}" text-anchor="middle" fill="{color}">{text}</text>"#
        ));
    }

    /// Adds a text element to the SVG canvas.
    ///
    /// # Parameters
    /// - `x`, `y`: Coordinates of the text's position.
    /// - `text`: The text content.
    /// - `font_size`: Font size of the text.
    /// - `color`: Text color.
    pub fn draw_title(&mut self, x: f64, y: f64, text: &str, font_size: f64, color: &str) {
        self.elements.push(format!(
            r#"<text x="{x:.2}" y="{y:.2}" font-size="{font_size:.2}" text-anchor="middle" fill="{color}">{text}</text>"#
        ));
    }

    /// Draws a grid on the SVG canvas.
    ///
    /// # Parameters
    /// - `x_min`, `x_max`: Horizontal range for the grid.
    /// - `y_min`, `y_max`: Vertical range for the grid.
    /// - `x_ticks`: Number of grid lines along the x-axis.
    /// - `y_ticks`: Number of grid lines along the y-axis.
    /// - `color`: Color of the grid lines.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_grid(
        &mut self,
        x_min: f64,
        x_max: f64,
        y_min: f64,
        y_max: f64,
        x_ticks: usize,
        y_ticks: usize,
        color: &str,
    ) {
        let x_step = (x_max - x_min) / x_ticks as f64;
        let y_step = (y_max - y_min) / y_ticks as f64;

        for i in 0..=x_ticks {
            let x = x_min + i as f64 * x_step;
            self.draw_line(x, y_min, x, y_max, color, 0.5);
        }

        for i in 0..=y_ticks {
            let y = y_min + i as f64 * y_step;
            self.draw_line(x_min, y, x_max, y, color, 0.5);
        }
    }

    /// Saves the SVG content to a file.
    ///
    /// # Parameters
    /// - `file_path`: The path to save the SVG file.
    ///
    /// # Errors
    /// Returns an `io::Result` if saving fails.
    pub fn save(&self, file_path: &str) -> io::Result<()> {
        let mut file = File::create(file_path)?;
        for element in &self.elements {
            writeln!(file, "{element}")?;
        }
        writeln!(file, "</svg>")?;
        Ok(())
    }

    /// Retrieves the SVG content as a single string.
    ///
    /// # Returns
    /// The complete SVG as a string.
    pub fn get_svg_as_text(&self) -> String {
        let mut svg = String::new();
        for element in &self.elements {
            svg.push_str(element);
        }
        svg.push_str("</svg>");
        svg
    }
}
