//! # DataViz Library
//!
//! **DataViz** is a versatile and modular Rust library for creating and managing a wide variety of charts and graphs.
//! Designed with flexibility and extensibility in mind, DataViz provides a powerful API for data visualization,
//! supporting different chart types, customization options, and interactive features.
//!
//! ## Features
//! - **Comprehensive Chart Types**: Support for bar charts, scatter graphs, pie charts, histograms, area charts, and Cartesian graphs.
//! - **Data Management**: Modular datasets for structured and reusable data handling.
//! - **Customizable Renderers**: Drawers for rendering charts on pixel-based or vector-based canvases.
//! - **Scalable Configuration**: Centralized figure configuration for appearance settings, including fonts, colors, and grid styles.
//! - **Interactive Capabilities**: Hover-based interactivity to enhance user experience with dynamic updates and tooltips.
//!
//! ## Modules
//! DataViz is organized into a set of modules that define the core components of the library:
//!
//! ### [`figure`]
//! The main module that houses all types of charts and graphs. It includes:
//! - **Chart Types**:
//!   - [`areachart`](crate::figure::figuretypes::areachart): Create area charts for visualizing data trends.
//!   - [`cartesiangraph`](crate::figure::figuretypes::cartesiangraph): Cartesian graphs for mathematical and data plotting.
//!   - [`groupbarchart`](crate::figure::figuretypes::groupbarchart): Grouped bar charts for comparative data visualization.
//!   - [`histogram`](crate::figure::figuretypes::histogram): Histograms for frequency distribution analysis.
//!   - [`piechart`](crate::figure::figuretypes::piechart): Pie charts for proportional data representation.
//!   - [`quadrant1graph`](crate::figure::figuretypes::quadrant1graph): Graphs restricted to the first quadrant.
//!   - [`scattergraph`](crate::figure::figuretypes::scattergraph): Scatter plots for individual data point visualization.
//!
//! ## Datasets
//! Defines reusable and modular datasets for different chart types, enabling structured data representation. Includes:
//! - [`areachartdataset`](crate::figure::datasets::areachartdataset)
//! - [`bardataset`](crate::figure::datasets::bardataset)
//! - [`cartesiangraphdataset`](crate::figure::datasets::cartesiangraphdataset)
//! - [`scattergraphdataset`](crate::figure::datasets::scattergraphdataset)
//! - [`dataset`](crate::figure::datasets::dataset): Common dataset traits for unifying data operations.
//!
//! ## Drawers
//! Provides customizable renderers for each chart type. Drawers allow charts to be rendered on various canvases,
//! such as pixel-based or vector-based canvases. Includes:
//! - [`drawer`](crate::figure::drawers::drawer): Core drawing functionality.
//! - Specialized drawers for specific chart types like:
//!   - [`drawerareachart`](crate::figure::drawers::drawerareachart)
//!   - [`drawerbarchart`](crate::figure::drawers::drawerbarchart)
//!   - [`drawerpiechart`](crate::figure::drawers::drawerpiechart), and more.
//!
//! ## Utilities
//! Utility modules for managing chart attributes and behaviors. Includes:
//! - [`axistype`](crate::figure::utilities::axistype): Enum for axis types (X or Y).
//! - [`linetype`](crate::figure::utilities::linetype): Styles for chart lines (solid, dashed, dotted).
//! - [`orientation`](crate::figure::utilities::orientation): Orientation handling (horizontal or vertical).
//! - [`scatterdottype`](crate::figure::utilities::scatterdottype): Dot styles for scatter plots (circle, square, triangle, etc.).
//!
//! ## Configuration
//! Centralized configuration for charts, providing a single source for appearance settings. Includes:
//! - [`figureconfig`](crate::figure::configuration::figureconfig): Control colors, fonts, grid settings, and more.
//!
//! ## Canvas
//! Abstractions for rendering surfaces, including:
//! - [`pixelcanvas`](crate::figure::canvas::pixelcanvas): Raster-based rendering for charts.
//! - [`svgcanvas`](crate::figure::canvas::svgcanvas): Scalable vector graphics rendering for high-quality outputs.
//!
//! ## Display
//! Modules for interactivity and display management. Includes:
//! - Hover functionality for charts like:
//!   - [`hovercartesian`](crate::figure::display::hovercartesian)
//!   - [`hoverpiechart`](crate::figure::display::hoverpiechart), and more.
//! - [`winop`](crate::figure::display::winop): Manage window operations for interactive displays.
//!
//! ## Getting Started
//! Every chart implements [`Drawer`](crate::figure::drawers::drawer::Drawer):
//! `draw` renders it on a [`PixelCanvas`](crate::figure::canvas::pixelcanvas::PixelCanvas)
//! (saved as PNG, JPEG, …) and `draw_svg` on an
//! [`SvgCanvas`](crate::figure::canvas::svgcanvas::SvgCanvas). Both outputs
//! use the same layout and honour every [`FigureConfig`] setting (tick and
//! grid counts, colours, font sizes).
//!
//! ```
//! use dataviz::figure::{
//!     canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
//!     configuration::figureconfig::FigureConfig,
//!     datasets::{cartesiangraphdataset::CartesianDataset, dataset::Dataset},
//!     drawers::drawer::Drawer,
//!     figuretypes::quadrant1graph::Quadrant1Graph,
//!     utilities::{linetype::LineType, scatterdottype::ScatterDotType},
//! };
//!
//! let config = FigureConfig {
//!     num_axis_ticks: 5,
//!     ..FigureConfig::default() // no font path needed: a system font is used
//! };
//! let mut graph = Quadrant1Graph::new("Throughput", "Seconds", "MB", config);
//!
//! let mut dataset = CartesianDataset::new([30, 144, 255], "upload", LineType::Dashed(8));
//! for t in 0..=10 {
//!     dataset.add_point((t as f64, (t * t) as f64));
//! }
//! graph.add_dataset(dataset);
//!
//! let mut png = PixelCanvas::new(800, 600, [255, 255, 255], 80);
//! graph.draw(&mut png);
//! // png.save_as_image("throughput.png");
//!
//! let mut svg = SvgCanvas::new(800, 600, "white", 80);
//! graph.draw_svg_with_markers(&mut svg, ScatterDotType::Circle(3)); // or draw_svg()
//! assert!(svg.get_svg_as_text().contains("<polyline"));
//! // svg.save("throughput.svg")?;
//! ```
//!
//! ## Fonts
//! PNG text is rendered with the TrueType/OpenType files in
//! [`FigureConfig::font_label`] and [`FigureConfig::font_title`]. If they are
//! not set or cannot be read, an installed sans-serif font (DejaVu Sans,
//! Liberation Sans, Arial, …) is used; without any font, text is skipped.
//! SVG text uses the generic `sans-serif` family of the viewer.
//!
//! [`FigureConfig`]: crate::figure::configuration::figureconfig::FigureConfig
//! [`FigureConfig::font_label`]: crate::figure::configuration::figureconfig::FigureConfig::font_label
//! [`FigureConfig::font_title`]: crate::figure::configuration::figureconfig::FigureConfig::font_title
//!
//! ## License
//! DataViz is open-source and licensed under the MIT license. Contributions are welcome!

pub mod figure {
    pub mod figuretypes {
        pub mod areachart;
        pub mod cartesiangraph;
        pub mod groupbarchart;
        pub mod histogram;
        pub mod piechart;
        pub mod quadrant1graph;
        pub mod scattergraph;
    }

    pub mod datasets {
        pub mod areachartdataset;
        pub mod bardataset;
        pub mod cartesiangraphdataset;
        pub mod dataset;
        pub mod scattergraphdataset;
    }

    pub mod drawers {
        pub mod drawer;
        pub mod drawerareachart;
        pub mod drawerbarchart;
        pub mod drawercartesiangraph;
        pub mod drawerhistogram;
        pub mod drawerpiechart;
        pub mod drawerquadrant1graph;
        pub mod drawerscattergraph;
    }

    pub mod utilities {
        pub mod axistype;
        pub mod linetype;
        pub mod orientation;
        pub mod scatterdottype;
    }

    pub mod configuration {
        pub mod figureconfig;
    }

    pub mod canvas {
        pub mod pixelcanvas;
        pub mod svgcanvas;
    }

    pub mod display {
        pub mod hover;
        pub mod hoverareachart;
        pub mod hovercartesian;
        pub mod hovergroupbarchart;
        pub mod hoverhistogram;
        pub mod hoverpiechart;
        pub mod hoverquadrant1graph;
        pub mod hoverscatterchart;
        pub mod winop;
    }

    pub mod figurefactory;

    pub(crate) mod render;
}

pub mod plot;

/// Everything needed for the [`plot`] API: `use dataviz::prelude::*;`
pub mod prelude {
    pub use crate::plot::series::bar::{BarMode, LabelPosition};
    pub use crate::plot::series::line::Step;
    pub use crate::plot::time::{date, datetime, parse_datetime};
    pub use crate::plot::{
        Axes, AxesKind, Axis, Color, Colormap, Dash, Error, Figure, Format, LegendPosition, Marker,
        Palette, Scale, Series, Theme, TickFormat, TickSpec,
    };
}

// Compile and run the code examples of the README as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
