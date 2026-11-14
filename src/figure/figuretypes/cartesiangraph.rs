use crate::figure::{
    configuration::figureconfig::FigureConfig, datasets::cartesiangraphdataset::CartesianDataset,
    render::Bounds,
};

/// A graph with both axes crossing at the centre of the plot (the ranges are
/// symmetric around zero), suitable for mathematical functions.
pub struct CartesianGraph {
    /// A collection of datasets to be visualized in the graph.
    pub datasets: Vec<CartesianDataset>,
    /// Title of the graph.
    pub title: String,
    /// Label for the X-axis.
    pub x_label: String,
    /// Label for the Y-axis.
    pub y_label: String,
    /// Minimum x-value (see [`CartesianGraph::update_range`]).
    pub x_min: f64,
    /// Maximum x-value.
    pub x_max: f64,
    /// Minimum y-value.
    pub y_min: f64,
    /// Maximum y-value.
    pub y_max: f64,
    /// Configuration settings for rendering the graph (e.g., colors, fonts, grid).
    pub config: FigureConfig,
}

impl CartesianGraph {
    /// Creates a new, empty `CartesianGraph`.
    ///
    /// # Example
    /// ```
    /// use dataviz::figure::configuration::figureconfig::FigureConfig;
    /// use dataviz::figure::figuretypes::cartesiangraph::CartesianGraph;
    ///
    /// let graph = CartesianGraph::new("Functions", "x", "y", &FigureConfig::default());
    /// assert!(graph.datasets.is_empty());
    /// ```
    pub fn new(title: &str, x_label: &str, y_label: &str, config: &FigureConfig) -> Self {
        Self {
            datasets: Vec::new(),
            title: title.to_string(),
            x_label: x_label.to_string(),
            y_label: y_label.to_string(),
            x_min: f64::INFINITY,     // Initialize to max range
            x_max: f64::NEG_INFINITY, // Initialize to min range
            y_min: f64::INFINITY,     // Initialize to max range
            y_max: f64::NEG_INFINITY, // Initialize to min range
            config: config.clone(),
        }
    }

    /// Adds a dataset and extends the range to include it.
    pub fn add_dataset(&mut self, dataset: CartesianDataset) {
        self.datasets.push(dataset);
        self.update_range();
    }

    /// Extends `x_min..x_max` / `y_min..y_max` to include every finite data
    /// point and makes both ranges symmetric around zero.
    ///
    /// The range only grows: values set manually (or by earlier data) that
    /// are larger are kept. Both [`Drawer::draw`] and [`Drawer::draw_svg`]
    /// call this before drawing.
    ///
    /// [`Drawer::draw`]: crate::figure::drawers::drawer::Drawer::draw
    /// [`Drawer::draw_svg`]: crate::figure::drawers::drawer::Drawer::draw_svg
    pub fn update_range(&mut self) {
        for dataset in &self.datasets {
            for &(x, y) in &dataset.points {
                if !x.is_finite() || !y.is_finite() {
                    continue;
                }
                self.x_min = self.x_min.min(x);
                self.x_max = self.x_max.max(x);
                self.y_min = self.y_min.min(y);
                self.y_max = self.y_max.max(y);
            }
        }

        if self.x_min <= self.x_max {
            let m = self.x_min.abs().max(self.x_max.abs());
            self.x_min = -m;
            self.x_max = m;
        }
        if self.y_min <= self.y_max {
            let m = self.y_min.abs().max(self.y_max.abs());
            self.y_min = -m;
            self.y_max = m;
        }
    }

    /// Bounds used for drawing and hit-testing: the stored range, symmetric
    /// around zero, `-1..1` when there is no data.
    /// Bounds used for drawing and hit-testing, widened to round tick values.
    pub(crate) fn bounds(&self) -> Bounds {
        self.raw_bounds().nice_symmetric(self.config.num_axis_ticks)
    }

    fn raw_bounds(&self) -> Bounds {
        let mut b = Bounds {
            x_min: self.x_min,
            x_max: self.x_max,
            y_min: self.y_min,
            y_max: self.y_max,
        };
        if !b.x_min.is_finite() || !b.x_max.is_finite() || b.x_min > b.x_max {
            b.x_min = -1.0;
            b.x_max = 1.0;
        }
        if !b.y_min.is_finite() || !b.y_max.is_finite() || b.y_min > b.y_max {
            b.y_min = -1.0;
            b.y_max = 1.0;
        }
        let b = b.symmetric();
        Bounds {
            x_min: if b.x_max == 0.0 { -1.0 } else { b.x_min },
            x_max: if b.x_max == 0.0 { 1.0 } else { b.x_max },
            y_min: if b.y_max == 0.0 { -1.0 } else { b.y_min },
            y_max: if b.y_max == 0.0 { 1.0 } else { b.y_max },
        }
    }
}
