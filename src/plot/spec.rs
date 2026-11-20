//! Figures from a JSON spec (feature `spec`): a grid of panels, each one
//! chart type (or several `layers` on the same axes) with its data and
//! options. Used by the MCP server and the Python bindings; handy for
//! charts described in configuration files or sent over a network.
//!
//! ```
//! # #[cfg(feature = "spec")] {
//! let spec = serde_json::json!({
//!     "title": "Sales",
//!     "panels": [
//!         {"type": "bar", "categories": ["Q1", "Q2"], "values": [3, 5]},
//!         {"layers": [{"type": "scatter", "x": [1, 2, 3], "y": [2, 4, 3]},
//!                     {"type": "line", "x": [1, 3], "y": [2, 3.5]}]}
//!     ]
//! });
//! let mut fig = dataviz::plot::spec::figure(&spec).unwrap();
//! let png = fig.encode(dataviz::plot::Format::Png).unwrap();
//! # }
//! ```

use crate::plot::geo::Projection;
use crate::plot::grammar::{self, Aggregate, Column, Data, Field, Kind, Mark};
use crate::plot::series::bar::BarMode;
use crate::prelude::*;
use serde_json::Value;

/// Chart types of a panel's `type`, with their fields (shown by the
/// `chart_types` tool).
pub const CHART_TYPES: &[(&str, &str, &str)] = &[
    (
        "line",
        "x, y, name? (or series: [{name, x, y, color}])",
        "Lines; several series via `series`.",
    ),
    (
        "scatter",
        "x, y, name?, color?, size?  (or series)",
        "Points.",
    ),
    ("area", "x, y (or series)", "Filled area under a line."),
    (
        "bar",
        "categories (or labels/x), values (or y); series: [{name, values}] for grouped bars",
        "Vertical bars.",
    ),
    ("barh", "categories, values", "Horizontal bars."),
    ("hist", "values, bins?", "Histogram."),
    ("kde", "values (or groups)", "Kernel density curve(s)."),
    ("box", "groups: {name: [values]}", "Box plots."),
    ("violin", "groups", "Violin plots."),
    ("swarm", "groups", "Beeswarm plots."),
    ("raincloud", "groups", "Half violin + box + points."),
    ("ridgeline", "groups", "Densities on stacked rows."),
    (
        "regression",
        "x, y, degree?",
        "Points with a polynomial fit and R².",
    ),
    (
        "heatmap",
        "z (rows of numbers), x_labels?, y_labels?, colormap?, annotate?",
        "Heatmap / correlation matrix.",
    ),
    (
        "contour",
        "z, x_range [a,b], y_range [a,b], filled?",
        "Contour lines or bands of a grid.",
    ),
    ("pie", "labels, values, donut? (0..1 hole)", "Pie or donut."),
    (
        "radar",
        "labels (axes), series: {name: [values]}",
        "Radar (spider) chart.",
    ),
    ("funnel", "labels, values", "Funnel."),
    ("waffle", "labels, values", "Shares as a 10×10 grid."),
    (
        "sankey",
        "links: [[from, to, value], …]",
        "Flows between nodes.",
    ),
    (
        "chord",
        "labels, matrix",
        "Flows between groups around a circle.",
    ),
    (
        "arc_diagram",
        "nodes, edges: [[from, to, weight?], …] (names or indices), groups?, group_names?",
        "Nodes on a line, edges as arcs.",
    ),
    (
        "edge_bundling",
        "nodes, groups (one per node), edges: [[from, to, weight?], …], group_names?, tension?",
        "Hierarchical edge bundling: nodes on a circle, edges bundled by group.",
    ),
    (
        "streamgraph",
        "x, layers: {name: [values]}",
        "Stacked layers around a moving baseline.",
    ),
    (
        "bullet",
        "labels, values, targets, ranges: [[r1, r2, r3], …]",
        "KPIs against targets.",
    ),
    (
        "dumbbell",
        "labels, start, end, start_label?, end_label?",
        "Two values per category.",
    ),
    (
        "slope",
        "labels, before, after, before_label?, after_label?",
        "Change between two times.",
    ),
    ("pareto", "labels, values", "Sorted bars + cumulative %."),
    (
        "control_chart",
        "values",
        "SPC chart with ±3σ and rule violations.",
    ),
    (
        "candlestick",
        "x (or dates), open, high, low, close",
        "Candlesticks.",
    ),
    (
        "roc",
        "labels (true/false), scores, name?",
        "ROC curve with AUC.",
    ),
    (
        "calibration",
        "labels, scores, name?, bins?",
        "Reliability curve with Brier score.",
    ),
    (
        "confusion",
        "labels (class names), actual, predicted, normalize?",
        "Confusion matrix.",
    ),
    ("upset", "sets: {name: [items]}, max?", "Set intersections."),
    (
        "volcano",
        "names, log2fc, pvalues, fc_threshold?, p_threshold?, top?",
        "Differential expression.",
    ),
    (
        "phylo_tree",
        "newick, circular?",
        "Phylogenetic tree from Newick.",
    ),
    (
        "choropleth",
        "values: {ISO code or country name: value}, projection? (robinson, naturalearth, equalearth, mollweide, mercator, equirectangular, orthographic: center_lon?, center_lat?), label?",
        "World map colored by value, or a globe (orthographic).",
    ),
    (
        "density_map",
        "x, y, how? (eq_hist, log, linear), colormap?, colorbar?",
        "Density image of millions of points (counted per pixel).",
    ),
    (
        "volume",
        "values ([z][y][x] nested, or flat x-fastest with dims: [nx, ny, nz]) or path (.nii, .nii.gz, .vtk, .mhd, .mha, .dcm or a DICOM directory), x_range?, y_range?, z_range?, colormap?, opacity?, threshold?, mode? (composite, mip)",
        "Volume rendering of a 3D grid (densities, scans).",
    ),
    (
        "surface",
        "z (rows), x_range, y_range, colormap?",
        "3D surface (rotatable: camera, rotate).",
    ),
    ("scatter3d", "x, y, z, values?, colormap?", "3D points."),
    ("line3d", "x, y, z", "3D line."),
    (
        "dna_helix",
        "sequence, highlight? [start, end, label]",
        "3D DNA double helix.",
    ),
    (
        "molecule",
        "pdb (text) or pdb_path or pdb_id, style?, coloring?",
        "3D molecule (ball_and_stick, sticks, space_filling, backbone, cartoon).",
    ),
];

/// The grammar-of-graphics panel (`mark` instead of `type`): fields and
/// what it does.
pub const GRAMMAR: (&str, &str) = (
    "data ({field: [values]} or [{field: value}, …]), mark (point, line, area, bar, rect, boxplot), encoding {x, y, color, size, facet: a field name or {field, type? (quantitative, nominal, ordinal, temporal), aggregate? (count, sum, mean, median, min, max), bin?, title?, scale?: {type: log}}}, columns?, stack? (stacked, grouped, percent)",
    "Grammar of graphics (Vega-Lite / ggplot style): color groups with a legend, aggregation, histograms (binned x), heatmaps, box plots and small multiples (facet) follow from the encodings.",
);

/// Errors are messages naming the panel and the field.
pub type Result<T> = std::result::Result<T, String>;

fn nums(v: &Value, key: &str) -> Result<Vec<f64>> {
    let a = v.get(key).ok_or_else(|| format!("missing `{key}`"))?;
    to_nums(a).ok_or_else(|| format!("`{key}` must be an array of numbers"))
}

/// Arrays handed over in memory instead of as JSON numbers (see
/// [`with_arrays`]): a field `"@name"` takes array `name`.
#[derive(Clone, Copy, Debug)]
pub struct Array<'a> {
    /// The name a spec refers to as `"@name"`.
    pub name: &'a str,
    /// The values (row-major for matrices).
    pub data: &'a [f64],
    /// Row length for matrix fields (`z` of heatmaps, surfaces, …); 0 for
    /// plain arrays.
    pub cols: usize,
}

thread_local! {
    static ARRAYS: std::cell::RefCell<Vec<(String, *const f64, usize, usize)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Runs `f` (which builds figures from specs) with `arrays` available to
/// the specs as `"@name"` fields, so large data never goes through JSON
/// text or `Value` numbers: one copy from `data` into the chart.
///
/// ```
/// use dataviz::plot::spec::{self, Array};
/// let x = [1.0, 2.0, 3.0];
/// let y = [2.0, 1.0, 3.0];
/// let v = serde_json::json!({"type": "line", "x": "@x", "y": "@y"});
/// let fig = spec::with_arrays(
///     &[Array { name: "x", data: &x, cols: 0 }, Array { name: "y", data: &y, cols: 0 }],
///     || spec::figure(&v),
/// );
/// assert!(fig.is_ok());
/// ```
pub fn with_arrays<R>(arrays: &[Array<'_>], f: impl FnOnce() -> R) -> R {
    struct Clear;
    impl Drop for Clear {
        fn drop(&mut self) {
            ARRAYS.with(|a| a.borrow_mut().clear());
        }
    }
    ARRAYS.with(|a| {
        let mut a = a.borrow_mut();
        a.clear();
        a.extend(
            arrays
                .iter()
                .map(|x| (x.name.to_string(), x.data.as_ptr(), x.data.len(), x.cols)),
        );
    });
    let _clear = Clear;
    f()
}

/// The array named by a `"@name"` string and its row length.
fn external<R>(a: &Value, f: impl FnOnce(&[f64], usize) -> R) -> Option<R> {
    let name = a.as_str()?.strip_prefix('@')?;
    ARRAYS.with(|arrays| {
        let arrays = arrays.borrow();
        let &(_, p, n, cols) = arrays.iter().find(|x| x.0 == name)?;
        // SAFETY: registered by `with_arrays` from a live slice, and
        // cleared before that call returns.
        let data = unsafe { std::slice::from_raw_parts(p, n) };
        Some(f(data, cols))
    })
}

fn to_nums(a: &Value) -> Option<Vec<f64>> {
    if let Some(v) = external(a, |d, _| d.to_vec()) {
        return Some(v);
    }
    a.as_array()?
        .iter()
        .map(|x| match x {
            Value::Number(n) => n.as_f64(),
            Value::Null => Some(f64::NAN),
            Value::String(s) => s.trim().parse().ok().or(Some(f64::NAN)),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        })
        .collect()
}

/// `edges: [[from, to, weight?], …]` with node names or indices.
fn graph_edges(v: &Value, nodes: &[String]) -> Result<Vec<(usize, usize, f64)>> {
    let a = v
        .get("edges")
        .and_then(Value::as_array)
        .ok_or("`edges` must be [[from, to, weight?], …]")?;
    let node = |x: &Value| match x {
        Value::Number(n) => n.as_u64().map(|i| i as usize).filter(|&i| i < nodes.len()),
        other => {
            let t = text(other);
            nodes.iter().position(|n| *n == t)
        }
    };
    a.iter()
        .map(|e| {
            let e = e.as_array().ok_or("each edge is [from, to, weight?]")?;
            let (from, to) = (e.first().and_then(node), e.get(1).and_then(node));
            match (from, to) {
                (Some(a), Some(b)) => Ok((a, b, e.get(2).and_then(Value::as_f64).unwrap_or(1.0))),
                _ => Err(format!(
                    "edge {} names an unknown node",
                    Value::Array(e.clone())
                )),
            }
        })
        .collect()
}

fn opt_nums(v: &Value, key: &str) -> Option<Vec<f64>> {
    v.get(key).and_then(to_nums)
}

/// Names from the first of `keys` present (field-name aliases).
fn names_any(v: &Value, keys: &[&str]) -> Result<Vec<String>> {
    keys.iter()
        .find(|k| v.get(**k).is_some())
        .map_or_else(|| Err(format!("missing `{}`", keys[0])), |k| strs(v, k))
}

/// Numbers from the first of `keys` present.
fn nums_any(v: &Value, keys: &[&str]) -> Result<Vec<f64>> {
    keys.iter()
        .find(|k| v.get(**k).is_some())
        .map_or_else(|| Err(format!("missing `{}`", keys[0])), |k| nums(v, k))
}

const NAMES: &[&str] = &["labels", "categories", "names", "x"];
const VALUES: &[&str] = &["values", "y", "counts"];

fn strs(v: &Value, key: &str) -> Result<Vec<String>> {
    if let Some(a) = v.get(key).and_then(|a| {
        external(a, |d, _| {
            d.iter()
                .map(|x| Value::from(*x))
                .map(|x| text(&x))
                .collect()
        })
    }) {
        return Ok(a);
    }
    let a = v
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("`{key}` must be an array"))?;
    Ok(a.iter().map(text).collect())
}

fn text(x: &Value) -> String {
    match x {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn bools(v: &Value, key: &str) -> Result<Vec<bool>> {
    if let Some(a) = v
        .get(key)
        .and_then(|a| external(a, |d, _| d.iter().map(|x| *x != 0.0).collect()))
    {
        return Ok(a);
    }
    let a = v
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("`{key}` must be an array"))?;
    Ok(a.iter()
        .map(|x| match x {
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
            Value::String(s) => {
                matches!(s.to_lowercase().as_str(), "1" | "true" | "yes" | "positive")
            }
            _ => false,
        })
        .collect())
}

fn f(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn b(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn matrix(v: &Value, key: &str) -> Result<Vec<Vec<f64>>> {
    if let Some(m) = v.get(key).and_then(|a| {
        external(a, |d, cols| {
            if cols == 0 {
                return Err(format!("`{key}` needs a matrix (rows × cols) array"));
            }
            Ok(d.chunks_exact(cols).map(<[f64]>::to_vec).collect())
        })
    }) {
        return m;
    }
    let a = v
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("`{key}` must be an array of rows"))?;
    a.iter()
        .map(|r| to_nums(r).ok_or_else(|| format!("`{key}` rows must be arrays of numbers")))
        .collect()
}

/// Groups as `{name: [values]}`, `[[name, [values]], …]` or
/// `[{name, values}, …]`.
fn groups(v: &Value, key: &str) -> Result<Vec<(String, Vec<f64>)>> {
    let g = v.get(key).ok_or_else(|| format!("missing `{key}`"))?;
    let bad = || format!("`{key}` must be {{name: [values]}} or [{{name, values}}]");
    match g {
        Value::Object(m) => m
            .iter()
            .map(|(k, x)| Ok((k.clone(), to_nums(x).ok_or_else(bad)?)))
            .collect(),
        Value::Array(a) => a
            .iter()
            .map(|e| match e {
                Value::Array(p) if p.len() == 2 => {
                    Ok((text(&p[0]), to_nums(&p[1]).ok_or_else(bad)?))
                }
                Value::Object(_) => {
                    Ok((s(e, "name").unwrap_or("").to_string(), nums(e, "values")?))
                }
                _ => Err(bad()),
            })
            .collect(),
        _ => Err(bad()),
    }
}

/// A colormap by name (viridis, plasma, coolwarm, rdbu, …; default viridis).
pub fn colormap(name: &str) -> Colormap {
    match name.to_lowercase().as_str() {
        "plasma" => Colormap::Plasma,
        "inferno" => Colormap::Inferno,
        "magma" => Colormap::Magma,
        "cividis" => Colormap::Cividis,
        "turbo" => Colormap::Turbo,
        "gray" | "grey" => Colormap::Gray,
        "coolwarm" => Colormap::Coolwarm,
        "rdbu" => Colormap::RdBu,
        "blues" => Colormap::Blues,
        "reds" => Colormap::Reds,
        "greens" => Colormap::Greens,
        "ylgnbu" => Colormap::YlGnBu,
        "brbg" => Colormap::BrBG,
        _ => Colormap::Viridis,
    }
}

/// A theme by name (light, dark, minimal, ggplot, seaborn, publication,
/// solarized).
pub fn theme(name: &str) -> Theme {
    match name.to_lowercase().as_str() {
        "dark" => Theme::dark(),
        "minimal" => Theme::minimal(),
        "ggplot" => Theme::ggplot(),
        "seaborn" => Theme::seaborn(),
        "publication" => Theme::publication(),
        "solarized" => Theme::solarized(),
        "high_contrast" | "highcontrast" | "high-contrast" | "contrast" => Theme::high_contrast(),
        _ => Theme::light(),
    }
}

/// A legend position by name (upper_right, outside_bottom, hidden, …).
pub fn legend(name: &str) -> LegendPosition {
    match name.to_lowercase().replace(['-', '_', ' '], "").as_str() {
        "upperright" | "topright" => LegendPosition::UpperRight,
        "upperleft" | "topleft" => LegendPosition::UpperLeft,
        "lowerleft" | "bottomleft" => LegendPosition::LowerLeft,
        "lowerright" | "bottomright" => LegendPosition::LowerRight,
        "outsideright" | "right" => LegendPosition::OutsideRight,
        "outsidebottom" | "bottom" => LegendPosition::OutsideBottom,
        "outsidetop" | "top" => LegendPosition::OutsideTop,
        "hidden" | "none" | "off" => LegendPosition::Hidden,
        _ => LegendPosition::Best,
    }
}

/// A figure from a spec (see the `plot` tool's schema).
pub fn figure(spec: &Value) -> Result<Figure> {
    let w = f(spec, "width").unwrap_or(900.0).clamp(100.0, 6000.0) as u32;
    let h = f(spec, "height").unwrap_or(600.0).clamp(100.0, 6000.0) as u32;
    let mut fig = Figure::new(w, h);
    if let Some(t) = s(spec, "theme") {
        fig.theme(theme(t));
    }
    if let Some(t) = s(spec, "title") {
        fig.title(t);
    }
    // Grammar of graphics at the top level: facets fill the figure.
    if spec.get("mark").is_some() && spec.get("panels").is_none() {
        let mut p = spec.clone();
        if let Some(o) = p.as_object_mut() {
            o.remove("title");
        }
        grammar_chart(&p)?.draw(&mut fig)?;
        common(fig.axes(), &p);
        return Ok(fig);
    }
    // One panel can be given inline (type at the top level).
    let panels: Vec<Value> = match spec.get("panels").and_then(Value::as_array) {
        Some(p) => p.clone(),
        // The title is the figure's then, not the panel's as well.
        None if spec.get("type").is_some() => {
            let mut p = spec.clone();
            if let Some(o) = p.as_object_mut() {
                o.remove("title");
            }
            vec![p]
        }
        None => {
            return Err("give `panels: [{type, …}]` (or one panel's fields with `type`)".into())
        }
    };
    if panels.is_empty() {
        return Err("`panels` is empty".into());
    }
    let n = panels.len();
    let cols = f(spec, "cols")
        .map(|c| c as usize)
        .unwrap_or(if n <= 3 {
            n
        } else {
            (n as f64).sqrt().ceil() as usize
        })
        .max(1);
    let rows = f(spec, "rows")
        .map(|r| r as usize)
        .unwrap_or(n.div_ceil(cols))
        .max(1);
    if n > 1 {
        fig.grid(rows, cols);
    }
    for (i, p) in panels.iter().enumerate() {
        let ax = if n > 1 {
            fig.at(i / cols, i % cols)
        } else {
            fig.axes()
        };
        apply(ax, p).map_err(|e| {
            format!(
                "panel {} ({}): {e}",
                i + 1,
                s(p, "type").unwrap_or("layers")
            )
        })?;
    }
    Ok(fig)
}

/// Series given inline (`x`, `y`, …) or as `series: [{…}]`.
fn each_series(p: &Value) -> Vec<&Value> {
    match p.get("series").and_then(Value::as_array) {
        Some(a) => a.iter().collect(),
        None => vec![p],
    }
}

/// Draws one panel (a `type` with its fields, or `layers` of them) into an
/// axes, then applies the panel's options (titles, limits, camera, …).
pub fn apply(ax: &mut Axes, p: &Value) -> Result<()> {
    if p.get("mark").is_some() {
        grammar_chart(p)?.draw_into(ax)?;
        common(ax, p);
        return Ok(());
    }
    match p.get("layers").and_then(Value::as_array) {
        Some(layers) => {
            for l in layers {
                chart(ax, l)?;
            }
        }
        None => chart(ax, p)?,
    }
    common(ax, p);
    Ok(())
}

fn chart(ax: &mut Axes, p: &Value) -> Result<()> {
    let kind = s(p, "type")
        .ok_or("missing `type` (or `layers`)")?
        .to_lowercase();
    match kind.as_str() {
        "line" | "scatter" | "area" => {
            for sr in each_series(p) {
                let y = nums(sr, "y")?;
                let x =
                    opt_nums(sr, "x").unwrap_or_else(|| (0..y.len()).map(|i| i as f64).collect());
                let name = s(sr, "name").or(s(sr, "label")).map(str::to_string);
                let color = s(sr, "color");
                match kind.as_str() {
                    "scatter" => {
                        let sc = ax.scatter(x, y);
                        if let Some(n) = name {
                            sc.label(n);
                        }
                        if let Some(c) = color {
                            sc.color(c);
                        }
                        if let Some(z) = f(sr, "size") {
                            sc.size(z);
                        }
                    }
                    _ => {
                        let l = if kind == "area" {
                            ax.area(x, y)
                        } else {
                            ax.line(x, y)
                        };
                        if let Some(n) = name {
                            l.label(n);
                        }
                        if let Some(c) = color {
                            l.color(c);
                        }
                        if let Some(z) = f(sr, "width") {
                            l.width(z);
                        }
                    }
                }
            }
        }
        "bar" | "barh" => {
            let cats = names_any(p, &["categories", "labels", "names", "x"])?;
            for sr in each_series(p) {
                let v = nums_any(sr, VALUES)?;
                let bars = if kind == "barh" {
                    ax.barh(cats.clone(), v)
                } else {
                    ax.bar(cats.clone(), v)
                };
                if let Some(n) = s(sr, "name").or(s(sr, "label")) {
                    bars.label(n);
                }
                if let Some(c) = s(sr, "color") {
                    bars.color(c);
                }
            }
        }
        "hist" | "histogram" => {
            let h = ax.hist(nums_any(p, &["values", "x", "y"])?);
            if let Some(n) = f(p, "bins") {
                h.bin_count(n as usize);
            }
        }
        "kde" | "density" => match groups(p, "groups") {
            Ok(g) => {
                for (name, v) in g {
                    ax.kde(v, None).label(name);
                }
            }
            Err(_) => {
                ax.kde(nums(p, "values")?, None);
            }
        },
        "box" | "boxplot" => {
            ax.boxplot(groups(p, "groups")?);
        }
        "violin" => {
            ax.violin(groups(p, "groups")?);
        }
        "swarm" => {
            ax.swarm(groups(p, "groups")?);
        }
        "raincloud" => {
            ax.raincloud(groups(p, "groups")?);
        }
        "ridgeline" => {
            ax.ridgeline(groups(p, "groups")?);
        }
        "regression" => {
            let (x, y) = (nums(p, "x")?, nums(p, "y")?);
            ax.scatter(x.clone(), y.clone()).size(5.0);
            ax.regression(x, y, f(p, "degree").unwrap_or(1.0) as usize);
        }
        "heatmap" | "correlation" => {
            let z = matrix(p, "z")?;
            let xl = strs(p, "x_labels").unwrap_or_else(|_| {
                (0..z.first().map_or(0, Vec::len))
                    .map(|i| i.to_string())
                    .collect()
            });
            let yl = strs(p, "y_labels")
                .unwrap_or_else(|_| (0..z.len()).map(|i| i.to_string()).collect());
            let hm = ax.heatmap_labeled(xl, yl, z);
            hm.colormap(colormap(s(p, "colormap").unwrap_or("viridis")));
            if b(p, "annotate") {
                hm.annotate(true);
            }
            if let Some(l) = s(p, "label") {
                hm.colorbar(l);
            }
        }
        "contour" => {
            let z = matrix(p, "z")?;
            let xr = opt_nums(p, "x_range")
                .filter(|v| v.len() == 2)
                .unwrap_or(vec![0.0, z.first().map_or(1, Vec::len) as f64]);
            let yr = opt_nums(p, "y_range")
                .filter(|v| v.len() == 2)
                .unwrap_or(vec![0.0, z.len() as f64]);
            let c = ax.contour((xr[0], xr[1]), (yr[0], yr[1]), z);
            if b(p, "filled") {
                c.filled(true);
            }
        }
        "pie" | "donut" => {
            let pie = ax.pie(names_any(p, NAMES)?, nums_any(p, VALUES)?);
            if kind == "donut" || f(p, "donut").is_some() {
                pie.donut(f(p, "donut").unwrap_or(0.5));
            }
        }
        "radar" | "spider" => {
            let axes = names_any(p, NAMES)?;
            ax.radar(axes, groups(p, "series")?);
        }
        "funnel" => {
            ax.funnel(names_any(p, NAMES)?, nums_any(p, VALUES)?);
        }
        "waffle" => {
            ax.waffle(names_any(p, NAMES)?, nums_any(p, VALUES)?);
        }
        "sankey" => {
            let links = p
                .get("links")
                .and_then(Value::as_array)
                .ok_or("`links` must be [[from, to, value], …]")?;
            let l: Vec<(String, String, f64)> = links
                .iter()
                .filter_map(|e| {
                    let a = e.as_array()?;
                    Some((text(a.first()?), text(a.get(1)?), a.get(2)?.as_f64()?))
                })
                .collect();
            ax.sankey(l);
        }
        "chord" => {
            ax.chord(names_any(p, NAMES)?, &matrix(p, "matrix")?);
        }
        "density_map" | "datashade" => {
            let d = ax.density(nums(p, "x")?, nums(p, "y")?);
            match s(p, "how") {
                Some("log") => {
                    d.scale(crate::plot::series::density::DensityScale::Log);
                }
                Some("linear") => {
                    d.scale(crate::plot::series::density::DensityScale::Linear);
                }
                _ => {}
            }
            if let Some(c) = s(p, "colormap") {
                d.colormap(colormap(c));
            }
            if let Some(c) = s(p, "colorbar") {
                d.colorbar(c);
            }
        }
        "volume" if p.get("path").is_some() => {
            let path = s(p, "path").ok_or("`path` must be a file name")?;
            let data = crate::plot::volume_file::VolumeData::read(path)?;
            volume_options(ax.volume_data(&data), p);
        }
        "volume" => {
            let (values, dims) = match p.get("dims").and_then(to_nums) {
                Some(d) if d.len() == 3 => (
                    nums(p, "values")?,
                    [d[0] as usize, d[1] as usize, d[2] as usize],
                ),
                _ => {
                    // Nested [z][y][x].
                    let zs = p
                        .get("values")
                        .and_then(Value::as_array)
                        .ok_or("`values` must be [z][y][x] or flat with `dims`")?;
                    let mut flat = Vec::new();
                    let (mut ny, mut nx) = (0, 0);
                    for zl in zs {
                        let rows = zl.as_array().ok_or("`values` must be [z][y][x]")?;
                        ny = rows.len();
                        for r in rows {
                            let r = to_nums(r).ok_or("`values` must hold numbers")?;
                            nx = r.len();
                            flat.extend(r);
                        }
                    }
                    if flat.len() != nx * ny * zs.len() {
                        return Err("`values` rows must all have the same length".into());
                    }
                    (flat, [nx, ny, zs.len()])
                }
            };
            let v = ax.volume(values, dims);
            let r = |k: &str, n: usize| {
                opt_nums(p, k)
                    .filter(|r| r.len() == 2)
                    .map_or((0.0, n.saturating_sub(1) as f64), |r| (r[0], r[1]))
            };
            v.extent(
                r("x_range", dims[0]),
                r("y_range", dims[1]),
                r("z_range", dims[2]),
            );
            volume_options(v, p);
        }
        "arc_diagram" | "edge_bundling" => {
            let nodes = names_any(p, &["nodes", "labels", "names"])?;
            let edges = graph_edges(p, &nodes)?;
            let groups: Vec<usize> = opt_nums(p, "groups")
                .unwrap_or_default()
                .iter()
                .map(|g| g.max(0.0) as usize)
                .collect();
            let names = strs(p, "group_names").unwrap_or_default();
            if kind == "arc_diagram" {
                ax.arc_diagram(nodes, edges)
                    .groups(groups)
                    .group_names(names);
            } else {
                let b = ax.edge_bundling(nodes, groups, edges);
                b.group_names(names);
                if let Some(v) = f(p, "tension") {
                    b.tension(v);
                }
            }
        }
        "streamgraph" => {
            let layers = groups(p, "layers")?;
            ax.streamgraph(nums(p, "x")?, &layers);
        }
        "bullet" => {
            ax.bullet(
                strs(p, "labels")?,
                nums(p, "values")?,
                nums(p, "targets")?,
                &matrix(p, "ranges")?,
            );
        }
        "dumbbell" => {
            let d = ax.dumbbell(strs(p, "labels")?, nums(p, "start")?, nums(p, "end")?);
            d.labels(
                s(p, "start_label").unwrap_or("start"),
                s(p, "end_label").unwrap_or("end"),
            );
        }
        "slope" => {
            ax.slope(
                strs(p, "labels")?,
                nums(p, "before")?,
                nums(p, "after")?,
                s(p, "before_label").unwrap_or("before"),
                s(p, "after_label").unwrap_or("after"),
            );
        }
        "pareto" => {
            ax.pareto(names_any(p, NAMES)?, nums_any(p, VALUES)?);
        }
        "control_chart" | "spc" => {
            ax.control_chart(nums_any(p, VALUES)?);
        }
        "candlestick" => {
            let o = nums(p, "open")?;
            let x = match p.get("dates").and_then(Value::as_array) {
                Some(d) => d
                    .iter()
                    .map(|v| v.as_str().and_then(parse_datetime).unwrap_or(f64::NAN))
                    .collect(),
                None => {
                    opt_nums(p, "x").unwrap_or_else(|| (0..o.len()).map(|i| i as f64).collect())
                }
            };
            ax.candlestick(x, o, nums(p, "high")?, nums(p, "low")?, nums(p, "close")?);
        }
        "roc" => {
            ax.roc_curve(
                s(p, "name").unwrap_or("model"),
                &bools(p, "labels")?,
                nums(p, "scores")?,
            );
        }
        "calibration" => {
            ax.calibration_curve(
                s(p, "name").unwrap_or("model"),
                &bools(p, "labels")?,
                nums(p, "scores")?,
                f(p, "bins").unwrap_or(10.0) as usize,
            );
        }
        "confusion" | "confusion_matrix" => {
            // Class names (or indices) mapped to class indices.
            let classes = strs(p, "labels")?;
            let idx = |key: &str| -> Result<Vec<usize>> {
                strs(p, key)?
                    .iter()
                    .map(|c| {
                        classes
                            .iter()
                            .position(|k| k == c)
                            .or_else(|| c.parse().ok())
                            .ok_or_else(|| format!("`{c}` in `{key}` is not one of `labels`"))
                    })
                    .collect()
            };
            ax.confusion_matrix(
                classes.clone(),
                &idx("actual")?,
                &idx("predicted")?,
                b(p, "normalize"),
            );
        }
        "upset" => {
            let m = p
                .get("sets")
                .and_then(Value::as_object)
                .ok_or("`sets` must be {name: [items]}")?;
            let sets: Vec<(String, Vec<String>)> = m
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        external(v, |d, _| d.iter().map(|x| text(&Value::from(*x))).collect())
                            .or_else(|| v.as_array().map(|a| a.iter().map(text).collect()))
                            .unwrap_or_default(),
                    )
                })
                .collect();
            ax.upset(&sets, f(p, "max").unwrap_or(15.0) as usize);
        }
        "volcano" => {
            ax.volcano(
                strs(p, "names")?,
                nums(p, "log2fc")?,
                nums(p, "pvalues")?,
                f(p, "fc_threshold").unwrap_or(1.0),
                f(p, "p_threshold").unwrap_or(0.05),
                f(p, "top").unwrap_or(8.0) as usize,
            );
        }
        "phylo_tree" | "tree" => {
            let nw = s(p, "newick").ok_or("missing `newick`")?;
            let t = if b(p, "circular") {
                ax.phylo_tree_circular(nw)
            } else {
                ax.phylo_tree(nw)
            };
            t.ok_or("`newick` does not parse")?;
        }
        "choropleth" | "map" => {
            let proj = match s(p, "projection")
                .unwrap_or("robinson")
                .to_lowercase()
                .as_str()
            {
                "mercator" => Projection::Mercator,
                "equirectangular" | "platecarree" => Projection::Equirectangular,
                "naturalearth" | "natural_earth" => Projection::NaturalEarth,
                "equalearth" | "equal_earth" => Projection::EqualEarth,
                "mollweide" => Projection::Mollweide,
                "orthographic" | "globe" => Projection::Orthographic {
                    lon: f(p, "center_lon").unwrap_or(0.0),
                    lat: f(p, "center_lat").unwrap_or(20.0),
                },
                _ => Projection::Robinson,
            };
            ax.map(proj);
            let vals = p
                .get("values")
                .and_then(Value::as_object)
                .ok_or("`values` must be {country: value}")?;
            let v: Vec<(String, f64)> = vals
                .iter()
                .filter_map(|(k, x)| Some((k.clone(), x.as_f64()?)))
                .collect();
            let g = ax.choropleth(v);
            if let Some(cm) = s(p, "colormap") {
                g.colormap(colormap(cm));
            }
            g.colorbar(s(p, "label").unwrap_or(""));
        }
        "surface" => {
            let z = matrix(p, "z")?;
            let xr = opt_nums(p, "x_range")
                .filter(|v| v.len() == 2)
                .unwrap_or(vec![0.0, 1.0]);
            let yr = opt_nums(p, "y_range")
                .filter(|v| v.len() == 2)
                .unwrap_or(vec![0.0, 1.0]);
            ax.surface((xr[0], xr[1]), (yr[0], yr[1]), z)
                .colormap(colormap(s(p, "colormap").unwrap_or("viridis")));
        }
        "scatter3d" | "line3d" => {
            let (x, y, z) = (nums(p, "x")?, nums(p, "y")?, nums(p, "z")?);
            let pts = if kind == "line3d" {
                ax.line3d(x, y, z)
            } else {
                ax.scatter3d(x, y, z)
            };
            if let Some(v) = opt_nums(p, "values") {
                pts.values(v);
            }
            if let Some(cm) = s(p, "colormap") {
                pts.colormap(colormap(cm));
            }
            if let Some(c) = s(p, "color") {
                pts.color(c);
            }
        }
        "dna_helix" | "dna" => {
            let h = ax.dna_helix(s(p, "sequence").ok_or("missing `sequence`")?);
            if let Some(hl) = p.get("highlight").and_then(Value::as_array) {
                if let (Some(a), Some(z)) = (
                    hl.first().and_then(Value::as_u64),
                    hl.get(1).and_then(Value::as_u64),
                ) {
                    h.highlight(
                        a as usize..z as usize,
                        "#ffd400",
                        hl.get(2).and_then(Value::as_str),
                    );
                }
            }
        }
        "molecule" => {
            let pdb = match (s(p, "pdb"), s(p, "pdb_path")) {
                (Some(t), _) => t.to_string(),
                (None, Some(path)) => {
                    std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?
                }
                _ => return Err("give `pdb` (text) or `pdb_path`".into()),
            };
            let mol = Molecule::from_pdb(&pdb);
            if mol.atoms.is_empty() {
                return Err("no ATOM/HETATM records in the PDB text".into());
            }
            let m = ax.molecule(mol);
            m.style(mol_style(s(p, "style").unwrap_or("ball_and_stick")));
            m.coloring(mol_color(s(p, "coloring").unwrap_or("element")));
        }
        other => return Err(format!("unknown type `{other}` (see spec::CHART_TYPES)")),
    }
    Ok(())
}

/// A molecule style by name (cartoon, ball_and_stick, sticks, …).
pub fn mol_style(s: &str) -> MolStyle {
    match s.to_lowercase().replace(['-', ' '], "_").as_str() {
        "sticks" => MolStyle::Sticks,
        "space_filling" | "spacefilling" | "spheres" | "vdw" => MolStyle::SpaceFilling,
        "backbone" | "trace" => MolStyle::Backbone,
        "cartoon" | "ribbon" => MolStyle::Cartoon,
        _ => MolStyle::BallAndStick,
    }
}

/// A molecule coloring by name (element, chain, rainbow).
pub fn mol_color(s: &str) -> MolColor {
    match s.to_lowercase().as_str() {
        "chain" => MolColor::Chain,
        "rainbow" | "residue" => MolColor::Rainbow,
        _ => MolColor::Element,
    }
}

/// Options every panel takes: titles, labels, limits, log scales, legend,
/// 3D camera.
fn common(ax: &mut Axes, p: &Value) {
    // Chart drawings: trend lines, rays, price levels, Fibonacci.
    if let Some(ds) = p.get("drawings").and_then(Value::as_array) {
        let pt = |d: &Value, k: &str| {
            opt_nums(d, k)
                .filter(|v| v.len() == 2)
                .map(|v| (v[0], v[1]))
        };
        for d in ds {
            let a = match (s(d, "type").unwrap_or(""), pt(d, "from"), pt(d, "to")) {
                ("trend" | "trendline" | "line", Some(a), Some(b)) => ax.trendline(a, b),
                ("ray", Some(a), Some(b)) => ax.ray(a, b),
                ("fib" | "fibonacci", Some(a), Some(b)) => ax.fib_retracement(a, b),
                ("level" | "price_level", _, _) => match f(d, "y") {
                    Some(y) => ax.price_level(y),
                    None => continue,
                },
                _ => continue,
            };
            if let Some(c) = s(d, "color") {
                a.color(c);
            }
        }
    }
    if let Some(t) = s(p, "title") {
        ax.title(t);
    }
    if let Some(t) = s(p, "x_label") {
        ax.x_label(t);
    }
    if let Some(t) = s(p, "y_label") {
        ax.y_label(t);
    }
    if let Some(t) = s(p, "z_label") {
        ax.z_label(t);
    }
    if let Some(v) = opt_nums(p, "x_limits").filter(|v| v.len() == 2) {
        ax.x_limits(v[0], v[1]);
    }
    if let Some(v) = opt_nums(p, "y_limits").filter(|v| v.len() == 2) {
        ax.y_limits(v[0], v[1]);
    }
    if b(p, "x_log") {
        ax.x_scale(Scale::Log(10.0));
    }
    if b(p, "y_log") {
        ax.y_scale(Scale::Log(10.0));
    }
    if let Some(l) = s(p, "legend") {
        ax.legend(legend(l));
    }
    if let Some(c) = opt_nums(p, "camera").filter(|v| v.len() == 2) {
        ax.camera(c[0], c[1]);
    }
    if let Some(r) = p.get("rotate") {
        for (k, axis) in [("x", Axis3::X), ("y", Axis3::Y), ("z", Axis3::Z)] {
            if let Some(d) = f(r, k) {
                ax.rotate(axis, d);
            }
        }
    }
}

#[cfg(test)]
mod network_tests {
    #[test]
    fn arc_diagram_and_edge_bundling_from_spec() {
        for t in ["arc_diagram", "edge_bundling"] {
            let v = serde_json::json!({"type": t, "nodes": ["a", "b", "c"],
                "groups": [0, 0, 1], "edges": [["a", "c", 2], [0, 1]]});
            let mut fig = super::figure(&v).unwrap();
            assert!(fig.to_svg().contains("<path"));
        }
        let vol =
            serde_json::json!({"type": "volume", "values": [[[0, 1], [1, 2]], [[1, 2], [2, 3]]]});
        assert!(super::figure(&vol).unwrap().to_pixels().is_ok());
        let bad = serde_json::json!({"type": "arc_diagram", "nodes": ["a"], "edges": [["a", "z"]]});
        let err = super::figure(&bad).err().unwrap_or_default();
        assert!(err.contains("unknown node"), "{err}");
    }
}

fn volume_options(v: &mut crate::plot::series::volume::Volume, p: &Value) {
    if let Some(c) = s(p, "colormap") {
        v.colormap(colormap(c));
    }
    if let Some(a) = f(p, "opacity") {
        v.opacity(a);
    }
    if let Some(t) = f(p, "threshold") {
        v.threshold(t);
    }
    if matches!(s(p, "mode"), Some("mip" | "max" | "max_intensity")) {
        v.mode(crate::plot::series::volume::VolumeMode::MaxIntensity);
    }
}

/// A column from JSON: numbers (or an `"@name"` array) or text.
fn json_column(v: &Value) -> Column {
    if let Some(n) = external(v, |d, _| d.to_vec()) {
        return Column::Num(n);
    }
    let a = v.as_array().map(Vec::as_slice).unwrap_or(&[]);
    if a.iter().all(|x| x.is_number() || x.is_null()) {
        Column::Num(to_nums(v).unwrap_or_default())
    } else {
        Column::Text(
            a.iter()
                .map(|x| if x.is_null() { String::new() } else { text(x) })
                .collect(),
        )
    }
}

/// `data` as columns `{name: [values]}`, rows `[{name: value}, …]` or
/// Vega-Lite's `{"values": rows}`.
fn grammar_data(v: &Value) -> Result<Data> {
    let rows = match v {
        Value::Object(m) if m.len() == 1 && m.get("values").is_some_and(Value::is_array) => {
            m.get("values").and_then(Value::as_array)
        }
        Value::Array(a) => Some(a),
        _ => None,
    };
    let mut data = Data::new();
    match (rows, v) {
        (Some(rows), _) => {
            let mut names: Vec<&str> = Vec::new();
            for r in rows {
                for k in r.as_object().ok_or("`data` rows must be objects")?.keys() {
                    if !names.contains(&k.as_str()) {
                        names.push(k);
                    }
                }
            }
            for n in names {
                let col: Vec<Value> = rows
                    .iter()
                    .map(|r| r.get(n).cloned().unwrap_or(Value::Null))
                    .collect();
                data = data.column(n, json_column(&Value::Array(col)));
            }
        }
        (None, Value::Object(m)) => {
            for (k, c) in m {
                data = data.column(k.clone(), json_column(c));
            }
        }
        _ => return Err("`data` must be {field: [values]} or [{field: value}, …]".into()),
    }
    Ok(data)
}

/// An encoding channel: a field name or `{field, type, aggregate, bin,
/// title, scale: {type: "log"}}`.
fn grammar_field(v: &Value) -> Option<Field> {
    if let Some(name) = v.as_str() {
        return Some(Field::new(name));
    }
    let mut fd = Field::new(s(v, "field").unwrap_or(""));
    fd.kind = s(v, "type").and_then(|t| {
        Some(match t.to_ascii_lowercase().as_str() {
            "quantitative" | "q" | "number" => Kind::Quantitative,
            "nominal" | "n" | "category" => Kind::Nominal,
            "ordinal" | "o" => Kind::Ordinal,
            "temporal" | "t" | "time" | "date" => Kind::Temporal,
            _ => return None,
        })
    });
    fd.aggregate = s(v, "aggregate").and_then(Aggregate::parse);
    fd.bin = match v.get("bin") {
        Some(Value::Bool(true)) => Some(0),
        Some(Value::Number(n)) => n.as_f64().map(|n| n as usize),
        Some(o @ Value::Object(_)) => Some(f(o, "maxbins").unwrap_or(0.0) as usize),
        _ => None,
    };
    fd.title = s(v, "title").map(str::to_string);
    fd.log = v
        .get("scale")
        .and_then(|sc| s(sc, "type"))
        .is_some_and(|t| t == "log");
    (!fd.name.is_empty() || fd.aggregate.is_some()).then_some(fd)
}

/// A grammar-of-graphics panel: `data`, `mark` and `encoding`.
fn grammar_chart(p: &Value) -> Result<grammar::Chart> {
    let data = grammar_data(p.get("data").ok_or("`mark` needs `data`")?)?;
    let mark_name = match p.get("mark") {
        Some(Value::String(m)) => m.as_str(),
        Some(o) => s(o, "type").unwrap_or(""),
        None => "",
    };
    let (mark, marker) =
        Mark::parse(mark_name).ok_or_else(|| format!("unknown mark `{mark_name}`"))?;
    let mut c = grammar::Chart::new(data, mark).marker(marker);
    let e = p.get("encoding").cloned().unwrap_or(Value::Null);
    let field = |k: &str| e.get(k).and_then(grammar_field);
    // `{"aggregate": "count"}` without a field: counts (bars).
    let real = |f: Option<Field>| f.filter(|f| !f.name.is_empty());
    if let Some(x) = real(field("x")) {
        c = c.x(x);
    }
    if let Some(y) = real(field("y")) {
        c = c.y(y);
    }
    if let Some(col) = real(field("color").or_else(|| field("fill"))) {
        c = c.color(col);
    }
    if let Some(sz) = real(field("size")) {
        c = c.size(sz);
    }
    let facet = field("facet")
        .or_else(|| field("column"))
        .or_else(|| field("row"))
        .or_else(|| p.get("facet").and_then(grammar_field));
    if let Some(fc) = real(facet) {
        let cols = f(p, "columns").or_else(|| e.get("facet").and_then(|x| f(x, "columns")));
        let cols = match (cols, e.get("row").is_some()) {
            (Some(n), _) => n as usize,
            (None, true) => 1,
            _ => 0,
        };
        c = c.facet(fc, cols);
    }
    if let Some(m) = s(p, "stack").or_else(|| s(&e, "stack")) {
        c = c.stack(match m {
            "grouped" | "dodge" | "none" => BarMode::Grouped,
            "normalize" | "percent" => BarMode::Percent,
            "overlay" => BarMode::Overlay,
            _ => BarMode::Stacked,
        });
    }
    if p.get("stack") == Some(&Value::Bool(false)) {
        c = c.stack(BarMode::Grouped);
    }
    if let Some(cm) = s(p, "colormap") {
        c = c.colormap(colormap(cm));
    }
    if let Some(t) = s(p, "title") {
        c = c.title(t);
    }
    Ok(c)
}

#[cfg(test)]
mod grammar_tests {
    use serde_json::json;

    #[test]
    fn grammar_specs_from_columns_rows_and_arrays() {
        let rows = json!([
            {"g": "a", "x": 1, "y": 2}, {"g": "b", "x": 2, "y": 3},
            {"g": "a", "x": 3, "y": 1}, {"g": "b", "x": 4, "y": 5}
        ]);
        for (mark, enc) in [
            ("point", json!({"x": "x", "y": "y", "color": "g"})),
            ("line", json!({"x": "x", "y": "y", "color": "g"})),
            (
                "bar",
                json!({"x": "g", "y": {"field": "y", "aggregate": "mean"}}),
            ),
            ("bar", json!({"x": {"field": "x", "bin": true}})),
            ("bar", json!({"x": "g", "y": {"aggregate": "count"}})),
            ("boxplot", json!({"x": "g", "y": "y"})),
            (
                "rect",
                json!({"x": "g", "y": {"field": "x", "type": "ordinal"}, "color": "y"}),
            ),
        ] {
            let v = json!({"data": rows, "mark": mark, "encoding": enc, "title": "t"});
            let mut fig = super::figure(&v).unwrap_or_else(|e| panic!("{mark}: {e}"));
            assert!(fig.to_svg().contains(">t<"), "{mark}");
        }
        // Vega-Lite's {"values": rows}, columns, facets and panels.
        let v = json!({"data": {"values": rows}, "mark": "point",
            "encoding": {"x": "x", "y": "y", "facet": {"field": "g"}}});
        assert_eq!(super::figure(&v).unwrap().all_axes().count(), 2);
        let v = json!({"panels": [{"data": {"x": [1, 2], "y": [3, 4]}, "mark": "line",
            "encoding": {"x": "x", "y": "y"}}, {"type": "pie", "labels": ["a"], "values": [1]}]});
        assert!(super::figure(&v).is_ok());
        let x = [1.0, 2.0, 3.0];
        let v = json!({"data": {"x": "@x", "y": "@x"}, "mark": "point", "encoding": {"x": "x", "y": "y"}});
        let arrays = [super::Array {
            name: "x",
            data: &x,
            cols: 0,
        }];
        assert!(super::with_arrays(&arrays, || super::figure(&v)).is_ok());
        let bad = json!({"data": rows, "mark": "point", "encoding": {"x": "nope", "y": "y"}});
        assert!(super::figure(&bad)
            .err()
            .unwrap()
            .contains("no field `nope`"));
        let bad = json!({"data": rows, "mark": "blob"});
        assert!(super::figure(&bad).err().unwrap().contains("unknown mark"));
    }
}

#[cfg(test)]
mod drawing_tests {
    #[test]
    fn drawings_on_a_price_chart() {
        let v = serde_json::json!({"type": "candlestick", "open": [1, 2, 3], "high": [2, 3, 4],
            "low": [0.5, 1.5, 2.5], "close": [2, 3, 2.8],
            "drawings": [{"type": "trend", "from": [0, 1], "to": [2, 3]},
                         {"type": "ray", "from": [0, 1], "to": [1, 2], "color": "orange"},
                         {"type": "level", "y": 2.5},
                         {"type": "fib", "from": [0, 0.5], "to": [2, 4]},
                         {"type": "nonsense"}]});
        let mut fig = super::figure(&v).unwrap();
        assert_eq!(fig.axes().series_count(), 5);
        let svg = fig.to_svg();
        assert!(
            svg.contains("61.8 %") && svg.contains(">2.5<"),
            "fib levels and the level tag"
        );
    }
}
