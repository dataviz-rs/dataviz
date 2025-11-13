use ab_glyph::FontRef;
use image::ImageBuffer;
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut, text_size};

use crate::figure::{canvas::pixelcanvas::PixelCanvas, figuretypes::histogram::Histogram};

use super::hover::Hover;

impl Hover for Histogram {
    fn find_closest_point(
        &self,
        mouse_x: u32,
        _mouse_y: u32,
        canvas: &PixelCanvas,
    ) -> Option<((f64, f64), f64)> {
        let bin_data = self.calculate_bins();

        let bin_width = (bin_data[1].0 - bin_data[0].0).abs();
        let scale_x = (canvas.width - 2 * canvas.margin) as f64 / self.bins as f64;

        let mut closest_bin = None;
        let mut min_distance = f64::MAX;

        for (i, &(bin_start, freq)) in bin_data.iter().enumerate() {
            let bin_end = bin_start + bin_width; // End of the bin
            let bin_x = canvas.margin as f64 + i as f64 * scale_x;

            let distance = (mouse_x as f64 - bin_x).abs(); // Distance to mouse x
            if distance < min_distance {
                min_distance = distance;
                closest_bin = Some(((bin_start, bin_end), freq)); // Bin range and frequency
            }
        }

        closest_bin
    }

    fn to_canvas_coordinates(&self, x: f64, y: f64, canvas: &PixelCanvas) -> (u32, u32) {
        let bin_data = self.calculate_bins();
        let bin_width = (bin_data[1].0 - bin_data[0].0).abs();
        let x_min = bin_data[0].0; // Start of the first bin
        let x_max = x_min + bin_width * self.bins as f64;

        let scale_x = (canvas.width - 2 * canvas.margin) as f64 / (x_max - x_min);
        let scale_y = (canvas.height - 2 * canvas.margin) as f64
            / bin_data.iter().map(|(_, f)| *f).fold(0.0, f64::max);

        let px = ((x - x_min) * scale_x + canvas.margin as f64) as u32;
        let py = (canvas.height as f64 - canvas.margin as f64 - y * scale_y) as u32;

        (px, py)
    }

    /// Handle hover functionality and return updated buffer if applicable
    fn handle_hover(&self, mouse_x: u32, mouse_y: u32, canvas: &PixelCanvas) -> Option<Vec<u32>> {
        // Find the closest bin's range and total value
        if let Some(((bin_start, bin_end), freq)) =
            self.find_closest_point(mouse_x, mouse_y, canvas)
        {
            let mut img =
                ImageBuffer::from_raw(canvas.width, canvas.height, canvas.buffer.clone()).unwrap();

            // Format the text as: ([x-start, x-end], total: y-value)
            let bin_info = format!("([{bin_start:.2}, {bin_end:.2}], total: {freq:.2})");

            // Calculate text size for background rectangle
            let font_path = self
                .config
                .font_label
                .as_ref()
                .expect("Font path is not set");
            let font_bytes = std::fs::read(font_path).expect("Failed to read font file");
            let font = FontRef::try_from_slice(&font_bytes).unwrap();
            let scale = ab_glyph::PxScale { x: 12.0, y: 12.0 };
            let text_size = text_size(scale, &font, &bin_info).0 as i32;

            let rect_x = mouse_x as i32 + 15; // Rectangle start X, slightly offset from mouse
            let rect_y = mouse_y as i32 - 20; // Rectangle start Y
            let rect_width = text_size + 20; // Add padding for rectangle width
            let rect_height = 25; // Height of the rectangle

            // Adjust rectangle coordinates to stay within canvas bounds
            let rect_x = rect_x.max(0).min((canvas.width as i32 - rect_width) as i32);
            let rect_y = rect_y.max(0).min(canvas.height as i32 - rect_height);

            // Draw white rectangle as background
            for y in rect_y..(rect_y + rect_height) {
                for x in rect_x..(rect_x + rect_width) {
                    if x >= 0 && y >= 0 && (x as u32) < canvas.width && (y as u32) < canvas.height {
                        img.put_pixel(x as u32, y as u32, image::Rgb([255, 255, 255]));
                        // White
                    }
                }
            }

            // Draw edges for the rectangle
            for x in rect_x..(rect_x + rect_width) {
                if rect_y >= 0 && (x as u32) < canvas.width {
                    img.put_pixel(x as u32, rect_y as u32, image::Rgb([0, 0, 0])); // Top edge
                    img.put_pixel(
                        x as u32,
                        (rect_y + rect_height - 1) as u32,
                        image::Rgb([0, 0, 0]),
                    ); // Bottom edge
                }
            }
            for y in rect_y..(rect_y + rect_height) {
                if rect_x >= 0 && (y as u32) < canvas.height {
                    img.put_pixel(rect_x as u32, y as u32, image::Rgb([0, 0, 0])); // Left edge
                    img.put_pixel(
                        (rect_x + rect_width - 1) as u32,
                        y as u32,
                        image::Rgb([0, 0, 0]),
                    ); // Right edge
                }
            }

            // Draw the formatted text on top of the rectangle
            draw_text_mut(
                &mut img,
                image::Rgb([0, 0, 0]), // Text color
                rect_x + 10,           // Add padding to position text
                rect_y + 5,
                scale,
                &font,
                &bin_info,
            );

            // Draw a line from the bin's top point to the mouse location
            if let Some((bin_px, bin_py)) = self
                .to_canvas_coordinates((bin_start + bin_end) / 2.0, freq, canvas)
                .into()
            {
                draw_line_segment_mut(
                    &mut img,
                    (bin_px as f32, bin_py as f32),
                    (mouse_x as f32, mouse_y as f32),
                    image::Rgb([255, 0, 0]), // Line color
                );
            }

            return Some(
                img.pixels()
                    .map(|pixel: &image::Rgb<u8>| {
                        let [r, g, b] = pixel.0;
                        (r as u32) << 16 | (g as u32) << 8 | b as u32
                    })
                    .collect(),
            );
        }
        None
    }

    fn get_font<'a>(&self, font_data: &'a [u8]) -> FontRef<'a> {
        FontRef::try_from_slice(font_data).unwrap()
    }
}
