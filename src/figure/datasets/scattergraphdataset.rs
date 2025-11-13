use crate::figure::utilities::scatterdottype::ScatterDotType;

/// A dataset for scatter graphs, representing points and their appearance.
pub struct ScatterGraphDataset {
    /// A collection of `(x, y)` data points for the scatter graph.
    pub points: Vec<(f64, f64)>,
    /// Color of the scatter points in RGB format.
    pub color: [u8; 3],
    /// Label for the dataset, used in legends or annotations.
    pub label: String,
    /// Shape of the scatter points (circle, square, triangle, etc.).
    pub dot_type: ScatterDotType,
}

impl ScatterGraphDataset {
    /// Creates a new `ScatterGraphDataset` instance with the specified appearance and metadata.
    ///
    /// # Parameters
    /// - `color`: The RGB color of the scatter points.
    /// - `label`: A descriptive label for the dataset.
    /// - `dot_type`: The shape of the scatter points (`ScatterDotType`).
    ///
    /// # Returns
    /// A new `ScatterGraphDataset` instance with an empty list of points.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dataviz::figure::utilities::scatterdottype::ScatterDotType;
    ///
    /// let dataset = ScatterGraphDataset::new(
    ///     [255, 0, 0],
    ///     "Example Dataset",
    ///     ScatterDotType::Circle(5)
    /// );
    /// ```
    pub fn new(color: [u8; 3], label: &str, dot_type: ScatterDotType) -> Self {
        Self {
            points: Vec::new(),
            color,
            label: label.to_string(),
            dot_type,
        }
    }
}
