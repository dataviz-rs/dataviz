use ab_glyph::FontRef;
use image::ImageBuffer;
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut, text_size};

use crate::figure::{
    canvas::pixelcanvas::PixelCanvas, figuretypes::quadrant1graph::Quadrant1Graph,
};

use super::hover::Hover;

impl Hover for Quadrant1Graph {
    fn find_closest_point(
        &self,
        mouse_x: u32,
        mouse_y: u32,
        canvas: &PixelCanvas,
    ) -> Option<((f64, f64), f64)> {
        // Calculate dataset limits
        let (x_min, x_max) = self
            .datasets
            .iter()
            .flat_map(|dataset| dataset.points.iter().map(|&(x, _)| x))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), x| {
                (min.min(x), max.max(x))
            });

        let (y_min, y_max) = self
            .datasets
            .iter()
            .flat_map(|dataset| dataset.points.iter().map(|&(_, y)| y))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), y| {
                (min.min(y), max.max(y))
            });

        // Adjust limits to include (0, 0)
        let x_min = x_min.min(0.0);
        let y_min = y_min.min(0.0);

        // Calculate scales
        let scale_x = (canvas.width - 2 * canvas.margin) as f64 / (x_max - x_min);
        let scale_y = (canvas.height - 2 * canvas.margin) as f64 / (y_max - y_min);

        self.datasets
            .iter()
            .flat_map(|dataset| {
                dataset.points.iter().map(|&(x, y)| {
                    let px = canvas.margin as f64 + (x - x_min) * scale_x;
                    let py = canvas.height as f64 - canvas.margin as f64 - (y - y_min) * scale_y;
                    let dist =
                        ((mouse_x as f64 - px).powi(2) + (mouse_y as f64 - py).powi(2)).sqrt();
                    ((x, y), dist)
                })
            })
            .min_by(|&(_, d1), &(_, d2)| d1.partial_cmp(&d2).unwrap())
    }

    fn to_canvas_coordinates(&self, x: f64, y: f64, canvas: &PixelCanvas) -> (u32, u32) {
        // todo!(); add max min values to scatter graph
        // Calculate dataset limits
        let (x_min, x_max) = self
            .datasets
            .iter()
            .flat_map(|dataset| dataset.points.iter().map(|&(x, _)| x))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), x| {
                (min.min(x), max.max(x))
            });

        let (y_min, y_max) = self
            .datasets
            .iter()
            .flat_map(|dataset| dataset.points.iter().map(|&(_, y)| y))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), y| {
                (min.min(y), max.max(y))
            });

        // Adjust limits to include (0, 0)
        let x_min = x_min.min(0.0);
        let y_min = y_min.min(0.0);

        // Calculate scales
        let scale_x = (canvas.width - 2 * canvas.margin) as f64 / (x_max - x_min);
        let scale_y = (canvas.height - 2 * canvas.margin) as f64 / (y_max - y_min);

        let px = ((x - x_min) * scale_x + canvas.margin as f64) as u32;
        let py = (canvas.height as f64 - canvas.margin as f64 - (y - y_min) * scale_y) as u32;

        (px, py)
    }

    fn handle_hover(&self, mouse_x: u32, mouse_y: u32, canvas: &PixelCanvas) -> Option<Vec<u32>> {
        let mut img =
            ImageBuffer::from_raw(canvas.width, canvas.height, canvas.buffer.clone()).unwrap();

        if let Some(((x, y), _)) = self.find_closest_point(mouse_x, mouse_y, canvas) {
            let (px, py) = self.to_canvas_coordinates(x, y, canvas);

            // Draw the line from point to cursor
            draw_line_segment_mut(
                &mut img,
                (px as f32, py as f32),
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
            let coord_text = format!("({x:.2}, {y:.2})");
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
        FontRef::try_from_slice(font_data).unwrap()
    }
}
