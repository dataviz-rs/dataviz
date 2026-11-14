//! Axis configuration and data → pixel mapping.

use super::scale::Scale;
use super::ticks::TickFormat;

/// Where ticks go.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum TickSpec {
    /// Automatic nice ticks.
    #[default]
    Auto,
    /// About this many automatic ticks.
    Count(usize),
    /// Ticks at these values (labels from the formatter).
    At(Vec<f64>),
    /// Ticks at these values with these labels.
    Labeled(Vec<(f64, String)>),
    /// No ticks or tick labels.
    None,
}

/// Configuration of one axis. Obtain it with
/// [`Axes::x_axis`](crate::plot::Axes::x_axis) / `y_axis` / `y2_axis` and
/// chain setters.
#[derive(Clone, Debug)]
pub struct Axis {
    pub(crate) label: Option<String>,
    pub(crate) scale: Scale,
    pub(crate) min: Option<f64>,
    pub(crate) max: Option<f64>,
    pub(crate) inverted: bool,
    pub(crate) format: TickFormat,
    pub(crate) ticks: TickSpec,
    pub(crate) minor_ticks: bool,
    pub(crate) grid: Option<bool>,
    pub(crate) categories: Vec<String>,
    pub(crate) tick_rotation: Option<f64>,
    pub(crate) visible: bool,
    pub(crate) tick_labels: bool,
    /// Ranges left out of the axis (`break_at`), data units.
    pub(crate) breaks: Vec<(f64, f64)>,
    pub(crate) margin: f64,
}

impl Default for Axis {
    fn default() -> Self {
        Self {
            label: None,
            scale: Scale::Linear,
            min: None,
            max: None,
            inverted: false,
            format: TickFormat::Auto,
            ticks: TickSpec::Auto,
            minor_ticks: false,
            grid: None,
            categories: Vec::new(),
            tick_rotation: None,
            visible: true,
            tick_labels: true,
            breaks: Vec::new(),
            margin: 0.05,
        }
    }
}

impl Axis {
    /// Axis label.
    pub fn label(&mut self, label: impl Into<String>) -> &mut Self {
        self.label = Some(label.into());
        self
    }

    /// Scale (linear, log, symlog, time, category).
    pub fn scale(&mut self, scale: Scale) -> &mut Self {
        self.scale = scale;
        self
    }

    /// Base-10 logarithmic scale.
    pub fn log(&mut self) -> &mut Self {
        self.scale(Scale::log10())
    }

    /// Time scale (values in seconds since the Unix epoch).
    pub fn time(&mut self) -> &mut Self {
        self.scale(Scale::Time)
    }

    /// Fixed limits (either may be `f64::NAN` to keep it automatic).
    pub fn limits(&mut self, min: f64, max: f64) -> &mut Self {
        self.min = min.is_finite().then_some(min);
        self.max = max.is_finite().then_some(max);
        self
    }

    /// Fixed minimum.
    pub fn min(&mut self, min: f64) -> &mut Self {
        self.min = min.is_finite().then_some(min);
        self
    }

    /// Fixed maximum.
    pub fn max(&mut self, max: f64) -> &mut Self {
        self.max = max.is_finite().then_some(max);
        self
    }

    /// Reverses the direction.
    pub fn inverted(&mut self, inverted: bool) -> &mut Self {
        self.inverted = inverted;
        self
    }

    /// Tick label format.
    pub fn format(&mut self, format: super::ticks::TickFormat) -> &mut Self {
        self.format = format;
        self
    }

    /// Tick placement.
    pub fn ticks(&mut self, ticks: TickSpec) -> &mut Self {
        self.ticks = ticks;
        self
    }

    /// Minor tick marks (and minor grid lines if the theme enables them).
    pub fn minor_ticks(&mut self, on: bool) -> &mut Self {
        self.minor_ticks = on;
        self
    }

    /// Grid lines for this axis (overrides the theme).
    pub fn grid(&mut self, on: bool) -> &mut Self {
        self.grid = Some(on);
        self
    }

    /// Tick label rotation in degrees (default: automatic).
    pub fn tick_rotation(&mut self, degrees: f64) -> &mut Self {
        self.tick_rotation = Some(degrees);
        self
    }

    /// Shows or hides the tick labels (the ticks and grid stay), e.g. on
    /// inner plots of a grid sharing an axis. Their space is kept, so the
    /// plots of a grid keep the same size and line up.
    pub fn tick_labels(&mut self, on: bool) -> &mut Self {
        self.tick_labels = on;
        self
    }

    /// Leaves the range `from..to` out of the axis (broken axis), e.g. to
    /// show small values next to a few very large ones. The break is marked
    /// on the axis; data inside it is not drawn. Several breaks can be set.
    pub fn break_at(&mut self, from: f64, to: f64) -> &mut Self {
        if from.is_finite() && to.is_finite() && from != to {
            self.breaks.push((from.min(to), from.max(to)));
        }
        self
    }

    /// Hides ticks, labels and the axis label.
    pub fn hidden(&mut self) -> &mut Self {
        self.visible = false;
        self
    }

    /// Fraction of the data range added on each side (default 0.05).
    pub fn margin(&mut self, fraction: f64) -> &mut Self {
        self.margin = fraction.max(0.0);
        self
    }

    /// Explicit category order for a categorical axis.
    pub fn categories<S: ToString>(
        &mut self,
        categories: impl IntoIterator<Item = S>,
    ) -> &mut Self {
        self.categories = categories.into_iter().map(|c| c.to_string()).collect();
        self.scale = Scale::Category;
        self
    }

    /// Index of a category, adding it if new.
    pub(crate) fn category_index(&mut self, name: &str) -> f64 {
        self.scale = Scale::Category;
        match self.categories.iter().position(|c| c == name) {
            Some(i) => i as f64,
            None => {
                self.categories.push(name.to_string());
                (self.categories.len() - 1) as f64
            }
        }
    }
}

/// Resolved data → pixel mapping of one axis.
#[derive(Clone, Debug, PartialEq)]
pub struct AxisMap {
    pub(crate) scale: Scale,
    /// Data limits (min < max).
    pub min: f64,
    /// Data limits (min < max).
    pub max: f64,
    /// Pixel position of `min` (after inversion).
    pub(crate) p0: f64,
    /// Pixel position of `max`.
    pub(crate) p1: f64,
    t0: f64,
    t1: f64,
    /// Broken-out ranges in scale space (sorted, disjoint).
    breaks: Vec<(f64, f64)>,
}

impl AxisMap {
    pub(crate) fn new(scale: Scale, min: f64, max: f64, p0: f64, p1: f64) -> Self {
        let t0 = scale.forward(min);
        let t1 = scale.forward(max);
        Self {
            scale,
            min,
            max,
            p0,
            p1,
            t0,
            t1,
            breaks: Vec::new(),
        }
    }

    /// The same mapping with the data ranges `breaks` left out.
    pub(crate) fn with_breaks(mut self, breaks: &[(f64, f64)]) -> Self {
        let mut b: Vec<(f64, f64)> = breaks
            .iter()
            .map(|(a, c)| (self.scale.forward(*a), self.scale.forward(*c)))
            .filter(|(a, c)| a.is_finite() && c.is_finite())
            .map(|(a, c)| (a.min(c), a.max(c)))
            .collect();
        b.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for (a, c) in b {
            match merged.last_mut() {
                Some(last) if a <= last.1 => last.1 = last.1.max(c),
                _ => merged.push((a, c)),
            }
        }
        let (lo, hi) = (self.t0.min(self.t1), self.t0.max(self.t1));
        merged.retain(|(a, c)| *a > lo && *c < hi);
        if merged.is_empty() {
            return self;
        }
        self.breaks = merged;
        self.t0 = self.squeeze(self.t0);
        self.t1 = self.squeeze(self.t1);
        self
    }

    /// Scale space → compressed space (breaks removed; NaN inside one).
    fn squeeze(&self, t: f64) -> f64 {
        let mut out = t;
        for (a, c) in &self.breaks {
            if t >= *c {
                out -= c - a;
            } else if t > *a {
                return f64::NAN;
            }
        }
        out
    }

    /// Compressed space → scale space.
    fn unsqueeze(&self, u: f64) -> f64 {
        let mut t = u;
        for (a, c) in &self.breaks {
            if t >= *a {
                t += c - a;
            }
        }
        t
    }

    /// Whether the data segment from `a` to `b` jumps over a break.
    pub(crate) fn crosses_break(&self, a: f64, b: f64) -> bool {
        if self.breaks.is_empty() {
            return false;
        }
        let (ta, tb) = (self.scale.forward(a), self.scale.forward(b));
        let (lo, hi) = (ta.min(tb), ta.max(tb));
        self.breaks.iter().any(|(x, y)| lo <= *x && hi >= *y)
    }

    /// Whether the mapping has breaks.
    pub(crate) fn has_breaks(&self) -> bool {
        !self.breaks.is_empty()
    }

    /// Positions of the breaks in pixels.
    pub(crate) fn break_positions(&self) -> Vec<f64> {
        self.breaks
            .iter()
            .map(|(a, _)| self.t_to_px(self.squeeze(*a)))
            .collect()
    }

    fn t_to_px(&self, u: f64) -> f64 {
        let span = self.t1 - self.t0;
        if span == 0.0 || !span.is_finite() {
            return (self.p0 + self.p1) / 2.0;
        }
        self.p0 + (u - self.t0) / span * (self.p1 - self.p0)
    }

    /// Pixel position of a data value (NaN if not representable).
    pub fn px(&self, v: f64) -> f64 {
        if !self.scale.accepts(v) {
            return f64::NAN;
        }
        let mut t = self.scale.forward(v);
        if !self.breaks.is_empty() {
            t = self.squeeze(t);
        }
        self.t_to_px(t)
    }

    /// Data value at a pixel position.
    pub fn value(&self, px: f64) -> f64 {
        let span = self.p1 - self.p0;
        if span == 0.0 {
            return self.min;
        }
        let mut t = self.t0 + (px - self.p0) / span * (self.t1 - self.t0);
        if !self.breaks.is_empty() {
            t = self.unsqueeze(t);
        }
        self.scale.inverse(t)
    }

    /// Pixel length of a data distance `d` starting at `v` (for bar widths).
    pub fn length(&self, v: f64, d: f64) -> f64 {
        self.px(v + d) - self.px(v)
    }

    /// Data → pixel for values outside the limits too (clamped to a large
    /// range to keep paths finite).
    pub(crate) fn px_clamped(&self, v: f64) -> f64 {
        let p = self.px(v);
        let lo = self.p0.min(self.p1) - 1e5;
        let hi = self.p0.max(self.p1) + 1e5;
        if p.is_nan() {
            p
        } else {
            p.clamp(lo, hi)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaks_compress_the_axis() {
        let m = AxisMap::new(Scale::Linear, 0.0, 100.0, 0.0, 100.0).with_breaks(&[(10.0, 90.0)]);
        // 20 data units left over 100 pixels.
        assert!((m.px(10.0) - 50.0).abs() < 1e-9);
        assert!((m.px(90.0) - 50.0).abs() < 1e-9);
        assert!((m.px(100.0) - 100.0).abs() < 1e-9);
        assert!(m.px(50.0).is_nan());
        assert!((m.value(75.0) - 95.0).abs() < 1e-9);
        assert_eq!(m.break_positions(), vec![50.0]);
    }

    #[test]
    fn mapping() {
        let m = AxisMap::new(Scale::Linear, 0.0, 10.0, 100.0, 200.0);
        assert_eq!(m.px(5.0), 150.0);
        assert_eq!(m.value(150.0), 5.0);
        let inv = AxisMap::new(Scale::Linear, 0.0, 10.0, 200.0, 100.0);
        assert_eq!(inv.px(0.0), 200.0);
        let log = AxisMap::new(Scale::log10(), 1.0, 100.0, 0.0, 100.0);
        assert_eq!(log.px(10.0), 50.0);
        assert!(log.px(-1.0).is_nan());
        assert!((log.value(50.0) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn categories() {
        let mut a = Axis::default();
        assert_eq!(a.category_index("b"), 0.0);
        assert_eq!(a.category_index("a"), 1.0);
        assert_eq!(a.category_index("b"), 0.0);
        assert_eq!(a.scale, Scale::Category);
    }
}
