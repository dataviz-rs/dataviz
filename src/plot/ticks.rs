//! Tick placement and tick label formatting.

use super::scale::Scale;
use super::time;
use std::fmt;
use std::sync::Arc;

/// How tick values are turned into labels.
#[derive(Clone, Default)]
pub enum TickFormat {
    /// Chosen from the scale and tick spacing.
    #[default]
    Auto,
    /// Fixed number of decimals.
    Decimals(usize),
    /// `0.25` → `25%` with the given decimals.
    Percent(usize),
    /// SI prefixes: `1500` → `1.5k`, `2e6` → `2M` (given max decimals).
    Si(usize),
    /// Scientific notation: `1.50e3`.
    Scientific(usize),
    /// Time format (strftime subset, see [`time::format_time`]).
    Time(String),
    /// Text before and after an automatic label (e.g. `"$"`, `" ms"`).
    Affix(String, String),
    /// Any function.
    Custom(Arc<dyn Fn(f64) -> String + Send + Sync>),
}

impl TickFormat {
    /// Custom formatter.
    pub fn custom(f: impl Fn(f64) -> String + Send + Sync + 'static) -> Self {
        TickFormat::Custom(Arc::new(f))
    }
}

impl fmt::Debug for TickFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TickFormat::Auto => write!(f, "Auto"),
            TickFormat::Decimals(d) => write!(f, "Decimals({d})"),
            TickFormat::Percent(d) => write!(f, "Percent({d})"),
            TickFormat::Si(d) => write!(f, "Si({d})"),
            TickFormat::Scientific(d) => write!(f, "Scientific({d})"),
            TickFormat::Time(s) => write!(f, "Time({s:?})"),
            TickFormat::Affix(a, b) => write!(f, "Affix({a:?}, {b:?})"),
            TickFormat::Custom(_) => write!(f, "Custom(..)"),
        }
    }
}

/// Smallest "nice" step (1, 2, 2.5, 5 × 10ⁿ) ≥ `raw`.
pub(crate) fn nice_step(raw: f64) -> f64 {
    if !raw.is_finite() || raw <= 0.0 {
        return 1.0;
    }
    let mag = 10f64.powf(raw.log10().floor());
    for f in [1.0, 2.0, 2.5, 5.0, 10.0] {
        if f * mag >= raw * (1.0 - 1e-9) {
            return f * mag;
        }
    }
    10.0 * mag
}

/// Multiples of `step` within `[min, max]`.
fn multiples(min: f64, max: f64, step: f64) -> Vec<f64> {
    if !(step > 0.0) || !min.is_finite() || !max.is_finite() {
        return Vec::new();
    }
    let start = (min / step - 1e-9).ceil();
    let end = (max / step + 1e-9).floor();
    if end - start > 10_000.0 {
        return Vec::new();
    }
    let mut v = Vec::new();
    let mut k = start;
    while k <= end {
        let x = k * step;
        v.push(if x.abs() < step * 1e-9 { 0.0 } else { x });
        k += 1.0;
    }
    v
}

/// Linear ticks: about `target` nice values within `[min, max]`.
pub(crate) fn linear_ticks(min: f64, max: f64, target: usize) -> (Vec<f64>, f64) {
    let (lo, hi) = (min.min(max), min.max(max));
    let step = nice_step((hi - lo) / target.max(1) as f64);
    (multiples(lo, hi, step), step)
}

/// Minor ticks between linear major ticks.
pub(crate) fn linear_minor(min: f64, max: f64, step: f64) -> Vec<f64> {
    let mant = step / 10f64.powf(step.log10().floor());
    let sub = if (mant - 2.0).abs() < 1e-6 { 4.0 } else { 5.0 };
    let minor = step / sub;
    multiples(min.min(max), min.max(max), minor)
        .into_iter()
        .filter(|v| ((v / step).round() * step - v).abs() > minor * 1e-6)
        .collect()
}

/// Major and minor ticks for a scale.
pub(crate) struct TickSet {
    pub major: Vec<f64>,
    pub minor: Vec<f64>,
    /// Spacing of linear major ticks (for label precision).
    pub step: f64,
    /// Default time format for time scales.
    pub time_format: Option<&'static str>,
}

pub(crate) fn ticks(scale: &Scale, min: f64, max: f64, target: usize) -> TickSet {
    let (lo, hi) = (min.min(max), min.max(max));
    match scale {
        Scale::Log(base) => log_ticks(*base, lo, hi, target),
        Scale::SymLog(c) => symlog_ticks(*c, lo, hi, target),
        Scale::Time => time_ticks(lo, hi, target),
        _ => {
            let (major, step) = linear_ticks(lo, hi, target);
            TickSet {
                minor: linear_minor(lo, hi, step),
                major,
                step,
                time_format: None,
            }
        }
    }
}

fn log_ticks(base: f64, lo: f64, hi: f64, target: usize) -> TickSet {
    let base = if base > 1.0 { base } else { 10.0 };
    let lo = lo.max(f64::MIN_POSITIVE);
    let (a, b) = (
        (lo.ln() / base.ln() - 1e-9).ceil(),
        (hi.ln() / base.ln() + 1e-9).floor(),
    );
    if b - a < 1.0 {
        // Less than two decades: 1-2-5 (or 1..9) multiples of powers.
        let (k0, k1) = ((lo.log10() - 1e-9).floor(), (hi.log10() + 1e-9).ceil());
        for mults in [
            &[1.0, 2.0, 5.0][..],
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0][..],
        ] {
            let mut major = Vec::new();
            let mut k = k0;
            while k <= k1 {
                for m in mults {
                    let v = m * 10f64.powf(k);
                    if v >= lo * (1.0 - 1e-9) && v <= hi * (1.0 + 1e-9) {
                        major.push(v);
                    }
                }
                k += 1.0;
            }
            if major.len() >= 3 || mults.len() == 9 {
                return TickSet {
                    minor: Vec::new(),
                    major,
                    step: 0.0,
                    time_format: None,
                };
            }
        }
    }
    let decades = (b - a + 1.0) as usize;
    let every = decades.div_ceil(target.max(1)).max(1);
    let mut major = Vec::new();
    let mut minor = Vec::new();
    let mut k = a - 1.0;
    while k <= b {
        let v = base.powf(k);
        if k >= a && ((k - a) as usize) % every == 0 {
            major.push(v);
        } else if k >= a && v >= lo && v <= hi {
            minor.push(v);
        }
        if every == 1 && (base - 10.0).abs() < 1e-9 {
            for m in 2..10 {
                let mv = m as f64 * v;
                if mv >= lo && mv <= hi {
                    minor.push(mv);
                }
            }
        }
        k += 1.0;
    }
    TickSet {
        major,
        minor,
        step: 0.0,
        time_format: None,
    }
}

fn symlog_ticks(c: f64, lo: f64, hi: f64, target: usize) -> TickSet {
    let c = if c > 0.0 { c } else { 1.0 };
    let start = c.log10().floor();
    let max_abs = lo.abs().max(hi.abs()).max(c);
    let mut mags = Vec::new();
    let mut k = start;
    while 10f64.powf(k) <= max_abs * (1.0 + 1e-9) {
        mags.push(10f64.powf(k));
        k += 1.0;
    }
    let every = (mags.len() * 2 + 1).div_ceil(target.max(1)).max(1);
    let mags: Vec<f64> = mags.into_iter().rev().step_by(every).collect();
    let mut major: Vec<f64> = Vec::new();
    for m in &mags {
        if -m >= lo {
            major.push(-m);
        }
    }
    if lo <= 0.0 && hi >= 0.0 {
        major.push(0.0);
    }
    for m in mags.iter().rev() {
        if *m <= hi {
            major.push(*m);
        }
    }
    major.sort_by(f64::total_cmp);
    major.dedup();
    TickSet {
        major,
        minor: Vec::new(),
        step: 0.0,
        time_format: None,
    }
}

const SEC_STEPS: [(f64, &str); 20] = [
    (1.0, "%H:%M:%S"),
    (2.0, "%H:%M:%S"),
    (5.0, "%H:%M:%S"),
    (10.0, "%H:%M:%S"),
    (15.0, "%H:%M:%S"),
    (30.0, "%H:%M:%S"),
    (60.0, "%H:%M"),
    (120.0, "%H:%M"),
    (300.0, "%H:%M"),
    (600.0, "%H:%M"),
    (900.0, "%H:%M"),
    (1800.0, "%H:%M"),
    (3600.0, "%H:%M"),
    (7200.0, "%H:%M"),
    (10800.0, "%H:%M"),
    (21600.0, "%d %b %H:%M"),
    (43200.0, "%d %b %H:%M"),
    (86400.0, "%d %b"),
    (172800.0, "%d %b"),
    (604800.0, "%d %b"),
];

fn time_ticks(lo: f64, hi: f64, target: usize) -> TickSet {
    let span = hi - lo;
    let target = target.max(2) as f64;
    // Sub-second spans: plain linear ticks.
    if span < target {
        let (major, step) = linear_ticks(lo, hi, target as usize);
        return TickSet {
            major,
            minor: Vec::new(),
            step,
            time_format: Some("%H:%M:%S"),
        };
    }
    for (step, fmt) in SEC_STEPS {
        if span / step <= target {
            let major = if step >= 604800.0 {
                // Weeks start on Monday.
                let monday = time::days_from_civil(1970, 1, 5) as f64 * 86400.0;
                multiples(lo - monday, hi - monday, step)
                    .into_iter()
                    .map(|v| v + monday)
                    .collect()
            } else {
                multiples(lo, hi, step)
            };
            return TickSet {
                major,
                minor: Vec::new(),
                step,
                time_format: Some(fmt),
            };
        }
    }
    // Months and years.
    let months_span = span / (30.44 * 86400.0);
    for (months, fmt) in [(1u32, "%b %Y"), (2, "%b %Y"), (3, "%b %Y"), (6, "%b %Y")] {
        if months_span / months as f64 <= target {
            return TickSet {
                major: month_ticks(lo, hi, months),
                minor: Vec::new(),
                step: months as f64 * 30.44 * 86400.0,
                time_format: Some(fmt),
            };
        }
    }
    let years = (span / (365.25 * 86400.0)) / target;
    let ystep = nice_step(years).max(1.0).round() as i64;
    let first = time::breakdown(lo).year;
    let mut major = Vec::new();
    let mut y = first - first.rem_euclid(ystep);
    while y <= time::breakdown(hi).year + 1 {
        let t = time::date(y, 1, 1);
        if t >= lo && t <= hi {
            major.push(t);
        }
        y += ystep;
    }
    TickSet {
        major,
        minor: Vec::new(),
        step: ystep as f64 * 365.25 * 86400.0,
        time_format: Some("%Y"),
    }
}

fn month_ticks(lo: f64, hi: f64, step: u32) -> Vec<f64> {
    let start = time::breakdown(lo);
    let mut y = start.year;
    let mut m = start.month;
    // Align to the step (Jan, Apr, Jul, … for 3 months).
    m = ((m - 1) / step) * step + 1;
    let mut out = Vec::new();
    for _ in 0..2000 {
        let t = time::date(y, m, 1);
        if t > hi {
            break;
        }
        if t >= lo {
            out.push(t);
        }
        m += step;
        while m > 12 {
            m -= 12;
            y += 1;
        }
    }
    out
}

/// Decimals needed for labels `step` apart.
pub(crate) fn decimals_for_step(step: f64) -> usize {
    let step = step.abs();
    if !step.is_finite() || step == 0.0 {
        return 2;
    }
    for d in 0..=6 {
        let s = step * 10f64.powi(d);
        if (s - s.round()).abs() < 1e-6 * s.max(1.0) {
            return d as usize;
        }
    }
    6
}

fn trim_exp(s: String) -> String {
    // "1.50e3" -> "1.5e3"
    if let Some((m, e)) = s.split_once('e') {
        let m = if m.contains('.') {
            m.trim_end_matches('0').trim_end_matches('.')
        } else {
            m
        };
        format!("{m}e{e}")
    } else {
        s
    }
}

fn fixed(v: f64, d: usize) -> String {
    let s = format!("{v:.d$}");
    if s.trim_start_matches('-')
        .chars()
        .all(|c| c == '0' || c == '.')
    {
        s.trim_start_matches('-').to_string()
    } else {
        s
    }
}

fn superscript(n: i64) -> String {
    n.to_string()
        .chars()
        .map(|c| match c {
            '-' => '⁻',
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            _ => '⁹',
        })
        .collect()
}

/// SI-prefixed value.
pub(crate) fn si(v: f64, decimals: usize) -> String {
    const P: [(f64, &str); 10] = [
        (1e15, "P"),
        (1e12, "T"),
        (1e9, "G"),
        (1e6, "M"),
        (1e3, "k"),
        (1.0, ""),
        (1e-3, "m"),
        (1e-6, "µ"),
        (1e-9, "n"),
        (1e-12, "p"),
    ];
    if v == 0.0 || !v.is_finite() {
        return "0".into();
    }
    let (f, p) = P
        .iter()
        .find(|(f, _)| v.abs() >= *f * (1.0 - 1e-9))
        .copied()
        .unwrap_or((1e-12, "p"));
    let s = format!("{:.*}", decimals, v / f);
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    format!("{s}{p}")
}

/// Formats one value with a format, using the scale/step for `Auto`.
pub(crate) fn format_value(
    v: f64,
    format: &TickFormat,
    scale: &Scale,
    step: f64,
    max_abs: f64,
    time_format: Option<&str>,
) -> String {
    match format {
        TickFormat::Auto => auto(v, scale, step, max_abs, time_format),
        TickFormat::Decimals(d) => fixed(v, *d),
        TickFormat::Percent(d) => format!("{}%", fixed(v * 100.0, *d)),
        TickFormat::Si(d) => si(v, *d),
        TickFormat::Scientific(d) => trim_exp(format!("{v:.d$e}")),
        TickFormat::Time(f) => time::format_time(v, f),
        TickFormat::Affix(a, b) => {
            format!("{a}{}{b}", auto(v, scale, step, max_abs, time_format))
        }
        TickFormat::Custom(f) => f(v),
    }
}

fn auto(v: f64, scale: &Scale, step: f64, max_abs: f64, time_format: Option<&str>) -> String {
    match scale {
        Scale::Time => {
            let f = time_format.unwrap_or("%Y-%m-%d");
            if f.starts_with("%H") && v.rem_euclid(86400.0) == 0.0 && step >= 60.0 {
                // Show the date at midnight.
                time::format_time(v, "%d %b")
            } else {
                time::format_time(v, f)
            }
        }
        Scale::Log(base) | Scale::SymLog(base) if step == 0.0 && v != 0.0 => {
            let b = if matches!(scale, Scale::Log(_)) {
                *base
            } else {
                10.0
            };
            let k = (v.abs().ln() / b.ln()).round();
            let exact = (b.powf(k) - v.abs()).abs() < 1e-9 * v.abs();
            let sign = if v < 0.0 { "-" } else { "" };
            let big = max_abs >= 1e5 || max_abs < 1e-3;
            if exact && (b - 10.0).abs() < 1e-9 && (big || !(-3.0..=4.0).contains(&k)) {
                format!("{sign}10{}", superscript(k as i64))
            } else if exact && (b - 10.0).abs() > 1e-9 && b.fract() == 0.0 && k.abs() > 3.0 {
                format!("{sign}{}{}", b as i64, superscript(k as i64))
            } else {
                trim_float(v)
            }
        }
        Scale::SymLog(_) | Scale::Log(_) if v == 0.0 => "0".into(),
        _ => {
            if v != 0.0 && (max_abs >= 1e7 || max_abs < 1e-4) {
                let d = decimals_for_step(step / 10f64.powf(max_abs.log10().floor())).min(3);
                trim_exp(format!("{v:.d$e}"))
            } else {
                fixed(v, decimals_for_step(step))
            }
        }
    }
}

fn trim_float(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(scale: &Scale, min: f64, max: f64, target: usize, f: &TickFormat) -> Vec<String> {
        let t = ticks(scale, min, max, target);
        let max_abs = min.abs().max(max.abs());
        t.major
            .iter()
            .map(|v| format_value(*v, f, scale, t.step, max_abs, t.time_format))
            .collect()
    }

    #[test]
    fn linear() {
        assert_eq!(
            labels(&Scale::Linear, 0.0, 10.0, 5, &TickFormat::Auto),
            ["0", "2", "4", "6", "8", "10"]
        );
        assert_eq!(
            labels(&Scale::Linear, -0.3, 0.3, 6, &TickFormat::Auto),
            ["-0.3", "-0.2", "-0.1", "0.0", "0.1", "0.2", "0.3"]
        );
        assert_eq!(
            labels(&Scale::Linear, 0.0, 3e7, 3, &TickFormat::Auto),
            ["0", "1e7", "2e7", "3e7"]
        );
        let t = ticks(&Scale::Linear, 0.0, 10.0, 5);
        assert_eq!(t.minor.len(), 15); // 0.5 steps between the 2-steps
    }

    #[test]
    fn log() {
        assert_eq!(
            labels(&Scale::log10(), 1.0, 1e6, 10, &TickFormat::Auto),
            ["10⁰", "10¹", "10²", "10³", "10⁴", "10⁵", "10⁶"]
        );
        let t = ticks(&Scale::log10(), 1.0, 1000.0, 10);
        assert!(t.minor.contains(&20.0));
        // Narrow ranges fall back to linear ticks.
        assert!(ticks(&Scale::log10(), 2.0, 8.0, 5).major.len() >= 3);
    }

    #[test]
    fn symlog() {
        let l = labels(&Scale::SymLog(1.0), -100.0, 100.0, 10, &TickFormat::Auto);
        assert!(l.contains(&"0".to_string()) && l.contains(&"-100".to_string()));
    }

    #[test]
    fn time_ticks_pick_units() {
        let t0 = time::date(2024, 1, 1);
        let day = labels(&Scale::Time, t0, t0 + 6.0 * 3600.0, 6, &TickFormat::Auto);
        assert_eq!(day[0], "01 Jan");
        assert_eq!(day[1], "01:00");
        let months = labels(
            &Scale::Time,
            t0,
            time::date(2024, 12, 31),
            6,
            &TickFormat::Auto,
        );
        assert_eq!(months[0], "Jan 2024");
        assert_eq!(months[1], "Mar 2024");
        let years = labels(
            &Scale::Time,
            t0,
            time::date(2044, 1, 1),
            6,
            &TickFormat::Auto,
        );
        assert_eq!(years[0], "2025"); // 5-year steps
    }

    #[test]
    fn formats() {
        let s = Scale::Linear;
        assert_eq!(
            format_value(0.256, &TickFormat::Percent(1), &s, 0.1, 1.0, None),
            "25.6%"
        );
        assert_eq!(
            format_value(1500.0, &TickFormat::Si(1), &s, 1.0, 1.0, None),
            "1.5k"
        );
        assert_eq!(
            format_value(2.5e-6, &TickFormat::Si(1), &s, 1.0, 1.0, None),
            "2.5µ"
        );
        assert_eq!(
            format_value(1234.0, &TickFormat::Scientific(2), &s, 1.0, 1.0, None),
            "1.23e3"
        );
        assert_eq!(
            format_value(
                5.0,
                &TickFormat::Affix("$".into(), "k".into()),
                &s,
                1.0,
                5.0,
                None
            ),
            "$5k"
        );
        assert_eq!(
            format_value(
                3.0,
                &TickFormat::custom(|v| format!("<{v}>")),
                &s,
                1.0,
                1.0,
                None
            ),
            "<3>"
        );
    }
}
