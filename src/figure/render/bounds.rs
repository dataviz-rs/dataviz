/// Data-space bounds of a chart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Bounds {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            x_min: 0.0,
            x_max: 1.0,
            y_min: 0.0,
            y_max: 1.0,
        }
    }
}

impl Bounds {
    /// Smallest bounds containing every finite point, or `None` if there is
    /// no finite point. NaN and infinite coordinates are ignored.
    pub fn from_points<I>(points: I) -> Option<Self>
    where
        I: IntoIterator<Item = (f64, f64)>,
    {
        let mut b: Option<Self> = None;
        for (x, y) in points {
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            b = Some(match b {
                None => Self {
                    x_min: x,
                    x_max: x,
                    y_min: y,
                    y_max: y,
                },
                Some(b) => Self {
                    x_min: b.x_min.min(x),
                    x_max: b.x_max.max(x),
                    y_min: b.y_min.min(y),
                    y_max: b.y_max.max(y),
                },
            });
        }
        b
    }

    /// Extends the bounds so that they contain the origin.
    pub fn with_origin(self) -> Self {
        Self {
            x_min: self.x_min.min(0.0),
            x_max: self.x_max.max(0.0),
            y_min: self.y_min.min(0.0),
            y_max: self.y_max.max(0.0),
        }
    }

    /// Makes both axes symmetric around zero (`-m..m`, `m = max(|min|, |max|)`).
    pub fn symmetric(self) -> Self {
        let mx = self.x_min.abs().max(self.x_max.abs());
        let my = self.y_min.abs().max(self.y_max.abs());
        Self {
            x_min: -mx,
            x_max: mx,
            y_min: -my,
            y_max: my,
        }
    }

    /// Returns bounds that are safe to divide by: finite, ordered, and with a
    /// non-zero extent on both axes. A degenerate axis (single value) is
    /// widened around that value.
    pub fn sanitized(self) -> Self {
        let (x_min, x_max) = sanitize_axis(self.x_min, self.x_max);
        let (y_min, y_max) = sanitize_axis(self.y_min, self.y_max);
        Self {
            x_min,
            x_max,
            y_min,
            y_max,
        }
    }
}

fn sanitize_axis(min: f64, max: f64) -> (f64, f64) {
    let (min, max) = match (min.is_finite(), max.is_finite()) {
        (true, true) => (min.min(max), min.max(max)),
        (true, false) => (min, min),
        (false, true) => (max, max),
        (false, false) => return (0.0, 1.0),
    };
    let magnitude = min.abs().max(max.abs()).max(1.0);
    if max - min > magnitude * 1e-12 {
        return (min, max);
    }
    let pad = if min == 0.0 { 1.0 } else { min.abs() * 0.5 };
    (min - pad, max + pad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_non_finite_points() {
        let b = Bounds::from_points([
            (1.0, 2.0),
            (f64::NAN, 5.0),
            (3.0, f64::INFINITY),
            (-1.0, 0.0),
        ])
        .unwrap();
        assert_eq!(
            b,
            Bounds {
                x_min: -1.0,
                x_max: 1.0,
                y_min: 0.0,
                y_max: 2.0
            }
        );
        assert!(Bounds::from_points([(f64::NAN, 1.0)]).is_none());
        assert!(Bounds::from_points(std::iter::empty()).is_none());
    }

    #[test]
    fn sanitizes_degenerate_axes() {
        let b = Bounds {
            x_min: 5.0,
            x_max: 5.0,
            y_min: 0.0,
            y_max: 0.0,
        }
        .sanitized();
        assert_eq!((b.x_min, b.x_max), (2.5, 7.5));
        assert_eq!((b.y_min, b.y_max), (-1.0, 1.0));

        let b = Bounds {
            x_min: f64::INFINITY,
            x_max: f64::NEG_INFINITY,
            y_min: 3.0,
            y_max: 1.0,
        }
        .sanitized();
        assert_eq!((b.x_min, b.x_max), (0.0, 1.0));
        assert_eq!((b.y_min, b.y_max), (1.0, 3.0));
    }

    #[test]
    fn origin_and_symmetry() {
        let b = Bounds {
            x_min: 2.0,
            x_max: 4.0,
            y_min: -3.0,
            y_max: 1.0,
        };
        assert_eq!(
            b.with_origin(),
            Bounds {
                x_min: 0.0,
                x_max: 4.0,
                y_min: -3.0,
                y_max: 1.0
            }
        );
        assert_eq!(
            b.symmetric(),
            Bounds {
                x_min: -4.0,
                x_max: 4.0,
                y_min: -3.0,
                y_max: 3.0
            }
        );
    }
}
