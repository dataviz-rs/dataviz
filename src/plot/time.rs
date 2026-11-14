//! Date/time helpers for time axes.
//!
//! Time values are `f64` seconds since 1970-01-01 00:00:00 UTC, so any
//! timestamp source can be used (`SystemTime`, chrono, time, …) without a
//! dependency.

/// Days since 1970-01-01 for a civil date (proleptic Gregorian).
pub(crate) fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Civil date for days since 1970-01-01.
pub(crate) fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Seconds since the Unix epoch for a UTC date and time.
///
/// ```
/// use dataviz::plot::time::datetime;
/// assert_eq!(datetime(1970, 1, 2, 0, 0, 0.0), 86400.0);
/// ```
pub fn datetime(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: f64) -> f64 {
    days_from_civil(year, month, day) as f64 * 86400.0
        + hour as f64 * 3600.0
        + minute as f64 * 60.0
        + second
}

/// Seconds since the Unix epoch for a UTC date at midnight.
pub fn date(year: i64, month: u32, day: u32) -> f64 {
    datetime(year, month, day, 0, 0, 0.0)
}

/// Parses `YYYY-MM-DD`, `YYYY-MM-DD HH:MM`, `YYYY-MM-DDTHH:MM:SS[.fff][Z]`.
pub fn parse_datetime(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('Z');
    let (d, t) = match s.find(['T', ' ']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let mut dp = d.split('-');
    let y: i64 = dp.next()?.parse().ok()?;
    let m: u32 = dp.next()?.parse().ok()?;
    let day: u32 = dp.next().unwrap_or("1").parse().ok()?;
    if dp.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    let (mut h, mut mi, mut sec) = (0u32, 0u32, 0.0f64);
    if let Some(t) = t {
        let mut tp = t.split(':');
        h = tp.next()?.parse().ok()?;
        mi = tp.next().unwrap_or("0").parse().ok()?;
        sec = tp.next().unwrap_or("0").parse().ok()?;
        if h > 23 || mi > 59 || !(0.0..61.0).contains(&sec) {
            return None;
        }
    }
    Some(datetime(y, m, day, h, mi, sec))
}

/// Broken-down UTC time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tm {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: f64,
    pub weekday: u32, // 0 = Sunday
}

pub(crate) fn breakdown(t: f64) -> Tm {
    let days = (t / 86400.0).floor();
    let secs = t - days * 86400.0;
    let (year, month, day) = civil_from_days(days as i64);
    let hour = (secs / 3600.0).floor() as u32;
    let minute = ((secs - hour as f64 * 3600.0) / 60.0).floor() as u32;
    let second = secs - hour as f64 * 3600.0 - minute as f64 * 60.0;
    Tm {
        year,
        month,
        day,
        hour: hour.min(23),
        minute: minute.min(59),
        second,
        weekday: ((days as i64 + 4).rem_euclid(7)) as u32,
    }
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// Formats a timestamp with a strftime subset: `%Y %y %m %d %e %H %M %S %b
/// %B %a %j %%`.
///
/// ```
/// use dataviz::plot::time::{date, format_time};
/// assert_eq!(format_time(date(2024, 3, 5), "%d %b %Y"), "05 Mar 2024");
/// ```
pub fn format_time(t: f64, fmt: &str) -> String {
    if !t.is_finite() {
        return String::new();
    }
    let tm = breakdown(t);
    let mut out = String::new();
    let mut chars = fmt.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('Y') => out.push_str(&tm.year.to_string()),
            Some('y') => out.push_str(&format!("{:02}", tm.year.rem_euclid(100))),
            Some('m') => out.push_str(&format!("{:02}", tm.month)),
            Some('d') => out.push_str(&format!("{:02}", tm.day)),
            Some('e') => out.push_str(&tm.day.to_string()),
            Some('H') => out.push_str(&format!("{:02}", tm.hour)),
            Some('M') => out.push_str(&format!("{:02}", tm.minute)),
            Some('S') => out.push_str(&format!("{:02}", tm.second.floor() as u32)),
            Some('b') => out.push_str(MONTHS[(tm.month - 1) as usize]),
            Some('B') => out.push_str(match tm.month {
                1 => "January",
                2 => "February",
                3 => "March",
                4 => "April",
                5 => "May",
                6 => "June",
                7 => "July",
                8 => "August",
                9 => "September",
                10 => "October",
                11 => "November",
                _ => "December",
            }),
            Some('a') => out.push_str(DAYS[tm.weekday as usize]),
            Some('j') => {
                let doy =
                    days_from_civil(tm.year, tm.month, tm.day) - days_from_civil(tm.year, 1, 1) + 1;
                out.push_str(&format!("{doy:03}"));
            }
            Some('%') => out.push('%'),
            Some(o) => {
                out.push('%');
                out.push(o);
            }
            None => out.push('%'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trip() {
        for days in [-1_000_000, -1, 0, 1, 18_000, 19_876, 2_000_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
    }

    #[test]
    fn formatting_and_parsing() {
        let t = datetime(2024, 2, 29, 13, 5, 9.0);
        assert_eq!(format_time(t, "%Y-%m-%d %H:%M:%S"), "2024-02-29 13:05:09");
        assert_eq!(
            format_time(t, "%a %e %B %j %y %%"),
            "Thu 29 February 060 24 %"
        );
        assert_eq!(parse_datetime("2024-02-29T13:05:09Z"), Some(t));
        assert_eq!(parse_datetime("2024-02-29 13:05:09"), Some(t));
        assert_eq!(parse_datetime("2024-02-29"), Some(date(2024, 2, 29)));
        assert_eq!(parse_datetime("2024-13-01"), None);
        assert_eq!(parse_datetime("nope"), None);
        assert_eq!(format_time(date(1969, 12, 31), "%Y-%m-%d"), "1969-12-31");
    }
}
