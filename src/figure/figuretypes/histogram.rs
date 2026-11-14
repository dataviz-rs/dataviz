use crate::figure::{configuration::figureconfig::FigureConfig, render::Bounds};

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
    /// use dataviz::figure::figuretypes::histogram::Histogram;
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
    /// ```
    /// # use dataviz::figure::{configuration::figureconfig::FigureConfig, figuretypes::histogram::Histogram};
    /// let mut histogram = Histogram::new("h", "x", "n", 4, [0, 0, 255], FigureConfig::default());
    /// histogram.add_data_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    /// assert_eq!(histogram.bin_counts, vec![1.0, 1.0, 1.0, 2.0]);
    /// ```
    pub fn add_data_vec(&mut self, values: Vec<f64>) {
        self.data.extend(values);
        self.recalculate();
    }

    /// Adds a single data value to the histogram.
    ///
    /// # Parameters
    /// - `value`: An `f64` value to be added to the histogram.
    ///
    /// # Details
    /// Updates the cached minimum/maximum, the bin width and the bin counts
    /// (see [`Histogram::recalculate`]). When adding many values at once,
    /// [`Histogram::add_data_vec`] is faster.
    ///
    /// # Example
    /// ```
    /// # use dataviz::figure::{configuration::figureconfig::FigureConfig, figuretypes::histogram::Histogram};
    /// let mut histogram = Histogram::new("h", "x", "n", 2, [0, 0, 255], FigureConfig::default());
    /// histogram.add_data(1.0);
    /// histogram.add_data(3.0);
    /// assert_eq!(histogram.bin_counts, vec![1.0, 1.0]);
    /// ```
    pub fn add_data(&mut self, value: f64) {
        self.data.push(value);
        self.recalculate();
    }

    /// Recomputes `min`, `max`, `bin_width` and `bin_counts` from `data` and
    /// `bins`. Call it after changing those fields directly.
    ///
    /// Every finite value is counted exactly once: the bins are
    /// `[min + i * bin_width, min + (i + 1) * bin_width)`, and the last bin
    /// also includes `max`. When all values are equal, they are counted in
    /// the first bin and `bin_width` is `0`. Non-finite values are ignored.
    pub fn recalculate(&mut self) {
        let finite = || self.data.iter().copied().filter(|v| v.is_finite());
        self.min = finite().fold(f64::INFINITY, f64::min);
        self.max = finite().fold(f64::NEG_INFINITY, f64::max);
        self.bin_counts = vec![0.0; self.bins];
        self.bin_width = if self.bins > 0 && self.max > self.min {
            (self.max - self.min) / self.bins as f64
        } else {
            0.0
        };
        if self.bins == 0 {
            return;
        }
        for v in self.data.iter().copied().filter(|v| v.is_finite()) {
            let index = if self.bin_width > 0.0 {
                (((v - self.min) / self.bin_width).floor() as usize).min(self.bins - 1)
            } else {
                0
            };
            self.bin_counts[index] += 1.0;
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

    /// Start and width of the drawn bins: the real ones, or `min..min+1`
    /// split into `bins` when the data has no spread.
    pub(crate) fn display_bins(&self) -> (f64, f64) {
        let bins = self.bins.max(1) as f64;
        if self.bin_width > 0.0 && self.min.is_finite() {
            (self.min, self.bin_width)
        } else if self.min.is_finite() {
            (self.min, 1.0 / bins)
        } else {
            (0.0, 1.0 / bins)
        }
    }

    /// Bounds used for drawing and hit-testing.
    /// Bounds used for drawing and hit-testing, widened to round tick values.
    pub(crate) fn bounds(&self) -> Bounds {
        self.raw_bounds().nice(
            Some(self.config.num_axis_ticks),
            Some(self.config.num_axis_ticks),
        )
    }

    fn raw_bounds(&self) -> Bounds {
        let (start, width) = self.display_bins();
        let y_max = self.bin_counts.iter().copied().fold(0.0, f64::max);
        Bounds {
            x_min: start,
            x_max: start + width * self.bins.max(1) as f64,
            y_min: 0.0,
            y_max: if y_max > 0.0 { y_max } else { 1.0 },
        }
    }
}
