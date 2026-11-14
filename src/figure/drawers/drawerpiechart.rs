use crate::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    figuretypes::piechart::PieChart,
    render::{
        pixel::{self, HAlign, VAlign},
        svg, LegendEntry,
    },
};

use super::drawer::Drawer;
use std::any::Any;
use std::f64::consts::PI;

impl PieChart {
    fn legend_entries(&self) -> Vec<LegendEntry> {
        self.datasets
            .iter()
            .map(|(label, _, color)| (label.clone(), *color))
            .collect()
    }
}

/// Percentage label of a slice.
fn percent(sweep: f64) -> String {
    format!("{:.1}%", sweep / (2.0 * PI) * 100.0)
}

impl Drawer for PieChart {
    fn draw_svg(&mut self, svg_canvas: &mut SvgCanvas) {
        let cfg = &self.config;
        svg::background(svg_canvas, cfg);
        let ((cx, cy), r) =
            PieChart::geometry(svg_canvas.width, svg_canvas.height, svg_canvas.margin);
        let edge = svg::rgb(cfg.color_axis);

        let slices = self.slices();
        let mut group = format!(r#"<g stroke="{edge}" stroke-width="1">"#);
        for &(_, _, color, start, sweep) in &slices {
            if sweep <= 0.0 {
                continue;
            }
            let fill = svg::rgb(color);
            if sweep >= 2.0 * PI - 1e-9 {
                group.push_str(&format!(
                    r#"<circle cx="{cx:.2}" cy="{cy:.2}" r="{r:.2}" fill="{fill}"/>"#
                ));
                continue;
            }
            let end = start + sweep;
            // Counter-clockwise on screen, like the pixel renderer.
            let (x1, y1) = (cx + r * start.cos(), cy - r * start.sin());
            let (x2, y2) = (cx + r * end.cos(), cy - r * end.sin());
            let large = if sweep > PI { 1 } else { 0 };
            group.push_str(&format!(
                r#"<path d="M {cx:.2} {cy:.2} L {x1:.2} {y1:.2} A {r:.2} {r:.2} 0 {large} 0 {x2:.2} {y2:.2} Z" fill="{fill}"/>"#
            ));
        }
        group.push_str("</g>");
        svg_canvas.elements.push(group);

        let fs = cfg.font_size_label as f64;
        for &(_, _, _, start, sweep) in &slices {
            if sweep <= 0.0 {
                continue;
            }
            let mid = start + sweep / 2.0;
            svg_canvas.draw_text(
                cx + 0.6 * r * mid.cos(),
                cy - 0.6 * r * mid.sin() + fs * 0.35,
                &percent(sweep),
                fs,
                &edge,
            );
        }

        svg::title(svg_canvas, cfg, &self.title);
        svg::legend(svg_canvas, cfg, &self.legend_entries());
    }

    fn draw(&mut self, canvas: &mut PixelCanvas) {
        let cfg = &self.config;
        pixel::background(canvas, cfg);
        let fonts = pixel::Fonts::new(cfg);
        let ((cx, cy), r) = PieChart::geometry(canvas.width, canvas.height, canvas.margin);

        let slices = self.slices();
        for &(_, _, color, start, sweep) in &slices {
            if sweep > 0.0 {
                self.draw_slice(
                    canvas,
                    cx.round() as i32,
                    cy.round() as i32,
                    r.round() as i32,
                    start,
                    start + sweep,
                    color,
                );
            }
        }
        for &(_, _, _, start, sweep) in &slices {
            if sweep <= 0.0 {
                continue;
            }
            let mid = start + sweep / 2.0;
            pixel::text(
                canvas,
                fonts.label.as_ref(),
                cfg.font_size_label,
                cx + 0.6 * r * mid.cos(),
                cy - 0.6 * r * mid.sin(),
                &percent(sweep),
                cfg.color_axis,
                (HAlign::Center, VAlign::Middle),
            );
        }

        pixel::title(canvas, cfg, &fonts, &self.title);
        pixel::legend(canvas, cfg, &fonts, &self.legend_entries());
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
