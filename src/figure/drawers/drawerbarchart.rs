use super::drawer::Drawer;
use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    figuretypes::groupbarchart::GroupBarChart,
    render::{pixel, svg, LegendEntry},
    utilities::orientation::Orientation,
};
use std::any::Any;

impl Drawer for GroupBarChart {
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas) {
        let spec = self.spec(&self.orientation);
        let m = svg::begin(svg_canvas, &spec);
        let edge = svg::rgb(self.config.color_axis);
        for bar in self.bars() {
            let (p0, p1) = bar.corners(&self.orientation);
            let (x0, y0) = m.point(p0);
            let (x1, y1) = m.point(p1);
            svg_canvas.draw_rect(
                x0,
                y0,
                x1 - x0,
                y1 - y0,
                &svg::rgb(bar.color),
                &edge,
                1.0,
                1.0,
            );
        }
        svg::finish(svg_canvas, &spec, &m);
    }

    fn draw(&mut self, canvas: &mut PixelCanvas) {
        match self.orientation {
            Orientation::Vertical => self.draw_vertical(canvas),
            Orientation::Horizontal => self.draw_horizontal(canvas),
        }
    }

    fn draw_legend(&self, canvas: &mut PixelCanvas) {
        let entries: Vec<LegendEntry> = self
            .datasets
            .iter()
            .map(|d| (d.label.clone(), d.color))
            .collect();
        pixel::legend(
            canvas,
            &self.config,
            &pixel::Fonts::new(&self.config),
            &entries,
        );
    }

    fn as_any(&mut self) -> &mut (dyn Any + 'static) {
        self as &mut dyn Any
    }

    fn get_figure_config(&self) -> &FigureConfig {
        &self.config
    }
}
