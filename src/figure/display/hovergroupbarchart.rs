use ab_glyph::FontRef;
use image::ImageBuffer;
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut, text_size};

use crate::figure::{canvas::pixelcanvas::PixelCanvas, figuretypes::groupbarchart::GroupBarChart};

use super::hover::Hover;

impl Hover for GroupBarChart {
    fn handle_hover(&self, mouse_x: u32, mouse_y: u32, canvas: &PixelCanvas) -> Option<Vec<u32>> {
        let scale_x = (canvas.width - 2 * canvas.margin) as f64 / self.datasets.len() as f64;

        let mut img =
            ImageBuffer::from_raw(canvas.width, canvas.height, canvas.buffer.clone()).unwrap();

        let mut closest_bar_group: Option<(f64, Vec<f64>)> = None;
        let mut min_distance = f64::MAX;

        // Find the closest bar group
        for (group_index, _) in self.datasets.iter().enumerate() {
            let group_center_x = canvas.margin as f64 + (group_index as f64 + 0.5) * scale_x;
            let distance = (mouse_x as f64 - group_center_x).abs();

            if distance < min_distance {
                min_distance = distance;

                // Collect values for this group
                let values = self
                    .datasets
                    .iter()
                    .filter_map(|dataset| dataset.data.get(group_index).map(|&(_, value)| value))
                    .collect::<Vec<f64>>();

                closest_bar_group = Some((group_center_x, values));
            }
        }

        if let Some((group_center_x, group_values)) = closest_bar_group {
            // Draw a red line from the center of the bar group to the cursor
            draw_line_segment_mut(
                &mut img,
                (
                    group_center_x as f32,
                    (canvas.height - canvas.margin) as f32,
                ),
                (mouse_x as f32, mouse_y as f32),
                image::Rgb([255, 0, 0]), // Red line
            );

            // Format the tooltip text
            let mut tooltip_text = String::from("X Value: ");

            // Calculate tooltip dimensions
            let font_path = self
                .config
                .font_label
                .as_ref()
                .expect("Font path is not set");
            let font_bytes = std::fs::read(font_path).expect("Failed to read font file");
            let font = FontRef::try_from_slice(&font_bytes).unwrap();
            let scale = ab_glyph::PxScale { x: 12.0, y: 12.0 };
            let text_size = text_size(scale, &font, &tooltip_text).0 as i32;

            let rect_x = mouse_x as i32 + 10;
            let rect_y = mouse_y as i32 - 30;
            let rect_width = text_size + 20;
            let rect_height = 25 * (group_values.len() as i32 + 1);

            // Ensure tooltip stays within bounds
            let rect_x = rect_x.max(0).min((canvas.width as i32 - rect_width) as i32);
            let mut rect_y = rect_y.max(0).min(canvas.height as i32 - rect_height);

            // Draw tooltip background
            for y in rect_y..(rect_y + rect_height) {
                for x in rect_x..(rect_x + rect_width) {
                    if x >= 0 && y >= 0 && (x as u32) < canvas.width && (y as u32) < canvas.height {
                        img.put_pixel(x as u32, y as u32, image::Rgb([255, 255, 255]));
                    }
                }
            }

            // Draw tooltip border
            for x in rect_x..(rect_x + rect_width) {
                img.put_pixel(x as u32, rect_y as u32, image::Rgb([0, 0, 0])); // Top edge
                img.put_pixel(
                    x as u32,
                    (rect_y + rect_height - 1) as u32,
                    image::Rgb([0, 0, 0]),
                ); // Bottom edge
            }
            for y in rect_y..(rect_y + rect_height) {
                img.put_pixel(rect_x as u32, y as u32, image::Rgb([0, 0, 0])); // Left edge
                img.put_pixel(
                    (rect_x + rect_width - 1) as u32,
                    y as u32,
                    image::Rgb([0, 0, 0]),
                ); // Right edge
            }

            for (dataset_index, value) in group_values.iter().enumerate() {
                if dataset_index == 0 {
                    tooltip_text.push_str(&format!("{value:.2}"));
                } else {
                    let dataset_label = &self.datasets[dataset_index].label;
                    tooltip_text = format!("{dataset_label} : {value:.2}");
                }

                // Draw the tooltip text
                draw_text_mut(
                    &mut img,
                    image::Rgb([0, 0, 0]), // Text color
                    rect_x + 10,
                    rect_y + 5,
                    scale,
                    &font,
                    &tooltip_text,
                );

                rect_y += 20;
            }

            Some(
                img.pixels()
                    .map(|pixel: &image::Rgb<u8>| {
                        let [r, g, b] = pixel.0;
                        (r as u32) << 16 | (g as u32) << 8 | b as u32
                    })
                    .collect(),
            )
        } else {
            None
        }
    }

    fn find_closest_point(
        &self,
        mouse_x: u32,
        mouse_y: u32,
        canvas: &PixelCanvas,
    ) -> Option<((f64, f64), f64)> {
        let scale_x = (canvas.width - 2 * canvas.margin) as f64 / self.datasets.len() as f64;
        let scale_y = (canvas.height - 2 * canvas.margin) as f64 / 10.0; // Assume a fixed y-range for now

        let mut closest_bar = None;
        let mut min_distance = f64::MAX;

        for dataset in &self.datasets {
            for &(x, y) in &dataset.data {
                let px = (x) * scale_x + canvas.margin as f64;
                let py = canvas.height as f64 - canvas.margin as f64 - y * scale_y;

                let distance =
                    ((mouse_x as f64 - px).powi(2) + (mouse_y as f64 - py).powi(2)).sqrt();
                if distance < min_distance {
                    min_distance = distance;
                    closest_bar = Some(((x, y), y));
                }
            }
        }

        closest_bar
    }

    fn to_canvas_coordinates(&self, x: f64, y: f64, canvas: &PixelCanvas) -> (u32, u32) {
        let scale_x = (canvas.width - 2 * canvas.margin) as f64 / self.datasets.len() as f64;
        let scale_y = (canvas.height - 2 * canvas.margin) as f64 / 10.0; // Assume a fixed y-range for now

        let px = ((x) * scale_x + canvas.margin as f64) as u32;
        let py = (canvas.height as f64 - canvas.margin as f64 - y * scale_y) as u32;

        (px, py)
    }

    fn get_font<'a>(&self, font_data: &'a [u8]) -> FontRef<'a> {
        FontRef::try_from_slice(font_data).unwrap()
    }
}
