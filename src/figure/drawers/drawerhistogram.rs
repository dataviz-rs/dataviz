use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    figuretypes::histogram::Histogram,
    utilities::axistype::AxisType,
};

use super::drawer::Drawer;
use std::any::Any;
impl Drawer for Histogram {
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas) {
        // Clear existing SVG elements

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

        // Calculate range and scales
        let y_max = self.bin_counts.iter().cloned().fold(0.0, f64::max);
        let scale_x = (width - 2.0 * margin) / (self.max - self.min);
        let scale_y = (height - 2.0 * margin) / y_max;

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
        let origin_x = margin;
        let origin_y = height - margin;

        svg_canvas.draw_line(margin, origin_y, width - margin, origin_y, "black", 2.0); // X-axis
        svg_canvas.draw_line(margin, margin, margin, height - margin, "black", 2.0); // Y-axis

        // X-axis
        let mut x_axis_ticks = String::new();
        for i in 0..=num_ticks {
            let value = self.max + i as f64 * (self.max - self.min) / num_ticks as f64;
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
            let value = 0.0 + i as f64 * (y_max - 0.0) / num_ticks as f64;
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

        // Draw X-axis label
        svg_canvas.draw_text(
            width / 2.0,
            height - margin / 4.0,
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

        // Draw histogram bars
        for (i, &count) in self.bin_counts.iter().enumerate() {
            let bin_start = self.min + i as f64 * self.bin_width;
            let bin_end = bin_start + self.bin_width;

            let x_start = margin + (bin_start - self.min) * scale_x;
            let x_end = margin + (bin_end - self.min) * scale_x;
            let bar_width = x_end - x_start;
            let bar_height = count * scale_y;

            svg_canvas.draw_rect(
                x_start,
                origin_y - bar_height,
                bar_width,
                bar_height,
                &format!("rgb({},{},{})", self.color[0], self.color[1], self.color[2]),
                "black",
                1.0,
                1.0,
            );
        }
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

        let bin_data = self.calculate_bins();
        let y_max = bin_data.iter().map(|&(_, freq)| freq).fold(0.0, f64::max);

        let scale_x = (width - 2 * margin) as f64 / self.bins as f64;
        let scale_y = (height - 2 * margin) as f64 / y_max;

        canvas.draw_grid(
            &[cfg.num_grid_horizontal, cfg.num_grid_vertical],
            cfg.color_grid,
        );

        // Draw axes
        let origin_x = margin as i32;
        let origin_y = height as i32 - margin as i32;

        // Draw bars with edges
        let bin_start = bin_data[0].0; // First bin's start
        let bin_width = (bin_data[1].0 - bin_start).abs(); // Width of each bin

        for (i, &(_, freq)) in bin_data.iter().enumerate() {
            let bar_height = (freq * scale_y) as i32;
            let bar_left = origin_x + (i as f64 * scale_x) as i32;
            let bar_right = bar_left + scale_x as i32;

            // Fill the bar
            for x in bar_left..=bar_right {
                for y in (origin_y - bar_height)..origin_y {
                    canvas.draw_pixel(x as u32, y as u32, self.color);
                }
            }

            // Draw the edges (outline)
            let edge_color = [0, 0, 0]; // Black color for edges
                                        // Left edge
            for y in (origin_y - bar_height)..origin_y {
                canvas.draw_pixel(bar_left as u32, y as u32, edge_color);
            }
            // Right edge
            for y in (origin_y - bar_height)..origin_y {
                canvas.draw_pixel(bar_right as u32, y as u32, edge_color);
            }
            // Top edge
            for x in bar_left..=bar_right {
                canvas.draw_pixel(x as u32, (origin_y - bar_height) as u32, edge_color);
            }
        }

        // Add x-axis ticks and labels at bin edges
        for i in 0..=self.bins {
            let edge_x = origin_x + (i as f64 * scale_x) as i32;
            let edge_value = bin_start + i as f64 * bin_width;

            canvas.draw_pixel(edge_x as u32, origin_y as u32, [0, 0, 0]); // Tick mark
            let edge_label = format!("{edge_value:.1}");
            self.draw_axis_value(
                canvas,
                cfg,
                edge_x as u32,
                origin_y as u32 + 10,
                &edge_label,
                AxisType::AxisX,
            );
        }

        // Add y-axis ticks and labels
        let num_y_ticks = 10;
        for i in 0..=num_y_ticks {
            let tick_value = y_max * i as f64 / num_y_ticks as f64;
            let tick_y = origin_y - (tick_value * scale_y) as i32;

            canvas.draw_pixel(origin_x as u32, tick_y as u32, [0, 0, 0]); // Tick mark
            let tick_label = format!("{tick_value:.1}");
            self.draw_axis_value(
                canvas,
                cfg,
                origin_x as u32 - 10,
                tick_y as u32,
                &tick_label,
                AxisType::AxisY,
            );
        }

        let origin_y = height - margin;
        self.draw_label(canvas, cfg, width - margin / 2, origin_y, &self.y_label);
        self.draw_label(canvas, cfg, margin, margin / 2, &self.x_label);

        canvas.draw_vertical_line(canvas.margin, [0, 0, 0]);
        canvas.draw_vertical_line(canvas.width - canvas.margin, [0, 0, 0]);
        canvas.draw_horizontal_line(canvas.height - canvas.margin, [0, 0, 0]);
        canvas.draw_horizontal_line(canvas.margin, [0, 0, 0]);
    }

    fn draw_legend(&self, _canvas: &mut PixelCanvas) {
        // Histogram does not have a legend
    }

    fn as_any(&mut self) -> &mut (dyn Any + 'static) {
        self as &mut dyn Any
    }

    fn get_figure_config(&self) -> &FigureConfig {
        &self.config
    }
}
