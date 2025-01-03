use crate::figure::{
    configuration::figureconfig::FigureConfig, datasets::cartesiangraphdataset::CartesianDataset,
};

/// Represents a graph limited to the first quadrant (x >= 0, y >= 0),
/// including datasets and configuration settings.
pub struct Quadrant1Graph {
    /// A collection of datasets to be visualized in the graph.
    pub datasets: Vec<CartesianDataset>,
    /// Title of the graph.
    pub title: String,
    /// Label for the X-axis.
    pub x_label: String,
    /// Label for the Y-axis.
    pub y_label: String,
    /// Configuration settings for rendering the graph (e.g., colors, fonts, grid).
    pub config: FigureConfig,
}

impl Quadrant1Graph {
    /// Creates a new `Quadrant1Graph` instance with the specified title, labels, and configuration.
    ///
    /// # Parameters
    /// - `title`: The title of the graph.
    /// - `x_label`: The label for the X-axis.
    /// - `y_label`: The label for the Y-axis.
    /// - `config`: The `FigureConfig` containing appearance and behavior settings.
    ///
    /// # Returns
    /// A new `Quadrant1Graph` instance with an empty dataset.
    ///
    /// # Example
    /// ```rust
    /// use crate::figure::configuration::figureconfig::FigureConfig;
    /// use crate::figure::quadrant1graph::Quadrant1Graph;
    ///
    /// let config = FigureConfig::default();
    /// let graph = Quadrant1Graph::new("First Quadrant Graph", "X Axis", "Y Axis", config);
    /// ```
    pub fn new(title: &str, x_label: &str, y_label: &str, config: FigureConfig) -> Self {
        Self {
            datasets: Vec::new(),
            title: title.to_string(),
            x_label: x_label.to_string(),
            y_label: y_label.to_string(),
            config,
        }
    }

    /// Adds a dataset to the graph after filtering out points not in the first quadrant.
    ///
    /// # Parameters
    /// - `dataset`: The `CartesianDataset` to be added to the graph.
    ///
    /// # Details
    /// This method ensures that only points with `x >= 0.0` and `y >= 0.0` are included in the dataset.
    ///
    /// # Example
    /// ```rust
    /// use crate::figure::datasets::cartesiangraphdataset::CartesianDataset;
    /// use crate::figure::utilities::linetype::LineType;
    ///
    /// let mut graph = Quadrant1Graph::new("Example Graph", "X Axis", "Y Axis", config);
    /// let dataset = CartesianDataset {
    ///     points: vec![(1.0, 2.0), (-1.0, 3.0), (4.0, -2.0)],
    ///     color: [255, 0, 0],
    ///     label: "Dataset 1".to_string(),
    ///     line_type: LineType::Solid,
    /// };
    /// graph.add_dataset(dataset);
    /// ```
    pub fn add_dataset(&mut self, dataset: CartesianDataset) {
        let filtered_dataset = CartesianDataset {
            points: dataset
                .points
                .into_iter()
                .filter(|&(x, y)| x >= 0.0 && y >= 0.0)
                .collect(),
            color: dataset.color,
            label: dataset.label.clone(),
            line_type: dataset.line_type,
        };
        self.datasets.push(filtered_dataset);
    }
}
