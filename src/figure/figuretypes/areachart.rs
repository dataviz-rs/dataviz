use crate::figure::{
    canvas::pixelcanvas::PixelCanvas, configuration::figureconfig::FigureConfig,
    datasets::areachartdataset::AreaChartDataset,
};

/// Represents an area chart, including its title, axis labels, datasets, and configuration.
pub struct AreaChart {
    /// Title of the area chart.
    pub title: String,
    /// Label for the X-axis.
    pub x_label: String,
    /// Label for the Y-axis.
    pub y_label: String,
    /// A collection of datasets to be visualized in the area chart.
    pub datasets: Vec<AreaChartDataset>,
    /// Configuration settings for rendering the chart (e.g., colors, fonts, grid).
    pub config: FigureConfig,
    ///  Minimum x-value
    pub x_min: f64,
    /// Maximum x-value
    pub x_max: f64,
    /// Minimum y-value
    pub y_min: f64,
    /// Maximum y-value
    pub y_max: f64,
}

impl AreaChart {
    /// Creates a new `AreaChart` instance with the specified title, axis labels, and configuration.
    ///
    /// # Parameters
    /// - `title`: The title of the area chart.
    /// - `x_label`: The label for the X-axis.
    /// - `y_label`: The label for the Y-axis.
    /// - `config`: The `FigureConfig` containing appearance and behavior settings for the chart.
    ///
    /// # Returns
    /// A new `AreaChart` instance with an empty dataset.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dataviz::figure::configuration::figureconfig::FigureConfig;
    /// use dataviz::figure::areachart::AreaChart;
    ///
    /// let config = FigureConfig::default();
    /// let area_chart = AreaChart::new("Example Chart", "X Axis", "Y Axis", config);
    /// ```
    pub fn new(title: &str, x_label: &str, y_label: &str, config: FigureConfig) -> Self {
        Self {
            title: title.to_string(),
            x_label: x_label.to_string(),
            y_label: y_label.to_string(),
            datasets: Vec::new(),
            config,
            x_min: f64::INFINITY,     // Initialize to max range
            x_max: f64::NEG_INFINITY, // Initialize to min range
            y_min: f64::INFINITY,     // Initialize to max range
            y_max: f64::NEG_INFINITY, // Initialize to min range
        }
    }

    /// Adds a dataset to the area chart.
    ///
    /// # Parameters
    /// - `dataset`: The `AreaChartDataset` to be added to the chart.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dataviz::figure::datasets::areachartdataset::AreaChartDataset;
    /// let dataset = AreaChartDataset::new([255, 0, 0], "Example Dataset", 0.5);
    /// area_chart.add_dataset(dataset);
    /// ```
    pub fn add_dataset(&mut self, dataset: AreaChartDataset) {
        self.datasets.push(dataset);
        self.update_range();
    }

    /// Draws the area under a dataset on the canvas.
    ///
    /// This method fills the area under the dataset line, interpolating between points
    /// and blending the pixels into the canvas.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` on which to draw the area.
    /// - `dataset`: The `AreaChartDataset` whose area is to be drawn.
    /// - `origin_x`: The x-coordinate of the chart's origin on the canvas.
    /// - `origin_y`: The y-coordinate of the chart's origin on the canvas.
    /// - `scale_x`: The scaling factor for converting X-axis values to canvas coordinates.
    /// - `scale_y`: The scaling factor for converting Y-axis values to canvas coordinates.
    ///
    /// # Details
    /// The method interpolates between adjacent points in the dataset to fill the area
    /// under the line segment and blend it into the canvas using the dataset's color and transparency.
    ///
    /// # Example
    /// ```rust,ignore
    /// let scale_x = 10.0;
    /// let scale_y = 10.0;
    /// area_chart.draw_area(&mut canvas, &dataset, 50, 400, scale_x, scale_y);
    /// ```
    pub fn draw_area(
        &self,
        canvas: &mut PixelCanvas,
        dataset: &AreaChartDataset,
        origin_x: i32,
        origin_y: i32,
        scale_x: f64,
        scale_y: f64,
    ) {
        let mut points = dataset.points.clone();
        points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        for window in points.windows(2) {
            if let [p1, p2] = window {
                let x1 = origin_x + ((p1.0) * scale_x) as i32;
                let y1 = origin_y - ((p1.1) * scale_y) as i32;
                let x2 = origin_x + ((p2.0) * scale_x) as i32;
                let y2 = origin_y - ((p2.1) * scale_y) as i32;

                // Fill the area under the line
                for x in x1.min(x2)..=x1.max(x2) {
                    let interpolated_y =
                        y1 + ((x - x1) as f64 * (y2 - y1) as f64 / (x2 - x1).abs() as f64) as i32;
                    for y in interpolated_y..=origin_y {
                        canvas.blend_pixel(x as u32, y as u32, dataset.color, dataset.alpha);
                    }
                }
            }
        }
    }

    pub fn update_range(&mut self) {
        for dataset in &self.datasets {
            for &(x, y) in &dataset.points {
                if x < self.x_min {
                    self.x_min = x;
                }
                if x > self.x_max {
                    self.x_max = x;
                }
                if y < self.y_min {
                    self.y_min = y;
                }
                if y > self.y_max {
                    self.y_max = y;
                }
            }
        }
        let mut is_empty = self.datasets.is_empty();

        for dataset in &self.datasets {
            if dataset.points.is_empty() {
                is_empty = true;
                break;
            }
        }

        if !is_empty {
            let abs_x_min = self.x_min.abs();
            let abs_x_max = self.x_max.abs();

            if abs_x_min > abs_x_max {
                self.x_max = abs_x_min;
            } else {
                self.x_min = -abs_x_max;
            }

            let abs_y_min = self.y_min.abs();
            let abs_y_max = self.y_max.abs();

            if abs_y_min > abs_y_max {
                self.y_max = abs_y_min;
            } else {
                self.y_min = -abs_y_max;
            }
        }
    }
}
