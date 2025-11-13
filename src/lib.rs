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
//! Here's a quick example of creating and rendering a pie chart:
//!
//! ```rust,ignore
//! use dataviz::figure::figuretypes::piechart::PieChart;
//! use dataviz::figure::configuration::figureconfig::FigureConfig;
//!
//! let mut pie_chart = PieChart::new("Market Share", FigureConfig::default());
//! pie_chart.add_slice("Product A", 40.0, [255, 0, 0]);
//! pie_chart.add_slice("Product B", 30.0, [0, 255, 0]);
//! pie_chart.add_slice("Product C", 30.0, [0, 0, 255]);
//!
//! // Render the pie chart
//! pie_chart.draw();
//! ```
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
}
