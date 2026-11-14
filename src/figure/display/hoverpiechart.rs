use ab_glyph::FontRef;

use crate::figure::{
    canvas::pixelcanvas::PixelCanvas, figuretypes::piechart::PieChart, render::pixel,
};

use super::hover::Hover;

impl PieChart {
    /// Label, value and label anchor of the slice under the mouse.
    fn slice_at(
        &self,
        mouse_x: u32,
        mouse_y: u32,
        canvas: &PixelCanvas,
    ) -> Option<(&str, f64, (f64, f64))> {
        let ((cx, cy), r) = PieChart::geometry(canvas.width, canvas.height, canvas.margin);
        let (dx, dy) = (mouse_x as f64 - cx, cy - mouse_y as f64);
        if dx.hypot(dy) > r {
            return None;
        }
        let mut angle = dy.atan2(dx);
        if angle < 0.0 {
            angle += 2.0 * std::f64::consts::PI;
        }
        self.slices()
            .into_iter()
            .find(|&(_, _, _, start, sweep)| sweep > 0.0 && angle >= start && angle < start + sweep)
            .map(|(label, value, _, start, sweep)| {
                let mid = start + sweep / 2.0;
                (
                    label,
                    value,
                    (cx + 0.6 * r * mid.cos(), cy - 0.6 * r * mid.sin()),
                )
            })
    }
}

impl Hover for PieChart {
    /// Returns the canvas point in the middle of the slice under the mouse
    /// and that slice's value.
    fn find_closest_point(
        &self,
        mouse_x: u32,
        mouse_y: u32,
        canvas: &PixelCanvas,
    ) -> Option<((f64, f64), f64)> {
        self.slice_at(mouse_x, mouse_y, canvas)
            .map(|(_, value, anchor)| (anchor, value))
    }

    /// `(x, y)` are already canvas coordinates for a pie chart.
    fn to_canvas_coordinates(&self, x: f64, y: f64, canvas: &PixelCanvas) -> (u32, u32) {
        pixel::to_u32(canvas, (x, y))
    }

    fn get_font<'a>(&self, font_data: &'a [u8]) -> FontRef<'a> {
        FontRef::try_from_slice(font_data).expect("invalid font data")
    }

    fn handle_hover(&self, mouse_x: u32, mouse_y: u32, canvas: &PixelCanvas) -> Option<Vec<u32>> {
        let (label, value, anchor) = self.slice_at(mouse_x, mouse_y, canvas)?;
        Some(pixel::tooltip(
            canvas,
            &self.config,
            Some(anchor),
            (mouse_x, mouse_y),
            &format!("{label}: {value:.2}"),
        ))
    }
}
