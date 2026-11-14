//! Miscellaneous series: function sampling, stems, waterfalls, annotations, …

/// Samples `f` on `[x0, x1]` with at least `n` points, refining where the
/// curve bends; non-finite values split the curve.
pub(crate) fn sample_function(
    f: &dyn Fn(f64) -> f64,
    x0: f64,
    x1: f64,
    n: usize,
) -> (Vec<f64>, Vec<f64>) {
    let n = n.max(2);
    let mut xs: Vec<f64> = (0..n)
        .map(|i| x0 + (x1 - x0) * i as f64 / (n - 1) as f64)
        .collect();
    for _ in 0..3 {
        let ys: Vec<f64> = xs.iter().map(|x| f(*x)).collect();
        let span = ys
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(v), b.max(v))
            });
        let tol = (span.1 - span.0).abs() * 0.002;
        let mut out = Vec::with_capacity(xs.len() * 2);
        for i in 0..xs.len() - 1 {
            out.push(xs[i]);
            let xm = (xs[i] + xs[i + 1]) / 2.0;
            let ym = f(xm);
            let lin = (ys[i] + ys[i + 1]) / 2.0;
            if !(ym - lin).abs().is_finite() || (ym - lin).abs() > tol {
                out.push(xm);
            }
        }
        out.push(*xs.last().unwrap());
        if out.len() == xs.len() || out.len() > 20_000 {
            break;
        }
        xs = out;
    }
    let ys = xs
        .iter()
        .map(|x| f(*x))
        .map(|y| if y.is_finite() { y } else { f64::NAN })
        .collect();
    (xs, ys)
}
