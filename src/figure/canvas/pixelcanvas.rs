use ab_glyph::{Font, FontRef, PxScale};
use image::{ImageBuffer, Rgb};
use imageproc::drawing::{draw_text_mut, text_size};

use crate::figure::utilities::linetype::LineType;

/// A structure representing a pixel-based drawing canvas.
///
/// The buffer is stored row by row, three bytes (R, G, B) per pixel.
/// All drawing operations clip to the canvas: coordinates outside of it are
/// ignored instead of wrapping around or panicking.
pub struct PixelCanvas {
    /// Width of the canvas in pixels.
    pub width: u32,
    /// Height of the canvas in pixels.
    pub height: u32,
    /// Background color of the canvas (RGB format). Used by [`PixelCanvas::new`]
    /// and [`PixelCanvas::clear`]; the area inside the margins of a chart is
    /// painted with `FigureConfig::color_background` by the chart itself.
    pub background_color: [u8; 3],
    /// Buffer storing pixel data as a linear array.
    pub buffer: Vec<u8>,
    /// Margin around the canvas (in pixels).
    pub margin: u32,
}

impl PixelCanvas {
    /// Creates a new `PixelCanvas` with the specified dimensions, background color, and margin.
    ///
    /// # Parameters
    /// - `width`: The width of the canvas in pixels.
    /// - `height`: The height of the canvas in pixels.
    /// - `background_color`: The RGB color of the canvas background.
    /// - `margin`: Margin size in pixels.
    ///
    /// # Returns
    /// A new `PixelCanvas` instance filled with `background_color`.
    pub fn new(width: u32, height: u32, background_color: [u8; 3], margin: u32) -> Self {
        let pixels = width as usize * height as usize;
        let mut buffer = Vec::with_capacity(pixels * 3);
        for _ in 0..pixels {
            buffer.extend_from_slice(&background_color);
        }
        Self {
            width,
            height,
            background_color,
            buffer,
            margin,
        }
    }

    /// Clears the canvas by filling it with the background color.
    ///
    /// If `width`/`height` were changed after construction, the buffer is
    /// resized accordingly.
    pub fn clear(&mut self) {
        let len = self.width as usize * self.height as usize * 3;
        self.buffer.resize(len, 0);
        for px in self.buffer.chunks_exact_mut(3) {
            px.copy_from_slice(&self.background_color);
        }
    }

    /// Byte offset of pixel `(x, y)`, or `None` if it lies outside the canvas.
    #[inline]
    fn index(&self, x: i64, y: i64) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 3;
        (i + 2 < self.buffer.len()).then_some(i)
    }

    /// Sets a pixel given signed coordinates; out-of-canvas pixels are ignored.
    #[inline]
    pub(crate) fn put(&mut self, x: i64, y: i64, color: [u8; 3]) {
        if let Some(i) = self.index(x, y) {
            self.buffer[i..i + 3].copy_from_slice(&color);
        }
    }

    /// Blends a pixel given signed coordinates; out-of-canvas pixels are ignored.
    #[inline]
    pub(crate) fn blend(&mut self, x: i64, y: i64, color: [u8; 3], alpha: f64) {
        if let Some(i) = self.index(x, y) {
            let a = if alpha.is_nan() {
                0.0
            } else {
                alpha.clamp(0.0, 1.0)
            };
            for (dst, &src) in self.buffer[i..i + 3].iter_mut().zip(&color) {
                *dst = (src as f64 * a + *dst as f64 * (1.0 - a)).round() as u8;
            }
        }
    }

    /// Returns the color of a pixel, or `None` outside the canvas.
    pub fn get_pixel(&self, x: u32, y: u32) -> Option<[u8; 3]> {
        self.index(x as i64, y as i64)
            .map(|i| [self.buffer[i], self.buffer[i + 1], self.buffer[i + 2]])
    }

    /// Draws a single pixel at the specified coordinates with the given color.
    /// Pixels outside the canvas are ignored.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate of the pixel.
    /// - `y`: The y-coordinate of the pixel.
    /// - `color`: The RGB color of the pixel.
    pub fn draw_pixel(&mut self, x: u32, y: u32, color: [u8; 3]) {
        self.put(x as i64, y as i64, color);
    }

    /// Blends a pixel with the specified color and alpha value.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate of the pixel.
    /// - `y`: The y-coordinate of the pixel.
    /// - `color`: The RGB color to blend.
    /// - `alpha`: The opacity of `color` (0.0 to 1.0, clamped).
    pub fn blend_pixel(&mut self, x: u32, y: u32, color: [u8; 3], alpha: f64) {
        self.blend(x as i64, y as i64, color, alpha);
    }

    /// Draws a horizontal line at the specified y-coordinate, between the
    /// left and right margins.
    ///
    /// # Parameters
    /// - `y`: The y-coordinate of the line.
    /// - `color`: The RGB color of the line.
    pub fn draw_horizontal_line(&mut self, y: u32, color: [u8; 3]) {
        for x in self.margin..self.width.saturating_sub(self.margin) {
            self.draw_pixel(x, y, color);
        }
    }

    /// Draws a vertical line at the specified x-coordinate, between the top
    /// and bottom margins.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate of the line.
    /// - `color`: The RGB color of the line.
    pub fn draw_vertical_line(&mut self, x: u32, color: [u8; 3]) {
        for y in self.margin..self.height.saturating_sub(self.margin) {
            self.draw_pixel(x, y, color);
        }
    }

    /// Draws a grid inside the margins.
    ///
    /// Note that `grid_size` is the *spacing* between grid lines in pixels.
    /// The charts draw their grid from `FigureConfig::num_grid_horizontal` /
    /// `num_grid_vertical`, which are line *counts*.
    ///
    /// # Parameters
    /// - `grid_size`: Spacing of the vertical (`[0]`) and horizontal (`[1]`)
    ///   grid lines in pixels. A spacing of `0` draws no lines in that direction.
    /// - `color`: The RGB color of the grid lines.
    pub fn draw_grid(&mut self, grid_size: &[usize; 2], color: [u8; 3]) {
        let right = self.width.saturating_sub(self.margin);
        let bottom = self.height.saturating_sub(self.margin);
        if grid_size[0] > 0 {
            for x in (self.margin..=right).step_by(grid_size[0]) {
                self.draw_vertical_line(x, color);
            }
        }
        if grid_size[1] > 0 {
            for y in (self.margin..=bottom).step_by(grid_size[1]) {
                self.draw_horizontal_line(y, color);
            }
        }
    }

    /// Draws text vertically (one character below the other) at the specified position.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate for the text.
    /// - `y`: The y-coordinate for the text.
    /// - `text`: The text content.
    /// - `color`: The RGB color of the text.
    /// - `font`: The font reference for rendering text.
    /// - `scale`: The scaling factor for the font size.
    pub fn draw_text_vertical(
        &mut self,
        x: u32,
        y: u32,
        text: &str,
        color: [u8; 3],
        font: &ab_glyph::FontRef,
        scale: ab_glyph::PxScale,
    ) {
        let mut current_y = y;
        for ch in text.chars() {
            let char_as_str = ch.to_string();
            let (_char_width, char_height) = text_size(scale, font, &char_as_str);
            self.draw_text(x, current_y, &char_as_str, color, font, scale);
            current_y = current_y.saturating_add(char_height + 5);
        }
    }

    /// Draws text with its top-left corner at the specified position.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate for the text.
    /// - `y`: The y-coordinate for the text.
    /// - `text`: The text content.
    /// - `color`: The RGB color of the text.
    /// - `font`: The font reference for rendering text.
    /// - `scale`: The scaling factor for the font size.
    pub fn draw_text(
        &mut self,
        x: u32,
        y: u32,
        text: &str,
        color: [u8; 3],
        font: &FontRef,
        scale: PxScale,
    ) {
        self.draw_text_font(
            x.min(i32::MAX as u32) as i32,
            y.min(i32::MAX as u32) as i32,
            text,
            color,
            font,
            scale,
        );
    }

    /// Draws text with any `ab_glyph` font, directly into the buffer.
    pub(crate) fn draw_text_font<F: Font>(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        color: [u8; 3],
        font: &F,
        scale: PxScale,
    ) {
        let len = self.width as usize * self.height as usize * 3;
        if self.buffer.len() < len || len == 0 {
            return;
        }
        let Some(mut img) = ImageBuffer::<Rgb<u8>, &mut [u8]>::from_raw(
            self.width,
            self.height,
            &mut self.buffer[..len],
        ) else {
            return;
        };
        draw_text_mut(&mut img, Rgb(color), x, y, scale, font, text);
    }

    /// Draws a line with the specified type (solid, dashed, or dotted).
    ///
    /// The line is clipped to the canvas, so any coordinates are accepted.
    ///
    /// # Parameters
    /// - `x1`, `y1`: Coordinates of the start point.
    /// - `x2`, `y2`: Coordinates of the end point.
    /// - `color`: The RGB color of the line.
    /// - `line_type`: The type of line to draw (`LineType`).
    pub fn draw_line(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: [u8; 3],
        line_type: LineType,
    ) {
        let mut pattern = Pattern::new(&line_type);
        self.stroke_segment(
            (x1 as f64, y1 as f64),
            (x2 as f64, y2 as f64),
            color,
            &mut pattern,
            true,
        );
    }

    /// Draws one segment of a (possibly patterned) line. The pattern state is
    /// carried in `pattern` so that consecutive segments of a polyline keep
    /// the dash rhythm instead of restarting it at every data point.
    /// `include_start` is false for every segment but the first of a
    /// polyline, so shared vertices are not counted twice.
    pub(crate) fn stroke_segment(
        &mut self,
        p1: (f64, f64),
        p2: (f64, f64),
        color: [u8; 3],
        pattern: &mut Pattern,
        include_start: bool,
    ) {
        let Some(((x1, y1), (x2, y2))) = clip_segment(
            p1,
            p2,
            (-1.0, -1.0),
            (self.width as f64, self.height as f64),
        ) else {
            return;
        };
        let (x1, y1, x2, y2) = (
            x1.round() as i64,
            y1.round() as i64,
            x2.round() as i64,
            y2.round() as i64,
        );

        let dx = (x2 - x1).abs();
        let dy = -(y2 - y1).abs();
        let sx = if x1 < x2 { 1 } else { -1 };
        let sy = if y1 < y2 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x1, y1);
        let mut first = true;

        loop {
            if (!first || include_start) && pattern.next() {
                self.put(x, y, color);
            }
            first = false;
            if x == x2 && y == y2 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Saves the current canvas as an image file. The format is derived from
    /// the file extension (e.g. `.png`).
    ///
    /// # Parameters
    /// - `file_path`: The path to save the image file.
    ///
    /// # Panics
    /// Panics if the image cannot be saved. Use
    /// [`PixelCanvas::try_save_as_image`] to handle the error instead.
    pub fn save_as_image(&self, file_path: &str) {
        self.try_save_as_image(file_path)
            .expect("Failed to save image");
    }

    /// Saves the current canvas as an image file, returning an error instead
    /// of panicking.
    ///
    /// # Errors
    /// Returns an error if the buffer does not match the canvas size, the
    /// format is not supported, or the file cannot be written.
    pub fn try_save_as_image(&self, file_path: &str) -> std::io::Result<()> {
        use image::RgbImage;

        let img: RgbImage = ImageBuffer::from_raw(self.width, self.height, self.buffer.clone())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "buffer size does not match width * height * 3",
                )
            })?;
        img.save(file_path).map_err(std::io::Error::other)
    }
}

/// On/off state machine for dashed and dotted lines, advanced once per pixel.
#[derive(Clone, Debug)]
pub(crate) struct Pattern {
    on: u32,
    period: u32,
    pos: u32,
}

impl Pattern {
    pub(crate) fn new(line_type: &LineType) -> Self {
        let (on, off) = match *line_type {
            LineType::Solid => (1, 0),
            // `Dashed(0)` would be invisible; draw it solid instead.
            LineType::Dashed(0) => (1, 0),
            LineType::Dashed(len) => (len, len),
            // Two-pixel dots, `spacing` pixels apart.
            LineType::Dotted(spacing) => (2, spacing.max(1)),
        };
        Self {
            on,
            period: on.saturating_add(off).max(1),
            pos: 0,
        }
    }

    /// Whether the next pixel is drawn.
    #[inline]
    pub(crate) fn next(&mut self) -> bool {
        let draw = self.pos < self.on;
        self.pos = (self.pos + 1) % self.period;
        draw
    }
}

/// Clips the segment `p1`–`p2` to the rectangle `min`–`max`
/// (Liang–Barsky). Returns `None` if nothing of it is inside or if a
/// coordinate is not finite.
pub(crate) fn clip_segment(
    p1: (f64, f64),
    p2: (f64, f64),
    min: (f64, f64),
    max: (f64, f64),
) -> Option<((f64, f64), (f64, f64))> {
    if ![p1.0, p1.1, p2.0, p2.1].iter().all(|v| v.is_finite()) {
        return None;
    }
    let (dx, dy) = (p2.0 - p1.0, p2.1 - p1.1);
    let mut t0: f64 = 0.0;
    let mut t1: f64 = 1.0;
    for (p, q) in [
        (-dx, p1.0 - min.0),
        (dx, max.0 - p1.0),
        (-dy, p1.1 - min.1),
        (dy, max.1 - p1.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    if t0 > t1 {
        return None;
    }
    Some((
        (p1.0 + t0 * dx, p1.1 + t0 * dy),
        (p1.0 + t1 * dx, p1.1 + t1 * dy),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(canvas: &PixelCanvas, color: [u8; 3]) -> usize {
        canvas
            .buffer
            .chunks_exact(3)
            .filter(|p| *p == color)
            .count()
    }

    #[test]
    fn new_and_clear_use_background_color() {
        let mut c = PixelCanvas::new(4, 3, [1, 2, 3], 0);
        assert_eq!(c.buffer.len(), 36);
        assert_eq!(count(&c, [1, 2, 3]), 12);
        c.draw_pixel(0, 0, [9, 9, 9]);
        c.clear();
        assert_eq!(count(&c, [1, 2, 3]), 12);
    }

    #[test]
    fn out_of_bounds_pixels_do_not_wrap() {
        let mut c = PixelCanvas::new(10, 10, [0, 0, 0], 0);
        c.draw_pixel(10, 0, [255, 0, 0]); // would wrap to (0, 1) before
        c.draw_pixel(u32::MAX, u32::MAX, [255, 0, 0]);
        c.blend_pixel(10, 5, [255, 0, 0], 1.0);
        assert_eq!(count(&c, [255, 0, 0]), 0);
    }

    #[test]
    fn lines_are_clipped_and_never_panic() {
        let mut c = PixelCanvas::new(20, 20, [0, 0, 0], 0);
        c.draw_line(-100, 10, 100, 10, [255, 0, 0], LineType::Solid);
        assert_eq!(count(&c, [255, 0, 0]), 20);
        c.draw_line(
            i32::MIN,
            i32::MIN,
            i32::MAX,
            i32::MAX,
            [0, 255, 0],
            LineType::Solid,
        );
        assert!(count(&c, [0, 255, 0]) > 0);
        c.draw_line(-5, -5, -1, -1, [0, 0, 255], LineType::Dashed(3));
        assert_eq!(count(&c, [0, 0, 255]), 0);
    }

    #[test]
    fn dashed_and_dotted_have_gaps() {
        let mut c = PixelCanvas::new(100, 3, [0, 0, 0], 0);
        c.draw_line(0, 1, 99, 1, [255, 255, 255], LineType::Dashed(10));
        assert_eq!(count(&c, [255, 255, 255]), 50);
        c.clear();
        c.draw_line(0, 1, 99, 1, [255, 255, 255], LineType::Dotted(8));
        assert_eq!(count(&c, [255, 255, 255]), 20);
        c.clear();
        c.draw_line(0, 1, 99, 1, [255, 255, 255], LineType::Dashed(0));
        assert_eq!(count(&c, [255, 255, 255]), 100);
    }

    #[test]
    fn pattern_continues_across_segments() {
        // 100 one-pixel segments must still look dashed.
        let mut c = PixelCanvas::new(101, 3, [0, 0, 0], 0);
        let mut p = Pattern::new(&LineType::Dashed(10));
        for x in 0..100 {
            c.stroke_segment(
                (x as f64, 1.0),
                (x as f64 + 1.0, 1.0),
                [255, 255, 255],
                &mut p,
                x == 0,
            );
        }
        let lit = count(&c, [255, 255, 255]);
        assert!((50..=51).contains(&lit), "{lit}");
    }

    #[test]
    fn margins_larger_than_canvas_do_not_underflow() {
        let mut c = PixelCanvas::new(10, 10, [0, 0, 0], 50);
        c.draw_horizontal_line(5, [1, 1, 1]);
        c.draw_vertical_line(5, [1, 1, 1]);
        c.draw_grid(&[0, 0], [1, 1, 1]);
        c.draw_grid(&[3, 3], [1, 1, 1]);
    }

    #[test]
    fn blend_rounds_and_clamps() {
        let mut c = PixelCanvas::new(1, 1, [0, 0, 0], 0);
        c.blend_pixel(0, 0, [255, 255, 255], 0.5);
        assert_eq!(c.get_pixel(0, 0), Some([128, 128, 128]));
        c.blend_pixel(0, 0, [0, 0, 0], 7.0);
        assert_eq!(c.get_pixel(0, 0), Some([0, 0, 0]));
    }

    #[test]
    fn clip_segment_cases() {
        let r = clip_segment((-10.0, 5.0), (20.0, 5.0), (0.0, 0.0), (10.0, 10.0)).unwrap();
        assert_eq!(r, ((0.0, 5.0), (10.0, 5.0)));
        assert!(clip_segment((-10.0, -5.0), (-1.0, -1.0), (0.0, 0.0), (10.0, 10.0)).is_none());
        assert!(clip_segment((f64::NAN, 0.0), (1.0, 1.0), (0.0, 0.0), (10.0, 10.0)).is_none());
    }
}
