use ab_glyph::{FontRef, PxScale};
use imageproc::drawing::text_size;

use super::drawer::Drawer;
use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    figuretypes::areachart::AreaChart,
    utilities::axistype::AxisType,
};
use std::any::Any;

impl Drawer for AreaChart {
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas) {
        let width = svg_canvas.width as f64;
        let height = svg_canvas.height as f64;
        let margin = svg_canvas.margin as f64;
        let font_size = 12.0;
        let cfg = &self.config;

        // Draw margin background (using SvgCanvas background_color parameter)
        let margin_bg_color = svg_canvas.background_color.clone();
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

        // Determine dataset range
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

        let scale_x = (width - 2.0 * margin) / (x_max - x_min);
        let scale_y = (height - 2.0 * margin) / (y_max - y_min);

        // Draw grid
        let num_ticks = 10;
        svg_canvas.draw_grid(
            margin,
            width - margin,
            margin,
            height - margin,
            num_ticks,
            num_ticks,
            "lightgray",
        );

        // Draw axes
        let origin_x = margin + (0.0 - x_min) * scale_x;
        let origin_y = height - margin - (0.0 - y_min) * scale_y;

        svg_canvas.draw_line(margin, origin_y, width - margin, origin_y, "black", 2.0); // X-axis
        svg_canvas.draw_line(origin_x, margin, origin_x, height - margin, "black", 2.0); // Y-axis

        // X-axis
        let mut x_axis_ticks = String::new();
        for i in 0..=num_ticks {
            let value = x_min + i as f64 * (x_max - x_min) / num_ticks as f64;
            let x = margin + i as f64 * (width - 2.0 * margin) / num_ticks as f64;
            let tick_start_y = origin_y - 5.0;
            let tick_end_y = origin_y + 5.0;

            x_axis_ticks.push_str(&format!(
                "M {x:.2},{tick_start_y:.2} L {x:.2},{tick_end_y:.2} "
            ));

            // Draw value as text (fallback to basic SVG <text>)
            svg_canvas.elements.push(format!(
            r#"<text x="{:.2}" y="{:.2}" font-size="{:.2}" text-anchor="middle" fill="black">{:.1}</text>"#,
            x, height - margin + font_size * 1.5, font_size, value));
        }
        svg_canvas.elements.push(format!(
            r#"<path d="{x_axis_ticks}" stroke="black" stroke-width="1" fill="none"/>"#
        ));

        // Y-axis
        let mut y_axis_ticks = String::new();
        for i in 0..=num_ticks {
            let value = y_min + i as f64 * (y_max - y_min) / num_ticks as f64;
            let y = height - margin - i as f64 * (height - 2.0 * margin) / num_ticks as f64;
            let tick_start_x = origin_x - 5.0;
            let tick_end_x = origin_x + 5.0;

            y_axis_ticks.push_str(&format!(
                "M {tick_start_x:.2},{y:.2} L {tick_end_x:.2},{y:.2} "
            ));

            // Draw value as text (fallback to basic SVG <text>)
            svg_canvas.elements.push(format!(
            r#"<text x="{:.2}" y="{:.2}" font-size="{:.2}" text-anchor="end" fill="black">{:.1}</text>"#,
            margin - 5.0, y + font_size * 0.3, font_size, value
        ));
        }
        svg_canvas.elements.push(format!(
            r#"<path d="{y_axis_ticks}" stroke="black" stroke-width="1" fill="none"/>"#
        ));

        svg_canvas.draw_text(
            width - margin,
            height - margin / 2.0,
            &self.x_label,
            font_size * 1.5,
            "black",
        );

        // Draw Y-axis label (rotated)
        svg_canvas.elements.push(format!(
            r#"<text x="{:.2}" y="{:.2}" font-size="{:.2}" text-anchor="middle" fill="black" transform="rotate(-90 {:.2} {:.2})">{}</text>"#,
            margin / 3.0,
            height / 2.0,
            font_size * 1.5,
            margin / 3.0,
            height / 2.0,
            self.y_label
        ));

        // Draw areas under the datasets
        for dataset in &self.datasets {
            let mut path_data = String::new();
            let mut first_point = true;

            // Move to the initial point
            for &(x, y) in &dataset.points {
                let svg_x = margin + (x - x_min) * scale_x;
                let svg_y = height - margin - (y - y_min) * scale_y;

                if first_point {
                    path_data.push_str(&format!("M {svg_x:.2},{origin_y:.2} "));
                    first_point = false;
                }

                path_data.push_str(&format!("L {svg_x:.2},{svg_y:.2} "));
            }

            // Close the path back to the x-axis
            if let Some(&(last_x, _)) = dataset.points.last() {
                let svg_x = margin + (last_x - x_min) * scale_x;
                path_data.push_str(&format!("L {svg_x:.2},{origin_y:.2} Z"));
            }

            svg_canvas.elements.push(format!(
                r#"<path d="{}" fill="rgba({}, {}, {}, 0.5)" stroke="rgb({}, {}, {})" stroke-width="1"/>"#,
                path_data,
                dataset.color[0],
                dataset.color[1],
                dataset.color[2],
                dataset.color[0],
                dataset.color[1],
                dataset.color[2],
            ));
        }

        // Draw legend
        let legend_x_start = margin + 10.0; // Start inside chart area with margin spacing
        let legend_y = height - margin + font_size * 1.5 + 10.0; // Position below x-axis labels
        let mut legend_x = legend_x_start; // Reset starting position for legend items
        let mut elements = String::new();
        let legend_bg_color = svg_canvas.background_color.clone();

        for dataset in &self.datasets {
            // Draw color square
            elements.push_str(&format!(
                r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="rgb({},{},{})"/>"#,
                legend_x,
                legend_y,
                font_size,
                font_size,
                dataset.color[0],
                dataset.color[1],
                dataset.color[2]
            ));

            // Draw label text next to the color square
            elements.push_str(&format!(
                r#"<text x="{:.2}" y="{:.2}" font-size="{:.2}" fill="rgb({},{},{})">{}</text>"#,
                legend_x + font_size * 1.3,
                legend_y + font_size - 2.0,
                font_size,
                dataset.color[0],
                dataset.color[1],
                dataset.color[2],
                dataset.label
            ));

            // Update legend_x to position the next item
            legend_x += font_size * 5.0 + dataset.label.len() as f64 * font_size * 0.6;
        }

        // Draw a background rectangle for the legend
        let legend_width = legend_x - legend_x_start + 5.0;
        let legend_height = font_size + 10.0;
        svg_canvas.draw_rect(
            legend_x_start - 5.0,
            legend_y - 5.0,
            legend_width,
            legend_height,
            &legend_bg_color,
            "black",
            0.5,
            0.5,
        );

        // Add the legend elements to the canvas
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
        let scale_x = (width - 2 * margin) as f64 / (x_max - x_min);
        let scale_y = (height - 2 * margin) as f64 / (y_max - y_min);

        // Draw grids
        canvas.draw_grid(
            &[cfg.num_grid_horizontal, cfg.num_grid_vertical],
            cfg.color_grid,
        );

        // Draw axes
        let origin_x = canvas.margin + ((0.0 - x_min) * scale_x) as u32;
        let origin_y = height - margin - ((0.0 - y_min) * scale_y) as u32;

        self.draw_label(canvas, cfg, width - margin / 2, origin_y, &self.x_label);
        self.draw_label(canvas, cfg, margin, margin / 2, &self.y_label);

        // Draw axis tick values
        let num_ticks = cfg.num_axis_ticks;

        // X-axis ticks
        let x_tick_step = (x_max - x_min) / num_ticks as f64;
        for i in 0..=num_ticks {
            let value_x = x_min + i as f64 * x_tick_step;
            let tick_x = origin_x + ((value_x - x_min) * scale_x) as u32;

            let value_label = format!("{value_x:.2}");
            self.draw_axis_value(canvas, cfg, tick_x, origin_y, &value_label, AxisType::AxisX);
        }

        // Y-axis ticks
        let y_tick_step = (y_max - y_min) / num_ticks as f64;
        for i in 0..=num_ticks {
            let value_y = y_min + i as f64 * y_tick_step;
            let tick_y = origin_y - ((value_y - y_min) * scale_y) as u32;

            let value_label = format!("{value_y:.2}");
            self.draw_axis_value(
                canvas,
                cfg,
                origin_x - 10,
                tick_y,
                &value_label,
                AxisType::AxisY,
            );
        }

        // Draw areas under the curves
        for dataset in &self.datasets {
            self.draw_area(
                canvas,
                dataset,
                origin_x as i32,
                origin_y as i32,
                scale_x,
                scale_y,
            );
        }

        canvas.draw_vertical_line(canvas.margin, [0, 0, 0]);
        canvas.draw_vertical_line(canvas.width - canvas.margin, [0, 0, 0]);
        canvas.draw_horizontal_line(canvas.height - canvas.margin, [0, 0, 0]);
        canvas.draw_horizontal_line(canvas.margin, [0, 0, 0]);

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
            let (w, h) = text_size(scale, &font, &dataset.label);
            // Draw the square
            for dy in 0..square_size {
                for dx in 0..square_size {
                    canvas.draw_pixel(
                        x + dx,
                        y + square_size * 2 + dy + h, // Adjust to align above baseline
                        dataset.color,
                    );
                }
            }

            // Draw the label text next to the square
            let text_x: u32 = x + square_size + padding;
            canvas.draw_text(
                text_x,
                y + 2 * square_size + h,
                &dataset.label,
                dataset.color,
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
