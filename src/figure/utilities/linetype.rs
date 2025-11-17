/// Represents the style of a line in a graph or chart.
#[derive(Clone)]
pub enum LineType {
    /// A solid line with optional dots at data points.
    /// - `bool`: Whether to draw dots at data points (true = with dots, false = line only).
    Solid(bool),
    /// A dashed line with configurable dash length and optional dots at data points.
    /// - First `u32`: The length of each dash in pixels.
    /// - `bool`: Whether to draw dots at data points (true = with dots, false = line only).
    Dashed(u32, bool),
    /// A dotted line with intermediate dots drawn between data points.
    /// - First `u32`: The spacing between intermediate dots in pixels.
    /// - `bool`: Whether to draw intermediate dots (true = draw dots between points, false = no intermediate dots).
    Dotted(u32, bool),
}
