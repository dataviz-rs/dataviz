use crate::figure::configuration::figureconfig::FigureConfig;

/// Represents a histogram, including title, axis labels, bin configuration, and cached data.
pub struct Histogram {
    /// Title of the histogram.
    pub title: String,
    /// Label for the X-axis.
    pub x_label: String,
    /// Label for the Y-axis.
    pub y_label: String,
    /// Number of bins in the histogram.
    pub bins: usize,
    /// Raw data values to be represented in the histogram.
    pub data: Vec<f64>,
    /// Color of the histogram bars in RGB format.
    pub color: [u8; 3],
    /// Cached minimum value in the dataset.
    pub min: f64,
    /// Cached maximum value in the dataset.
    pub max: f64,
    /// Cached frequencies for each bin.
    pub bin_counts: Vec<f64>,
    /// Cached width of each bin.
    pub bin_width: f64,
    /// Configuration settings for rendering the histogram.
    pub config: FigureConfig,
}

impl Histogram {
    /// Creates a new `Histogram` instance with the specified configuration.
    ///
    /// # Parameters
    /// - `title`: The title of the histogram.
    /// - `x_label`: The label for the X-axis.
    /// - `y_label`: The label for the Y-axis.
    /// - `bins`: The number of bins in the histogram.
    /// - `color`: The RGB color of the histogram bars.
    /// - `config`: The `FigureConfig` containing appearance and behavior settings.
    ///
    /// # Returns
    /// A new `Histogram` instance with empty data and initialized bins.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dataviz::figure::configuration::figureconfig::FigureConfig;
    /// use dataviz::figure::histogram::Histogram;
    ///
    /// let config = FigureConfig::default();
    /// let histogram = Histogram::new("Data Distribution", "Values", "Frequency", 10, [255, 0, 0], config);
    /// ```
    pub fn new(
        title: &str,
        x_label: &str,
        y_label: &str,
        bins: usize,
        color: [u8; 3],
        config: FigureConfig,
    ) -> Self {
        Self {
            title: title.to_string(),
            x_label: x_label.to_string(),
            y_label: y_label.to_string(),
            bins,
            data: Vec::new(),
            color,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            bin_counts: vec![0.0; bins],
            bin_width: 0.0,
            config,
        }
    }

    /// Adds multiple data values to the histogram.
    ///
    /// # Parameters
    /// - `values`: A vector of `f64` values to be added to the histogram.
    ///
    /// # Example
    /// ```rust,ignore
    /// histogram.add_data_vec(vec![1.2, 2.5, 3.1, 4.8]);
    /// ```
    pub fn add_data_vec(&mut self, values: Vec<f64>) {
        for value in values {
            self.add_data(value);
        }
    }

    /// Adds a single data value to the histogram.
    ///
    /// # Parameters
    /// - `value`: An `f64` value to be added to the histogram.
    ///
    /// # Details
    /// - Updates the cached minimum and maximum values.
    /// - Recalculates the bin width.
    /// - Updates the appropriate bin count based on the value.
    ///
    /// # Example
    /// ```rust,ignore
    /// histogram.add_data(3.5);
    /// ```
    pub fn add_data(&mut self, value: f64) {
        self.data.push(value);

        // Update min and max
        if value < self.min {
            self.min = value;
        }
        if value > self.max {
            self.max = value;
        }

        // Recalculate bin width and update bin counts
        self.bin_width = (self.max - self.min) / self.bins as f64;
        if self.bin_width > 0.0 {
            let bin_index = ((value - self.min) / self.bin_width).floor() as usize;
            if bin_index < self.bins {
                self.bin_counts[bin_index] += 1.0;
            }
        }
    }

    /// Calculates the bin ranges and frequencies for the histogram.
    ///
    /// # Returns
    /// A vector of tuples where each tuple contains:
    /// - The starting value of the bin.
    /// - The frequency count for that bin.
    ///
    /// # Example
    /// ```rust,ignore
    /// let bins = histogram.calculate_bins();
    /// for (start, count) in bins {
    ///     println!("Bin starts at {}, count is {}", start, count);
    /// }
    /// ```
    pub fn calculate_bins(&self) -> Vec<(f64, f64)> {
        self.bin_counts
            .iter()
            .enumerate()
            .map(|(i, &freq)| (self.min + i as f64 * self.bin_width, freq))
            .collect()
    }
}
