use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    figuretypes::quadrant1graph::Quadrant1Graph,
    render::{pixel, AxesSpec},
    utilities::scatterdottype::ScatterDotType,
};

use super::{
    drawer::Drawer,
    drawercartesiangraph::{legend_entries, line_chart_pixel, line_chart_spec, line_chart_svg},
};
use std::any::Any;

impl Quadrant1Graph {
    fn spec(&self) -> AxesSpec<'_> {
        line_chart_spec(
            &self.title,
            &self.x_label,
            &self.y_label,
            &self.config,
            self.bounds(),
            &self.datasets,
        )
    }

    /// Like [`Drawer::draw`], but also draws `marker` (in the dataset colour)
    /// at every data point.
    ///
    /// # Example
    /// ```
    /// use dataviz::figure::{
    ///     canvas::pixelcanvas::PixelCanvas, configuration::figureconfig::FigureConfig,
    ///     datasets::cartesiangraphdataset::CartesianDataset,
    ///     figuretypes::quadrant1graph::Quadrant1Graph,
    ///     utilities::{linetype::LineType, scatterdottype::ScatterDotType},
    /// };
    ///
    /// let mut graph = Quadrant1Graph::new("Latency", "t", "ms", FigureConfig::default());
    /// let mut ds = CartesianDataset::new([0, 0, 255], "p99", LineType::Solid);
    /// ds.points = vec![(0.0, 1.0), (1.0, 3.0), (2.0, 2.0)];
    /// graph.add_dataset(ds);
    ///
    /// let mut canvas = PixelCanvas::new(400, 300, [255, 255, 255], 40);
    /// graph.draw_with_markers(&mut canvas, ScatterDotType::Circle(3));
    /// ```
    pub fn draw_with_markers(&mut self, canvas: &mut PixelCanvas, marker: ScatterDotType) {
        self.update_range();
        line_chart_pixel(canvas, &self.spec(), &self.datasets, Some(&marker));
    }

    /// Like [`Drawer::draw_svg`], but also draws `marker` (in the dataset
    /// colour) at every data point.
    pub fn draw_svg_with_markers(&mut self, svg_canvas: &mut SvgCanvas, marker: ScatterDotType) {
        self.update_range();
        line_chart_svg(svg_canvas, &self.spec(), &self.datasets, Some(&marker));
    }
}

impl Drawer for Quadrant1Graph {
    /// Draws the lines only. Data points are no longer marked with
    /// black circles; use [`Quadrant1Graph::draw_svg_with_markers`] to mark
    /// them (in the dataset colour).
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas) {
        self.update_range();
        line_chart_svg(svg_canvas, &self.spec(), &self.datasets, None);
    }

    fn draw(&mut self, canvas: &mut PixelCanvas) {
        self.update_range();
        line_chart_pixel(canvas, &self.spec(), &self.datasets, None);
    }

    fn draw_legend(&self, canvas: &mut PixelCanvas) {
        pixel::legend(
            canvas,
            &self.config,
            &pixel::Fonts::new(&self.config),
            &legend_entries(&self.datasets),
        );
    }

    fn as_any(&mut self) -> &mut (dyn Any + 'static) {
        self as &mut dyn Any
    }

    fn get_figure_config(&self) -> &FigureConfig {
        &self.config
    }
}
