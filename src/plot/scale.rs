//! Axis scales: how data values map to positions.

/// The mapping of an axis.
#[derive(Clone, Debug, PartialEq, Default)]
#[non_exhaustive]
pub enum Scale {
    /// Linear.
    #[default]
    Linear,
    /// Logarithmic with the given base (values ≤ 0 are not shown).
    Log(f64),
    /// Symmetric log: linear within `±linthresh`, logarithmic outside;
    /// handles zero and negative values.
    SymLog(f64),
    /// Date/time: values are seconds since the Unix epoch (UTC); see
    /// [`crate::plot::time`].
    Time,
    /// Categorical: values are category indices (set automatically when a
    /// series is added with string categories).
    Category,
}

impl Scale {
    /// Base-10 logarithmic scale.
    pub fn log10() -> Self {
        Scale::Log(10.0)
    }

    pub(crate) fn forward(&self, v: f64) -> f64 {
        match *self {
            Scale::Log(base) => {
                if v > 0.0 {
                    v.ln() / base.ln()
                } else {
                    f64::NAN
                }
            }
            Scale::SymLog(c) => {
                let c = if c > 0.0 { c } else { 1.0 };
                v.signum() * (1.0 + v.abs() / c).log10()
            }
            _ => v,
        }
    }

    pub(crate) fn inverse(&self, t: f64) -> f64 {
        match *self {
            Scale::Log(base) => base.powf(t),
            Scale::SymLog(c) => {
                let c = if c > 0.0 { c } else { 1.0 };
                t.signum() * c * (10f64.powf(t.abs()) - 1.0)
            }
            _ => t,
        }
    }

    /// Whether a value can be shown on this scale.
    pub(crate) fn accepts(&self, v: f64) -> bool {
        v.is_finite() && (!matches!(self, Scale::Log(_)) || v > 0.0)
    }

    pub(crate) fn is_log(&self) -> bool {
        matches!(self, Scale::Log(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for s in [
            Scale::Linear,
            Scale::log10(),
            Scale::Log(2.0),
            Scale::SymLog(1.0),
        ] {
            for v in [0.5, 1.0, 7.0, 1000.0] {
                let back = s.inverse(s.forward(v));
                assert!((back - v).abs() < 1e-9 * v.max(1.0), "{s:?} {v} {back}");
            }
        }
        assert!(Scale::log10().forward(0.0).is_nan());
        assert_eq!(Scale::SymLog(1.0).forward(0.0), 0.0);
        assert!(Scale::SymLog(1.0).forward(-9.0) < 0.0);
        assert!(!Scale::log10().accepts(-1.0));
    }
}
