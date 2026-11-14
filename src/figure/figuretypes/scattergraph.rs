use crate::figure::{
    canvas::pixelcanvas::PixelCanvas,
    configuration::figureconfig::FigureConfig,
    datasets::scattergraphdataset::ScatterGraphDataset,
    render::{pixel, Bounds},
    utilities::scatterdottype::ScatterDotType,
};

/// Represents a scatter graph, including title, axis labels, datasets, and configuration settings.
pub struct ScatterGraph {
    /// Title of the scatter graph.
    pub title: String,
    /// Label for the X-axis.
    pub x_label: String,
    /// Label for the Y-axis.
    pub y_label: String,
    /// A collection of datasets to be visualized on the scatter graph.
    pub datasets: Vec<ScatterGraphDataset>,
    /// Configuration settings for rendering the graph (e.g., colors, fonts, grid).
    pub config: FigureConfig,
}

impl ScatterGraph {
    /// Creates a new `ScatterGraph` instance with the specified title, labels, and configuration.
    ///
    /// # Parameters
    /// - `title`: The title of the scatter graph.
    /// - `x_label`: The label for the X-axis.
    /// - `y_label`: The label for the Y-axis.
    /// - `config`: The `FigureConfig` containing appearance and behavior settings.
    ///
    /// # Returns
    /// A new `ScatterGraph` instance with an empty dataset.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dataviz::figure::configuration::figureconfig::FigureConfig;
    /// use dataviz::figure::figuretypes::scattergraph::ScatterGraph;
    ///
    /// let config = FigureConfig::default();
    /// let scatter_graph = ScatterGraph::new("Data Points", "X Axis", "Y Axis", config);
    /// ```
    pub fn new(title: &str, x_label: &str, y_label: &str, config: FigureConfig) -> Self {
        Self {
            title: title.to_string(),
            x_label: x_label.to_string(),
            y_label: y_label.to_string(),
            datasets: Vec::new(),
            config,
        }
    }

    /// Adds a dataset to the scatter graph.
    ///
    /// # Parameters
    /// - `dataset`: The `ScatterGraphDataset` to be added to the graph.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dataviz::figure::datasets::scattergraphdataset::ScatterGraphDataset;
    /// use dataviz::figure::utilities::scatterdottype::ScatterDotType;
    ///
    /// let dataset = ScatterGraphDataset {
    ///     points: vec![(1.0, 2.0), (3.0, 4.0)],
    ///     color: [255, 0, 0],
    ///     label: "Dataset 1".to_string(),
    ///     dot_type: ScatterDotType::Circle(5),
    /// };
    /// scatter_graph.add_dataset(dataset);
    /// ```
    pub fn add_dataset(&mut self, dataset: ScatterGraphDataset) {
        self.datasets.push(dataset);
    }

    /// Draws a single dot on the canvas using the specified dot type and color.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the dot on.
    /// - `x`: The x-coordinate of the dot's center on the canvas.
    /// - `y`: The y-coordinate of the dot's center on the canvas.
    /// - `dot_type`: The shape and size of the dot (`ScatterDotType`).
    /// - `color`: The RGB color of the dot.
    ///
    /// Pixels outside the canvas are skipped.
    ///
    /// # Details
    /// Supports different dot types:
    /// - `Circle`: A circular dot with a specified radius.
    /// - `Square`: A square-shaped dot with a specified size.
    /// - `Cross`: A cross-shaped dot with a specified thickness.
    /// - `Triangle`: An upward-pointing triangular dot with a specified base width.
    ///
    /// # Example
    /// ```rust,ignore
    /// scatter_graph.draw_dot(&mut canvas, 100, 100, ScatterDotType::Circle(5), [255, 0, 0]);
    /// ```
    pub fn draw_dot(
        &self,
        canvas: &mut PixelCanvas,
        x: i32,
        y: i32,
        dot_type: ScatterDotType,
        color: [u8; 3],
    ) {
        pixel::marker(canvas, x as f64, y as f64, &dot_type, color);
    }

    /// Bounds used for drawing and hit-testing: the data range, extended to
    /// include the origin.
    pub(crate) fn bounds(&self) -> Bounds {
        Bounds::from_points(self.datasets.iter().flat_map(|d| d.points.iter().copied()))
            .unwrap_or_default()
            .with_origin()
    }
}
