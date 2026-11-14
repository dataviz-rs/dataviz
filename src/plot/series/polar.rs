//! Polar and radar charts.

use crate::plot::axes::{Axes, Layout};
use crate::plot::scene::Node;
use crate::plot::text::FontSet;
use crate::plot::theme::Theme;

/// Configuration of a polar frame.
#[derive(Clone, Debug, Default)]
pub struct PolarFrame {}

impl PolarFrame {
    pub(crate) fn draw_frame(
        &self,
        _a: &Axes,
        _l: &Layout,
        _t: &Theme,
        _f: &FontSet,
        _o: &mut Vec<Node>,
        _below: bool,
    ) {
    }
}
