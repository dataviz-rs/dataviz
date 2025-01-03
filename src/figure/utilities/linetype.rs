/// Represents the style of a line in a graph or chart.
#[derive(Clone)]
pub enum LineType {
    /// A solid line with no gaps.
    Solid,
    /// A dashed line with configurable dash length.
    /// - The `u32` value specifies the length of each dash in pixels.
    Dashed(u32),
    /// A dotted line with configurable dot spacing.
    /// - The `u32` value specifies the spacing between dots in pixels.
    Dotted(u32),
}
