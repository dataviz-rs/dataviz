use super::drawer::Drawer;
use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    figuretypes::groupbarchart::GroupBarChart,
    utilities::orientation::Orientation,
};
use ab_glyph::{FontRef, PxScale};
use imageproc::drawing::text_size;
use std::any::Any;

impl Drawer for GroupBarChart {
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas) {
        let width = svg_canvas.width as f64;
        let height = svg_canvas.height as f64;
        let margin = svg_canvas.margin as f64;
        let font_size = 12.0;
        let cfg = &self.config;
        let margin_bg_color = svg_canvas.background_color.clone();

        match self.orientation {
            Orientation::Vertical => {
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

                // Calculate unique axis values
                let unique_x_values: Vec<u32> = self
                    .datasets
                    .iter()
                    .flat_map(|dataset| dataset.data.iter().map(|(x, _)| *x as u32))
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();

                let x_count = unique_x_values.len();

                let y_max = self
                    .datasets
                    .iter()
                    .flat_map(|dataset| dataset.data.iter().map(|(_, y)| *y))
                    .fold(0.0_f64, |max, y| max.max(y));

                // Calculate scales
                let scale_x = (width - 2.0 * margin) / x_count as f64;
                let scale_y = (height - 2.0 * margin) / y_max;

                // Draw grid
                svg_canvas.draw_grid(
                    margin,
                    width - margin,
                    margin,
                    height - margin,
                    10,
                    10,
                    "lightgray",
                );

                // Draw axes
                let origin_x = margin;
                let origin_y = height - margin;

                svg_canvas.draw_line(origin_x, margin, origin_x, origin_y, "black", 2.0); // Y-axis
                svg_canvas.draw_line(origin_x, origin_y, width - margin, origin_y, "black", 2.0); // X-axis

                // Y-axis
                let num_ticks = 10;
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
            margin - 10.0, y + font_size * 0.3, font_size, value
        ));
                }
                svg_canvas.elements.push(format!(
                    r#"<path d="{y_axis_ticks}" stroke="black" stroke-width="1" fill="none"/>"#
                ));

                // Draw X-axis labels and grouped bars
                let group_width = scale_x * 0.8; // Width of each group of bars
                let bar_width = group_width / self.datasets.len() as f64; // Width of each bar

                for (group_index, x_label) in unique_x_values.iter().enumerate() {
                    let group_center_x = origin_x + (group_index as f64 + 0.4) * scale_x;
                    // let x = margin + group_index as f64 * (width - 2.0 * margin) / unique_x_values.len() as f64;

                    // Draw X-axis label
                    svg_canvas.draw_text(
                        group_center_x,
                        origin_y + font_size * 1.5,
                        &x_label.to_string(),
                        font_size,
                        "black",
                    );

                    // Draw bars for each dataset in the group
                    for (dataset_index, dataset) in self.datasets.iter().enumerate() {
                        if let Some(&(_, value)) = dataset
                            .data
                            .iter()
                            .find(|(x, _)| (*x as u32).to_string() == x_label.to_string())
                        {
                            let bar_height = value * scale_y;
                            let bar_left = group_center_x - group_width / 2.0
                                + dataset_index as f64 * bar_width;

                            // Draw bar
                            svg_canvas.elements.push(format!(
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="rgb({},{},{})" stroke="black" stroke-width="1"/>"#,
                        bar_left,
                        origin_y - bar_height,
                        bar_width,
                        bar_height,
                        dataset.color[0],
                        dataset.color[1],
                        dataset.color[2]
                    ));
                        }
                    }
                }
            }
            Orientation::Horizontal => {
                let width = svg_canvas.width as f64;
                let height = svg_canvas.height as f64;
                let margin = svg_canvas.margin as f64;
                let font_size = 12.0;

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

                // Calculate unique axis values
                let unique_y_values: Vec<u32> = self
                    .datasets
                    .iter()
                    .flat_map(|dataset| dataset.data.iter().map(|(y, _)| *y as u32))
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();

                let y_count = unique_y_values.len();

                let x_max = self
                    .datasets
                    .iter()
                    .flat_map(|dataset| dataset.data.iter().map(|(_, x)| *x))
                    .fold(0.0_f64, |max, x| max.max(x));

                // Calculate scales
                let scale_y = (height - 2.0 * margin) / y_count as f64;
                let scale_x = (width - 2.0 * margin) / x_max;

                // Draw grid
                svg_canvas.draw_grid(
                    margin,
                    width - margin,
                    margin,
                    height - margin,
                    10,
                    10,
                    "lightgray",
                );

                // Draw axes
                let origin_x = margin;
                let origin_y = height - margin;

                svg_canvas.draw_line(origin_x, margin, origin_x, origin_y, "black", 2.0); // Y-axis
                svg_canvas.draw_line(origin_x, origin_y, width - margin, origin_y, "black", 2.0); // X-axis

                // Draw X-axis tick marks and labels
                let num_ticks = 10;
                let x_tick_step = x_max / num_ticks as f64;
                for i in 0..=num_ticks {
                    let value_x = i as f64 * x_tick_step;
                    let tick_x = origin_x + (value_x * scale_x);
                    let x = margin + i as f64 * (width - 2.0 * margin) / num_ticks as f64;
                    // Draw tick line
                    svg_canvas.draw_line(tick_x, origin_y, tick_x, origin_y + 5.0, "black", 1.0);

                    // Draw tick label
                    svg_canvas.draw_text(
                        x,
                        origin_y + font_size * 1.5,
                        &format!("{value_x:.1}"),
                        font_size,
                        "black",
                    );
                }

                // Draw Y-axis labels and grouped bars
                let group_height = scale_y * 0.8; // Height of each group of bars
                let bar_height = group_height / self.datasets.len() as f64; // Height of each bar

                for (group_index, y_label) in unique_y_values.iter().enumerate() {
                    let group_center_y = origin_y - (group_index as f64 + 0.5) * scale_y;

                    // Draw Y-axis label
                    svg_canvas.draw_text(
                        origin_x - font_size * 3.0,
                        group_center_y,
                        &y_label.to_string(),
                        font_size,
                        "black",
                    );

                    // Draw bars for each dataset in the group
                    for (dataset_index, dataset) in self.datasets.iter().enumerate() {
                        if let Some(&(_, value)) = dataset
                            .data
                            .iter()
                            .find(|(y, _)| (*y as u32).to_string() == y_label.to_string())
                        {
                            let bar_length = value * scale_x;
                            let bar_top = group_center_y - group_height / 2.0
                                + dataset_index as f64 * bar_height;

                            // Draw bar
                            svg_canvas.elements.push(format!(
                        r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="rgb({},{},{})" stroke="black" stroke-width="1"/>"#,
                        origin_x,
                        bar_top,
                        bar_length,
                        bar_height,
                        dataset.color[0],
                        dataset.color[1],
                        dataset.color[2]
                    ));
                        }
                    }
                }
            }
        }
        // Draw legend in the bottom-left corner
        let legend_x_start = margin + 10.0; // Start inside chart area with margin spacing
        let legend_y = height - margin + font_size * 1.5 + 10.0; // Position below x-axis labels

        let mut legend_x = legend_x_start;
        let mut elements = String::new();
        let legend_bg_color = svg_canvas.background_color.clone();

        for dataset in &self.datasets {
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

            elements.push_str(&format!(
                r#"<text x="{:.2}" y="{:.2}" font-size="{:.2}" fill="black">{}</text>"#,
                legend_x + font_size * 1.3,
                legend_y + font_size - 2.0,
                font_size,
                dataset.label
            ));

            legend_x += font_size * 5.0 + dataset.label.len() as f64 * font_size * 0.6;
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
        match self.orientation {
            Orientation::Vertical => self.draw_vertical(canvas),
            Orientation::Horizontal => self.draw_horizontal(canvas),
        }
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
