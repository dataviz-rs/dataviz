use crate::figure::{canvas::pixelcanvas::PixelCanvas, configuration::figureconfig::FigureConfig};

/// Represents a pie chart with title, datasets, and configuration settings.
pub struct PieChart {
    /// Title of the pie chart.
    pub title: String,
    /// A collection of datasets, where each dataset contains:
    /// - A label (`String`).
    /// - A value (`f64`).
    /// - A color in RGB format (`[u8; 3]`).
    pub datasets: Vec<(String, f64, [u8; 3])>,
    /// Configuration settings for rendering the chart (e.g., fonts, colors, grid).
    pub config: FigureConfig,
}

impl PieChart {
    /// Creates a new `PieChart` instance with the specified title and configuration.
    ///
    /// # Parameters
    /// - `title`: The title of the pie chart.
    /// - `config`: The `FigureConfig` containing appearance and behavior settings.
    ///
    /// # Returns
    /// A new `PieChart` instance with no datasets.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dataviz::figure::configuration::figureconfig::FigureConfig;
    /// use dataviz::figure::piechart::PieChart;
    ///
    /// let config = FigureConfig::default();
    /// let pie_chart = PieChart::new("Market Share", config);
    /// ```
    pub fn new(title: &str, config: FigureConfig) -> Self {
        Self {
            title: title.to_string(),
            datasets: Vec::new(),
            config,
        }
    }

    /// Adds a slice to the pie chart.
    ///
    /// # Parameters
    /// - `label`: The label for the slice.
    /// - `value`: The value representing the proportion of the slice.
    /// - `color`: The RGB color of the slice.
    ///
    /// # Example
    /// ```rust,ignore
    /// pie_chart.add_slice("Product A", 30.0, [255, 0, 0]);
    /// pie_chart.add_slice("Product B", 50.0, [0, 255, 0]);
    /// pie_chart.add_slice("Product C", 20.0, [0, 0, 255]);
    /// ```
    pub fn add_slice(&mut self, label: &str, value: f64, color: [u8; 3]) {
        self.datasets.push((label.to_string(), value, color));
    }

    /// Draws a slice of the pie chart on the canvas.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw the slice on.
    /// - `center_x`: The x-coordinate of the center of the pie chart.
    /// - `center_y`: The y-coordinate of the center of the pie chart.
    /// - `radius`: The radius of the pie chart.
    /// - `start_angle`: The starting angle of the slice in radians.
    /// - `end_angle`: The ending angle of the slice in radians.
    /// - `color`: The RGB color of the slice.
    ///
    /// # Details
    /// This method fills a portion of the circle defined by `start_angle` and `end_angle`.
    /// It ensures that only points within the slice and the circle are drawn.
    ///
    /// # Example
    /// ```rust,ignore
    /// pie_chart.draw_slice(&mut canvas, 200, 200, 100, 0.0, 1.0, [255, 0, 0]);
    /// ```
    #[allow(clippy::too_many_arguments)]
    pub fn draw_slice(
        &self,
        canvas: &mut PixelCanvas,
        center_x: i32,
        center_y: i32,
        radius: i32,
        start_angle: f64,
        end_angle: f64,
        color: [u8; 3],
    ) {
        let start_angle_rad = start_angle;
        let end_angle_rad = end_angle;

        for y in -radius..=radius {
            for x in -radius..=radius {
                // Check if the point is within the circle
                let distance = (x * x + y * y) as f64;
                if distance <= (radius * radius) as f64 {
                    // Calculate the angle of the point
                    let angle = (y as f64).atan2(x as f64);
                    let normalized_angle = if angle < 0.0 {
                        angle + 2.0 * std::f64::consts::PI
                    } else {
                        angle
                    };

                    // Check if the angle is within the slice range
                    if normalized_angle >= start_angle_rad && normalized_angle < end_angle_rad {
                        canvas.draw_pixel((center_x + x) as u32, (center_y - y) as u32, color);
                    }
                }
            }
        }
    }
}
