/// Represents the orientation of a chart, graph, or component.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// Vertical orientation, typically used for charts where data is arranged along a vertical axis.
    Vertical,
    /// Horizontal orientation, typically used for charts where data is arranged along a horizontal axis.
    Horizontal,
}
