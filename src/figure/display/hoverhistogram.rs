use ab_glyph::FontRef;

use crate::figure::{
    canvas::pixelcanvas::PixelCanvas,
    figuretypes::histogram::Histogram,
    render::{pixel, Mapper},
};

use super::hover::Hover;

impl Histogram {
    fn hover_mapper(&self, canvas: &PixelCanvas) -> Mapper {
        Mapper::for_canvas(canvas.width, canvas.height, canvas.margin, self.bounds())
    }

    /// Index of the bin under canvas x, if any.
    fn bin_at(&self, mouse_x: u32, canvas: &PixelCanvas) -> Option<usize> {
        if self.bins == 0 {
            return None;
        }
        let m = self.hover_mapper(canvas);
        let (start, width) = self.display_bins();
        let b = m.bounds;
        let value =
            b.x_min + (mouse_x as f64 - m.frame.left) / m.frame.width() * (b.x_max - b.x_min);
        let index = ((value - start) / width).floor();
        (index >= 0.0 && index < self.bins as f64).then_some(index as usize)
    }
}

impl Hover for Histogram {
    /// Returns `((bin_start, bin_end), frequency)` of the bin under the
    /// mouse.
    fn find_closest_point(
        &self,
        mouse_x: u32,
        _mouse_y: u32,
        canvas: &PixelCanvas,
    ) -> Option<((f64, f64), f64)> {
        let i = self.bin_at(mouse_x, canvas)?;
        let (start, width) = self.display_bins();
        let left = start + i as f64 * width;
        Some(((left, left + width), *self.bin_counts.get(i)?))
    }

    fn to_canvas_coordinates(&self, x: f64, y: f64, canvas: &PixelCanvas) -> (u32, u32) {
        pixel::to_u32(canvas, self.hover_mapper(canvas).point((x, y)))
    }

    fn get_font<'a>(&self, font_data: &'a [u8]) -> FontRef<'a> {
        FontRef::try_from_slice(font_data).expect("invalid font data")
    }

    fn handle_hover(&self, mouse_x: u32, mouse_y: u32, canvas: &PixelCanvas) -> Option<Vec<u32>> {
        let ((start, end), freq) = self.find_closest_point(mouse_x, mouse_y, canvas)?;
        let anchor = self.hover_mapper(canvas).point(((start + end) / 2.0, freq));
        Some(pixel::tooltip(
            canvas,
            &self.config,
            Some(anchor),
            (mouse_x, mouse_y),
            &format!("[{start:.2}, {end:.2}): {freq}"),
        ))
    }
}
