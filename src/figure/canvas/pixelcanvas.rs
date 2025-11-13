use ab_glyph::{FontRef, PxScale};
use image::Rgb;
use imageproc::drawing::{draw_text_mut, text_size};

use crate::figure::utilities::linetype::LineType;

/// A structure representing a pixel-based drawing canvas.
pub struct PixelCanvas {
    /// Width of the canvas in pixels.
    pub width: u32,
    /// Height of the canvas in pixels.
    pub height: u32,
    /// Background color of the canvas (RGB format).
    pub background_color: [u8; 3],
    /// Buffer storing pixel data as a linear array.
    pub buffer: Vec<u8>,
    /// Margin around the canvas (in pixels).
    pub margin: u32,
}

impl PixelCanvas {
    /// Creates a new `PixelCanvas` with the specified dimensions, background color, and margin.
    ///
    /// # Parameters
    /// - `width`: The width of the canvas in pixels.
    /// - `height`: The height of the canvas in pixels.
    /// - `background_color`: The RGB color of the canvas background.
    /// - `margin`: Margin size in pixels.
    ///
    /// # Returns
    /// A new `PixelCanvas` instance.
    pub fn new(width: u32, height: u32, background_color: [u8; 3], margin: u32) -> Self {
        let buffer = vec![0; (width * height * 3) as usize];
        Self {
            width,
            height,
            background_color,
            buffer,
            margin,
        }
    }

    /// Clears the canvas by filling it with the background color.
    pub fn clear(&mut self) {
        for i in (0..self.buffer.len()).step_by(3) {
            self.buffer[i] = self.background_color[0];
            self.buffer[i + 1] = self.background_color[1];
            self.buffer[i + 2] = self.background_color[2];
        }
    }

    /// Draws a single pixel at the specified coordinates with the given color.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate of the pixel.
    /// - `y`: The y-coordinate of the pixel.
    /// - `color`: The RGB color of the pixel.
    pub fn draw_pixel(&mut self, x: u32, y: u32, color: [u8; 3]) {
        let index = ((y * self.width + x) * 3) as usize;
        if index + 2 < self.buffer.len() {
            self.buffer[index] = color[0];
            self.buffer[index + 1] = color[1];
            self.buffer[index + 2] = color[2];
        }
    }

    /// Blends a pixel with the specified color and alpha value.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate of the pixel.
    /// - `y`: The y-coordinate of the pixel.
    /// - `color`: The RGB color to blend.
    /// - `alpha`: The transparency value (0.0 to 1.0).
    pub fn blend_pixel(&mut self, x: u32, y: u32, color: [u8; 3], alpha: f64) {
        let index = ((y * self.width + x) * 3) as usize;
        if index + 2 < self.buffer.len() {
            let existing_color = [
                self.buffer[index],
                self.buffer[index + 1],
                self.buffer[index + 2],
            ];

            let blended_color = [
                (color[0] as f64 * alpha + existing_color[0] as f64 * (1.0 - alpha)) as u8,
                (color[1] as f64 * alpha + existing_color[1] as f64 * (1.0 - alpha)) as u8,
                (color[2] as f64 * alpha + existing_color[2] as f64 * (1.0 - alpha)) as u8,
            ];

            self.buffer[index] = blended_color[0];
            self.buffer[index + 1] = blended_color[1];
            self.buffer[index + 2] = blended_color[2];
        }
    }

    /// Draws a horizontal line at the specified y-coordinate.
    ///
    /// # Parameters
    /// - `y`: The y-coordinate of the line.
    /// - `color`: The RGB color of the line.
    pub fn draw_horizontal_line(&mut self, y: u32, color: [u8; 3]) {
        for x in self.margin..self.width - self.margin {
            self.draw_pixel(x, y, color);
        }
    }

    /// Draws a vertical line at the specified x-coordinate.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate of the line.
    /// - `color`: The RGB color of the line.
    pub fn draw_vertical_line(&mut self, x: u32, color: [u8; 3]) {
        for y in self.margin..self.height - self.margin {
            self.draw_pixel(x, y, color);
        }
    }

    /// Draws a grid on the canvas.
    ///
    /// # Parameters
    /// - `grid_size`: An array specifying the spacing of grid lines in the x and y directions.
    /// - `color`: The RGB color of the grid lines.
    pub fn draw_grid(&mut self, grid_size: &[usize; 2], color: [u8; 3]) {
        for x in (self.margin..=self.width - self.margin).step_by(grid_size[0]) {
            self.draw_vertical_line(x, color);
        }
        for y in (self.margin..=self.height - self.margin).step_by(grid_size[1]) {
            self.draw_horizontal_line(y, color);
        }
    }

    /// Draws text vertically at the specified position.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate for the text.
    /// - `y`: The y-coordinate for the text.
    /// - `text`: The text content.
    /// - `color`: The RGB color of the text.
    /// - `font`: The font reference for rendering text.
    /// - `scale`: The scaling factor for the font size.
    pub fn draw_text_vertical(
        &mut self,
        x: u32,
        y: u32,
        text: &str,
        color: [u8; 3],
        font: &ab_glyph::FontRef,
        scale: ab_glyph::PxScale,
    ) {
        let mut current_y = y;

        // Draw each character vertically
        for ch in text.chars() {
            let char_as_str = ch.to_string();
            let (_char_width, char_height) = text_size(scale, font, &char_as_str);

            // Draw the character
            self.draw_text(x, current_y, &char_as_str, color, font, scale);

            // Move down for the next character
            current_y += char_height + 5; // Adjust spacing between characters
        }
    }

    /// Draws text at the specified position.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate for the text.
    /// - `y`: The y-coordinate for the text.
    /// - `text`: The text content.
    /// - `color`: The RGB color of the text.
    /// - `font`: The font reference for rendering text.
    /// - `scale`: The scaling factor for the font size.
    pub fn draw_text(
        &mut self,
        x: u32,
        y: u32,
        text: &str,
        color: [u8; 3],
        font: &FontRef,
        scale: PxScale,
    ) {
        let img: &mut [u8] = &mut self.buffer;
        let mut buffer =
            image::ImageBuffer::from_raw(self.width, self.height, img.to_vec()).unwrap();
        draw_text_mut(
            &mut buffer,
            Rgb(color),
            x as i32,
            y as i32,
            scale,
            &font,
            text,
        );

        self.buffer = buffer.into_raw();
    }

    /// Draws a line with the specified type (solid, dashed, or dotted).
    ///
    /// # Parameters
    /// - `x1`, `y1`: Coordinates of the start point.
    /// - `x2`, `y2`: Coordinates of the end point.
    /// - `color`: The RGB color of the line.
    /// - `line_type`: The type of line to draw (`LineType`).
    pub fn draw_line(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: [u8; 3],
        line_type: LineType,
    ) {
        let dx = (x2 - x1).abs();
        let dy = -(y2 - y1).abs();
        let sx = if x1 < x2 { 1 } else { -1 };
        let sy = if y1 < y2 { 1 } else { -1 };
        let mut err = dx + dy;

        let mut x = x1;
        let mut y = y1;

        match line_type {
            LineType::Solid => {
                // Draw a continuous line without any gaps
                while x != x2 || y != y2 {
                    self.draw_pixel(x as u32, y as u32, color);

                    let e2 = 2 * err;
                    if e2 >= dy {
                        err += dy;
                        x += sx;
                    }
                    if e2 <= dx {
                        err += dx;
                        y += sy;
                    }
                }
                // Draw the final pixel
                self.draw_pixel(x2 as u32, y2 as u32, color);
            }
            LineType::Dashed(dash_length) => {
                let mut is_drawing = true;
                let mut segment_length = 0;

                while x != x2 || y != y2 {
                    if is_drawing {
                        self.draw_pixel(x as u32, y as u32, color);
                    }

                    segment_length += 1;
                    if segment_length == dash_length {
                        is_drawing = !is_drawing; // Toggle drawing
                        segment_length = 0; // Reset segment length
                    }

                    let e2 = 2 * err;
                    if e2 >= dy {
                        err += dy;
                        x += sx;
                    }
                    if e2 <= dx {
                        err += dx;
                        y += sy;
                    }
                }
                // Ensure the final pixel is drawn in drawing mode
                if is_drawing {
                    self.draw_pixel(x2 as u32, y2 as u32, color);
                }
            }
            LineType::Dotted(dot_spacing) => {
                let mut is_drawing = true;
                let mut segment_length = 0;
                // For dotted lines: always use small dots (2-3 pixels) with variable spacing
                let dot_length = 3;

                while x != x2 || y != y2 {
                    if is_drawing {
                        self.draw_pixel(x as u32, y as u32, color);
                    }

                    segment_length += 1;
                    let threshold = if is_drawing { dot_length } else { dot_spacing };
                    if segment_length == threshold {
                        is_drawing = !is_drawing; // Toggle drawing
                        segment_length = 0; // Reset segment length
                    }

                    let e2 = 2 * err;
                    if e2 >= dy {
                        err += dy;
                        x += sx;
                    }
                    if e2 <= dx {
                        err += dx;
                        y += sy;
                    }
                }
                // Ensure the final pixel is drawn in drawing mode
                if is_drawing {
                    self.draw_pixel(x2 as u32, y2 as u32, color);
                }
            }
        }
    }

    /// Saves the current canvas as an image file.
    ///
    /// # Parameters
    /// - `file_path`: The path to save the image file.
    ///
    /// # Panics
    /// Panics if the image cannot be saved.
    pub fn save_as_image(&self, file_path: &str) {
        use image::{ImageBuffer, RgbImage};

        let img: RgbImage = ImageBuffer::from_raw(self.width, self.height, self.buffer.clone())
            .expect("Failed to create image buffer");
        img.save(file_path).expect("Failed to save image");
    }
}
