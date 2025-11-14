use ab_glyph::{FontRef, PxScale};
use imageproc::drawing::text_size;
use std::f64::consts::PI;

use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    figuretypes::piechart::PieChart,
};

use super::drawer::Drawer;
use std::any::Any;
impl Drawer for PieChart {
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas) {
        let width = svg_canvas.width as f64;
        let height = svg_canvas.height as f64;
        let margin = svg_canvas.margin as f64;
        let font_size = 12.0;
        let cfg = &self.config;
        let margin_bg_color = svg_canvas.background_color.clone();

        // Draw margin background (using SvgCanvas background_color parameter)
        svg_canvas.draw_rect(0.0, 0.0, width, height, &margin_bg_color, "black", 1.0, 1.0);
        // Draw chart background (using FigureConfig color)
        self.fill_svg_background(svg_canvas, cfg);

        // Draw Title
        svg_canvas.draw_title(
            width / 2.0,
            margin / 2.0,
            &self.title,
            font_size * 2.0,
            "black",
        );

        // Calculate total value of all slices
        let total: f64 = self.datasets.iter().map(|dataset| dataset.1).sum();

        // Calculate center and radius
        let cx = width / 2.0;
        let cy = height / 2.0;
        let radius = (width.min(height) - 2.0 * margin) / 2.0;

        // Begin group for pie chart with transformation
        svg_canvas.elements.push(format!(
            r#"<g transform="translate({cx:.2},{cy:.2})" stroke="black" stroke-width="1">"#
        ));

        // Track the starting angle in radians
        let mut start_angle = 0.0;

        // Draw pie slices
        for dataset in &self.datasets {
            let value_ratio = dataset.1 / total; // Ratio of this slice to the total
            let sweep_angle = value_ratio * 2.0 * std::f64::consts::PI; // Convert ratio to radians
            let end_angle = start_angle + sweep_angle;

            // Calculate start and end points of the slice
            let x1 = radius * start_angle.cos();
            let y1 = radius * start_angle.sin();
            let x2 = radius * end_angle.cos();
            let y2 = radius * end_angle.sin();

            // Determine if the slice is larger than 180 degrees
            let large_arc_flag = if sweep_angle > std::f64::consts::PI {
                1
            } else {
                0
            };

            // Generate the path for the slice
            svg_canvas.elements.push(format!(
               r#"<path d="M 0 0 L {:.2} {:.2} A {:.2} {:.2} 0 {} 1 {:.2} {:.2} Z" fill="rgb({},{},{})"/>"#,
               x1, y1, radius, radius, large_arc_flag, x2, y2,
               dataset.2[0], dataset.2[1], dataset.2[2]
           ));

            // Calculate label position (midpoint of the slice angle)
            let mid_angle = start_angle + sweep_angle / 2.0;
            let label_x = (radius * 0.6) * mid_angle.cos(); // 60% of radius for better placement
            let label_y = (radius * 0.6) * mid_angle.sin();

            // Draw percentage label
            svg_canvas.elements.push(format!(
               r#"<text x="{:.2}" y="{:.2}" font-size="{:.2}" fill="black" text-anchor="middle" alignment-baseline="middle">{:.1}%</text>"#,
               label_x, label_y, font_size, value_ratio * 100.0
           ));

            // Update start angle for the next slice
            start_angle = end_angle;
        }

        // Close group
        svg_canvas.elements.push("</g>".to_string());

        // Draw legend in the bottom-left corner
        let legend_x_start = margin + 10.0; // Start inside chart area with margin spacing
        let legend_y = height - margin + font_size * 1.5 + 10.0; // Position below x-axis labels

        let mut legend_x = legend_x_start;
        let mut elements = String::new();
        let legend_bg_color = svg_canvas.background_color.clone();

        for dataset in &self.datasets {
            elements.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="rgb({},{},{})"/>"#,
                legend_x, legend_y, font_size, font_size, dataset.2[0], dataset.2[1], dataset.2[2]
            ));

            elements.push_str(&format!(
                r#"<text x="{:.2}" y="{:.2}" font-size="{:.2}" fill="black">{}</text>"#,
                legend_x + font_size * 1.3,
                legend_y + font_size - 2.0,
                font_size,
                dataset.0
            ));

            legend_x += font_size * 5.0 + dataset.0.len() as f64 * font_size * 0.6;
        }

        svg_canvas.draw_rect(
            legend_x_start - 5.0,
            legend_y - 5.0,
            legend_x - legend_x_start + 5.0,
            font_size + 10.0,
            &legend_bg_color,
            "black",
            0.5,
            0.5,
        );

        svg_canvas.elements.push(elements);
    }

    fn draw(&mut self, canvas: &mut PixelCanvas) {
        canvas.clear();

        let cfg = &self.config;
        self.fill_background(canvas, cfg);

        let margin = canvas.margin;
        let width = canvas.width;
        let height = canvas.height;

        // Draw the title
        self.draw_title(canvas, cfg, width / 2, margin / 2, &self.title);

        // Calculate total value
        let total: f64 = self.datasets.iter().map(|(_, value, _)| value).sum();
        if total == 0.0 {
            return;
        }

        // Center and radius of the pie chart
        let center_x = width / 2;
        let center_y = height / 2;
        let radius = (width.min(height) / 2 - margin) as i32;

        let mut start_angle = 0.0;
        for (_label, value, color) in &self.datasets {
            let percentage = value / total;
            let sweep_angle = 2.0 * PI * percentage;

            // Draw the slice
            self.draw_slice(
                canvas,
                center_x as i32,
                center_y as i32,
                radius,
                start_angle,
                start_angle + sweep_angle,
                *color,
            );

            // Calculate mid-angle for label placement
            let mid_angle = start_angle + sweep_angle / 2.0;
            let label_x = center_x as f64 + (radius as f64 * 0.6 * mid_angle.cos());
            let label_y = center_y as f64 - (radius as f64 * 0.6 * mid_angle.sin());
            self.draw_label(
                canvas,
                cfg,
                label_x as u32,
                label_y as u32,
                &format!("{:.1}%", percentage * 100.0),
            );

            start_angle += sweep_angle;
        }

        // Draw legend
        self.draw_legend(canvas);
    }

    fn draw_legend(&self, canvas: &mut PixelCanvas) {
        let font_path = self
            .config
            .font_label
            .as_ref()
            .expect("Font path is not set");
        let font_bytes = std::fs::read(font_path).expect("Failed to read font file");
        let font = FontRef::try_from_slice(&font_bytes).unwrap();
        let scale = PxScale { x: 10.0, y: 10.0 }; // Font size

        let square_size = 10; // Size of the colored square
        let padding = 5; // Space between the square and text
        let line_height = 20; // Vertical space for each legend entry
        let legend_margin = canvas.margin; // Margin from the bottom of the canvas

        let mut x = canvas.margin;
        let mut y = canvas.height - legend_margin; // Legend starts from the bottom

        for dataset in &self.datasets {
            let (w, h) = text_size(scale, &font, &dataset.0);
            // Draw the square
            for dy in 0..square_size {
                for dx in 0..square_size {
                    canvas.draw_pixel(
                        x + dx,
                        y + square_size * 2 + dy + h, // Adjust to align above baseline
                        dataset.2,
                    );
                }
            }

            // Draw the label text next to the square
            let text_x: u32 = x + square_size + padding;
            canvas.draw_text(
                text_x,
                y + 2 * square_size + h,
                &dataset.0,
                dataset.2,
                &font,
                scale,
            );

            // Move to the next legend entry
            x += square_size + padding + w + padding;
            if x > canvas.width - canvas.margin {
                // If the width exceeds, wrap to the next row
                x = canvas.margin;
                y -= line_height;
            }
        }
    }

    fn as_any(&mut self) -> &mut (dyn Any + 'static) {
        self as &mut dyn Any
    }

    fn get_figure_config(&self) -> &FigureConfig {
        &self.config
    }
}
