use crate::figure::{
    canvas::pixelcanvas::PixelCanvas,
    configuration::figureconfig::FigureConfig,
    datasets::bardataset::BarDataset,
    drawers::drawer::Drawer,
    utilities::{axistype::AxisType, orientation::Orientation},
};

/// A grouped bar chart representation, supporting horizontal and vertical orientations.
pub struct GroupBarChart {
    /// A collection of datasets to be visualized in the grouped bar chart.
    pub datasets: Vec<BarDataset>,
    /// Title of the bar chart.
    pub title: String,
    /// Label for the X-axis.
    pub x_label: String,
    /// Label for the Y-axis.
    pub y_label: String,
    /// Orientation of the bar chart (`Horizontal` or `Vertical`).
    pub orientation: Orientation,
    /// Configuration settings for rendering the chart (e.g., colors, fonts, grid).
    pub config: FigureConfig,
}

impl GroupBarChart {
    /// Creates a new `GroupBarChart` instance with the specified title, labels, orientation, and configuration.
    ///
    /// # Parameters
    /// - `title`: The title of the bar chart.
    /// - `x_label`: The label for the X-axis.
    /// - `y_label`: The label for the Y-axis.
    /// - `orientation`: The orientation of the bar chart (`Horizontal` or `Vertical`).
    /// - `config`: The `FigureConfig` containing appearance and behavior settings for the chart.
    ///
    /// # Returns
    /// A new `GroupBarChart` instance with an empty dataset.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dataviz::figure::configuration::figureconfig::FigureConfig;
    /// use dataviz::figure::figuretypes::groupbarchart::GroupBarChart;
    /// use dataviz::figure::utilities::orientation::Orientation;
    ///
    /// let config = FigureConfig::default();
    /// let bar_chart = GroupBarChart::new("Sales Chart", "Year", "Revenue", Orientation::Vertical, config);
    /// ```
    pub fn new(
        title: &str,
        x_label: &str,
        y_label: &str,
        orientation: Orientation,
        config: FigureConfig,
    ) -> Self {
        Self {
            datasets: Vec::new(),
            title: title.to_string(),
            x_label: x_label.to_string(),
            y_label: y_label.to_string(),
            orientation,
            config,
        }
    }

    /// Adds a dataset to the grouped bar chart.
    ///
    /// # Parameters
    /// - `dataset`: The `BarDataset` to be added to the chart.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dataviz::figure::datasets::bardataset::BarDataset;
    ///
    /// let dataset = BarDataset::new("Company A", [255, 0, 0]);
    /// bar_chart.add_dataset(dataset);
    /// ```
    pub fn add_dataset(&mut self, dataset: BarDataset) {
        self.datasets.push(dataset);
    }

    /// Draws a horizontal grouped bar chart.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to render the bar chart on.
    ///
    /// # Details
    /// This method draws a horizontally oriented bar chart with grouped bars.
    /// It uses the `FigureConfig` settings for appearance and adjusts the canvas accordingly.
    ///
    /// # Example
    /// ```rust,ignore
    /// bar_chart.draw_horizontal(&mut canvas);
    /// ```
    pub fn draw_horizontal(&self, canvas: &mut PixelCanvas) {
        canvas.clear();

        let margin = canvas.margin;
        let width = canvas.width;
        let height = canvas.height;
        let cfg = &self.config;

        // Draw the title
        self.draw_title(canvas, cfg, width / 2, margin / 2, &self.title);

        // Get unique y-axis values
        let unique_y_values: Vec<u32> = self
            .datasets
            .iter()
            .flat_map(|dataset| dataset.data.iter().map(|(y, _)| *y as u32))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        let y_count = unique_y_values.len();

        let (x_min, x_max) = self
            .datasets
            .iter()
            .flat_map(|dataset| dataset.data.iter().map(|&(_, y)| y))
            .fold((0.0_f64, 0.0_f64), |(min, max), y| (min.min(y), max.max(y)));

        // Adjust limits to include (0, 0)
        let x_min = x_min.min(0.0);

        // Calculate scales
        let scale_y = (height - 2 * margin) as f64 / y_count as f64;
        let scale_x = (width - 2 * margin) as f64 / x_max;

        // Draw grids
        self.draw_grid(canvas, cfg);

        // Draw axes
        let origin_x = margin;
        let origin_y = height - margin;

        self.draw_label(canvas, cfg, width - margin / 2, origin_y, &self.y_label);
        self.draw_label(canvas, cfg, margin, margin / 2, &self.x_label);

        // X-axis ticks
        let x_tick_step = (x_max - x_min) / cfg.num_axis_ticks as f64;
        for i in 0..=cfg.num_axis_ticks {
            let value_x = x_min + i as f64 * x_tick_step;
            let tick_x = origin_x + ((value_x - x_min) * scale_x) as u32;

            let value_label = format!("{value_x:.1}");

            self.draw_axis_value(canvas, cfg, tick_x, origin_y, &value_label, AxisType::AxisX);
        }

        // Draw grouped horizontal bars
        let group_height = scale_y * 0.8; // Height of each group
        let bar_height = group_height / self.datasets.len() as f64; // Height of each bar

        for (group_index, y_label) in unique_y_values.iter().enumerate() {
            let group_center_y = origin_y - ((group_index as f64 + 0.5) * scale_y) as u32;

            self.draw_axis_value(
                canvas,
                cfg,
                origin_x - 10,
                group_center_y,
                &y_label.to_string(),
                AxisType::AxisY,
            );

            // Draw bars for each company in the group
            for (company_index, dataset) in self.datasets.iter().enumerate() {
                if let Some(&(_, value)) = dataset
                    .data
                    .iter()
                    .find(|(y, _)| (*y as u32).to_string() == y_label.to_string())
                {
                    let bar_length = (value * scale_x) as u32;
                    let bar_top = group_center_y - (group_height / 2.0) as u32
                        + (company_index as f64 * bar_height) as u32;
                    let bar_bottom = bar_top + bar_height as u32;

                    for x in origin_x..(origin_x + bar_length) {
                        for y in bar_top..bar_bottom {
                            canvas.draw_pixel(x, y, dataset.color);
                        }
                    }
                }
            }
        }

        canvas.draw_vertical_line(margin, cfg.color_axis);
        canvas.draw_horizontal_line(height - margin, cfg.color_axis);
        // Draw legend
        self.draw_legend(canvas);
    }

    /// Draws a vertical grouped bar chart.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to render the bar chart on.
    ///
    /// # Details
    /// This method draws a vertically oriented bar chart with grouped bars.
    /// It uses the `FigureConfig` settings for appearance and adjusts the canvas accordingly.
    ///
    /// # Example
    /// ```rust,ignore
    /// bar_chart.draw_vertical(&mut canvas);
    /// ```
    pub fn draw_vertical(&self, canvas: &mut PixelCanvas) {
        canvas.clear();
        let margin = canvas.margin;
        let width = canvas.width;
        let height = canvas.height;
        let cfg = &self.config;

        // Draw the title
        self.draw_title(canvas, cfg, width / 2, margin / 2, &self.title);

        // Get unique x-axis values
        let unique_x_values: Vec<u32> = self
            .datasets
            .iter()
            .flat_map(|d| d.data.iter().map(|(x, _)| *x as u32))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        let x_count = unique_x_values.len();
        let y_max = self
            .datasets
            .iter()
            .flat_map(|d| d.data.iter().map(|(_, y)| *y))
            .fold(0.0_f64, |max, y| max.max(y));

        // Calculate scales
        let scale_x = (width - 2 * margin) as f64 / x_count as f64;
        let scale_y = (height - 2 * margin) as f64 / y_max;
        let origin_x = margin;
        let origin_y = height - margin;

        // Draw grids
        self.draw_grid(canvas, cfg);

        // Draw axes
        canvas.draw_vertical_line(margin, cfg.color_axis);
        canvas.draw_vertical_line(width - margin, cfg.color_axis);
        canvas.draw_horizontal_line(height - margin, cfg.color_axis);
        canvas.draw_horizontal_line(margin, cfg.color_axis);

        // Draw axis labels
        self.draw_label(canvas, cfg, width - margin / 2, origin_y, &self.x_label);
        self.draw_label(canvas, cfg, margin, margin / 2, &self.y_label);

        // Y-axis ticks
        let y_tick_step = y_max / cfg.num_axis_ticks as f64;
        for i in 0..=cfg.num_axis_ticks {
            let value_y = i as f64 * y_tick_step;
            let tick_y = origin_y - (value_y * scale_y) as u32;

            let value_label = format!("{value_y:.2}");

            self.draw_axis_value(
                canvas,
                cfg,
                origin_x - 5,
                tick_y,
                &value_label,
                AxisType::AxisY,
            );
        }

        // Draw x-axis labels and grouped bars
        let group_width = scale_x * 0.8; // Width of each group of bars
        let bar_width = group_width / self.datasets.len() as f64; // Width of each bar

        for (group_index, x_label) in unique_x_values.iter().enumerate() {
            let group_center_x = origin_x + (((group_index as f64 + 0.5) * scale_x) as u32);

            self.draw_axis_value(
                canvas,
                cfg,
                group_center_x,
                origin_y,
                &x_label.to_string(),
                AxisType::AxisX,
            );

            // Draw bars for each company in the group
            for (company_index, dataset) in self.datasets.iter().enumerate() {
                if let Some(&(_, income)) = dataset
                    .data
                    .iter()
                    .find(|(x, _)| (*x as u32).to_string() == x_label.to_string())
                {
                    let bar_height = (income * scale_y) as u32;
                    let bar_left = group_center_x - (group_width / 2.0) as u32
                        + (company_index as f64 * bar_width) as u32;
                    let bar_right = bar_left + bar_width as u32;

                    for x in bar_left..=bar_right {
                        for y in (origin_y - bar_height)..origin_y {
                            canvas.draw_pixel(x, y, dataset.color);
                        }
                    }
                }
            }
        }

        // Draw legend
        self.draw_legend(canvas);
    }
}
