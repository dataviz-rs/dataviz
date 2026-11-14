use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    datasets::cartesiangraphdataset::CartesianDataset,
    figuretypes::cartesiangraph::CartesianGraph,
    render::{pixel, svg, AxesSpec, Bounds, LegendEntry, Ticks},
    utilities::scatterdottype::ScatterDotType,
};

use super::drawer::Drawer;
use std::any::Any;

pub(crate) fn legend_entries(datasets: &[CartesianDataset]) -> Vec<LegendEntry> {
    datasets
        .iter()
        .map(|d| (d.label.clone(), d.color))
        .collect()
}

pub(crate) fn line_chart_spec<'a>(
    title: &'a str,
    x_label: &'a str,
    y_label: &'a str,
    config: &'a FigureConfig,
    bounds: Bounds,
    datasets: &[CartesianDataset],
) -> AxesSpec<'a> {
    AxesSpec {
        title,
        x_label,
        y_label,
        config,
        bounds,
        x_ticks: Ticks::Linear(config.num_axis_ticks),
        y_ticks: Ticks::Linear(config.num_axis_ticks),
        legend: legend_entries(datasets),
    }
}

/// Draws line datasets (and optional markers) on an SVG canvas.
pub(crate) fn line_chart_svg(
    canvas: &mut SvgCanvas,
    spec: &AxesSpec,
    datasets: &[CartesianDataset],
    markers: Option<&ScatterDotType>,
) {
    let m = svg::begin(canvas, spec);
    for d in datasets {
        let points: Vec<_> = d.points.iter().map(|&p| m.point(p)).collect();
        canvas.draw_polyline_rgb_styled(&points, d.color, 2.0, d.line_type.clone());
        if let Some(dot) = markers {
            for &(x, y) in &points {
                svg::marker(canvas, x, y, dot, d.color);
            }
        }
    }
    svg::finish(canvas, spec, &m);
}

/// Draws line datasets (and optional markers) on a pixel canvas.
pub(crate) fn line_chart_pixel(
    canvas: &mut PixelCanvas,
    spec: &AxesSpec,
    datasets: &[CartesianDataset],
    markers: Option<&ScatterDotType>,
) {
    let m = pixel::begin(canvas, spec);
    for d in datasets {
        let points: Vec<_> = d.points.iter().map(|&p| m.point(p)).collect();
        pixel::polyline(canvas, &points, d.color, &d.line_type);
        if let Some(dot) = markers {
            for &(x, y) in &points {
                pixel::marker(canvas, x, y, dot, d.color);
            }
        }
    }
    pixel::finish(canvas, spec, &m);
}

impl CartesianGraph {
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

impl Drawer for CartesianGraph {
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
