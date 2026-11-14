use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    figuretypes::scattergraph::ScatterGraph,
    render::{pixel, svg, AxesSpec, LegendEntry, Ticks},
};

use super::drawer::Drawer;
use std::any::Any;

impl ScatterGraph {
    fn legend_entries(&self) -> Vec<LegendEntry> {
        self.datasets
            .iter()
            .map(|d| (d.label.clone(), d.color))
            .collect()
    }

    fn spec(&self) -> AxesSpec<'_> {
        AxesSpec {
            title: &self.title,
            x_label: &self.x_label,
            y_label: &self.y_label,
            config: &self.config,
            bounds: self.bounds(),
            x_ticks: Ticks::Linear(self.config.num_axis_ticks),
            y_ticks: Ticks::Linear(self.config.num_axis_ticks),
            legend: self.legend_entries(),
        }
    }
}

impl Drawer for ScatterGraph {
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas) {
        let spec = self.spec();
        let m = svg::begin(svg_canvas, &spec);
        for d in &self.datasets {
            for &p in &d.points {
                let (x, y) = m.point(p);
                svg::marker(svg_canvas, x, y, &d.dot_type, d.color);
            }
        }
        svg::finish(svg_canvas, &spec, &m);
    }

    fn draw(&mut self, canvas: &mut PixelCanvas) {
        let spec = self.spec();
        let m = pixel::begin(canvas, &spec);
        for d in &self.datasets {
            for &p in &d.points {
                let (x, y) = m.point(p);
                pixel::marker(canvas, x, y, &d.dot_type, d.color);
            }
        }
        pixel::finish(canvas, &spec, &m);
    }

    fn draw_legend(&self, canvas: &mut PixelCanvas) {
        pixel::legend(
            canvas,
            &self.config,
            &pixel::Fonts::new(&self.config),
            &self.legend_entries(),
        );
    }

    fn as_any(&mut self) -> &mut (dyn Any + 'static) {
        self as &mut dyn Any
    }

    fn get_figure_config(&self) -> &FigureConfig {
        &self.config
    }
}
