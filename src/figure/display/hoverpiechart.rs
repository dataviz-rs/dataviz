use ab_glyph::FontRef;
use image::ImageBuffer;
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut, text_size};

use crate::figure::{canvas::pixelcanvas::PixelCanvas, figuretypes::piechart::PieChart};

use super::hover::Hover;

impl Hover for PieChart {
    fn find_closest_point(
        &self,
        mouse_x: u32,
        mouse_y: u32,
        canvas: &PixelCanvas,
    ) -> Option<((f64, f64), f64)> {
        let center_x = canvas.width as f64 / 2.0;
        let center_y = canvas.height as f64 / 2.0;
        let total_value: f64 = self.datasets.iter().map(|(_, value, _)| *value).sum();
        let mut start_angle = 0.0;

        let dx = mouse_x as f64 - center_x;
        let dy = mouse_y as f64 - center_y;
        let mut angle = dy.atan2(dx);
        if angle < 0.0 {
            angle += 2.0 * std::f64::consts::PI;
        }

        for (_, value, _) in &self.datasets {
            let sweep_angle = (value / total_value) * 2.0 * std::f64::consts::PI;
            let mid_angle = start_angle + sweep_angle / 2.0;

            if angle >= start_angle && angle < start_angle + sweep_angle {
                let x = center_x + mid_angle.cos() * (canvas.width as f64 / 4.0);
                let y = center_y + mid_angle.sin() * (canvas.height as f64 / 4.0);
                return Some(((x, y), *value));
            }
            start_angle += sweep_angle;
        }

        None
    }

    fn to_canvas_coordinates(&self, x: f64, y: f64, _canvas: &PixelCanvas) -> (u32, u32) {
        (x as u32, y as u32)
    }

    fn handle_hover(&self, mouse_x: u32, mouse_y: u32, canvas: &PixelCanvas) -> Option<Vec<u32>> {
        let mut img =
            ImageBuffer::from_raw(canvas.width, canvas.height, canvas.buffer.clone()).unwrap();

        if let Some(((x, y), value)) = self.find_closest_point(mouse_x, mouse_y, canvas) {
            // Draw the line from slice center to cursor
            draw_line_segment_mut(
                &mut img,
                (x as f32, y as f32),
                (mouse_x as f32, mouse_y as f32),
                image::Rgb([255, 0, 0]),
            );

            // Draw an edged rectangle for the hover information
            let font_path = self
                .config
                .font_label
                .as_ref()
                .expect("Font path is not set");
            let font_bytes = std::fs::read(font_path).expect("Failed to read font file");
            let font = FontRef::try_from_slice(&font_bytes).unwrap();

            let scale = ab_glyph::PxScale { x: 12.0, y: 12.0 };
            let coord_text = format!("{}: {:.2}", self.title, value);
            let text_size = text_size(scale, &font, &coord_text).0 as i32;

            let rect_x = mouse_x as i32 + 10;
            let rect_y = mouse_y as i32 - 30;
            let rect_width = text_size + 20;
            let rect_height = 25;

            let rect_x = rect_x.max(0).min((canvas.width as i32 - rect_width) as i32);
            let rect_y = rect_y.max(0).min(canvas.height as i32 - rect_height);

            for y in rect_y..(rect_y + rect_height) {
                for x in rect_x..(rect_x + rect_width) {
                    if x >= 0 && y >= 0 && (x as u32) < canvas.width && (y as u32) < canvas.height {
                        img.put_pixel(x as u32, y as u32, image::Rgb([255, 255, 255]));
                    }
                }
            }

            for x in rect_x..(rect_x + rect_width) {
                if rect_y >= 0 && (x as u32) < canvas.width {
                    img.put_pixel(x as u32, rect_y as u32, image::Rgb([0, 0, 0]));
                    img.put_pixel(
                        x as u32,
                        (rect_y + rect_height - 1) as u32,
                        image::Rgb([0, 0, 0]),
                    );
                }
            }

            for y in rect_y..(rect_y + rect_height) {
                if rect_x >= 0 && (y as u32) < canvas.height {
                    img.put_pixel(rect_x as u32, y as u32, image::Rgb([0, 0, 0]));
                    img.put_pixel(
                        (rect_x + rect_width - 1) as u32,
                        y as u32,
                        image::Rgb([0, 0, 0]),
                    );
                }
            }

            draw_text_mut(
                &mut img,
                image::Rgb([0, 0, 0]),
                rect_x + 10,
                rect_y + 5,
                scale,
                &font,
                &coord_text,
            );
        }

        Some(
            img.pixels()
                .map(|pixel: &image::Rgb<u8>| {
                    let [r, g, b] = pixel.0;
                    (r as u32) << 16 | (g as u32) << 8 | b as u32
                })
                .collect(),
        )
    }

    fn get_font<'a>(&self, font_data: &'a [u8]) -> FontRef<'a> {
        println!("font_data: {font_data:?}");
        FontRef::try_from_slice(font_data).unwrap()
    }
}
