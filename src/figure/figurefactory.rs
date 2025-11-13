use super::{
    configuration::figureconfig::FigureConfig,
    drawers::drawer::Drawer,
    figuretypes::{
        areachart::AreaChart, cartesiangraph::CartesianGraph, groupbarchart::GroupBarChart,
        histogram::Histogram, piechart::PieChart, scattergraph::ScatterGraph,
    },
};

/// Represents the types of plots that can be created.
pub enum FigureType {
    /// A group vertical bar chart, which uses rectangular bars to represent data.
    GroupBarChartVertical,
    /// A group horizontal bar chart, which uses rectangular bars to represent data.
    GroupBarChartHorizontal,
    /// A Cartesian graph, which plots points and lines on a coordinate grid.
    CartesianGraph,
    /// A pie chart, which represents data as slices of a circle.
    PieChart,
    /// A scatter graph, which plots individual data points.
    ScatterGraph,
    /// An area chart, which represents data with filled areas under lines.
    AreaChart,
    /// A histogram, which shows the frequency distribution of data.
    Histogram,
}

/// Represents the output format for the generated plots.
pub enum OutputFormat {
    /// Output as a `PixelCanvas`, which is a raster-based rendering format.
    PixelCanvas,
    /// Output as an `Svg`, which is a scalable vector graphics format.
    Svg,
}

/// A factory for creating various types of plots.
///
/// This factory simplifies the creation of plot instances by abstracting the
/// initialization process and providing default configurations.
pub struct FigureFactory;

impl FigureFactory {
    /// Creates a plot of the specified type with default settings.
    ///
    /// # Parameters
    /// - `plot_type`: The type of plot to create (`FigureType`).
    ///
    /// # Returns
    /// A boxed `Drawer` object representing the created plot.
    ///
    /// # Example
    /// ```rust,ignore
    /// use dataviz::figure::figurefactory::{FigureFactory, FigureType};
    ///
    /// let bar_chart = FigureFactory::create_figure(FigureType::BarChart);
    /// ```
    pub fn create_figure(plot_type: FigureType) -> Box<dyn Drawer> {
        match plot_type {
            FigureType::GroupBarChartHorizontal => Box::new(GroupBarChart::new(
                "Bar Chart",
                "X Axis",
                "Y Axis",
                super::utilities::orientation::Orientation::Horizontal,
                FigureConfig::default(),
            )),
            FigureType::GroupBarChartVertical => Box::new(GroupBarChart::new(
                "Bar Chart",
                "X Axis",
                "Y Axis",
                super::utilities::orientation::Orientation::Vertical,
                FigureConfig::default(),
            )),
            FigureType::CartesianGraph => Box::new(CartesianGraph::new(
                "Cartesian Graph",
                "X Axis",
                "Y Axis",
                &FigureConfig::default(),
            )),
            FigureType::PieChart => Box::new(PieChart::new("Pie Chart", FigureConfig::default())),
            FigureType::ScatterGraph => Box::new(ScatterGraph::new(
                "Scatter Graph",
                "X Axis",
                "Y Axis",
                FigureConfig::default(),
            )),
            FigureType::AreaChart => Box::new(AreaChart::new(
                "Area Chart",
                "X Axis",
                "Y Axis",
                FigureConfig::default(),
            )),
            FigureType::Histogram => Box::new(Histogram::new(
                "Histogram",
                "Bins",
                "Frequency",
                0,
                [0, 0, 255],
                FigureConfig::default(),
            )),
        }
    }
}
