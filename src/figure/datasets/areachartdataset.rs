/// A dataset for an area chart, containing data points, appearance properties, and metadata.
pub struct AreaChartDataset {
    /// Transparency level of the area fill (0.0 for fully transparent, 1.0 for fully opaque).
    pub alpha: f64,
    /// A collection of `(x, y)` data points for the area chart.
    pub points: Vec<(f64, f64)>,
    /// Color of the area fill in RGB format.
    pub color: [u8; 3],
    /// Label for the dataset, used in legends or annotations.
    pub label: String,
}

impl AreaChartDataset {
    /// Creates a new `AreaChartDataset` instance with the specified appearance and metadata.
    ///
    /// # Parameters
    /// - `color`: The RGB color of the area fill.
    /// - `label`: A descriptive label for the dataset.
    /// - `alpha`: The transparency level of the area fill (0.0 to 1.0).
    ///
    /// # Returns
    /// A new `AreaChartDataset` instance with an empty list of points.
    ///
    /// # Example
    /// ```rust,ignore
    /// let dataset = AreaChartDataset::new([255, 0, 0], "Example Dataset", 0.5);
    /// ```
    pub fn new(color: [u8; 3], label: &str, alpha: f64) -> Self {
        Self {
            points: Vec::new(),
            color,
            label: label.to_string(),
            alpha,
        }
    }
}
