use crate::figure::{
    canvas::pixelcanvas::PixelCanvas,
    configuration::figureconfig::FigureConfig,
    datasets::bardataset::BarDataset,
    render::{pixel, ticks, AxesSpec, Bounds, Ticks},
    utilities::orientation::Orientation,
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
    /// This method draws a horizontally oriented bar chart with grouped bars:
    /// one group per distinct x value of the datasets (from bottom to top),
    /// one bar per dataset in each group.
    /// It uses the `FigureConfig` settings for appearance and adjusts the canvas accordingly.
    ///
    /// # Example
    /// ```rust,ignore
    /// bar_chart.draw_horizontal(&mut canvas);
    /// ```
    pub fn draw_horizontal(&self, canvas: &mut PixelCanvas) {
        self.draw_pixel(canvas, &Orientation::Horizontal);
    }

    /// Draws a vertical grouped bar chart.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to render the bar chart on.
    ///
    /// # Details
    /// This method draws a vertically oriented bar chart with grouped bars:
    /// one group per distinct x value of the datasets (from left to right),
    /// one bar per dataset in each group.
    /// It uses the `FigureConfig` settings for appearance and adjusts the canvas accordingly.
    ///
    /// # Example
    /// ```rust,ignore
    /// bar_chart.draw_vertical(&mut canvas);
    /// ```
    pub fn draw_vertical(&self, canvas: &mut PixelCanvas) {
        self.draw_pixel(canvas, &Orientation::Vertical);
    }

    fn draw_pixel(&self, canvas: &mut PixelCanvas, orientation: &Orientation) {
        let spec = self.spec(orientation);
        let m = pixel::begin(canvas, &spec);
        for bar in self.bars() {
            let (p0, p1) = bar.corners(orientation);
            let (x0, y0) = m.point(p0);
            let (x1, y1) = m.point(p1);
            pixel::fill_rect(canvas, x0, y0, x1, y1, bar.color);
        }
        pixel::finish(canvas, &spec, &m);
    }

    /// Distinct, finite x values of all datasets, sorted ascending.
    pub(crate) fn categories(&self) -> Vec<f64> {
        let mut c: Vec<f64> = self
            .datasets
            .iter()
            .flat_map(|d| d.data.iter().map(|&(x, _)| x))
            .filter(|x| x.is_finite())
            .collect();
        c.sort_by(f64::total_cmp);
        c.dedup();
        c
    }

    /// Every bar, in category units: group `i` spans `i..i + 1`, the bars of
    /// a group share the middle 80 % of it.
    pub(crate) fn bars(&self) -> Vec<Bar> {
        let categories = self.categories();
        let n = self.datasets.len().max(1) as f64;
        let width = 0.8 / n;
        let mut bars = Vec::new();
        for (i, &category) in categories.iter().enumerate() {
            for (j, d) in self.datasets.iter().enumerate() {
                let value = d
                    .data
                    .iter()
                    .find(|&&(x, _)| x == category)
                    .map(|&(_, v)| v);
                if let Some(value) = value.filter(|v| v.is_finite()) {
                    let start = i as f64 + 0.1 + j as f64 * width;
                    bars.push(Bar {
                        category,
                        start,
                        end: start + width,
                        value,
                        color: d.color,
                        label: d.label.clone(),
                    });
                }
            }
        }
        bars
    }

    /// Bounds used for drawing and hit-testing.
    pub(crate) fn bounds_for(&self, orientation: &Orientation) -> Bounds {
        let slots = self.categories().len().max(1) as f64;
        let values = self
            .datasets
            .iter()
            .flat_map(|d| d.data.iter().map(|&(_, v)| v))
            .filter(|v| v.is_finite());
        let (lo, hi) = values.fold((0.0_f64, 0.0_f64), |(lo, hi), v| (lo.min(v), hi.max(v)));
        let hi = if lo == hi { lo + 1.0 } else { hi };
        match orientation {
            Orientation::Vertical => Bounds {
                x_min: 0.0,
                x_max: slots,
                y_min: lo,
                y_max: hi,
            },
            Orientation::Horizontal => Bounds {
                x_min: lo,
                x_max: hi,
                y_min: 0.0,
                y_max: slots,
            },
        }
    }

    pub(crate) fn spec(&self, orientation: &Orientation) -> AxesSpec<'_> {
        let categories = Ticks::Labels(
            self.categories()
                .iter()
                .enumerate()
                .map(|(i, &c)| (i as f64 + 0.5, ticks::format_category(c)))
                .collect(),
        );
        let values = Ticks::Linear(self.config.num_axis_ticks);
        let legend = self
            .datasets
            .iter()
            .map(|d| (d.label.clone(), d.color))
            .collect();
        let bounds = self.bounds_for(orientation);
        match orientation {
            Orientation::Vertical => AxesSpec {
                title: &self.title,
                x_label: &self.x_label,
                y_label: &self.y_label,
                config: &self.config,
                bounds,
                x_ticks: categories,
                y_ticks: values,
                legend,
            },
            // The categories (`x_label`) are on the vertical axis.
            Orientation::Horizontal => AxesSpec {
                title: &self.title,
                x_label: &self.y_label,
                y_label: &self.x_label,
                config: &self.config,
                bounds,
                x_ticks: values,
                y_ticks: categories,
                legend,
            },
        }
    }
}

/// One bar of a [`GroupBarChart`], in category units.
pub(crate) struct Bar {
    pub category: f64,
    pub start: f64,
    pub end: f64,
    pub value: f64,
    pub color: [u8; 3],
    pub label: String,
}

impl Bar {
    /// Opposite corners of the bar in data coordinates.
    pub(crate) fn corners(&self, orientation: &Orientation) -> ((f64, f64), (f64, f64)) {
        match orientation {
            Orientation::Vertical => ((self.start, 0.0), (self.end, self.value)),
            Orientation::Horizontal => ((0.0, self.start), (self.value, self.end)),
        }
    }
}
