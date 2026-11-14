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

/// Count, minimum and maximum of the finite values.
/// One thread: with AVX2 a single pass is memory bound and already faster
/// than splitting it (which costs more CPU time in total).
fn finite_min_max(values: &[f64]) -> (usize, f64, f64) {
    finite_min_max_1(values)
}

/// [`finite_min_max_1`] with AVX2: 4 values per step, finiteness by
/// `v - v == 0`. Returns (count, min, max, values processed).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn finite_min_max_avx2(values: &[f64]) -> (usize, f64, f64, usize) {
    use std::arch::x86_64::*;
    let (chunks, _) = values.as_chunks::<4>();
    let (mut lo, mut hi) = (
        _mm256_set1_pd(f64::INFINITY),
        _mm256_set1_pd(f64::NEG_INFINITY),
    );
    let mut cnt = _mm256_setzero_si256();
    let zero = _mm256_setzero_pd();
    for c in chunks {
        // SAFETY: `c` is 4 contiguous f64 (unaligned load).
        let v = unsafe { _mm256_loadu_pd(c.as_ptr()) };
        let ok = _mm256_cmp_pd::<_CMP_EQ_OQ>(_mm256_sub_pd(v, v), zero);
        lo = _mm256_blendv_pd(lo, _mm256_min_pd(lo, v), ok);
        hi = _mm256_blendv_pd(hi, _mm256_max_pd(hi, v), ok);
        // ok lanes are all ones (-1 as i64): subtracting counts them.
        cnt = _mm256_sub_epi64(cnt, _mm256_castpd_si256(ok));
    }
    let (mut l, mut h, mut k) = ([0.0f64; 4], [0.0f64; 4], [0i64; 4]);
    // SAFETY: 4 × 8 bytes each.
    unsafe {
        _mm256_storeu_pd(l.as_mut_ptr(), lo);
        _mm256_storeu_pd(h.as_mut_ptr(), hi);
        _mm256_storeu_si256(k.as_mut_ptr().cast(), cnt);
    }
    let lo = l.iter().fold(f64::INFINITY, |a, b| a.min(*b));
    let hi = h.iter().fold(f64::NEG_INFINITY, |a, b| a.max(*b));
    (k.iter().sum::<i64>() as usize, lo, hi, chunks.len() * 4)
}

/// [`finite_min_max`] on one thread, in one vectorizable pass (8 lanes).
fn finite_min_max_1(values: &[f64]) -> (usize, f64, f64) {
    #[cfg(target_arch = "x86_64")]
    if std::arch::is_x86_feature_detected!("avx2") {
        // SAFETY: AVX2 support was checked at run time.
        let (n, lo, hi, done) = unsafe { finite_min_max_avx2(values) };
        let (m, l, h) = finite_min_max_scalar(&values[done..]);
        return (n + m, lo.min(l), hi.max(h));
    }
    finite_min_max_scalar(values)
}

fn finite_min_max_scalar(values: &[f64]) -> (usize, f64, f64) {
    let mut lo = [f64::INFINITY; 8];
    let mut hi = [f64::NEG_INFINITY; 8];
    let mut n = [0usize; 8];
    let (chunks, rest) = values.as_chunks::<8>();
    for c in chunks {
        for j in 0..8 {
            let v = c[j];
            let ok = v.is_finite();
            lo[j] = if ok && v < lo[j] { v } else { lo[j] };
            hi[j] = if ok && v > hi[j] { v } else { hi[j] };
            n[j] += ok as usize;
        }
    }
    let (mut l, mut h, mut k) = (f64::INFINITY, f64::NEG_INFINITY, n.iter().sum::<usize>());
    for j in 0..8 {
        l = l.min(lo[j]);
        h = h.max(hi[j]);
    }
    for &v in rest.iter().filter(|v| v.is_finite()) {
        (l, h, k) = (l.min(v), h.max(v), k + 1);
    }
    (k, l, h)
}

/// Like [`quantile_sorted`] for unsorted finite data, in O(n) (reorders `v`).
fn quantile_select(v: &mut [f64], q: f64) -> f64 {
    let n = v.len();
    if n < 2 {
        return v.first().copied().unwrap_or(f64::NAN);
    }
    let pos = q.clamp(0.0, 1.0) * (n - 1) as f64;
    let i = pos.floor() as usize;
    let f = pos - i as f64;
    let (_, a, right) = v.select_nth_unstable_by(i, f64::total_cmp);
    let a = *a;
    match right.iter().copied().min_by(f64::total_cmp) {
        Some(b) if i + 1 < n => a * (1.0 - f) + b * f,
        _ => a,
    }
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
    if s.len() > 4096 {
        return (xs.clone(), kde_binned(&s, h, lo, hi, &xs, norm));
    }
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

/// KDE of many values: linear binning onto a fine grid (spacing ≤ h/8),
/// convolution with the truncated Gaussian, then linear interpolation at
/// `xs`. O(n + grid · kernel) instead of O(n · points); the error is far
/// below what can be seen.
fn kde_binned(s: &[f64], h: f64, lo: f64, hi: f64, xs: &[f64], norm: f64) -> Vec<f64> {
    let m = (((hi - lo) / (h / 8.0)).ceil() as usize + 1).clamp(256, 16_384);
    let dx = (hi - lo) / (m - 1) as f64;
    let mut w = vec![0.0f64; m];
    for &v in s {
        let t = (v - lo) / dx;
        let i = (t.floor() as usize).min(m - 2);
        let f = t - i as f64;
        w[i] += 1.0 - f;
        w[i + 1] += f;
    }
    let half = ((5.0 * h / dx).ceil() as usize).min(m);
    let kernel: Vec<f64> = (0..=half)
        .map(|k| (-0.5 * (k as f64 * dx / h).powi(2)).exp())
        .collect();
    let mut grid = vec![0.0f64; m];
    for (i, &wi) in w.iter().enumerate() {
        if wi == 0.0 {
            continue;
        }
        let a = i.saturating_sub(half);
        let b = (i + half).min(m - 1);
        for (j, g) in grid[a..=b].iter_mut().enumerate() {
            *g += wi * kernel[(a + j).abs_diff(i)];
        }
    }
    xs.iter()
        .map(|x| {
            let t = ((x - lo) / dx).clamp(0.0, (m - 1) as f64);
            let i = (t.floor() as usize).min(m - 2);
            let f = t - i as f64;
            (grid[i] * (1.0 - f) + grid[i + 1] * f) * norm
        })
        .collect()
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
    // One pass for count/min/max; quartiles (FD/Auto) by selection, no sort.
    let (cnt, lo, hi) = finite_min_max(values);
    if cnt == 0 {
        return match bins {
            Bins::Edges(e) => e.clone(),
            _ => vec![0.0, 1.0],
        };
    }
    let n = cnt as f64;
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
        Bins::Scott => width(3.49 * std_dev(values) * n.powf(-1.0 / 3.0)),
        Bins::FreedmanDiaconis | Bins::Auto => {
            let mut s: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
            let iqr = quantile_select(&mut s, 0.75) - quantile_select(&mut s, 0.25);
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

/// Equal-width binning of 4 values per step with AVX2: range check, bin
/// estimate and the near-edge test are vector operations; lanes near an edge
/// go through `exact`. Counts land in `n` (slot `nb` = outside). Returns the
/// number of values processed (a multiple of 4).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[allow(clippy::too_many_arguments)]
unsafe fn count_avx2(
    values: &[f64],
    e0: f64,
    en: f64,
    inv: f64,
    top: f64,
    nb: usize,
    exact: &dyn Fn(f64, usize) -> usize,
    n: &mut [u32],
) -> usize {
    use std::arch::x86_64::*;
    let (chunks, _) = values.as_chunks::<4>();
    let (ve0, ven, vinv, vtop) = (
        _mm256_set1_pd(e0),
        _mm256_set1_pd(en),
        _mm256_set1_pd(inv),
        _mm256_set1_pd(top),
    );
    let (lo_eps, hi_eps) = (_mm256_set1_pd(1e-9), _mm256_set1_pd(1.0 - 1e-9));
    let zero = _mm256_setzero_pd();
    for c in chunks {
        // SAFETY: `c` is 4 contiguous f64 (unaligned load).
        let v = unsafe { _mm256_loadu_pd(c.as_ptr()) };
        // inside: e0 <= v <= en (false for NaN).
        let inside = _mm256_and_pd(
            _mm256_cmp_pd::<_CMP_GE_OQ>(v, ve0),
            _mm256_cmp_pd::<_CMP_LE_OQ>(v, ven),
        );
        let f = _mm256_min_pd(_mm256_mul_pd(_mm256_sub_pd(v, ve0), vinv), vtop);
        let f = _mm256_blendv_pd(zero, f, inside);
        let fl = _mm256_floor_pd(f);
        let frac = _mm256_sub_pd(f, fl);
        let near = _mm256_or_pd(
            _mm256_cmp_pd::<_CMP_LT_OQ>(frac, lo_eps),
            _mm256_cmp_pd::<_CMP_GT_OQ>(frac, hi_eps),
        );
        let idx = _mm256_cvttpd_epi32(fl);
        let mut ix = [0i32; 4];
        // SAFETY: 4 × i32 = 16 bytes.
        unsafe { _mm_storeu_si128(ix.as_mut_ptr().cast(), idx) };
        let (inside, near) = (_mm256_movemask_pd(inside), _mm256_movemask_pd(near));
        for j in 0..4 {
            let b = if inside & (1 << j) == 0 {
                nb
            } else if near & (1 << j) != 0 {
                exact(c[j], ix[j] as usize)
            } else {
                ix[j] as usize
            };
            n[b] += 1;
        }
    }
    chunks.len() * 4
}

/// Counts per bin (`edges.len() - 1` bins; the last bin includes its right
/// edge; values outside are ignored).
pub fn histogram(values: &[f64], edges: &[f64]) -> Vec<f64> {
    let nb = edges.len().saturating_sub(1);
    let mut counts = vec![0.0; nb];
    if nb == 0 {
        return counts;
    }
    // Equal-width bins: direct index, corrected against the edges so the
    // result matches the binary search exactly.
    let (e0, en) = (edges[0], edges[nb]);
    let step = (en - e0) / nb as f64;
    let uniform = step > 0.0
        && step.is_finite()
        && edges
            .windows(2)
            .all(|w| ((w[1] - w[0]) - step).abs() <= step * 1e-9);
    if uniform {
        // The float estimate is exact except within rounding distance of an
        // edge; only those values (rare, predictable branch) are checked
        // against the edges so the result matches the binary search.
        const LANES: usize = 1;
        let inv = 1.0 / step;
        let top = (nb - 1) as f64;
        let exact = |v: f64, i: usize| -> usize {
            let mut i = i.min(nb - 1);
            while i > 0 && v < edges[i] {
                i -= 1;
            }
            while i + 1 < nb && v >= edges[i + 1] {
                i += 1;
            }
            i
        };
        let bin = |v: f64| -> usize {
            if !(v >= e0 && v <= en) {
                return nb;
            }
            let f = ((v - e0) * inv).min(top);
            // SAFETY: v is in [e0, en] (checked above), so f is finite and in
            // [0, nb - 1].
            let i = unsafe { f.to_int_unchecked::<usize>() };
            let frac = f - i as f64;
            if !(1e-9..=1.0 - 1e-9).contains(&frac) {
                return exact(v, i);
            }
            i
        };
        let count = |values: &[f64]| -> Vec<u32> {
            let mut n = vec![0u32; (nb + 1) * LANES];
            #[cfg(target_arch = "x86_64")]
            if std::arch::is_x86_feature_detected!("avx2") {
                // SAFETY: AVX2 support was checked at run time.
                let done = unsafe { count_avx2(values, e0, en, inv, top, nb, &exact, &mut n) };
                for &v in &values[done..] {
                    n[bin(v)] += 1;
                }
                return n;
            }
            let (chunks, rest) = values.as_chunks::<LANES>();
            for c in chunks {
                for j in 0..LANES {
                    n[j * (nb + 1) + bin(c[j])] += 1;
                }
            }
            for &v in rest {
                n[bin(v)] += 1;
            }
            n
        };
        let n = count(values);
        return (0..nb)
            .map(|i| (0..LANES).map(|j| n[j * (nb + 1) + i] as f64).sum())
            .collect();
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
    fn binned_kde_matches_exact() {
        let mut r = 12345u64;
        let mut next = || {
            r ^= r << 13;
            r ^= r >> 7;
            r ^= r << 17;
            (r >> 11) as f64 / (1u64 << 53) as f64
        };
        let v: Vec<f64> = (0..20_000)
            .map(|i| {
                if i % 3 == 0 {
                    next() * 2.0
                } else {
                    5.0 + next() + next() + next()
                }
            })
            .collect();
        let (xs, fast) = kde(&v, None, 200);
        let s = sorted(&v);
        let h = silverman_bandwidth(&s);
        let norm = 1.0 / (s.len() as f64 * h * (2.0 * std::f64::consts::PI).sqrt());
        let peak = fast.iter().copied().fold(0.0, f64::max);
        for (x, y) in xs.iter().zip(&fast) {
            let exact: f64 = s
                .iter()
                .map(|v| (-0.5 * ((x - v) / h).powi(2)).exp())
                .sum::<f64>()
                * norm;
            assert!((exact - y).abs() < peak * 2e-3, "{x}: {exact} vs {y}");
        }
    }

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

#[cfg(test)]
mod fast_paths {
    use super::*;

    fn reference(values: &[f64], edges: &[f64]) -> Vec<f64> {
        let nb = edges.len() - 1;
        let mut c = vec![0.0; nb];
        for &v in values.iter().filter(|v| v.is_finite()) {
            if v >= edges[0] && v <= edges[nb] {
                c[edges
                    .partition_point(|e| *e <= v)
                    .saturating_sub(1)
                    .min(nb - 1)] += 1.0;
            }
        }
        c
    }

    #[test]
    fn uniform_histogram_matches_search() {
        let mut x = 0x2545F4914F6CDD1Du64;
        let mut vals: Vec<f64> = (0..20_000)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x % 100_000) as f64 / 997.0 - 30.0
            })
            .collect();
        vals.extend([f64::NAN, f64::INFINITY, -1e9, 1e9]);
        for bins in [
            Bins::Count(7),
            Bins::Count(50),
            Bins::Width(0.3),
            Bins::Auto,
            Bins::Scott,
        ] {
            let e = bin_edges(&vals, &bins);
            // Exact edge values must land in the same bin as with the search.
            let mut v = vals.clone();
            v.extend(e.iter().copied());
            assert_eq!(histogram(&v, &e), reference(&v, &e), "{bins:?}");
        }
    }

    #[test]
    fn large_histogram_matches_search() {
        let mut x = 7u64;
        let vals: Vec<f64> = (0..2_600_000)
            .map(|i| {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                if i % 100_003 == 0 {
                    f64::NAN
                } else {
                    (x >> 11) as f64 / (1u64 << 53) as f64 * 37.0 - 5.0
                }
            })
            .collect();
        let e = bin_edges(&vals, &Bins::Count(64));
        assert_eq!(histogram(&vals, &e), reference(&vals, &e));
        let (n, lo, hi) = finite_min_max(&vals);
        let s = sorted(&vals);
        assert_eq!((n, lo, hi), (s.len(), s[0], s[s.len() - 1]));
    }

    #[test]
    fn select_quantile_matches_sorted() {
        let v: Vec<f64> = (0..101).map(|i| ((i * 37) % 101) as f64 * 0.5).collect();
        let s = sorted(&v);
        for q in [0.0, 0.25, 0.33, 0.5, 0.75, 1.0] {
            assert_eq!(quantile_select(&mut v.clone(), q), quantile_sorted(&s, q));
        }
    }
}
