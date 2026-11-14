//! Shared, crate-private rendering machinery.
//!
//! Every chart used to carry its own copy of range computation, coordinate
//! mapping, grid/tick/legend drawing and font loading, and the copies had
//! drifted apart (SVG ignored [`FigureConfig`], hover used a different
//! mapping than drawing, …). Everything that must be identical between the
//! pixel renderer, the SVG renderer and the hover code lives here.
//!
//! [`FigureConfig`]: crate::figure::configuration::figureconfig::FigureConfig

pub(crate) mod bounds;
pub(crate) mod font;
pub(crate) mod pixel;
pub(crate) mod svg;
pub(crate) mod ticks;

pub(crate) use bounds::Bounds;

/// The plot area of a canvas: the rectangle inside the margins, in canvas
/// coordinates (y grows downwards).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frame {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Frame {
    /// Builds the plot area of a `width` x `height` canvas with the given
    /// margin. A margin that would leave no room for the plot is reduced so
    /// the plot area is always at least one pixel wide and high.
    pub fn new(width: u32, height: u32, margin: u32) -> Self {
        let (w, h) = (width as f64, height as f64);
        let max_margin = ((w.min(h) - 1.0) / 2.0).max(0.0).floor();
        let m = (margin as f64).min(max_margin);
        Self {
            left: m,
            top: m,
            right: (w - m).max(m + 1.0),
            bottom: (h - m).max(m + 1.0),
        }
    }

    pub fn width(&self) -> f64 {
        self.right - self.left
    }

    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }

    /// The margin actually used (see [`Frame::new`]).
    pub fn margin(&self) -> f64 {
        self.left
    }
}

/// Maps data coordinates into canvas coordinates for a given frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Mapper {
    pub frame: Frame,
    pub bounds: Bounds,
}

impl Mapper {
    pub fn new(frame: Frame, bounds: Bounds) -> Self {
        Self {
            frame,
            bounds: bounds.sanitized(),
        }
    }

    pub fn for_canvas(width: u32, height: u32, margin: u32, bounds: Bounds) -> Self {
        Self::new(Frame::new(width, height, margin), bounds)
    }

    pub fn x(&self, value: f64) -> f64 {
        let b = &self.bounds;
        self.frame.left + (value - b.x_min) / (b.x_max - b.x_min) * self.frame.width()
    }

    pub fn y(&self, value: f64) -> f64 {
        let b = &self.bounds;
        self.frame.bottom - (value - b.y_min) / (b.y_max - b.y_min) * self.frame.height()
    }

    pub fn point(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (self.x(x), self.y(y))
    }

    /// Canvas x of the vertical axis: x = 0 when it is visible, otherwise
    /// the left edge of the frame.
    pub fn y_axis_x(&self) -> f64 {
        if self.bounds.x_min <= 0.0 && 0.0 <= self.bounds.x_max {
            self.x(0.0)
        } else {
            self.frame.left
        }
    }

    /// Canvas y of the horizontal axis: y = 0 when it is visible, otherwise
    /// the bottom edge of the frame.
    pub fn x_axis_y(&self) -> f64 {
        if self.bounds.y_min <= 0.0 && 0.0 <= self.bounds.y_max {
            self.y(0.0)
        } else {
            self.frame.bottom
        }
    }
}

/// How the ticks of one axis are produced.
#[derive(Clone, Debug)]
pub(crate) enum Ticks {
    /// `divisions` equal steps between the axis bounds (`divisions + 1` labels).
    Linear(usize),
    /// Explicit `(data position, label)` pairs, used for categorical axes.
    Labels(Vec<(f64, String)>),
}

impl Ticks {
    /// Resolves the ticks to `(data position, label)` pairs.
    pub fn resolve(&self, min: f64, max: f64) -> Vec<(f64, String)> {
        match self {
            Ticks::Linear(divisions) => ticks::linear(min, max, *divisions),
            Ticks::Labels(labels) => labels.clone(),
        }
    }
}

/// Every how many ticks a label is drawn so that labels of the given extent
/// (width on the x-axis, height on the y-axis) do not overlap. Tick marks
/// are still drawn for every tick.
pub(crate) fn label_stride(positions: &[f64], max_extent: f64) -> usize {
    let spacing = positions
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(f64::INFINITY, f64::min);
    if !spacing.is_finite() || spacing <= 0.0 {
        return 1;
    }
    ((max_extent + 4.0) / spacing).ceil().max(1.0) as usize
}

/// One legend entry: label and colour.
pub(crate) type LegendEntry = (String, [u8; 3]);

/// Everything needed to draw the frame of an x/y chart (background, grid,
/// axes, ticks, labels, title and legend). The data itself is drawn by the
/// chart between [`pixel::begin`]/[`svg::begin`] and the matching `finish`.
pub(crate) struct AxesSpec<'a> {
    pub title: &'a str,
    pub x_label: &'a str,
    pub y_label: &'a str,
    pub config: &'a crate::figure::configuration::figureconfig::FigureConfig,
    pub bounds: Bounds,
    pub x_ticks: Ticks,
    pub y_ticks: Ticks,
    pub legend: Vec<LegendEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_clamps_oversized_margin() {
        let f = Frame::new(100, 50, 80);
        assert!(f.width() >= 1.0 && f.height() >= 1.0);
        assert!(f.right <= 100.0 && f.bottom <= 50.0);
        let f = Frame::new(0, 0, 10);
        assert!(f.width() >= 1.0 && f.height() >= 1.0);
    }

    #[test]
    fn mapper_maps_corners() {
        let m = Mapper::for_canvas(
            200,
            100,
            10,
            Bounds {
                x_min: -1.0,
                x_max: 1.0,
                y_min: 0.0,
                y_max: 10.0,
            },
        );
        assert_eq!(m.point((-1.0, 0.0)), (10.0, 90.0));
        assert_eq!(m.point((1.0, 10.0)), (190.0, 10.0));
        assert_eq!(m.y_axis_x(), 100.0);
        assert_eq!(m.x_axis_y(), 90.0);
    }

    #[test]
    fn stride() {
        assert_eq!(label_stride(&[0.0, 50.0, 100.0], 30.0), 1);
        assert_eq!(label_stride(&[0.0, 20.0, 40.0, 60.0], 30.0), 2);
        assert_eq!(label_stride(&[0.0], 30.0), 1);
    }

    #[test]
    fn axis_falls_back_to_frame_edge() {
        let m = Mapper::for_canvas(
            200,
            100,
            10,
            Bounds {
                x_min: 5.0,
                x_max: 6.0,
                y_min: 5.0,
                y_max: 6.0,
            },
        );
        assert_eq!(m.y_axis_x(), 10.0);
        assert_eq!(m.x_axis_y(), 90.0);
    }
}
