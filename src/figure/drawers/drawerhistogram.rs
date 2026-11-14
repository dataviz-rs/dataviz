use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    figuretypes::histogram::Histogram,
    render::{pixel, svg, AxesSpec, Ticks},
};

use super::drawer::Drawer;
use std::any::Any;

impl Histogram {
    fn spec(&self) -> AxesSpec<'_> {
        AxesSpec {
            title: &self.title,
            x_label: &self.x_label,
            y_label: &self.y_label,
            config: &self.config,
            bounds: self.bounds(),
            x_ticks: Ticks::Linear(self.config.num_axis_ticks),
            y_ticks: Ticks::Linear(self.config.num_axis_ticks),
            legend: Vec::new(),
        }
    }

    /// `(left, right, count)` of every bin, in data coordinates.
    fn bars(&self) -> impl Iterator<Item = (f64, f64, f64)> + '_ {
        let (start, width) = self.display_bins();
        self.bin_counts.iter().enumerate().map(move |(i, &count)| {
            let left = start + i as f64 * width;
            (left, left + width, count)
        })
    }
}

impl Drawer for Histogram {
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas) {
        let spec = self.spec();
        let m = svg::begin(svg_canvas, &spec);
        let fill = svg::rgb(self.color);
        let edge = svg::rgb(self.config.color_axis);
        for (left, right, count) in self.bars() {
            if count <= 0.0 {
                continue;
            }
            let (x0, y0) = m.point((left, count));
            let (x1, y1) = m.point((right, 0.0));
            svg_canvas.draw_rect(x0, y0, x1 - x0, y1 - y0, &fill, &edge, 1.0, 1.0);
        }
        svg::finish(svg_canvas, &spec, &m);
    }

    fn draw(&mut self, canvas: &mut PixelCanvas) {
        let spec = self.spec();
        let m = pixel::begin(canvas, &spec);
        for (left, right, count) in self.bars() {
            if count <= 0.0 {
                continue;
            }
            let (x0, y0) = m.point((left, count));
            let (x1, y1) = m.point((right, 0.0));
            pixel::fill_rect(canvas, x0, y0, x1, y1, self.color);
            pixel::stroke_rect(canvas, x0, y0, x1, y1, self.config.color_axis);
        }
        pixel::finish(canvas, &spec, &m);
    }

    fn draw_legend(&self, _canvas: &mut PixelCanvas) {
        // Histogram does not have a legend
    }

    fn as_any(&mut self) -> &mut (dyn Any + 'static) {
        self as &mut dyn Any
    }

    fn get_figure_config(&self) -> &FigureConfig {
        &self.config
    }
}
