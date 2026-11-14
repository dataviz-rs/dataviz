/// Configuration structure for customizing the appearance of a figure.
#[derive(Clone, Debug, PartialEq)]
pub struct FigureConfig {
    /// Number of tick intervals along each value axis (`n` intervals give
    /// `n + 1` labelled ticks). Labels that would overlap are thinned out.
    pub num_axis_ticks: usize,
    /// Number of intervals between horizontal grid lines.
    pub num_grid_horizontal: usize,
    /// Number of intervals between vertical grid lines.
    pub num_grid_vertical: usize,
    /// Color of the grid lines in RGB format.
    pub color_grid: [u8; 3],
    /// Color of the axes in RGB format.
    pub color_axis: [u8; 3],
    /// Background color of the figure in RGB format.
    pub color_background: [u8; 3],
    /// Color of the title text in RGB format.
    pub color_title: [u8; 3],
    /// Font size for labels.
    pub font_size_label: f32,
    /// Font size for the title.
    pub font_size_title: f32,
    /// Font size for the legend text.
    pub font_size_legend: f32,
    /// Font size for axis labels.
    pub font_size_axis: f32,
    /// File path to the font (TTF/OTF) used for labels, tick values and
    /// legends on a `PixelCanvas`. When `None` or unreadable, an installed
    /// sans-serif font is used.
    pub font_label: Option<String>,
    /// File path to the font used for the title on a `PixelCanvas`. When
    /// `None`, `font_label` (or a system font) is used.
    pub font_title: Option<String>,
}

impl Default for FigureConfig {
    /// Provides a default configuration for a figure.
    ///
    /// # Default Values
    /// - `num_axis_ticks`: 10
    /// - `num_grid_horizontal`: 10
    /// - `num_grid_vertical`: 10
    /// - `color_grid`: `[200, 200, 200]` (light gray)
    /// - `color_axis`: `[0, 0, 0]` (black)
    /// - `color_background`: `[255, 255, 255]` (white)
    /// - `color_title`: `[0, 0, 0]` (black)
    /// - `font_size_label`: 12.0
    /// - `font_size_title`: 24.0
    /// - `font_size_legend`: 10.0
    /// - `font_size_axis`: 10.0
    /// - `font_label`: `None` (an installed sans-serif font is used)
    /// - `font_title`: `None` (falls back to `font_label`, then to a system font)
    ///
    /// # Returns
    /// A `FigureConfig` instance with default settings.
    fn default() -> Self {
        Self {
            num_axis_ticks: 10,
            num_grid_horizontal: 10,
            num_grid_vertical: 10,
            color_grid: [200, 200, 200],       // Light gray
            color_axis: [0, 0, 0],             // Black
            color_background: [255, 255, 255], // White
            color_title: [0, 0, 0],            // Black
            font_size_label: 12.0,
            font_size_title: 24.0,
            font_size_legend: 10.0,
            font_size_axis: 10.0,
            font_label: None,
            font_title: None,
        }
    }
}

impl FigureConfig {
    /// Sets both font paths.
    pub fn set_font_paths(&mut self, label_path: String, title_path: String) {
        self.font_label = Some(label_path);
        self.font_title = Some(title_path);
    }

    /// Returns an error unless both font paths are set. Fonts are optional
    /// for drawing (a system font is used instead), so this is only useful
    /// to require explicit fonts.
    pub fn validate(&self) -> Result<(), String> {
        if self.font_label.is_none() || self.font_title.is_none() {
            Err("Both font_label and font_title must be provided.".to_string())
        } else {
            Ok(())
        }
    }
}
