/// Represents the shape and size of scatter plot points.
#[derive(Clone)]
pub enum ScatterDotType {
    /// A circular point.
    /// - The `u32` value specifies the radius of the circle in pixels.
    Circle(u32),
    /// A square-shaped point.
    /// - The `u32` value specifies the side length of the square in pixels.
    Square(u32),
    /// A cross-shaped point.
    /// - The `u32` value specifies the line thickness of the cross in pixels.
    Cross(u32),
    /// A triangular point.
    /// - The `u32` value specifies the base width of the triangle in pixels.
    Triangle(u32),
}
