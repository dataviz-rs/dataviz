use ab_glyph::FontRef;

use crate::figure::{
    canvas::pixelcanvas::PixelCanvas,
    figuretypes::quadrant1graph::Quadrant1Graph,
    render::{pixel, Mapper},
};

use super::hover::Hover;

impl Quadrant1Graph {
    fn hover_mapper(&self, canvas: &PixelCanvas) -> Mapper {
        Mapper::for_canvas(canvas.width, canvas.height, canvas.margin, self.bounds())
    }
}

impl Hover for Quadrant1Graph {
    /// Returns the data point closest to the mouse (in canvas distance) and
    /// that distance in pixels.
    fn find_closest_point(
        &self,
        mouse_x: u32,
        mouse_y: u32,
        canvas: &PixelCanvas,
    ) -> Option<((f64, f64), f64)> {
        let m = self.hover_mapper(canvas);
        pixel::nearest(
            &m,
            (mouse_x, mouse_y),
            self.datasets.iter().flat_map(|d| d.points.iter()),
        )
    }

    fn to_canvas_coordinates(&self, x: f64, y: f64, canvas: &PixelCanvas) -> (u32, u32) {
        pixel::to_u32(canvas, self.hover_mapper(canvas).point((x, y)))
    }

    fn get_font<'a>(&self, font_data: &'a [u8]) -> FontRef<'a> {
        FontRef::try_from_slice(font_data).expect("invalid font data")
    }

    fn handle_hover(&self, mouse_x: u32, mouse_y: u32, canvas: &PixelCanvas) -> Option<Vec<u32>> {
        let ((x, y), _) = self.find_closest_point(mouse_x, mouse_y, canvas)?;
        let anchor = self.hover_mapper(canvas).point((x, y));
        Some(pixel::tooltip(
            canvas,
            &self.config,
            Some(anchor),
            (mouse_x, mouse_y),
            &format!("({x:.2}, {y:.2})"),
        ))
    }
}
