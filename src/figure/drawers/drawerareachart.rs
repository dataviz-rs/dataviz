use super::drawer::Drawer;
use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    figuretypes::areachart::{fill_area, AreaChart},
    render::{pixel, svg, AxesSpec, LegendEntry, Ticks},
    utilities::linetype::LineType,
};
use std::any::Any;

impl AreaChart {
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

    /// Finite points of a dataset, sorted by x.
    fn sorted(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
        let mut p: Vec<_> = points
            .iter()
            .copied()
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .collect();
        p.sort_by(|a, b| a.0.total_cmp(&b.0));
        p
    }
}

impl Drawer for AreaChart {
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas) {
        let spec = self.spec();
        let m = svg::begin(svg_canvas, &spec);
        let base = m.x_axis_y();
        for d in &self.datasets {
            let pts: Vec<_> = Self::sorted(&d.points)
                .into_iter()
                .map(|p| m.point(p))
                .collect();
            let (Some(first), Some(last)) = (pts.first(), pts.last()) else {
                continue;
            };
            let mut path = format!("M {:.2},{base:.2}", first.0);
            for (x, y) in &pts {
                path.push_str(&format!(" L {x:.2},{y:.2}"));
            }
            path.push_str(&format!(" L {:.2},{base:.2} Z", last.0));
            let color = svg::rgb(d.color);
            let alpha = if d.alpha.is_nan() {
                0.0
            } else {
                d.alpha.clamp(0.0, 1.0)
            };
            svg_canvas.elements.push(format!(
                r#"<path d="{path}" fill="{color}" fill-opacity="{alpha:.3}" stroke="{color}" stroke-width="1"/>"#
            ));
        }
        svg::finish(svg_canvas, &spec, &m);
    }

    fn draw(&mut self, canvas: &mut PixelCanvas) {
        let spec = self.spec();
        let m = pixel::begin(canvas, &spec);
        let base = m.x_axis_y();
        for d in &self.datasets {
            let pts: Vec<_> = Self::sorted(&d.points)
                .into_iter()
                .map(|p| m.point(p))
                .collect();
            fill_area(canvas, &pts, base, d.color, d.alpha);
            pixel::polyline(canvas, &pts, d.color, &LineType::Solid);
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
