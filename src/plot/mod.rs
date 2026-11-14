//! Full-featured plotting API.
//!
//! ```
//! use dataviz::prelude::*;
//!
//! let mut fig = Figure::new(800, 500);
//! let ax = fig.axes();
//! ax.title("Signals").x_label("t [s]").y_label("V");
//! let t: Vec<f64> = (0..200).map(|i| i as f64 / 20.0).collect();
//! ax.line(&t, t.iter().map(|v| v.sin())).label("sin");
//! ax.scatter(&t, t.iter().map(|v| v.cos())).label("cos").size(4.0);
//! let svg = fig.to_svg();
//! assert!(svg.contains("sin"));
//! ```
//!
//! Everything is drawn into a [`Scene`] that is written as SVG, rasterized
//! to PNG/JPEG/WebP/BMP, converted to PDF (`pdf` feature), shown in a window
//! (`window` feature) or printed to a terminal.

pub mod anim;
pub mod axes;
pub mod axis;
pub mod color;
pub mod data;
pub mod facet;
pub mod figure;
#[cfg(feature = "gpu")]
pub(crate) mod gpu;
pub mod legend;
pub mod math;
pub mod output;
pub(crate) mod raster;
pub(crate) mod render;
pub mod scale;
pub mod scene;
pub mod series;
pub mod stats;
pub mod style;
pub mod terminal;
pub(crate) mod text;
pub mod theme;
pub mod ticks;
pub mod time;
#[cfg(feature = "window")]
pub mod window;

pub use axes::{Axes, AxesKind};
pub use axis::{Axis, TickSpec};
pub use color::{Color, Colormap, Palette};
pub use figure::Figure;
pub use legend::LegendPosition;
pub use output::{Backend, Error, Format, Pixels};
pub use scale::Scale;
pub use scene::Scene;
pub use series::Series;
pub use style::{Dash, Marker};
pub use theme::Theme;
pub use ticks::TickFormat;
