use crate::figure::{
    canvas::pixelcanvas::PixelCanvas, configuration::figureconfig::FigureConfig, render::Frame,
};

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
    /// use dataviz::figure::figuretypes::piechart::PieChart;
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
    /// Angles are measured counter-clockwise from the positive x-axis (3
    /// o'clock), in `0..=2π`. A slice ending at `2π` (or later) includes the
    /// pixels just below the positive x-axis. Pixels outside the canvas are
    /// skipped.
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
        if radius <= 0 || !start_angle.is_finite() || !end_angle.is_finite() {
            return;
        }
        let full = 2.0 * std::f64::consts::PI;
        let r = radius as i64;
        let (cx, cy) = (center_x as i64, center_y as i64);
        for y in -r..=r {
            for x in -r..=r {
                if x * x + y * y > r * r {
                    continue;
                }
                let mut angle = (y as f64).atan2(x as f64);
                if angle < 0.0 {
                    angle += full;
                }
                let inside =
                    angle >= start_angle && (angle < end_angle || end_angle >= full - 1e-9);
                if inside {
                    canvas.put(cx + x, cy - y, color);
                }
            }
        }
    }

    /// Slices as `(label, value, color, start angle, sweep)`; negative and
    /// non-finite values count as `0`. Angles as in [`PieChart::draw_slice`].
    pub(crate) fn slices(&self) -> Vec<(&str, f64, [u8; 3], f64, f64)> {
        let value = |v: f64| if v.is_finite() && v > 0.0 { v } else { 0.0 };
        let total: f64 = self.datasets.iter().map(|d| value(d.1)).sum();
        let mut start = 0.0;
        self.datasets
            .iter()
            .map(|(label, v, color)| {
                let sweep = if total > 0.0 {
                    value(*v) / total * 2.0 * std::f64::consts::PI
                } else {
                    0.0
                };
                let slice = (label.as_str(), *v, *color, start, sweep);
                start += sweep;
                slice
            })
            .collect()
    }

    /// Centre and radius of the pie inside the plot area.
    pub(crate) fn geometry(width: u32, height: u32, margin: u32) -> ((f64, f64), f64) {
        let f = Frame::new(width, height, margin);
        (
            ((f.left + f.right) / 2.0, (f.top + f.bottom) / 2.0),
            (f.width().min(f.height()) / 2.0).max(1.0),
        )
    }
}
