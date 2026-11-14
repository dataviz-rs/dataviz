/// `divisions` equal steps from `min` to `max` (`divisions + 1` ticks), each
/// with a label formatted with just enough decimals for the step size.
/// `divisions == 0` is treated as 1.
pub(crate) fn linear(min: f64, max: f64, divisions: usize) -> Vec<(f64, String)> {
    let n = divisions.max(1);
    let step = (max - min) / n as f64;
    (0..=n)
        .map(|i| {
            // Compute the last tick exactly to avoid accumulated error.
            let v = if i == n { max } else { min + step * i as f64 };
            (v, format(v, step))
        })
        .collect()
}

/// Number of decimals for ticks `step` apart: exact for "round" steps
/// (`600` → 0, `0.25` → 2), otherwise three significant digits of the step
/// (`0.6283` → 3). At most 6.
pub(crate) fn decimals_for_step(step: f64) -> usize {
    let step = step.abs();
    if !step.is_finite() || step == 0.0 {
        return 2;
    }
    for d in 0..=3 {
        let scaled = step * 10f64.powi(d);
        if (scaled - scaled.round()).abs() < 1e-9 * scaled.max(1.0) {
            return d as usize;
        }
    }
    (2 - step.log10().floor() as i32).clamp(0, 6) as usize
}

/// Formats a tick value for a given step. Never prints `-0`.
pub(crate) fn format(value: f64, step: f64) -> String {
    let d = decimals_for_step(step);
    let s = format!("{value:.d$}");
    if s.trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        s.trim_start_matches('-').to_string()
    } else {
        s
    }
}

/// Formats a category value (e.g. a year on a bar chart) without a useless
/// fractional part: `2020.0` → `"2020"`, `2.5` → `"2.5"`.
pub(crate) fn format_category(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        let s = format!("{value:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honours_division_count() {
        for n in [1, 5, 7, 10, 40] {
            assert_eq!(linear(0.0, 3000.0, n).len(), n + 1);
        }
        assert_eq!(linear(0.0, 1.0, 0).len(), 2);
    }

    #[test]
    fn labels_have_sensible_precision() {
        let labels: Vec<String> = linear(0.0, 3000.0, 5).into_iter().map(|t| t.1).collect();
        assert_eq!(labels, ["0", "600", "1200", "1800", "2400", "3000"]);
        let labels: Vec<String> = linear(0.0, 1.0, 4).into_iter().map(|t| t.1).collect();
        assert_eq!(labels, ["0.00", "0.25", "0.50", "0.75", "1.00"]);
        let labels: Vec<String> = linear(-1.0, 1.0, 2).into_iter().map(|t| t.1).collect();
        assert_eq!(labels, ["-1", "0", "1"]);
        assert_eq!(format(-0.0001, 0.5), "0.0");
        let pi = std::f64::consts::PI;
        let labels: Vec<String> = linear(-pi, pi, 4).into_iter().map(|t| t.1).collect();
        assert_eq!(labels, ["-3.14", "-1.57", "0.00", "1.57", "3.14"]);
    }

    #[test]
    fn categories() {
        assert_eq!(format_category(2020.0), "2020");
        assert_eq!(format_category(2.5), "2.5");
        assert_eq!(format_category(-3.0), "-3");
    }
}
