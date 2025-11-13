/// A dataset for a bar chart, containing data points, appearance properties, and metadata.
pub struct BarDataset {
    /// Label for the dataset, used in legends or annotations.
    pub label: String,
    /// Color of the bars in RGB format.
    pub color: [u8; 3],
    /// A collection of `(x, y)` data points where `x` is the category and `y` is the value.
    pub data: Vec<(f64, f64)>,
}

impl BarDataset {
    /// Creates a new `BarDataset` instance with the specified label and color.
    ///
    /// # Parameters
    /// - `label`: A descriptive label for the dataset.
    /// - `color`: The RGB color of the bars.
    ///
    /// # Returns
    /// A new `BarDataset` instance with an empty list of data points.
    ///
    /// # Example
    /// ```rust,ignore
    /// let dataset = BarDataset::new("Sales Data", [0, 128, 255]);
    /// ```
    pub fn new(label: &str, color: [u8; 3]) -> Self {
        Self {
            data: Vec::new(),
            label: label.to_string(),
            color,
        }
    }

    /// Adds a data point to the dataset.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate representing the category or group (e.g., year).
    /// - `y`: The y-coordinate representing the value for the category.
    ///
    /// # Example
    /// ```rust,ignore
    /// let mut dataset = BarDataset::new("Revenue", [255, 0, 0]);
    /// dataset.add_data(2020.0, 1500.0);
    /// dataset.add_data(2021.0, 2000.0);
    /// ```
    pub fn add_data(&mut self, x: f64, y: f64) {
        self.data.push((x, y));
    }
}
