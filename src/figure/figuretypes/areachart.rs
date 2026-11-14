use crate::figure::{
    canvas::pixelcanvas::PixelCanvas, configuration::figureconfig::FigureConfig,
    datasets::areachartdataset::AreaChartDataset, render::Bounds,
};

/// Fills the area between the polyline through `points` (canvas coordinates,
/// any order) and the horizontal line `baseline`, blending `color` with
/// `alpha`. Non-finite points are ignored.
pub(crate) fn fill_area(
    canvas: &mut PixelCanvas,
    points: &[(f64, f64)],
    baseline: f64,
    color: [u8; 3],
    alpha: f64,
) {
    if !baseline.is_finite() {
        return;
    }
    let mut pts: Vec<(f64, f64)> = points
        .iter()
        .copied()
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .collect();
    if pts.len() < 2 {
        return;
    }
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));

    let (w, h) = (canvas.width as f64, canvas.height as f64);
    let first = pts[0].0.ceil().max(0.0);
    let last = pts[pts.len() - 1].0.floor().min(w - 1.0);
    if first > last {
        return;
    }
    let mut seg = 0;
    let mut cx = first;
    while cx <= last {
        while seg + 2 < pts.len() && pts[seg + 1].0 < cx {
            seg += 1;
        }
        let ((x1, y1), (x2, y2)) = (pts[seg], pts[seg + 1]);
        let y = if x2 > x1 {
            y1 + (cx - x1) * (y2 - y1) / (x2 - x1)
        } else {
            y1.min(y2)
        };
        let top = y.min(baseline).round().max(0.0);
        let bottom = y.max(baseline).round().min(h - 1.0);
        let mut cy = top;
        while cy <= bottom {
            canvas.blend(cx as i64, cy as i64, color, alpha);
            cy += 1.0;
        }
        cx += 1.0;
    }
}

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
    /// use dataviz::figure::figuretypes::areachart::AreaChart;
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
    /// The points are sorted by x, then every pixel column between the first and
    /// the last point is filled from the interpolated line to `origin_y` (above
    /// or below it), blended with the dataset's color and `alpha`. Each pixel
    /// is blended exactly once, so the opacity is uniform.
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
        let points: Vec<(f64, f64)> = dataset
            .points
            .iter()
            .map(|&(x, y)| (origin_x as f64 + x * scale_x, origin_y as f64 - y * scale_y))
            .collect();
        fill_area(
            canvas,
            &points,
            origin_y as f64,
            dataset.color,
            dataset.alpha,
        );
    }

    /// Bounds used for drawing and hit-testing: the data range, extended to
    /// include the origin.
    /// Bounds used for drawing and hit-testing, widened to round tick values.
    pub(crate) fn bounds(&self) -> Bounds {
        self.raw_bounds().nice(
            Some(self.config.num_axis_ticks),
            Some(self.config.num_axis_ticks),
        )
    }

    fn raw_bounds(&self) -> Bounds {
        Bounds::from_points(self.datasets.iter().flat_map(|d| d.points.iter().copied()))
            .unwrap_or_default()
            .with_origin()
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
