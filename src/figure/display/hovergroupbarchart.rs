use ab_glyph::FontRef;

use crate::figure::{
    canvas::pixelcanvas::PixelCanvas,
    figuretypes::groupbarchart::GroupBarChart,
    render::{pixel, ticks, Mapper},
    utilities::orientation::Orientation,
};

use super::hover::Hover;

impl GroupBarChart {
    fn hover_mapper(&self, canvas: &PixelCanvas) -> Mapper {
        Mapper::for_canvas(
            canvas.width,
            canvas.height,
            canvas.margin,
            self.bounds_for(&self.orientation),
        )
    }

    /// Canvas position of the end of a bar (its value).
    fn bar_tip(&self, m: &Mapper, start: f64, end: f64, value: f64) -> (f64, f64) {
        let mid = (start + end) / 2.0;
        match self.orientation {
            Orientation::Vertical => m.point((mid, value)),
            Orientation::Horizontal => m.point((value, mid)),
        }
    }
}

impl Hover for GroupBarChart {
    /// Returns `((x, value), value)` of the bar closest to the mouse (the bar
    /// under the mouse, or the nearest bar tip).
    fn find_closest_point(
        &self,
        mouse_x: u32,
        mouse_y: u32,
        canvas: &PixelCanvas,
    ) -> Option<((f64, f64), f64)> {
        let m = self.hover_mapper(canvas);
        let (mx, my) = (mouse_x as f64, mouse_y as f64);
        self.bars()
            .into_iter()
            .map(|bar| {
                let (p0, p1) = bar.corners(&self.orientation);
                let (x0, y0) = m.point(p0);
                let (x1, y1) = m.point(p1);
                let inside =
                    mx >= x0.min(x1) && mx <= x0.max(x1) && my >= y0.min(y1) && my <= y0.max(y1);
                let (tx, ty) = self.bar_tip(&m, bar.start, bar.end, bar.value);
                let d = if inside {
                    0.0
                } else {
                    (tx - mx).hypot(ty - my)
                };
                (bar, d)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(bar, _)| ((bar.category, bar.value), bar.value))
    }

    /// Maps `(x, value)` of a bar group to the canvas: the centre of the
    /// group along the category axis, `value` along the value axis.
    fn to_canvas_coordinates(&self, x: f64, y: f64, canvas: &PixelCanvas) -> (u32, u32) {
        let m = self.hover_mapper(canvas);
        let slot = self
            .categories()
            .iter()
            .position(|&c| c == x)
            .map_or(0.5, |i| i as f64 + 0.5);
        pixel::to_u32(canvas, self.bar_tip(&m, slot, slot, y))
    }

    fn get_font<'a>(&self, font_data: &'a [u8]) -> FontRef<'a> {
        FontRef::try_from_slice(font_data).expect("invalid font data")
    }

    fn handle_hover(&self, mouse_x: u32, mouse_y: u32, canvas: &PixelCanvas) -> Option<Vec<u32>> {
        let ((category, value), _) = self.find_closest_point(mouse_x, mouse_y, canvas)?;
        let m = self.hover_mapper(canvas);
        let bar = self
            .bars()
            .into_iter()
            .find(|b| b.category == category && b.value == value)?;
        let anchor = self.bar_tip(&m, bar.start, bar.end, bar.value);
        let text = format!(
            "{}: {} = {}",
            ticks::format_category(category),
            bar.label,
            ticks::format_category(value)
        );
        Some(pixel::tooltip(
            canvas,
            &self.config,
            Some(anchor),
            (mouse_x, mouse_y),
            &text,
        ))
    }
}
