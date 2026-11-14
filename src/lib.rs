//! # dataviz
//!
//! Pure-Rust plotting and data visualization: 40+ chart types rendered to
//! SVG, PNG, JPEG, WebP, BMP, PDF (`pdf` feature), raw pixels, interactive
//! windows (`window` feature, default), live streaming windows and the
//! terminal. Raster output comes from a built-in CPU rasterizer (single or
//! multi-threaded) or the GPU (`gpu` feature).
//!
//! ```
//! use dataviz::prelude::*;
//!
//! let x: Vec<f64> = (0..200).map(|i| i as f64 / 20.0).collect();
//! let mut fig = Figure::new(900, 500);
//! let ax = fig.axes();
//! ax.title("Signals").x_label("time [s]").y_label("amplitude");
//! ax.line(&x, x.iter().map(|t| t.sin())).label("sin");
//! ax.line(&x, x.iter().map(|t| t.cos())).label("cos").dash(Dash::Dashed);
//! let svg = fig.to_svg();
//! assert!(svg.starts_with("<svg"));
//! // fig.save("signals.png")?;   // .svg .png .jpg .webp .bmp (.pdf with `pdf`)
//! // fig.show("Signals")?;       // interactive window
//! ```
//!
//! Start with [`plot::Figure`] and [`plot::Axes`]; every chart type is a
//! method on `Axes` returning its series for further styling (see
//! [`plot::series`]). `use dataviz::prelude::*;` imports the common types.

pub mod plot;

/// Everything needed for the [`plot`] API: `use dataviz::prelude::*;`
pub mod prelude {
    pub use crate::plot::a11y::Cvd;
    pub use crate::plot::anim::Recorder;
    pub use crate::plot::series::bar::{BarMode, LabelPosition};
    pub use crate::plot::series::line::Step;
    pub use crate::plot::time::{date, datetime, parse_datetime};
    pub use crate::plot::{
        Axes, AxesKind, Axis, Backend, Color, Colormap, Dash, Error, Figure, Format,
        LegendPosition, Marker, Palette, Pixels, Scale, Series, Theme, TickFormat, TickSpec,
    };
}

// Compile and run the code examples of the README as doctests.
#[cfg(all(doctest, feature = "window"))]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
