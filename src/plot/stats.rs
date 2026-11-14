//! Statistics helpers used by the statistical charts (also usable directly).
//!
//! All functions ignore non-finite values.

/// Finite values, sorted ascending.
pub fn sorted(values: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    v.sort_by(f64::total_cmp);
    v
}

/// Arithmetic mean (NaN for no data).
pub fn mean(values: &[f64]) -> f64 {
    let v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Sample standard deviation (n − 1).
pub fn std_dev(values: &[f64]) -> f64 {
    let v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    if v.len() < 2 {
        return 0.0;
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

/// Quantile `q` (0..1) of sorted data with linear interpolation.
pub fn quantile_sorted(sorted: &[f64], q: f64) -> f64 {
    match sorted.len() {
        0 => f64::NAN,
        1 => sorted[0],
        n => {
            let pos = q.clamp(0.0, 1.0) * (n - 1) as f64;
            let i = pos.floor() as usize;
            let f = pos - i as f64;
            if i + 1 < n {
                sorted[i] * (1.0 - f) + sorted[i + 1] * f
            } else {
                sorted[n - 1]
            }
        }
    }
}

/// Quantile `q` (0..1) of unsorted data.
pub fn quantile(values: &[f64], q: f64) -> f64 {
    quantile_sorted(&sorted(values), q)
}

/// Median.
pub fn median(values: &[f64]) -> f64 {
    quantile(values, 0.5)
}

/// Box-plot statistics (Tukey: whiskers at the most extreme data within
/// 1.5 × IQR of the quartiles).
#[derive(Clone, Debug, PartialEq)]
pub struct BoxStats {
    /// Lower whisker.
    pub whisker_low: f64,
    /// First quartile.
    pub q1: f64,
    /// Median.
    pub median: f64,
    /// Third quartile.
    pub q3: f64,
    /// Upper whisker.
    pub whisker_high: f64,
    /// Mean.
    pub mean: f64,
    /// Points beyond the whiskers.
    pub outliers: Vec<f64>,
    /// Number of values.
    pub n: usize,
}

/// Box-plot statistics, or `None` for no data.
pub fn box_stats(values: &[f64]) -> Option<BoxStats> {
    let s = sorted(values);
    if s.is_empty() {
        return None;
    }
    let q1 = quantile_sorted(&s, 0.25);
    let q3 = quantile_sorted(&s, 0.75);
    let iqr = q3 - q1;
    let (lo, hi) = (q1 - 1.5 * iqr, q3 + 1.5 * iqr);
    let whisker_low = s.iter().copied().find(|v| *v >= lo).unwrap_or(q1);
    let whisker_high = s.iter().rev().copied().find(|v| *v <= hi).unwrap_or(q3);
    Some(BoxStats {
        whisker_low,
        q1,
        median: quantile_sorted(&s, 0.5),
        q3,
        whisker_high,
        mean: s.iter().sum::<f64>() / s.len() as f64,
        outliers: s
            .iter()
            .copied()
            .filter(|v| *v < whisker_low || *v > whisker_high)
            .collect(),
        n: s.len(),
    })
}

/// Silverman's rule-of-thumb KDE bandwidth.
pub fn silverman_bandwidth(values: &[f64]) -> f64 {
    let s = sorted(values);
    let n = s.len() as f64;
    if n < 2.0 {
        return 1.0;
    }
    let sd = std_dev(&s);
    let iqr = quantile_sorted(&s, 0.75) - quantile_sorted(&s, 0.25);
    let a = if iqr > 0.0 { sd.min(iqr / 1.34) } else { sd };
    let a = if a > 0.0 {
        a
    } else {
        s[s.len() - 1].abs().max(1.0) * 0.1
    };
    0.9 * a * n.powf(-0.2)
}

/// Gaussian kernel density estimate evaluated at `points` positions spanning
/// the data ± 3 bandwidths. Returns `(x, density)`.
pub fn kde(values: &[f64], bandwidth: Option<f64>, points: usize) -> (Vec<f64>, Vec<f64>) {
    let s = sorted(values);
    if s.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let h = bandwidth
        .filter(|b| *b > 0.0)
        .unwrap_or_else(|| silverman_bandwidth(&s));
    let (lo, hi) = (s[0] - 3.0 * h, s[s.len() - 1] + 3.0 * h);
    let n = points.max(2);
    let norm = 1.0 / (s.len() as f64 * h * (2.0 * std::f64::consts::PI).sqrt());
    let xs: Vec<f64> = (0..n)
        .map(|i| lo + (hi - lo) * i as f64 / (n - 1) as f64)
        .collect();
    let ys = xs
        .iter()
        .map(|x| {
            // Only nearby points contribute noticeably.
            let a = s.partition_point(|v| *v < x - 5.0 * h);
            let b = s.partition_point(|v| *v <= x + 5.0 * h);
            s[a..b]
                .iter()
                .map(|v| (-0.5 * ((x - v) / h).powi(2)).exp())
                .sum::<f64>()
                * norm
        })
        .collect();
    (xs, ys)
}

/// How histogram bins are chosen.
#[derive(Clone, Debug, PartialEq)]
pub enum Bins {
    /// Freedman–Diaconis, falling back to Sturges (default).
    Auto,
    /// Fixed number of equal bins.
    Count(usize),
    /// Fixed bin width.
    Width(f64),
    /// Explicit edges (ascending).
    Edges(Vec<f64>),
    /// Sturges' rule: `log2(n) + 1` bins.
    Sturges,
    /// Scott's rule: width `3.49 σ n^(-1/3)`.
    Scott,
    /// Freedman–Diaconis: width `2 IQR n^(-1/3)`.
    FreedmanDiaconis,
}

/// Bin edges for data according to a rule.
pub fn bin_edges(values: &[f64], bins: &Bins) -> Vec<f64> {
    let s = sorted(values);
    if s.is_empty() {
        return match bins {
            Bins::Edges(e) => e.clone(),
            _ => vec![0.0, 1.0],
        };
    }
    let (lo, hi) = (s[0], s[s.len() - 1]);
    let n = s.len() as f64;
    let count = |c: usize| -> Vec<f64> {
        let c = c.clamp(1, 10_000);
        if hi == lo {
            return (0..=c).map(|i| lo - 0.5 + i as f64 / c as f64).collect();
        }
        // The last edge is exactly `hi` (lo + (hi - lo) can round below it).
        (0..=c)
            .map(|i| {
                if i == c {
                    hi
                } else {
                    lo + (hi - lo) * i as f64 / c as f64
                }
            })
            .collect()
    };
    let width = |w: f64| -> Vec<f64> {
        if w.is_nan() || w <= 0.0 || hi == lo {
            return count(1);
        }
        let c = ((hi - lo) / w).ceil().max(1.0) as usize;
        count(c.min(10_000))
    };
    let sturges = || (n.log2().ceil() as usize + 1).max(1);
    match bins {
        Bins::Count(c) => count(*c),
        Bins::Width(w) => {
            if w.is_nan() || *w <= 0.0 {
                return count(sturges());
            }
            let start = (lo / w).floor() * w;
            let c = (((hi - start) / w).floor() as usize + 1).min(10_000);
            (0..=c).map(|i| start + i as f64 * w).collect()
        }
        Bins::Edges(e) => e.clone(),
        Bins::Sturges => count(sturges()),
        Bins::Scott => width(3.49 * std_dev(&s) * n.powf(-1.0 / 3.0)),
        Bins::FreedmanDiaconis | Bins::Auto => {
            let iqr = quantile_sorted(&s, 0.75) - quantile_sorted(&s, 0.25);
            if iqr > 0.0 {
                let fd = width(2.0 * iqr * n.powf(-1.0 / 3.0));
                if *bins == Bins::Auto && fd.len() - 1 < sturges() {
                    count(sturges())
                } else {
                    fd
                }
            } else {
                count(sturges())
            }
        }
    }
}

/// Counts per bin (`edges.len() - 1` bins; the last bin includes its right
/// edge; values outside are ignored).
pub fn histogram(values: &[f64], edges: &[f64]) -> Vec<f64> {
    let nb = edges.len().saturating_sub(1);
    let mut counts = vec![0.0; nb];
    if nb == 0 {
        return counts;
    }
    for v in values.iter().copied().filter(|v| v.is_finite()) {
        if v < edges[0] || v > edges[nb] {
            continue;
        }
        let i = edges
            .partition_point(|e| *e <= v)
            .saturating_sub(1)
            .min(nb - 1);
        counts[i] += 1.0;
    }
    counts
}

/// Least-squares line `y = slope·x + intercept` and R².
pub fn linear_regression(x: &[f64], y: &[f64]) -> Option<(f64, f64, f64)> {
    let c = polyfit(x, y, 1)?;
    let r2 = r_squared(x, y, &c);
    Some((c[1], c[0], r2))
}

/// Least-squares polynomial; returns coefficients `c[0] + c[1]·x + …`.
pub fn polyfit(x: &[f64], y: &[f64], degree: usize) -> Option<Vec<f64>> {
    let pts: Vec<(f64, f64)> = x
        .iter()
        .zip(y)
        .map(|(a, b)| (*a, *b))
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .collect();
    let m = degree + 1;
    if pts.len() < m {
        return None;
    }
    // Normal equations with x centred and scaled for stability.
    let mx = pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64;
    let sx = pts
        .iter()
        .map(|p| (p.0 - mx).abs())
        .fold(0.0, f64::max)
        .max(1e-300);
    let mut a = vec![vec![0.0; m + 1]; m];
    for &(px, py) in &pts {
        let t = (px - mx) / sx;
        let pw: Vec<f64> = (0..2 * m).map(|k| t.powi(k as i32)).collect();
        for i in 0..m {
            for j in 0..m {
                a[i][j] += pw[i + j];
            }
            a[i][m] += pw[i] * py;
        }
    }
    let c = solve(a)?;
    // Expand back from t = (x - mx) / sx.
    let mut out = vec![0.0; m];
    for (k, ck) in c.iter().enumerate() {
        // ck * ((x - mx)/sx)^k = ck/sx^k * sum_j C(k,j) x^j (-mx)^(k-j)
        let scale = ck / sx.powi(k as i32);
        for (j, o) in out.iter_mut().enumerate().take(k + 1) {
            *o += scale * binomial(k, j) * (-mx).powi((k - j) as i32);
        }
    }
    Some(out)
}

fn binomial(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

fn solve(mut a: Vec<Vec<f64>>) -> Option<Vec<f64>> {
    let n = a.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        for r in 0..n {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in c..=n {
                    a[r][k] -= f * a[c][k];
                }
            }
        }
    }
    Some((0..n).map(|i| a[i][n] / a[i][i]).collect())
}

/// Evaluates polynomial coefficients at `x`.
pub fn polyval(c: &[f64], x: f64) -> f64 {
    c.iter().rev().fold(0.0, |acc, k| acc * x + k)
}

/// Coefficient of determination of a polynomial fit.
pub fn r_squared(x: &[f64], y: &[f64], c: &[f64]) -> f64 {
    let pts: Vec<(f64, f64)> = x
        .iter()
        .zip(y)
        .map(|(a, b)| (*a, *b))
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .collect();
    let my = pts.iter().map(|p| p.1).sum::<f64>() / pts.len().max(1) as f64;
    let ss_tot: f64 = pts.iter().map(|p| (p.1 - my).powi(2)).sum();
    let ss_res: f64 = pts.iter().map(|p| (p.1 - polyval(c, p.0)).powi(2)).sum();
    if ss_tot == 0.0 {
        1.0
    } else {
        1.0 - ss_res / ss_tot
    }
}

/// Trailing moving average over `window` values (first values average what
/// is available).
pub fn moving_average(y: &[f64], window: usize) -> Vec<f64> {
    let w = window.max(1);
    let mut out = Vec::with_capacity(y.len());
    let mut sum = 0.0;
    let mut count = 0usize;
    for i in 0..y.len() {
        if y[i].is_finite() {
            sum += y[i];
            count += 1;
        }
        if i >= w && y[i - w].is_finite() {
            sum -= y[i - w];
            count -= 1;
        }
        out.push(if count > 0 {
            sum / count as f64
        } else {
            f64::NAN
        });
    }
    out
}

/// Empirical cumulative distribution: sorted values and fractions `(i+1)/n`.
pub fn ecdf(values: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let s = sorted(values);
    let n = s.len() as f64;
    let y = (1..=s.len()).map(|i| i as f64 / n).collect();
    (s, y)
}

/// Largest-Triangle-Three-Buckets downsampling to `threshold` points,
/// preserving the visual shape of long series.
pub fn lttb(x: &[f64], y: &[f64], threshold: usize) -> (Vec<f64>, Vec<f64>) {
    let n = x.len().min(y.len());
    if threshold >= n || threshold < 3 {
        return (x[..n].to_vec(), y[..n].to_vec());
    }
    let every = (n - 2) as f64 / (threshold - 2) as f64;
    let (mut ox, mut oy) = (vec![x[0]], vec![y[0]]);
    let mut a = 0usize;
    for i in 0..threshold - 2 {
        let start = ((i as f64 + 1.0) * every) as usize + 1;
        let end = (((i as f64 + 2.0) * every) as usize + 1).min(n);
        let (ax, ay) = if start < end {
            let k = (end - start) as f64;
            (
                x[start..end].iter().sum::<f64>() / k,
                y[start..end].iter().sum::<f64>() / k,
            )
        } else {
            (x[n - 1], y[n - 1])
        };
        let rs = (i as f64 * every) as usize + 1;
        let re = ((i as f64 + 1.0) * every) as usize + 1;
        let mut best = rs;
        let mut best_area = -1.0;
        for j in rs..re.min(n) {
            let area = ((x[a] - ax) * (y[j] - y[a]) - (x[a] - x[j]) * (ay - y[a])).abs();
            if area > best_area {
                best_area = area;
                best = j;
            }
        }
        ox.push(x[best]);
        oy.push(y[best]);
        a = best;
    }
    ox.push(x[n - 1]);
    oy.push(y[n - 1]);
    (ox, oy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        let v = [3.0, 1.0, f64::NAN, 2.0, 4.0];
        assert_eq!(mean(&v), 2.5);
        assert_eq!(median(&v), 2.5);
        assert_eq!(quantile(&v, 0.0), 1.0);
        assert_eq!(quantile(&v, 1.0), 4.0);
        assert!((std_dev(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]) - 2.138).abs() < 1e-3);
    }

    #[test]
    fn box_plot() {
        let mut v: Vec<f64> = (1..=9).map(|i| i as f64).collect();
        v.push(100.0);
        let b = box_stats(&v).unwrap();
        assert_eq!(b.median, 5.5);
        assert_eq!(b.outliers, vec![100.0]);
        assert_eq!(b.whisker_high, 9.0);
        assert!(box_stats(&[]).is_none());
    }

    #[test]
    fn histogram_counts_everything() {
        let v: Vec<f64> = (0..1000).map(|i| (i as f64 * 0.37).sin()).collect();
        for rule in [
            Bins::Auto,
            Bins::Count(7),
            Bins::Width(0.1),
            Bins::Sturges,
            Bins::Scott,
            Bins::FreedmanDiaconis,
        ] {
            let e = bin_edges(&v, &rule);
            assert!(e.windows(2).all(|w| w[1] > w[0]), "{rule:?}");
            assert_eq!(histogram(&v, &e).iter().sum::<f64>(), 1000.0, "{rule:?}");
        }
        assert_eq!(
            histogram(&[5.0; 3], &bin_edges(&[5.0; 3], &Bins::Count(2))),
            vec![0.0, 3.0]
        );
    }

    #[test]
    fn kde_integrates_to_one() {
        let v: Vec<f64> = (0..200).map(|i| (i as f64 * 0.1).sin() * 3.0).collect();
        let (x, y) = kde(&v, None, 400);
        let dx = x[1] - x[0];
        let area: f64 = y.iter().sum::<f64>() * dx;
        assert!((area - 1.0).abs() < 0.02, "{area}");
    }

    #[test]
    fn regression() {
        let x: Vec<f64> = (0..10).map(|i| i as f64 + 1000.0).collect();
        let y: Vec<f64> = x.iter().map(|v| 2.0 * v - 3.0).collect();
        let (m, b, r2) = linear_regression(&x, &y).unwrap();
        assert!((m - 2.0).abs() < 1e-6 && (b + 3.0).abs() < 1e-3 && (r2 - 1.0).abs() < 1e-9);
        let yq: Vec<f64> = x.iter().map(|v| (v - 1000.0).powi(2)).collect();
        let c = polyfit(&x, &yq, 2).unwrap();
        assert!((polyval(&c, 1005.0) - 25.0).abs() < 1e-3);
    }

    #[test]
    fn helpers() {
        assert_eq!(
            moving_average(&[1.0, 2.0, 3.0, 4.0], 2),
            vec![1.0, 1.5, 2.5, 3.5]
        );
        assert_eq!(ecdf(&[3.0, 1.0]), (vec![1.0, 3.0], vec![0.5, 1.0]));
        let x: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|v| (v / 50.0).sin()).collect();
        let (lx, ly) = lttb(&x, &y, 100);
        assert_eq!(lx.len(), 100);
        assert_eq!((lx[0], lx[99]), (0.0, 999.0));
        assert!(ly.iter().cloned().fold(f64::MIN, f64::max) > 0.99);
    }
}
