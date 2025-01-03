use crate::figure::canvas::pixelcanvas::PixelCanvas;
use ab_glyph::FontRef;

/// A trait for plots that support hover functionality, allowing interactive
/// features like highlighting and displaying information about data points.
pub trait Hover {
    /// Finds the closest point to the mouse position on the plot.
    ///
    /// # Parameters
    /// - `mouse_x`: The x-coordinate of the mouse position in canvas space.
    /// - `mouse_y`: The y-coordinate of the mouse position in canvas space.
    /// - `canvas`: The `PixelCanvas` being used for rendering the plot.
    ///
    /// # Returns
    /// An optional tuple containing:
    /// - `((f64, f64), f64)`:
    ///   - The `(x, y)` coordinates of the closest point.
    ///   - The distance from the mouse position to the point.
    ///
    /// Returns `None` if no points are found.
    fn find_closest_point(
        &self,
        mouse_x: u32,
        mouse_y: u32,
        canvas: &PixelCanvas,
    ) -> Option<((f64, f64), f64)>;

    /// Converts plot coordinates into canvas pixel coordinates.
    ///
    /// # Parameters
    /// - `x`: The x-coordinate in the plot's coordinate system.
    /// - `y`: The y-coordinate in the plot's coordinate system.
    /// - `canvas`: The `PixelCanvas` being used for rendering the plot.
    ///
    /// # Returns
    /// A tuple `(u32, u32)` representing the corresponding pixel coordinates on the canvas.
    fn to_canvas_coordinates(&self, x: f64, y: f64, canvas: &PixelCanvas) -> (u32, u32);

    /// Retrieves the font used for hover text rendering.
    ///
    /// # Parameters
    /// - `font_data`: A slice containing the font data (e.g., bytes of a TTF file).
    ///
    /// # Returns
    /// A `FontRef` representing the parsed font, tied to the lifetime of the `font_data`.
    ///
    /// # Panics
    /// Panics if the font data cannot be parsed.
    fn get_font<'a>(&self, font_data: &'a [u8]) -> FontRef<'a>;

    /// Handles hover functionality and returns an updated buffer if applicable.
    ///
    /// This method is used to modify the canvas buffer in response to hover events,
    /// such as highlighting a data point or displaying additional information.
    ///
    /// # Parameters
    /// - `mouse_x`: The x-coordinate of the mouse position in canvas space.
    /// - `mouse_y`: The y-coordinate of the mouse position in canvas space.
    /// - `canvas`: The `PixelCanvas` being used for rendering the plot.
    ///
    /// # Returns
    /// An optional vector of `u32` representing the updated pixel buffer.
    /// If no changes are made, returns `None`.
    fn handle_hover(&self, mouse_x: u32, mouse_y: u32, canvas: &PixelCanvas) -> Option<Vec<u32>>;
}
