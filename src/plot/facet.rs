//! Small multiples: one plot per group ([`Figure::facet_wrap`]) and the
//! scatter matrix of several variables ([`Figure::pair_plot`]).

use super::axes::Axes;
use super::color::Color;
use super::figure::Figure;
use super::series::IntoValues;

/// Data range with a 5 % margin on each side (None without finite data).
fn padded_range(v: &[f64]) -> Option<(f64, f64)> {
    let (lo, hi) = v
        .iter()
        .filter(|x| x.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
            (a.min(*x), b.max(*x))
        });
    if !lo.is_finite() {
        return None;
    }
    let pad = if hi > lo { (hi - lo) * 0.05 } else { 0.5 };
    Some((lo - pad, hi + pad))
}

impl Figure {
    /// One plot per `(name, data)` group in a grid with `cols` columns,
    /// titled with the name: `draw(axes, data)` fills each plot. The plots
    /// share their x and y ranges; tick labels are shown on the outer plots
    /// only.
    ///
    /// ```
    /// use dataviz::prelude::*;
    /// let mut fig = Figure::new(900, 600);
    /// let groups = [("a", vec![1.0, 3.0, 2.0]), ("b", vec![2.0, 1.0, 4.0]), ("c", vec![0.5, 2.5, 1.5])];
    /// fig.facet_wrap(groups, 2, |ax, y| {
    ///     ax.line([0.0, 1.0, 2.0], y);
    /// });
    /// ```
    pub fn facet_wrap<K: ToString, T>(
        &mut self,
        groups: impl IntoIterator<Item = (K, T)>,
        cols: usize,
        mut draw: impl FnMut(&mut Axes, &T),
    ) -> &mut Self {
        let groups: Vec<(String, T)> = groups
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        let n = groups.len();
        let cols = cols.clamp(1, n.max(1));
        self.grid(n.div_ceil(cols).max(1), cols);
        self.share_x(true).share_y(true);
        for (i, (name, data)) in groups.iter().enumerate() {
            let ax = self.at(i / cols, i % cols);
            ax.title(name.clone());
            draw(ax, data);
            // Outer plots only: the bottom one of each column, the first
            // of each row.
            if i + cols < n {
                ax.x_axis().tick_labels(false);
            }
            if i % cols != 0 {
                ax.y_axis().tick_labels(false);
            }
        }
        self
    }

    /// Scatter matrix (pair plot) of `(name, values)` columns: histograms
    /// on the diagonal and every pair of variables as a scatter plot
    /// elsewhere, with one x range per column and one y range per row.
    pub fn pair_plot<L: ToString, V: IntoValues>(
        &mut self,
        columns: impl IntoIterator<Item = (L, V)>,
    ) -> &mut Self {
        let cols: Vec<(String, Vec<f64>)> = columns
            .into_iter()
            .map(|(l, v)| (l.to_string(), v.into_values()))
            .collect();
        let n = cols.len();
        if n == 0 {
            return self;
        }
        self.grid(n, n);
        self.spacing(8.0);
        let ranges: Vec<Option<(f64, f64)>> = cols.iter().map(|(_, v)| padded_range(v)).collect();
        let color = self
            .theme_mut()
            .palette
            .first()
            .copied()
            .unwrap_or(Color::BLUE);
        let len = cols.iter().map(|c| c.1.len()).min().unwrap_or(0);
        // Small, translucent points for many rows; larger for few.
        let (size, alpha) = match len {
            0..=200 => (5.0, 0.8),
            201..=2000 => (3.5, 0.5),
            _ => (2.0, 0.3),
        };
        for r in 0..n {
            for c in 0..n {
                let ax = self.at(r, c);
                if r == c {
                    ax.hist(&cols[c].1[..]).color(color);
                    ax.y_axis().tick_labels(false);
                } else {
                    ax.scatter(&cols[c].1[..len], &cols[r].1[..len])
                        .color(color)
                        .size(size)
                        .opacity(alpha);
                    if let Some((a, b)) = ranges[r] {
                        ax.y_limits(a, b);
                    }
                }
                if let Some((a, b)) = ranges[c] {
                    ax.x_limits(a, b);
                }
                if r + 1 == n {
                    ax.x_label(cols[c].0.clone());
                } else {
                    ax.x_axis().tick_labels(false);
                }
                if c == 0 {
                    ax.y_label(cols[r].0.clone());
                } else if r != c {
                    ax.y_axis().tick_labels(false);
                }
                ax.legend(super::LegendPosition::Hidden);
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facets_and_pairs_render() {
        let mut fig = Figure::new(600, 400);
        let groups: Vec<(String, Vec<f64>)> = (0..5)
            .map(|k| (format!("g{k}"), (0..20).map(|i| (i * k) as f64).collect()))
            .collect();
        fig.facet_wrap(groups, 3, |ax, y| {
            ax.line((0..20).map(|i| i as f64), y);
        });
        let (_, l) = fig.scene_and_layouts();
        assert_eq!(l.len(), 5);
        // Shared ranges.
        assert_eq!((l[0].y.min, l[0].y.max), (l[4].y.min, l[4].y.max));

        let mut fig = Figure::new(600, 600);
        let a: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let b: Vec<f64> = a.iter().map(|v| v * v).collect();
        let c: Vec<f64> = a.iter().map(|v| v.sin()).collect();
        fig.pair_plot([("a", &a), ("b", &b), ("c", &c)]);
        let (_, l) = fig.scene_and_layouts();
        assert_eq!(l.len(), 9);
        // Column 1 (b) has the same x range in every row.
        assert!((l[1].x.max - l[7].x.max).abs() < 1e-9);
        assert!(fig.to_svg().contains(">b<"));
    }
}
