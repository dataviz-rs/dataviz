//! Grammar of graphics: a chart described by data, a mark and encodings
//! (as in Vega-Lite, ggplot2 or Altair) instead of by chart type. Fields
//! map to x, y, color, size and facets; groups, aggregation, binning,
//! stacking, legends, axis titles and small multiples follow from the
//! encodings.
//!
//! ```
//! use dataviz::prelude::*;
//! use dataviz::plot::grammar::{Chart, Data, Mark};
//!
//! let data = Data::new()
//!     .num("flipper", [181.0, 186.0, 195.0, 210.0, 217.0, 230.0])
//!     .num("mass", [3750.0, 3800.0, 3250.0, 4200.0, 4900.0, 5700.0])
//!     .text("species", ["Adelie", "Adelie", "Adelie", "Gentoo", "Gentoo", "Gentoo"]);
//! let mut fig = Figure::new(700, 450);
//! Chart::new(data, Mark::Point)
//!     .x("flipper")
//!     .y("mass")
//!     .color("species")
//!     .draw(&mut fig)
//!     .unwrap();
//! assert!(fig.to_svg().contains("Gentoo"));
//! ```
//!
//! The same chart as a JSON spec (`plot::spec`, Python, JavaScript, C):
//! `{"data": {"flipper": […], "mass": […], "species": […]}, "mark":
//! "point", "encoding": {"x": "flipper", "y": "mass", "color": "species"}}`.

use super::axes::Axes;
use super::color::Colormap;
use super::figure::Figure;
use super::scale::Scale;
use super::series::bar::BarMode;
use super::series::IntoValues;
use super::style::Marker;
use super::time::parse_datetime;
use std::collections::HashMap;

/// A column of a [`Data`] table.
#[derive(Clone, Debug)]
pub enum Column {
    /// Numbers (NaN for missing values).
    Num(Vec<f64>),
    /// Text: categories, or dates (read as [`Kind::Temporal`]).
    Text(Vec<String>),
}

impl Column {
    fn len(&self) -> usize {
        match self {
            Column::Num(v) => v.len(),
            Column::Text(v) => v.len(),
        }
    }
}

/// A table of named columns.
#[derive(Clone, Debug, Default)]
pub struct Data {
    cols: Vec<(String, Column)>,
}

impl Data {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }
    /// Adds a numeric column.
    pub fn num(mut self, name: impl Into<String>, values: impl IntoValues) -> Self {
        self.cols
            .push((name.into(), Column::Num(values.into_values())));
        self
    }
    /// Adds a text column (categories or dates).
    pub fn text<S: ToString>(
        mut self,
        name: impl Into<String>,
        values: impl IntoIterator<Item = S>,
    ) -> Self {
        let v = values.into_iter().map(|s| s.to_string()).collect();
        self.cols.push((name.into(), Column::Text(v)));
        self
    }
    /// Adds a column.
    pub fn column(mut self, name: impl Into<String>, column: Column) -> Self {
        self.cols.push((name.into(), column));
        self
    }
    /// Number of rows (of the shortest column).
    pub fn len(&self) -> usize {
        self.cols.iter().map(|(_, c)| c.len()).min().unwrap_or(0)
    }
    /// True without rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn get(&self, name: &str) -> Result<&Column, String> {
        self.cols
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c)
            .ok_or_else(|| {
                let names: Vec<&str> = self.cols.iter().map(|(n, _)| n.as_str()).collect();
                format!(
                    "no field `{name}` in the data (fields: {})",
                    names.join(", ")
                )
            })
    }
}

/// How a field's values are read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Continuous numbers.
    Quantitative,
    /// Unordered categories.
    Nominal,
    /// Ordered categories (sorted).
    Ordinal,
    /// Dates and times.
    Temporal,
}

/// Summary of the rows that share a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aggregate {
    /// Number of rows.
    Count,
    /// Sum.
    Sum,
    /// Mean.
    Mean,
    /// Median.
    Median,
    /// Minimum.
    Min,
    /// Maximum.
    Max,
}

impl Aggregate {
    /// From a name: count, sum, mean (average), median, min, max.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "count" => Aggregate::Count,
            "sum" => Aggregate::Sum,
            "mean" | "average" | "avg" => Aggregate::Mean,
            "median" => Aggregate::Median,
            "min" => Aggregate::Min,
            "max" => Aggregate::Max,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Aggregate::Count => "count",
            Aggregate::Sum => "sum",
            Aggregate::Mean => "mean",
            Aggregate::Median => "median",
            Aggregate::Min => "min",
            Aggregate::Max => "max",
        }
    }
    fn apply(self, v: &mut [f64]) -> f64 {
        let n = v.len();
        if self == Aggregate::Count {
            return n as f64;
        }
        if n == 0 {
            return f64::NAN;
        }
        match self {
            Aggregate::Sum => v.iter().sum(),
            Aggregate::Mean => v.iter().sum::<f64>() / n as f64,
            Aggregate::Min => v.iter().copied().fold(f64::INFINITY, f64::min),
            Aggregate::Max => v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            Aggregate::Median => {
                v.sort_by(|a, b| a.total_cmp(b));
                if n % 2 == 1 {
                    v[n / 2]
                } else {
                    (v[n / 2 - 1] + v[n / 2]) / 2.0
                }
            }
            Aggregate::Count => n as f64,
        }
    }
}

/// A field mapped to a channel, with how to read and summarize it.
#[derive(Clone, Debug, Default)]
pub struct Field {
    /// Column name.
    pub name: String,
    /// How values are read (default: numbers are quantitative, text
    /// nominal, dates temporal).
    pub kind: Option<Kind>,
    /// Summary of the rows at one position.
    pub aggregate: Option<Aggregate>,
    /// Bins for histograms (`Some(0)`: automatic).
    pub bin: Option<usize>,
    /// Axis or legend title (default: the field name).
    pub title: Option<String>,
    /// Logarithmic scale.
    pub log: bool,
}

impl Field {
    /// A field by column name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
    /// Sets how values are read.
    pub fn kind(mut self, kind: Kind) -> Self {
        self.kind = Some(kind);
        self
    }
    /// Summarizes rows that share a position.
    pub fn aggregate(mut self, a: Aggregate) -> Self {
        self.aggregate = Some(a);
        self
    }
    /// Bins the values (0: automatic count).
    pub fn bin(mut self, bins: usize) -> Self {
        self.bin = Some(bins);
        self
    }
    /// Title of the axis or legend.
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }
    /// Logarithmic scale.
    pub fn log(mut self, on: bool) -> Self {
        self.log = on;
        self
    }
    fn label(&self) -> String {
        if let Some(t) = &self.title {
            return t.clone();
        }
        match self.aggregate {
            Some(Aggregate::Count) if self.name.is_empty() => "count".into(),
            Some(a) => format!("{} of {}", a.name(), self.name),
            None => self.name.clone(),
        }
    }
}

impl From<&str> for Field {
    fn from(s: &str) -> Self {
        Field::new(s)
    }
}

impl From<String> for Field {
    fn from(s: String) -> Self {
        Field::new(s)
    }
}

/// The geometric shape that shows each row (or group of rows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Points (scatter, strip and bubble charts).
    Point,
    /// Lines, one per color group, sorted by x.
    Line,
    /// Filled areas (stacked per color group).
    Area,
    /// Bars: aggregated per category, stacked per color group; histograms
    /// with a binned x.
    Bar,
    /// Colored cells of a nominal x × y grid (heatmap).
    Rect,
    /// Box plots of y per x category.
    Boxplot,
}

impl Mark {
    /// From a name: point, circle, square, line, area, bar, rect, heatmap,
    /// boxplot.
    pub fn parse(s: &str) -> Option<(Self, Marker)> {
        Some(match s.to_ascii_lowercase().as_str() {
            "point" | "circle" | "scatter" => (Mark::Point, Marker::Circle),
            "square" => (Mark::Point, Marker::Square),
            "line" | "trail" => (Mark::Line, Marker::None),
            "area" => (Mark::Area, Marker::None),
            "bar" | "column" | "histogram" => (Mark::Bar, Marker::None),
            "rect" | "heatmap" | "tile" => (Mark::Rect, Marker::None),
            "boxplot" | "box" => (Mark::Boxplot, Marker::None),
            _ => return None,
        })
    }
}

/// A chart: data, a mark and encodings. See the [module docs](self).
#[derive(Clone, Debug)]
pub struct Chart {
    data: Data,
    mark: Mark,
    marker: Marker,
    x: Option<Field>,
    y: Option<Field>,
    color: Option<Field>,
    size: Option<Field>,
    facet: Option<Field>,
    columns: usize,
    stack: BarMode,
    colormap: Option<Colormap>,
    title: Option<String>,
}

/// A channel resolved against the data: numbers (positions) and, for
/// categories, the labels in order.
struct Resolved<'a> {
    values: std::borrow::Cow<'a, [f64]>,
    cats: Option<Vec<String>>,
    kind: Kind,
}

impl Chart {
    /// A chart of `data` drawn with `mark`.
    pub fn new(data: Data, mark: Mark) -> Self {
        Self {
            data,
            mark,
            marker: Marker::Circle,
            x: None,
            y: None,
            color: None,
            size: None,
            facet: None,
            columns: 0,
            stack: BarMode::Stacked,
            colormap: None,
            title: None,
        }
    }
    /// Point marker shape.
    pub fn marker(mut self, m: Marker) -> Self {
        self.marker = m;
        self
    }
    /// Field on the x axis.
    pub fn x(mut self, f: impl Into<Field>) -> Self {
        self.x = Some(f.into());
        self
    }
    /// Field on the y axis.
    pub fn y(mut self, f: impl Into<Field>) -> Self {
        self.y = Some(f.into());
        self
    }
    /// Field for color: categories give one series per value (with a
    /// legend), numbers a colormap.
    pub fn color(mut self, f: impl Into<Field>) -> Self {
        self.color = Some(f.into());
        self
    }
    /// Field for point size (bubbles).
    pub fn size(mut self, f: impl Into<Field>) -> Self {
        self.size = Some(f.into());
        self
    }
    /// One plot per value of the field (small multiples), `columns` per
    /// row (0: automatic).
    pub fn facet(mut self, f: impl Into<Field>, columns: usize) -> Self {
        self.facet = Some(f.into());
        self.columns = columns;
        self
    }
    /// How bars of several color groups combine (default stacked).
    pub fn stack(mut self, mode: BarMode) -> Self {
        self.stack = mode;
        self
    }
    /// Colormap for quantitative colors and heatmaps.
    pub fn colormap(mut self, cm: Colormap) -> Self {
        self.colormap = Some(cm);
        self
    }
    /// Title of the plot (of the figure with facets).
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    /// Draws into a figure: one plot, or a grid of plots with a facet.
    pub fn draw(&self, fig: &mut Figure) -> Result<(), String> {
        let Some(facet) = &self.facet else {
            return self.draw_into(fig.axes());
        };
        let all: Vec<usize> = (0..self.data.len()).collect();
        let groups = self.groups(facet, &all)?;
        let n = groups.len().max(1);
        let cols = if self.columns > 0 {
            self.columns
        } else {
            (n as f64).sqrt().ceil() as usize
        };
        if let Some(t) = &self.title {
            fig.title(t.clone());
        }
        let mut err = None;
        fig.facet_wrap(groups, cols, |ax, rows| {
            if let Err(e) = self.draw_rows(ax, rows) {
                err.get_or_insert(e);
            }
        });
        // One legend (the groups are the same in every plot).
        for ax in fig.all_axes().skip(1) {
            ax.legend(super::legend::LegendPosition::Hidden);
        }
        err.map_or(Ok(()), Err)
    }

    /// Draws into one plot (a facet is ignored).
    pub fn draw_into(&self, ax: &mut Axes) -> Result<(), String> {
        if let Some(t) = &self.title {
            ax.title(t.clone());
        }
        let all: Vec<usize> = (0..self.data.len()).collect();
        self.draw_rows(ax, &all)
    }

    /// Rows split by the values of a field, in order of appearance (sorted
    /// for numbers and ordinal fields).
    fn groups(&self, f: &Field, rows: &[usize]) -> Result<Vec<(String, Vec<usize>)>, String> {
        let col = self.data.get(&f.name)?;
        let mut out: Vec<(String, Vec<usize>)> = Vec::new();
        // Keys without per-row allocation: bit patterns of numbers,
        // borrowed text.
        match col {
            Column::Num(v) => {
                let mut keys = Keys::default();
                for &r in rows {
                    let x = v[r];
                    let bits = if x.is_nan() {
                        f64::NAN.to_bits()
                    } else {
                        (x + 0.0).to_bits()
                    };
                    let i = keys.index(bits, || {
                        out.push((fmt_num(x), Vec::new()));
                        out.len() - 1
                    });
                    out[i].1.push(r);
                }
            }
            Column::Text(v) => {
                let mut keys = Keys::default();
                for &r in rows {
                    let i = keys.index(v[r].as_str(), || {
                        out.push((v[r].clone(), Vec::new()));
                        out.len() - 1
                    });
                    out[i].1.push(r);
                }
            }
        }
        let sort_num = matches!(col, Column::Num(_)) || f.kind == Some(Kind::Ordinal);
        if sort_num {
            out.sort_by(|a, b| match (a.0.parse::<f64>(), b.0.parse::<f64>()) {
                (Ok(x), Ok(y)) => x.total_cmp(&y),
                _ => a.0.cmp(&b.0),
            });
        }
        Ok(out)
    }

    /// A channel's values for all rows of the table.
    fn resolve(&self, f: &Field) -> Result<Resolved<'_>, String> {
        let col = self.data.get(&f.name)?;
        let kind = self.kind(f, col);
        Ok(match (col, kind) {
            (Column::Num(v), Kind::Quantitative | Kind::Temporal) => Resolved {
                values: v.as_slice().into(),
                cats: None,
                kind,
            },
            (Column::Text(v), Kind::Temporal) => Resolved {
                values: v
                    .iter()
                    .map(|s| parse_datetime(s).unwrap_or(f64::NAN))
                    .collect::<Vec<_>>()
                    .into(),
                cats: None,
                kind,
            },
            (Column::Text(v), Kind::Quantitative) => Resolved {
                values: v
                    .iter()
                    .map(|s| s.trim().parse().unwrap_or(f64::NAN))
                    .collect::<Vec<_>>()
                    .into(),
                cats: None,
                kind,
            },
            (col, kind) => {
                // Categories: positions 0, 1, 2, … in order of appearance
                // (sorted when ordinal or numeric).
                let all: Vec<usize> = (0..col.len()).collect();
                let groups = self.groups(
                    &Field {
                        kind: Some(kind),
                        ..f.clone()
                    },
                    &all,
                )?;
                let mut values = vec![f64::NAN; col.len()];
                for (i, (_, rows)) in groups.iter().enumerate() {
                    for &r in rows {
                        values[r] = i as f64;
                    }
                }
                Resolved {
                    values: values.into(),
                    cats: Some(groups.into_iter().map(|(k, _)| k).collect()),
                    kind,
                }
            }
        })
    }

    /// How a field is read: as given, else numbers are quantitative, text
    /// of dates temporal and other text nominal.
    fn kind(&self, f: &Field, col: &Column) -> Kind {
        f.kind.unwrap_or(match col {
            Column::Num(_) => Kind::Quantitative,
            Column::Text(v) => {
                let dates = !v.is_empty()
                    && v.iter().take(50).all(|s| parse_datetime(s).is_some())
                    && v.iter().any(|s| s.contains('-') || s.contains(':'));
                if dates {
                    Kind::Temporal
                } else {
                    Kind::Nominal
                }
            }
        })
    }

    fn draw_rows(&self, ax: &mut Axes, rows: &[usize]) -> Result<(), String> {
        let x = self.x.as_ref().map(|f| self.resolve(f)).transpose()?;
        let y = self.y.as_ref().map(|f| self.resolve(f)).transpose()?;
        // Color groups (categorical color) or a colormap (numeric color).
        let numeric_color = match &self.color {
            Some(f) => matches!(
                self.kind(f, self.data.get(&f.name)?),
                Kind::Quantitative | Kind::Temporal
            ),
            None => false,
        };
        let color = match &self.color {
            Some(f) if numeric_color => Some(self.resolve(f)?),
            _ => None,
        };
        // Every group of the whole table, in the same order in every facet
        // (same colors; empty where a facet lacks the group).
        let groups: Vec<(Option<String>, Vec<usize>)> = match (&self.color, numeric_color) {
            (Some(f), false) => {
                let all: Vec<usize> = (0..self.data.len()).collect();
                let groups = self.groups(f, &all)?;
                if rows.len() == all.len() {
                    groups.into_iter().map(|(k, r)| (Some(k), r)).collect()
                } else {
                    let mut inside = vec![false; self.data.len()];
                    for &r in rows {
                        inside[r] = true;
                    }
                    groups
                        .into_iter()
                        .map(|(k, r)| (Some(k), r.into_iter().filter(|&i| inside[i]).collect()))
                        .collect()
                }
            }
            _ => vec![(None, rows.to_vec())],
        };
        for (field, axis) in [(&self.x, 0), (&self.y, 1)] {
            if let Some(f) = field {
                let (r, a) = if axis == 0 {
                    (x.as_ref(), ax.x_axis())
                } else {
                    (y.as_ref(), ax.y_axis())
                };
                a.label(f.label());
                if f.log {
                    a.scale(Scale::Log(10.0));
                } else if r.is_some_and(|r| r.kind == Kind::Temporal) {
                    a.scale(Scale::Time);
                }
            }
        }
        let pick = |r: &Resolved, rows: &[usize]| -> Vec<f64> {
            rows.iter().map(|&i| r.values[i]).collect()
        };
        match self.mark {
            Mark::Point => {
                let (x, y) = need_xy(&x, &y, "point")?;
                for (name, rows) in &groups {
                    let sc = ax.scatter(pick(x, rows), pick(y, rows));
                    sc.marker(self.marker);
                    if let Some(n) = name {
                        sc.label(n.clone());
                    }
                    if numeric_color {
                        if let (Some(c), Some(f)) = (&color, &self.color) {
                            sc.values(pick(c, rows));
                            sc.colormap(self.colormap.unwrap_or(Colormap::Viridis));
                            sc.colorbar(f.label());
                        }
                    }
                    if let Some(f) = &self.size {
                        // Area proportional to the value within the range
                        // of the whole table.
                        let s = self.resolve(f)?;
                        let (lo, hi) = finite_range(&s.values);
                        let px = rows.iter().map(|&i| {
                            let t = ((s.values[i] - lo) / (hi - lo).max(1e-300)).clamp(0.0, 1.0);
                            if s.values[i].is_finite() {
                                3.0 + 15.0 * t.sqrt()
                            } else {
                                3.0
                            }
                        });
                        sc.sizes(px.collect::<Vec<_>>());
                        sc.opacity(0.75);
                    }
                }
                categories(ax, &x.cats, &y.cats);
            }
            Mark::Line | Mark::Area => {
                let (x, y) = need_xy(&x, &y, "line")?;
                let agg = self.y.as_ref().and_then(|f| f.aggregate);
                let mut layers = Vec::new();
                for (name, rows) in &groups {
                    let (xs, ys) = summarize(&pick(x, rows), &pick(y, rows), agg);
                    layers.push((name.clone(), xs, ys));
                }
                if self.mark == Mark::Area && layers.len() > 1 {
                    // Stacked areas over the union of x positions.
                    let mut xs: Vec<f64> = layers.iter().flat_map(|l| l.1.clone()).collect();
                    xs.sort_by(|a, b| a.total_cmp(b));
                    xs.dedup();
                    let stacked = layers.iter().map(|(n, lx, ly)| {
                        let v = xs
                            .iter()
                            .map(|xv| lx.iter().position(|a| a == xv).map_or(0.0, |i| ly[i]))
                            .collect();
                        (n.clone().unwrap_or_default(), v)
                    });
                    ax.stacked_area(xs.clone(), stacked.collect::<Vec<_>>(), false);
                } else {
                    for (name, xs, ys) in layers {
                        let l = if self.mark == Mark::Area {
                            ax.area(xs, ys)
                        } else {
                            ax.line(xs, ys)
                        };
                        if let Some(n) = name {
                            l.label(n);
                        }
                    }
                }
                categories(ax, &x.cats, &None);
            }
            Mark::Bar => self.bars(ax, &x, &y, &groups)?,
            Mark::Rect => {
                let (x, y) = need_xy(&x, &y, "rect")?;
                let (Some(xc), Some(yc)) = (&x.cats, &y.cats) else {
                    return Err("rect needs categorical x and y".into());
                };
                let (c, agg) = match (&color, &self.color) {
                    (Some(c), Some(f)) if c.cats.is_none() => {
                        (Some(c), f.aggregate.unwrap_or(Aggregate::Mean))
                    }
                    _ => (None, Aggregate::Count),
                };
                let mut cells: Vec<Vec<Vec<f64>>> = vec![vec![Vec::new(); xc.len()]; yc.len()];
                for &r in rows {
                    let (cx, cy) = (x.values[r], y.values[r]);
                    if cx.is_finite() && cy.is_finite() {
                        cells[cy as usize][cx as usize].push(c.map_or(1.0, |c| c.values[r]));
                    }
                }
                let z: Vec<Vec<f64>> = cells
                    .into_iter()
                    .map(|row| row.into_iter().map(|mut v| agg.apply(&mut v)).collect())
                    .collect();
                let h = ax.heatmap_labeled(xc.clone(), yc.clone(), z);
                if let Some(cm) = self.colormap {
                    h.colormap(cm);
                }
                if let Some(f) = &self.color {
                    h.colorbar(f.label());
                }
            }
            Mark::Boxplot => {
                let (x, y) = need_xy(&x, &y, "boxplot")?;
                let Some(cats) = &x.cats else {
                    return Err("boxplot needs a categorical x".into());
                };
                let mut g: Vec<(String, Vec<f64>)> =
                    cats.iter().map(|c| (c.clone(), Vec::new())).collect();
                for &r in rows {
                    let cx = x.values[r];
                    if cx.is_finite() {
                        g[cx as usize].1.push(y.values[r]);
                    }
                }
                ax.boxplot(g);
            }
        }
        Ok(())
    }

    fn bars(
        &self,
        ax: &mut Axes,
        x: &Option<Resolved<'_>>,
        y: &Option<Resolved<'_>>,
        groups: &[(Option<String>, Vec<usize>)],
    ) -> Result<(), String> {
        let x_field = self.x.as_ref().ok_or("bar needs x")?;
        let x = x.as_ref().ok_or("bar needs x")?;
        // Histogram: binned x.
        if x_field.bin.is_some() || (y.is_none() && x.cats.is_none()) {
            // The same nice bin edges for every group (and facet).
            let (lo, hi) = finite_range(&x.values);
            let n = x.values.len();
            let want = x_field
                .bin
                .filter(|b| *b > 0)
                .unwrap_or(((n as f64).sqrt() as usize).clamp(10, 50));
            let step = super::ticks::nice_step((hi - lo).max(1e-12) / want as f64);
            let first = (lo / step).floor() * step;
            let count = (((hi - first) / step).floor() as usize + 1).min(10_000);
            let edges: Vec<f64> = (0..=count).map(|i| first + step * i as f64).collect();
            for (name, rows) in groups {
                let h = ax.hist(rows.iter().map(|&i| x.values[i]).collect::<Vec<_>>());
                if lo.is_finite() && hi > lo {
                    h.bins(super::stats::Bins::Edges(edges.clone()));
                }
                if let Some(n) = name {
                    h.label(n.clone());
                    h.opacity(0.65);
                }
            }
            ax.y_axis().label("count");
            return Ok(());
        }
        // Horizontal bars when y holds the categories.
        let horizontal = x.cats.is_none() && y.as_ref().is_some_and(|y| y.cats.is_some());
        let (cat, val, val_field) = if horizontal {
            (y.as_ref().ok_or("bar needs y")?, Some(x), Some(x_field))
        } else {
            (x, y.as_ref(), self.y.as_ref())
        };
        let agg = val_field
            .and_then(|f| f.aggregate)
            .unwrap_or(if val.is_some() {
                Aggregate::Sum
            } else {
                Aggregate::Count
            });
        if val.is_none() {
            ax.y_axis().label("count");
        }
        let Some(cats) = &cat.cats else {
            // Numeric positions: one bar per distinct x (summarized).
            for (name, rows) in groups {
                let xs: Vec<f64> = rows.iter().map(|&i| cat.values[i]).collect();
                let ys: Vec<f64> = match val {
                    Some(v) => rows.iter().map(|&i| v.values[i]).collect(),
                    None => vec![1.0; rows.len()],
                };
                let (xs, ys) = summarize(&xs, &ys, Some(agg));
                let b = ax.bar_xy(xs, ys);
                if let Some(n) = name {
                    b.label(n.clone());
                }
            }
            return Ok(());
        };
        let series: Vec<(String, Vec<f64>)> = groups
            .iter()
            .map(|(name, rows)| {
                let mut cells: Vec<Vec<f64>> = vec![Vec::new(); cats.len()];
                for &r in rows {
                    let c = cat.values[r];
                    if c.is_finite() {
                        cells[c as usize].push(val.map_or(1.0, |v| v.values[r]));
                    }
                }
                let v = cells.iter_mut().map(|c| agg.apply(c)).collect();
                (name.clone().unwrap_or_default(), v)
            })
            .collect();
        if series.len() == 1 && groups[0].0.is_none() {
            let (_, v) = series.into_iter().next().unwrap_or_default();
            if horizontal {
                ax.barh(cats.clone(), v);
            } else {
                ax.bar(cats.clone(), v);
            }
        } else {
            ax.bars(cats.clone(), series, self.stack, horizontal);
        }
        Ok(())
    }
}

fn need_xy<'a, 'b>(
    x: &'a Option<Resolved<'b>>,
    y: &'a Option<Resolved<'b>>,
    mark: &str,
) -> Result<(&'a Resolved<'b>, &'a Resolved<'b>), String> {
    match (x, y) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(format!("{mark} needs x and y")),
    }
}

/// Category names as tick labels of axes holding category positions.
fn categories(ax: &mut Axes, x: &Option<Vec<String>>, y: &Option<Vec<String>>) {
    if let Some(c) = x {
        ax.x_axis().categories(c.clone());
    }
    if let Some(c) = y {
        ax.y_axis().categories(c.clone());
    }
}

/// Points sorted by x; with an aggregate, one point per distinct x.
fn summarize(x: &[f64], y: &[f64], agg: Option<Aggregate>) -> (Vec<f64>, Vec<f64>) {
    let mut p: Vec<(f64, f64)> = x
        .iter()
        .zip(y)
        .map(|(a, b)| (*a, *b))
        .filter(|(a, _)| !a.is_nan())
        .collect();
    p.sort_by(|a, b| a.0.total_cmp(&b.0));
    let Some(agg) = agg else {
        return p.into_iter().unzip();
    };
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    let mut i = 0;
    while i < p.len() {
        let mut j = i;
        while j < p.len() && p[j].0 == p[i].0 {
            j += 1;
        }
        let mut v: Vec<f64> = p[i..j].iter().map(|q| q.1).collect();
        xs.push(p[i].0);
        ys.push(agg.apply(&mut v));
        i = j;
    }
    (xs, ys)
}

/// Group indices by key: a linear scan while there are few keys (the
/// usual case for colors and facets), a hash map beyond.
struct Keys<K> {
    small: Vec<(K, usize)>,
    map: HashMap<K, usize>,
}

impl<K> Default for Keys<K> {
    fn default() -> Self {
        Self {
            small: Vec::new(),
            map: HashMap::new(),
        }
    }
}

impl<K: Copy + Eq + std::hash::Hash> Keys<K> {
    fn index(&mut self, k: K, new: impl FnOnce() -> usize) -> usize {
        if self.map.is_empty() {
            if let Some(&(_, i)) = self.small.iter().find(|e| e.0 == k) {
                return i;
            }
            if self.small.len() < 16 {
                let i = new();
                self.small.push((k, i));
                return i;
            }
            self.map.extend(self.small.drain(..));
        }
        *self.map.entry(k).or_insert_with(new)
    }
}

fn finite_range(v: &[f64]) -> (f64, f64) {
    v.iter()
        .filter(|x| x.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
            (a.min(*x), b.max(*x))
        })
}

fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn penguins() -> Data {
        Data::new()
            .text("species", ["A", "A", "B", "B", "C", "C"])
            .text("island", ["x", "y", "x", "y", "x", "y"])
            .num("mass", [3.0, 4.0, 5.0, 6.0, 4.5, 3.5])
            .num("len", [10.0, 12.0, 15.0, 16.0, 13.0, 11.0])
    }

    #[test]
    fn color_groups_become_series_with_a_legend() {
        let mut fig = Figure::new(500, 400);
        Chart::new(penguins(), Mark::Point)
            .x("len")
            .y("mass")
            .color("species")
            .draw(&mut fig)
            .unwrap();
        assert_eq!(fig.axes().series_count(), 3);
        let svg = fig.to_svg();
        assert!(svg.contains(">len<") && svg.contains(">mass<"));
    }

    #[test]
    fn bars_aggregate_and_stack_and_facets_split() {
        let mut fig = Figure::new(500, 400);
        Chart::new(penguins(), Mark::Bar)
            .x("species")
            .y(Field::new("mass").aggregate(Aggregate::Mean))
            .color("island")
            .draw(&mut fig)
            .unwrap();
        assert_eq!(fig.axes().series_count(), 2);
        assert!(fig.to_svg().contains("mean of mass"));

        let mut fig = Figure::new(600, 400);
        Chart::new(penguins(), Mark::Point)
            .x("len")
            .y("mass")
            .facet("island", 2)
            .draw(&mut fig)
            .unwrap();
        assert_eq!(fig.all_axes().count(), 2);
    }

    #[test]
    fn histogram_heatmap_boxplot_and_errors() {
        let mut fig = Figure::new(500, 400);
        let c = Chart::new(penguins(), Mark::Bar).x(Field::new("mass").bin(4));
        c.draw(&mut fig).unwrap();
        let mut fig = Figure::new(500, 400);
        Chart::new(penguins(), Mark::Rect)
            .x("species")
            .y("island")
            .color("mass")
            .draw(&mut fig)
            .unwrap();
        let mut fig = Figure::new(500, 400);
        Chart::new(penguins(), Mark::Boxplot)
            .x("species")
            .y("mass")
            .draw(&mut fig)
            .unwrap();
        let err = Chart::new(penguins(), Mark::Point)
            .x("nope")
            .y("mass")
            .draw(&mut Figure::new(100, 100))
            .unwrap_err();
        assert!(err.contains("fields: species"), "{err}");
    }

    #[test]
    fn dates_are_temporal_and_lines_aggregate() {
        let d = Data::new()
            .text(
                "day",
                ["2024-01-01", "2024-01-02", "2024-01-02", "2024-01-03"],
            )
            .num("v", [1.0, 2.0, 4.0, 3.0]);
        let mut fig = Figure::new(500, 400);
        Chart::new(d, Mark::Line)
            .x("day")
            .y(Field::new("v").aggregate(Aggregate::Sum))
            .draw(&mut fig)
            .unwrap();
        assert!(fig.to_svg().contains("sum of v"));
    }
}
