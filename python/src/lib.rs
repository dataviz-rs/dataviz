//! The Rust core of the `dataviz` Python package. The public, documented
//! API is the thin Python layer in `python/dataviz/__init__.py`; this
//! module turns its calls into dataviz calls. Numbers come in as NumPy
//! arrays (read in place), lists or anything `numpy.asarray` accepts
//! (pandas and Polars series, ranges, …).

mod arrow;

use dataviz::plot::spec;
use dataviz::prelude::*;
use numpy::{PyReadonlyArray1, PyReadonlyArray2};
use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// A plot cell: (row, col, rowspan, colspan), or the single plot.
type Cell = Option<(usize, usize, usize, usize)>;

/// One pass over a NumPy array of any numeric type into `f64`.
fn numpy_values(obj: &Bound<'_, PyAny>) -> Option<Vec<f64>> {
    macro_rules! try_type {
        ($($t:ty),*) => {$(
            if let Ok(a) = obj.extract::<PyReadonlyArray1<$t>>() {
                return Some(match a.as_slice() {
                    Ok(s) => s.iter().map(|&v| v as f64).collect(),
                    Err(_) => a.as_array().iter().map(|&v| v as f64).collect(),
                });
            }
        )*};
    }
    try_type!(f64, f32, i64, i32, i16, i8, u64, u32, u16, u8);
    None
}

fn values(obj: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    if let Some(v) = numpy_values(obj) {
        return Ok(v);
    }
    // Polars, pyarrow, Arrow-backed pandas, …: straight from the Arrow
    // buffers (anything unexpected falls through to NumPy).
    if let Ok(Some(v)) = arrow::values(obj) {
        return Ok(v);
    }
    if let Ok(v) = obj.extract::<Vec<f64>>() {
        return Ok(v);
    }
    let np = obj.py().import("numpy")?;
    let kwargs = pyo3::types::PyDict::new(obj.py());
    kwargs.set_item("dtype", "float64")?;
    let arr = np.call_method("asarray", (obj,), Some(&kwargs))?;
    let arr = arr.call_method0("ravel")?;
    let a = arr.extract::<PyReadonlyArray1<f64>>()?;
    Ok(a.as_array().to_vec())
}

fn opt_values(obj: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Vec<f64>>> {
    obj.map(values).transpose()
}

fn labels(obj: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    obj.try_iter()?
        .map(|x| x?.str().map(|s| s.to_string()))
        .collect()
}

fn matrix(obj: &Bound<'_, PyAny>) -> PyResult<Vec<Vec<f64>>> {
    if let Ok(a) = obj.extract::<PyReadonlyArray2<f64>>() {
        return Ok(a
            .as_array()
            .rows()
            .into_iter()
            .map(|r| r.to_vec())
            .collect());
    }
    obj.try_iter()?.map(|r| values(&r?)).collect()
}

/// `{name: values}` or `[(name, values), …]`.
fn groups(obj: &Bound<'_, PyAny>) -> PyResult<Vec<(String, Vec<f64>)>> {
    let items = if let Ok(d) = obj.cast::<pyo3::types::PyDict>() {
        d.items().into_any()
    } else {
        obj.clone()
    };
    items
        .try_iter()?
        .map(|it| {
            let it: Bound<'_, PyAny> = it?;
            let k = it.get_item(0)?;
            let v = it.get_item(1)?;
            Ok((k.str()?.to_string(), values(&v)?))
        })
        .collect()
}

fn err(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

fn colormap(name: &str) -> Colormap {
    spec::colormap(name)
}

fn dash(name: &str) -> Dash {
    match name {
        "dashed" | "--" => Dash::Dashed,
        "dotted" | ":" => Dash::Dotted,
        "dashdot" | "-." => Dash::DashDot,
        _ => Dash::Solid,
    }
}

fn marker(name: &str) -> Option<Marker> {
    Some(match name {
        "o" | "circle" => Marker::Circle,
        "s" | "square" => Marker::Square,
        "^" | "triangle" => Marker::TriangleUp,
        "D" | "d" | "diamond" => Marker::Diamond,
        "+" | "plus" => Marker::Plus,
        "x" | "cross" => Marker::Cross,
        _ => return None,
    })
}

#[pyclass(unsendable, module = "dataviz._dataviz")]
struct Core {
    fig: Figure,
}

impl Core {
    fn ax(&mut self, cell: Cell) -> &mut Axes {
        match cell {
            Some((r, c, rs, cs)) => self.fig.span(r, c, rs.max(1), cs.max(1)),
            None => self.fig.axes(),
        }
    }
}

#[pymethods]
impl Core {
    #[new]
    fn new(width: u32, height: u32) -> Self {
        Self {
            fig: Figure::new(width.clamp(50, 10_000), height.clamp(50, 10_000)),
        }
    }

    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let v: serde_json::Value = serde_json::from_str(json).map_err(err)?;
        Ok(Self {
            fig: spec::figure(&v).map_err(err)?,
        })
    }

    fn grid(&mut self, rows: usize, cols: usize) {
        self.fig.grid(rows.max(1), cols.max(1));
    }
    fn title(&mut self, t: &str) {
        self.fig.title(t);
    }
    fn theme(&mut self, name: &str) {
        self.fig.theme(spec::theme(name));
    }
    fn size(&self) -> (u32, u32) {
        let (w, h) = self.fig.size();
        (w.round() as u32, h.round() as u32)
    }
    fn resize(&mut self, width: u32, height: u32) {
        self.fig.resize(width, height);
    }
    fn share_x(&mut self, on: bool) {
        self.fig.share_x(on);
    }

    // ----- output ---------------------------------------------------------

    fn encode<'py>(&mut self, py: Python<'py>, format: &str) -> PyResult<Bound<'py, PyBytes>> {
        let f = match format {
            "png" => Format::Png,
            "svg" => Format::Svg,
            "pdf" => Format::Pdf,
            "jpg" | "jpeg" => Format::Jpeg,
            "webp" => Format::WebP,
            "html" => Format::Html,
            other => return Err(err(format!("unknown format {other}"))),
        };
        let bytes = self.fig.encode(f).map_err(err)?;
        Ok(PyBytes::new(py, &bytes))
    }
    fn save(&mut self, path: &str) -> PyResult<()> {
        self.fig
            .save(path)
            .map_err(|e| PyIOError::new_err(e.to_string()))
    }
    fn rgba<'py>(&mut self, py: Python<'py>) -> PyResult<(u32, u32, Bound<'py, PyBytes>)> {
        let mut px = Pixels::default();
        self.fig.render_into(&mut px).map_err(err)?;
        Ok((px.width, px.height, PyBytes::new(py, &px.rgba)))
    }
    fn alt_text(&mut self) -> String {
        self.fig.alt_text()
    }
    fn link_brushing(&mut self, on: bool) {
        self.fig.link_brushing(on);
    }
    fn brush_rows(&mut self, rows: usize, selected: Vec<usize>) {
        self.fig.brush_rows(rows, selected);
    }
    fn clear_brush(&mut self) {
        self.fig.clear_brush();
    }
    fn brushed(&self) -> Vec<usize> {
        self.fig.brushed().to_vec()
    }

    // ----- axes options ---------------------------------------------------

    #[pyo3(signature = (cell, title=None, x_label=None, y_label=None, z_label=None, x_limits=None, y_limits=None, x_log=None, y_log=None, legend=None, grid=None, equal_aspect=None))]
    #[allow(clippy::too_many_arguments)]
    fn set(
        &mut self,
        cell: Cell,
        title: Option<String>,
        x_label: Option<String>,
        y_label: Option<String>,
        z_label: Option<String>,
        x_limits: Option<(f64, f64)>,
        y_limits: Option<(f64, f64)>,
        x_log: Option<bool>,
        y_log: Option<bool>,
        legend: Option<String>,
        grid: Option<bool>,
        equal_aspect: Option<bool>,
    ) -> PyResult<()> {
        let ax = self.ax(cell);
        if let Some(t) = title {
            ax.title(t);
        }
        if let Some(t) = x_label {
            ax.x_label(t);
        }
        if let Some(t) = y_label {
            ax.y_label(t);
        }
        if let Some(t) = z_label {
            ax.z_label(t);
        }
        if let Some((a, b)) = x_limits {
            ax.x_limits(a, b);
        }
        if let Some((a, b)) = y_limits {
            ax.y_limits(a, b);
        }
        if x_log == Some(true) {
            ax.x_scale(Scale::Log(10.0));
        }
        if y_log == Some(true) {
            ax.y_scale(Scale::Log(10.0));
        }
        if let Some(g) = grid {
            ax.grid(g);
        }
        if let Some(e) = equal_aspect {
            ax.equal_aspect(e);
        }
        if let Some(l) = legend {
            ax.legend(spec::legend(&l));
        }
        Ok(())
    }

    fn camera(&mut self, cell: Cell, azimuth: f64, elevation: f64) {
        self.ax(cell).camera(azimuth, elevation);
    }
    fn rotate(&mut self, cell: Cell, axis: &str, degrees: f64) -> PyResult<()> {
        let a = match axis {
            "x" | "X" => Axis3::X,
            "y" | "Y" => Axis3::Y,
            "z" | "Z" => Axis3::Z,
            _ => return Err(err("axis must be 'x', 'y' or 'z'")),
        };
        self.ax(cell).rotate(a, degrees);
        Ok(())
    }

    // ----- charts with a fast path ------------------------------------------

    #[pyo3(signature = (cell, kind, x, y, label=None, color=None, width=None, size=None, dash=None, marker=None, opacity=None))]
    #[allow(clippy::too_many_arguments)]
    fn xy(
        &mut self,
        cell: Cell,
        kind: &str,
        x: Option<&Bound<'_, PyAny>>,
        y: &Bound<'_, PyAny>,
        label: Option<String>,
        color: Option<String>,
        width: Option<f64>,
        size: Option<f64>,
        dash: Option<String>,
        marker: Option<String>,
        opacity: Option<f32>,
    ) -> PyResult<()> {
        let y = values(y)?;
        let x = opt_values(x)?.unwrap_or_else(|| (0..y.len()).map(|i| i as f64).collect());
        let ax = self.ax(cell);
        match kind {
            "scatter" => {
                let s = ax.scatter(x, y);
                if let Some(l) = label {
                    s.label(l);
                }
                if let Some(c) = color {
                    s.color(c.as_str());
                }
                if let Some(z) = size {
                    s.size(z);
                }
                if let Some(m) = marker.as_deref().and_then(self::marker) {
                    s.marker(m);
                }
                if let Some(o) = opacity {
                    s.opacity(o);
                }
            }
            _ => {
                let l = if kind == "area" {
                    ax.area(x, y)
                } else {
                    ax.line(x, y)
                };
                if let Some(t) = label {
                    l.label(t);
                }
                if let Some(c) = color {
                    l.color(c.as_str());
                }
                if let Some(w) = width {
                    l.width(w);
                }
                if let Some(d) = dash {
                    l.dash(self::dash(&d));
                }
                if let Some(m) = marker.as_deref().and_then(self::marker) {
                    l.marker(m);
                }
            }
        }
        Ok(())
    }

    #[pyo3(signature = (cell, x, y1, y2, label=None, color=None, opacity=None))]
    #[allow(clippy::too_many_arguments)]
    fn fill_between(
        &mut self,
        cell: Cell,
        x: &Bound<'_, PyAny>,
        y1: &Bound<'_, PyAny>,
        y2: &Bound<'_, PyAny>,
        label: Option<String>,
        color: Option<String>,
        opacity: Option<f32>,
    ) -> PyResult<()> {
        let f = self
            .ax(cell)
            .fill_between(values(x)?, values(y1)?, values(y2)?);
        if let Some(l) = label {
            f.label(l);
        }
        if let Some(c) = color {
            f.color(c.as_str());
        }
        if let Some(o) = opacity {
            f.opacity(o);
        }
        Ok(())
    }

    #[pyo3(signature = (cell, categories, heights, horizontal=false, label=None, color=None))]
    fn bar(
        &mut self,
        cell: Cell,
        categories: &Bound<'_, PyAny>,
        heights: &Bound<'_, PyAny>,
        horizontal: bool,
        label: Option<String>,
        color: Option<String>,
    ) -> PyResult<()> {
        let (c, h) = (labels(categories)?, values(heights)?);
        let ax = self.ax(cell);
        let b = if horizontal {
            ax.barh(c, h)
        } else {
            ax.bar(c, h)
        };
        if let Some(l) = label {
            b.label(l);
        }
        if let Some(c) = color {
            b.color(c.as_str());
        }
        Ok(())
    }

    #[pyo3(signature = (cell, values, bins=None, label=None, color=None, density=false))]
    fn hist(
        &mut self,
        cell: Cell,
        values: &Bound<'_, PyAny>,
        bins: Option<usize>,
        label: Option<String>,
        color: Option<String>,
        density: bool,
    ) -> PyResult<()> {
        let h = self.ax(cell).hist(self::values(values)?);
        if let Some(b) = bins {
            h.bin_count(b);
        }
        if let Some(l) = label {
            h.label(l);
        }
        if let Some(c) = color {
            h.color(c.as_str());
        }
        if density {
            h.density(true);
        }
        Ok(())
    }

    #[pyo3(signature = (cell, kind, groups))]
    fn distribution(&mut self, cell: Cell, kind: &str, groups: &Bound<'_, PyAny>) -> PyResult<()> {
        let g = self::groups(groups)?;
        let ax = self.ax(cell);
        match kind {
            "box" => {
                ax.boxplot(g);
            }
            "violin" => {
                ax.violin(g);
            }
            "swarm" => {
                ax.swarm(g);
            }
            "raincloud" => {
                ax.raincloud(g);
            }
            "ridgeline" => {
                ax.ridgeline(g);
            }
            _ => {
                for (n, v) in g {
                    ax.kde(v, None).label(n);
                }
            }
        }
        Ok(())
    }

    #[pyo3(signature = (cell, z, x_labels=None, y_labels=None, colormap=None, annotate=false, colorbar=None, range=None))]
    #[allow(clippy::too_many_arguments)]
    fn heatmap(
        &mut self,
        cell: Cell,
        z: &Bound<'_, PyAny>,
        x_labels: Option<&Bound<'_, PyAny>>,
        y_labels: Option<&Bound<'_, PyAny>>,
        colormap: Option<String>,
        annotate: bool,
        colorbar: Option<String>,
        range: Option<(f64, f64)>,
    ) -> PyResult<()> {
        let z = matrix(z)?;
        let xl = match x_labels {
            Some(l) => labels(l)?,
            None => (0..z.first().map_or(0, Vec::len))
                .map(|i| i.to_string())
                .collect(),
        };
        let yl = match y_labels {
            Some(l) => labels(l)?,
            None => (0..z.len()).map(|i| i.to_string()).collect(),
        };
        let h = self.ax(cell).heatmap_labeled(xl, yl, z);
        h.colormap(self::colormap(colormap.as_deref().unwrap_or("viridis")));
        if annotate {
            h.annotate(true);
        }
        if let Some(c) = colorbar {
            h.colorbar(c);
        }
        if let Some((a, b)) = range {
            h.range(a, b);
        }
        Ok(())
    }

    #[pyo3(signature = (cell, labels, values, donut=None))]
    fn pie(
        &mut self,
        cell: Cell,
        labels: &Bound<'_, PyAny>,
        values: &Bound<'_, PyAny>,
        donut: Option<f64>,
    ) -> PyResult<()> {
        let p = self
            .ax(cell)
            .pie(self::labels(labels)?, self::values(values)?);
        if let Some(d) = donut {
            p.donut(d);
        }
        Ok(())
    }

    fn candlestick(
        &mut self,
        cell: Cell,
        x: &Bound<'_, PyAny>,
        open: &Bound<'_, PyAny>,
        high: &Bound<'_, PyAny>,
        low: &Bound<'_, PyAny>,
        close: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.ax(cell).candlestick(
            values(x)?,
            values(open)?,
            values(high)?,
            values(low)?,
            values(close)?,
        );
        Ok(())
    }

    #[pyo3(signature = (cell, z, x_range, y_range, colormap=None))]
    fn surface(
        &mut self,
        cell: Cell,
        z: &Bound<'_, PyAny>,
        x_range: (f64, f64),
        y_range: (f64, f64),
        colormap: Option<String>,
    ) -> PyResult<()> {
        let s = self.ax(cell).surface(x_range, y_range, matrix(z)?);
        if let Some(c) = colormap {
            s.colormap(self::colormap(&c));
        }
        Ok(())
    }

    #[pyo3(signature = (cell, values, dims, extent=None, colormap=None, opacity=None, threshold=None, mip=false))]
    #[allow(clippy::too_many_arguments)]
    fn volume(
        &mut self,
        cell: Cell,
        values: &Bound<'_, PyAny>,
        dims: [usize; 3],
        extent: Option<[(f64, f64); 3]>,
        colormap: Option<String>,
        opacity: Option<f64>,
        threshold: Option<f64>,
        mip: bool,
    ) -> PyResult<()> {
        let v = self::values(values)?;
        if v.len() != dims[0] * dims[1] * dims[2] {
            return Err(PyValueError::new_err(
                "volume: values do not match the shape",
            ));
        }
        let vol = self.ax(cell).volume(v, dims);
        if let Some([x, y, z]) = extent {
            vol.extent(x, y, z);
        }
        if let Some(c) = colormap {
            vol.colormap(self::colormap(&c));
        }
        if let Some(a) = opacity {
            vol.opacity(a);
        }
        if let Some(t) = threshold {
            vol.threshold(t);
        }
        if mip {
            vol.mode(VolumeMode::MaxIntensity);
        }
        Ok(())
    }

    #[pyo3(signature = (cell, x, y, z, line=false, values=None, colormap=None, size=None, color=None, label=None))]
    #[allow(clippy::too_many_arguments)]
    fn xyz(
        &mut self,
        cell: Cell,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        z: &Bound<'_, PyAny>,
        line: bool,
        values: Option<&Bound<'_, PyAny>>,
        colormap: Option<String>,
        size: Option<f64>,
        color: Option<String>,
        label: Option<String>,
    ) -> PyResult<()> {
        let (x, y, z) = (self::values(x)?, self::values(y)?, self::values(z)?);
        let v = opt_values(values)?;
        let ax = self.ax(cell);
        let p = if line {
            ax.line3d(x, y, z)
        } else {
            ax.scatter3d(x, y, z)
        };
        if let Some(v) = v {
            p.values(v);
        }
        if let Some(c) = colormap {
            p.colormap(self::colormap(&c));
        }
        if let Some(s) = size {
            p.size(s);
        }
        if let Some(c) = color {
            p.color(c.as_str());
        }
        if let Some(l) = label {
            p.label(l);
        }
        Ok(())
    }

    #[pyo3(signature = (cell, pdb, style=None, coloring=None))]
    fn molecule(
        &mut self,
        cell: Cell,
        pdb: &str,
        style: Option<String>,
        coloring: Option<String>,
    ) -> PyResult<()> {
        let mol = Molecule::from_pdb(pdb);
        if mol.atoms.is_empty() {
            return Err(err("no ATOM/HETATM records in the PDB text"));
        }
        let m = self.ax(cell).molecule(mol);
        m.style(spec::mol_style(
            style.as_deref().unwrap_or("ball_and_stick"),
        ));
        m.coloring(spec::mol_color(coloring.as_deref().unwrap_or("element")));
        Ok(())
    }

    #[pyo3(signature = (cell, kind, value, label=None, color=None))]
    fn reference(
        &mut self,
        cell: Cell,
        kind: &str,
        value: f64,
        label: Option<String>,
        color: Option<String>,
    ) {
        let ax = self.ax(cell);
        let a = if kind == "v" {
            ax.vline(value)
        } else {
            ax.hline(value)
        };
        if let Some(l) = label {
            a.label(l);
        }
        if let Some(c) = color {
            a.color(c.as_str());
        }
    }

    #[pyo3(signature = (cell, x, y, text, size=None, color=None))]
    fn text(
        &mut self,
        cell: Cell,
        x: f64,
        y: f64,
        text: &str,
        size: Option<f64>,
        color: Option<String>,
    ) {
        let t = self.ax(cell).text(x, y, text);
        if let Some(s) = size {
            t.size(s);
        }
        if let Some(c) = color {
            t.color(c.as_str());
        }
    }

    /// Any of the 40 spec chart types (or `layers`) as JSON.
    fn chart_json(&mut self, cell: Cell, json: &str) -> PyResult<()> {
        let v: serde_json::Value = serde_json::from_str(json).map_err(err)?;
        spec::apply(self.ax(cell), &v).map_err(err)
    }
}

/// The chart types of `Axes.chart` with their fields.
#[pyfunction]
fn chart_types() -> Vec<(String, String, String)> {
    spec::CHART_TYPES
        .iter()
        .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
        .collect()
}

/// Numbers as the plots read them (for tests of the input paths).
#[pyfunction]
fn _values(obj: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    values(obj)
}

#[pymodule]
fn _dataviz(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Core>()?;
    m.add_function(wrap_pyfunction!(_values, m)?)?;
    m.add_function(wrap_pyfunction!(chart_types, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
