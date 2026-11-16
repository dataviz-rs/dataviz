//! Figure themes: colors, fonts, sizes and default styles.

use super::color::{Color, Colormap, Palette};
use super::style::Dash;

/// Which sides of the plot frame are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Spines {
    /// Top edge.
    pub top: bool,
    /// Right edge.
    pub right: bool,
    /// Bottom edge.
    pub bottom: bool,
    /// Left edge.
    pub left: bool,
}

impl Spines {
    /// All four sides.
    pub const ALL: Spines = Spines {
        top: true,
        right: true,
        bottom: true,
        left: true,
    };
    /// Left and bottom only.
    pub const LEFT_BOTTOM: Spines = Spines {
        top: false,
        right: false,
        bottom: true,
        left: true,
    };
    /// No frame.
    pub const NONE: Spines = Spines {
        top: false,
        right: false,
        bottom: false,
        left: false,
    };
}

/// Visual defaults for a figure. Start from a preset and adjust fields.
///
/// ```
/// use dataviz::plot::theme::Theme;
/// let mut theme = Theme::dark();
/// theme.title_size = 22.0;
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Figure background.
    pub background: Color,
    /// Plot area background.
    pub plot_background: Color,
    /// Titles, labels and tick labels.
    pub text: Color,
    /// Frame and tick marks.
    pub axis: Color,
    /// Major grid lines.
    pub grid: Color,
    /// Minor grid lines.
    pub minor_grid: Color,
    /// Grid line width.
    pub grid_width: f64,
    /// Grid line style.
    pub grid_dash: Dash,
    /// Draw major grid lines on x / y.
    pub grid_x: bool,
    /// Draw major grid lines on y.
    pub grid_y: bool,
    /// Draw minor grid lines.
    pub minor_grid_lines: bool,
    /// Frame sides.
    pub spines: Spines,
    /// Frame / tick width.
    pub axis_width: f64,
    /// Tick mark length (negative = inside).
    pub tick_length: f64,
    /// Font file (TTF/OTF) for all text; `None` = installed sans-serif.
    pub font: Option<String>,
    /// Figure title size.
    pub title_size: f64,
    /// Axes title size.
    pub axes_title_size: f64,
    /// Axis label size.
    pub label_size: f64,
    /// Tick label size.
    pub tick_size: f64,
    /// Legend text size.
    pub legend_size: f64,
    /// Annotation / data label size.
    pub annotation_size: f64,
    /// Series color cycle.
    pub palette: Vec<Color>,
    /// Default colormap.
    pub colormap: Colormap,
    /// Default line width.
    pub line_width: f64,
    /// Default marker diameter.
    pub marker_size: f64,
    /// Legend frame background.
    pub legend_background: Color,
    /// Legend frame border.
    pub legend_border: Color,
    /// Space around the whole figure.
    pub padding: f64,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::WHITE,
            plot_background: Color::WHITE,
            text: Color::rgb(34, 34, 34),
            axis: Color::rgb(60, 60, 60),
            grid: Color::rgb(225, 225, 225),
            minor_grid: Color::rgb(242, 242, 242),
            grid_width: 1.0,
            grid_dash: Dash::Solid,
            grid_x: true,
            grid_y: true,
            minor_grid_lines: false,
            spines: Spines::ALL,
            axis_width: 1.0,
            tick_length: 5.0,
            font: None,
            title_size: 20.0,
            axes_title_size: 16.0,
            label_size: 14.0,
            tick_size: 12.0,
            legend_size: 12.0,
            annotation_size: 11.0,
            palette: Palette::Tab10.colors(),
            colormap: Colormap::Viridis,
            line_width: 2.0,
            marker_size: 7.0,
            legend_background: Color::rgba(255, 255, 255, 0.85),
            legend_border: Color::rgb(200, 200, 200),
            padding: 12.0,
        }
    }
}

impl Theme {
    /// Light theme with a full frame and light grid (default).
    pub fn light() -> Self {
        Self::default()
    }

    /// Dark background, bright palette.
    pub fn dark() -> Self {
        Self {
            background: Color::rgb(24, 26, 31),
            plot_background: Color::rgb(30, 33, 40),
            text: Color::rgb(225, 228, 235),
            axis: Color::rgb(140, 146, 160),
            grid: Color::rgb(55, 60, 72),
            minor_grid: Color::rgb(42, 46, 56),
            palette: Palette::Neon.colors(),
            colormap: Colormap::Plasma,
            legend_background: Color::rgba(30, 33, 40, 0.9),
            legend_border: Color::rgb(70, 75, 88),
            ..Self::default()
        }
    }

    /// No frame, horizontal grid only.
    pub fn minimal() -> Self {
        Self {
            spines: Spines::NONE,
            grid_x: false,
            tick_length: 0.0,
            grid: Color::rgb(232, 232, 232),
            legend_border: Color::TRANSPARENT,
            ..Self::default()
        }
    }

    /// Gray plot background with white grid (ggplot2 look).
    pub fn ggplot() -> Self {
        Self {
            plot_background: Color::rgb(235, 235, 235),
            grid: Color::WHITE,
            minor_grid: Color::rgb(245, 245, 245),
            minor_grid_lines: true,
            spines: Spines::NONE,
            axis: Color::rgb(80, 80, 80),
            palette: vec![
                Color::rgb(248, 118, 109),
                Color::rgb(0, 186, 56),
                Color::rgb(97, 156, 255),
                Color::rgb(183, 159, 0),
                Color::rgb(0, 191, 196),
                Color::rgb(245, 100, 227),
            ],
            legend_border: Color::TRANSPARENT,
            legend_background: Color::rgba(255, 255, 255, 0.0),
            ..Self::default()
        }
    }

    /// Light blue-gray background with white grid (seaborn "darkgrid").
    pub fn seaborn() -> Self {
        Self {
            plot_background: Color::rgb(234, 234, 242),
            grid: Color::WHITE,
            spines: Spines::NONE,
            tick_length: 0.0,
            palette: Palette::Muted.colors(),
            legend_border: Color::rgb(220, 220, 220),
            ..Self::default()
        }
    }

    /// Black and white, serif-friendly, thin lines, left/bottom frame
    /// with inward ticks, for papers.
    pub fn publication() -> Self {
        Self {
            text: Color::BLACK,
            axis: Color::BLACK,
            grid_x: false,
            grid_y: false,
            spines: Spines::ALL,
            tick_length: -4.0,
            line_width: 1.5,
            marker_size: 6.0,
            palette: Palette::OkabeIto.colors(),
            title_size: 16.0,
            axes_title_size: 14.0,
            label_size: 12.0,
            tick_size: 10.0,
            legend_size: 10.0,
            legend_border: Color::BLACK,
            ..Self::default()
        }
    }

    /// High contrast for low vision and projectors (WCAG AAA text): black
    /// on white, heavier lines and frame, larger text, and dark Okabe–Ito
    /// colors (each at least 4.5:1 against white) that stay apart for
    /// color-blind readers.
    pub fn high_contrast() -> Self {
        Self {
            background: Color::WHITE,
            plot_background: Color::WHITE,
            text: Color::BLACK,
            axis: Color::BLACK,
            grid: Color::rgb(150, 150, 150),
            minor_grid: Color::rgb(205, 205, 205),
            grid_width: 1.0,
            axis_width: 1.6,
            spines: Spines::ALL,
            tick_length: 6.0,
            title_size: 24.0,
            axes_title_size: 19.0,
            label_size: 17.0,
            tick_size: 15.0,
            legend_size: 15.0,
            annotation_size: 14.0,
            // Okabe–Ito hues darkened to at least 4.5:1 against white.
            palette: vec![
                Color::rgb(0, 90, 160),
                Color::rgb(190, 75, 0),
                Color::rgb(0, 120, 90),
                Color::rgb(160, 60, 125),
                Color::rgb(0, 0, 0),
                Color::rgb(125, 95, 0),
                Color::rgb(80, 60, 150),
                Color::rgb(90, 90, 90),
            ],
            colormap: Colormap::Cividis,
            line_width: 3.0,
            marker_size: 9.0,
            legend_background: Color::WHITE,
            legend_border: Color::BLACK,
            ..Self::default()
        }
    }

    /// Warm beige "solarized light" theme.
    pub fn solarized() -> Self {
        Self {
            background: Color::rgb(253, 246, 227),
            plot_background: Color::rgb(253, 246, 227),
            text: Color::rgb(88, 110, 117),
            axis: Color::rgb(147, 161, 161),
            grid: Color::rgb(238, 232, 213),
            palette: vec![
                Color::rgb(38, 139, 210),
                Color::rgb(220, 50, 47),
                Color::rgb(133, 153, 0),
                Color::rgb(203, 75, 22),
                Color::rgb(108, 113, 196),
                Color::rgb(42, 161, 152),
                Color::rgb(211, 54, 130),
                Color::rgb(181, 137, 0),
            ],
            spines: Spines::LEFT_BOTTOM,
            legend_background: Color::rgba(253, 246, 227, 0.9),
            legend_border: Color::rgb(238, 232, 213),
            ..Self::default()
        }
    }

    /// Every preset with its name (for galleries).
    pub fn presets() -> Vec<(&'static str, Theme)> {
        vec![
            ("light", Theme::light()),
            ("dark", Theme::dark()),
            ("minimal", Theme::minimal()),
            ("ggplot", Theme::ggplot()),
            ("seaborn", Theme::seaborn()),
            ("publication", Theme::publication()),
            ("solarized", Theme::solarized()),
            ("high_contrast", Theme::high_contrast()),
        ]
    }
}

#[cfg(test)]
mod contrast_tests {
    use super::*;

    #[test]
    fn high_contrast_colors_reach_4_5_to_1() {
        let t = Theme::high_contrast();
        let ratio = |c: Color| (1.05) / (c.luminance() + 0.05);
        for c in t.palette.iter().chain([&t.text, &t.axis]) {
            assert!(ratio(*c) >= 4.5, "{c:?}: {}", ratio(*c));
        }
    }
}
