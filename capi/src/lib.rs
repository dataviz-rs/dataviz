//! dataviz for C and C++: the `dataviz_c` shared/static library behind
//! `include/dataviz.h` (C) and `include/dataviz.hpp` (C++ wrapper).
//!
//! A [`DvFigure`] is an opaque heap object owned by the caller. Plots and
//! series are small value handles (figure + index), so they stay valid when
//! more plots are added. Numbers are passed as pointers (`const double *`)
//! and copied once into the chart; any chart type of the JSON spec can take
//! its arrays the same way (`dv_plot`). Failures return 0 / null and leave a
//! message for `dv_last_error` (per thread). No Rust panic crosses the C
//! boundary.

#![allow(clippy::missing_safety_doc)]

use dataviz::plot::series::density::DensityScale;
use dataviz::plot::series::line::Line;
use dataviz::plot::series::scatter::Scatter;
use dataviz::plot::spec::{self, Array};
use dataviz::plot::stats::Bins;
use dataviz::prelude::*;
use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// A figure with its output buffers.
pub struct DvFigure {
    fig: Figure,
    px: Pixels,
    out: Vec<u8>,
}

/// A plot of a figure (grid cell).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DvAxes {
    pub fig: *mut DvFigure,
    pub index: u32,
}

/// A series of a plot.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DvSeries {
    pub fig: *mut DvFigure,
    pub axes: u32,
    pub index: u32,
}

/// Style of a series; all zero means the theme's defaults.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DvStyle {
    /// Legend label (UTF-8) or null.
    pub label: *const c_char,
    /// 0xRRGGBBAA; 0 = the next palette color.
    pub color: u32,
    /// Line width in pixels; 0 = default.
    pub width: f64,
    /// Marker size in pixels; 0 = default.
    pub size: f64,
    /// `DV_MARKER_*`; 0 = default.
    pub marker: c_int,
    /// `DV_DASH_*`; 0 = solid.
    pub dash: c_int,
    /// 0..1; 0 = default.
    pub opacity: f64,
    /// Colormap name (viridis, magma, …) or null.
    pub colormap: *const c_char,
}

/// One array for `dv_plot`: the spec refers to it as `"@name"`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DvArray {
    pub name: *const c_char,
    pub data: *const f64,
    pub len: usize,
    /// Row length for matrices (heatmap / surface `z`); 0 otherwise.
    pub cols: usize,
}

thread_local! {
    static ERROR: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn set_error(msg: impl std::fmt::Display) {
    ERROR.with(|e| {
        let mut e = e.borrow_mut();
        e.clear();
        e.extend_from_slice(msg.to_string().as_bytes());
        e.push(0);
    });
}

/// Runs `f`; a panic or error becomes `fallback` plus a message.
fn guard<R>(fallback: R, f: impl FnOnce() -> Result<R, String>) -> R {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            set_error(e);
            fallback
        }
        Err(p) => {
            let msg = p
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "internal error".into());
            set_error(format!("dataviz panicked: {msg}"));
            fallback
        }
    }
}

unsafe fn cstr<'a>(p: *const c_char) -> Result<&'a str, String> {
    if p.is_null() {
        return Ok("");
    }
    // SAFETY: the caller passes a NUL-terminated string.
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map_err(|_| "text must be UTF-8".to_string())
}

unsafe fn opt_cstr<'a>(p: *const c_char) -> Option<&'a str> {
    // SAFETY: as `cstr`.
    unsafe { cstr(p) }.ok().filter(|s| !s.is_empty())
}

unsafe fn slice<'a>(p: *const f64, n: usize) -> Result<&'a [f64], String> {
    if n == 0 {
        return Ok(&[]);
    }
    if p.is_null() {
        return Err("null data pointer".into());
    }
    // SAFETY: the caller passes `n` readable doubles.
    Ok(unsafe { std::slice::from_raw_parts(p, n) })
}

unsafe fn fig_mut<'a>(f: *mut DvFigure) -> Result<&'a mut DvFigure, String> {
    // SAFETY: a live figure from `dv_figure_new` (or null).
    unsafe { f.as_mut() }.ok_or_else(|| "null figure".to_string())
}

unsafe fn axes_mut<'a>(a: DvAxes) -> Result<&'a mut Axes, String> {
    // SAFETY: as `fig_mut`.
    let f = unsafe { fig_mut(a.fig) }?;
    f.fig
        .all_axes()
        .nth(a.index as usize)
        .ok_or_else(|| "unknown plot".to_string())
}

fn color(c: u32) -> Color {
    Color::rgba(
        (c >> 24) as u8,
        (c >> 16) as u8,
        (c >> 8) as u8,
        (c & 0xff) as f32 / 255.0,
    )
}

fn marker(m: c_int) -> Option<Marker> {
    Some(match m {
        1 => Marker::None,
        2 => Marker::Circle,
        3 => Marker::Square,
        4 => Marker::Diamond,
        5 => Marker::TriangleUp,
        6 => Marker::TriangleDown,
        7 => Marker::Plus,
        8 => Marker::Cross,
        9 => Marker::Star,
        10 => Marker::Hexagon,
        _ => return None,
    })
}

fn dash(d: c_int) -> Dash {
    match d {
        1 => Dash::Dashed,
        2 => Dash::Dotted,
        3 => Dash::DashDot,
        4 => Dash::LongDash,
        _ => Dash::Solid,
    }
}

const NO_STYLE: DvStyle = DvStyle {
    label: std::ptr::null(),
    color: 0,
    width: 0.0,
    size: 0.0,
    marker: 0,
    dash: 0,
    opacity: 0.0,
    colormap: std::ptr::null(),
};

unsafe fn style<'a>(s: *const DvStyle) -> &'a DvStyle {
    // SAFETY: null or a valid style.
    unsafe { s.as_ref() }.unwrap_or(&NO_STYLE)
}

fn series(a: DvAxes, ax: &Axes) -> DvSeries {
    DvSeries {
        fig: a.fig,
        axes: a.index,
        index: ax.series_count() as u32 - 1,
    }
}

const NO_SERIES: DvSeries = DvSeries {
    fig: std::ptr::null_mut(),
    axes: 0,
    index: 0,
};

// ----- library -------------------------------------------------------------

/// The library version, e.g. "0.18.0".
#[no_mangle]
pub extern "C" fn dv_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Message of the last failure on this thread ("" when none).
#[no_mangle]
pub extern "C" fn dv_last_error() -> *const c_char {
    ERROR.with(|e| {
        let mut e = e.borrow_mut();
        if e.is_empty() {
            e.push(0);
        }
        e.as_ptr().cast()
    })
}

// ----- figures -------------------------------------------------------------

/// A blank figure of `width` × `height` pixels.
#[no_mangle]
pub extern "C" fn dv_figure_new(width: u32, height: u32) -> *mut DvFigure {
    guard(std::ptr::null_mut(), || {
        Ok(Box::into_raw(Box::new(DvFigure {
            fig: Figure::new(width.max(1), height.max(1)),
            px: Pixels::default(),
            out: Vec::new(),
        })))
    })
}

fn figure_from(json: &str, arrays: &[Array<'_>]) -> Result<Figure, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("JSON: {e}"))?;
    spec::with_arrays(arrays, || spec::figure(&v))
}

unsafe fn arrays<'a>(a: *const DvArray, n: usize) -> Result<Vec<Array<'a>>, String> {
    if n == 0 {
        return Ok(Vec::new());
    }
    if a.is_null() {
        return Err("null arrays".into());
    }
    // SAFETY: the caller passes `n` arrays.
    let a = unsafe { std::slice::from_raw_parts(a, n) };
    a.iter()
        .map(|x| {
            Ok(Array {
                // SAFETY: as the caller's contract.
                name: unsafe { cstr(x.name) }?,
                data: unsafe { slice(x.data, x.len) }?,
                cols: x.cols,
            })
        })
        .collect()
}

/// A figure from a JSON spec (any of the chart types of
/// `dv_chart_types`), with `n` arrays the spec names as `"@name"`.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_from_json(
    json: *const c_char,
    arrays_: *const DvArray,
    n: usize,
) -> *mut DvFigure {
    guard(std::ptr::null_mut(), || {
        // SAFETY: the caller's contract.
        let fig = figure_from(unsafe { cstr(json) }?, &unsafe { arrays(arrays_, n) }?)?;
        Ok(Box::into_raw(Box::new(DvFigure {
            fig,
            px: Pixels::default(),
            out: Vec::new(),
        })))
    })
}

/// Frees a figure (null is fine).
#[no_mangle]
pub unsafe extern "C" fn dv_figure_free(f: *mut DvFigure) {
    if !f.is_null() {
        // SAFETY: from `Box::into_raw`, freed once.
        drop(unsafe { Box::from_raw(f) });
    }
}

/// Sets the title above all plots.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_title(f: *mut DvFigure, title: *const c_char) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let (f, t) = unsafe { (fig_mut(f)?, cstr(title)?) };
        f.fig.title(t);
        Ok(1)
    })
}

/// Theme by name: light, dark, minimal, publication, solarized, nord,
/// high_contrast, …
#[no_mangle]
pub unsafe extern "C" fn dv_figure_theme(f: *mut DvFigure, name: *const c_char) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let (f, t) = unsafe { (fig_mut(f)?, cstr(name)?) };
        f.fig.theme(spec::theme(t));
        Ok(1)
    })
}

/// Changes the size in pixels.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_resize(f: *mut DvFigure, width: u32, height: u32) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        unsafe { fig_mut(f) }?.fig.resize(width, height);
        Ok(1)
    })
}

/// Width and height in pixels.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_size(f: *mut DvFigure, width: *mut f64, height: *mut f64) {
    guard((), || {
        // SAFETY: the caller's contract.
        let (w, h) = unsafe { fig_mut(f) }?.fig.size();
        // SAFETY: null or writable.
        unsafe {
            if let Some(p) = width.as_mut() {
                *p = w;
            }
            if let Some(p) = height.as_mut() {
                *p = h;
            }
        }
        Ok(())
    })
}

/// Renders on the GPU (Vulkan, Metal, DirectX 12, OpenGL) when the library
/// was built with the `gpu` feature; 0 otherwise.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_gpu(f: *mut DvFigure, on: c_int) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let f = unsafe { fig_mut(f) }?;
        #[cfg(feature = "gpu")]
        {
            f.fig
                .backend(if on != 0 { Backend::Gpu } else { Backend::Auto });
            Ok(1)
        }
        #[cfg(not(feature = "gpu"))]
        {
            let _ = (f, on);
            Err("built without the `gpu` feature".into())
        }
    })
}

/// The plot in grid cell (`row`, `col`), created on first use (the first
/// plot is (0, 0)).
#[no_mangle]
pub unsafe extern "C" fn dv_figure_axes(f: *mut DvFigure, row: u32, col: u32) -> DvAxes {
    dv_figure_span(f, row, col, 1, 1)
}

/// A plot spanning `rowspan` × `colspan` grid cells.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_span(
    f: *mut DvFigure,
    row: u32,
    col: u32,
    rowspan: u32,
    colspan: u32,
) -> DvAxes {
    let none = DvAxes {
        fig: std::ptr::null_mut(),
        index: 0,
    };
    guard(none, || {
        // SAFETY: the caller's contract.
        let d = unsafe { fig_mut(f) }?;
        let p: *const Axes = d.fig.span(
            row as usize,
            col as usize,
            rowspan as usize,
            colspan as usize,
        );
        let index = d
            .fig
            .all_axes()
            .position(|a| std::ptr::eq(a, p))
            .ok_or("plot not found")?;
        Ok(DvAxes {
            fig: f,
            index: index as u32,
        })
    })
}

/// Renders RGBA pixels (straight alpha) at `scale` device pixels per
/// pixel. The buffer belongs to the figure and stays valid until the next
/// render or free; null on error.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_render(
    f: *mut DvFigure,
    scale: f64,
    width: *mut u32,
    height: *mut u32,
) -> *const u8 {
    guard(std::ptr::null(), || {
        // SAFETY: the caller's contract.
        let d = unsafe { fig_mut(f) }?;
        d.fig.scale(scale);
        d.fig.render_into(&mut d.px).map_err(|e| e.to_string())?;
        // SAFETY: null or writable.
        unsafe {
            if let Some(p) = width.as_mut() {
                *p = d.px.width;
            }
            if let Some(p) = height.as_mut() {
                *p = d.px.height;
            }
        }
        Ok(d.px.rgba.as_ptr())
    })
}

fn format(name: &str) -> Result<Format, String> {
    let ext = name.trim_start_matches('.');
    Format::from_path(std::path::Path::new(&format!("x.{ext}"))).map_err(|e| e.to_string())
}

/// Encodes as `format` ("png", "svg", "pdf", "html", "jpeg", "webp",
/// "bmp"). The bytes belong to the figure until the next call; null on
/// error. Text formats end with a NUL byte not counted in `len`.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_encode(
    f: *mut DvFigure,
    fmt: *const c_char,
    scale: f64,
    len: *mut usize,
) -> *const u8 {
    guard(std::ptr::null(), || {
        // SAFETY: the caller's contract.
        let (d, fmt) = unsafe { (fig_mut(f)?, cstr(fmt)?) };
        d.fig.scale(if scale > 0.0 { scale } else { 1.0 });
        d.out = d.fig.encode(format(fmt)?).map_err(|e| e.to_string())?;
        // SAFETY: null or writable.
        if let Some(p) = unsafe { len.as_mut() } {
            *p = d.out.len();
        }
        d.out.push(0);
        Ok(d.out.as_ptr())
    })
}

/// Saves to a file; the format comes from the extension (.png, .svg, .pdf,
/// .html, .jpg, .webp, .bmp). 1 on success.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_save(f: *mut DvFigure, path: *const c_char) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let (d, path) = unsafe { (fig_mut(f)?, cstr(path)?) };
        d.fig.save(path).map_err(|e| e.to_string())?;
        Ok(1)
    })
}

/// Called by `dv_figure_show_live` up to `fps` times a second; return
/// nonzero to redraw (new data), and call `dv_window_close` to stop.
pub type DvUpdate = Option<unsafe extern "C" fn(f: *mut DvFigure, user: *mut c_void) -> c_int>;

/// Opens an interactive window (zoom, pan, 3D turning, hover) and returns
/// when it is closed. 0 when windows are unavailable.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_show(f: *mut DvFigure, title: *const c_char) -> c_int {
    // SAFETY: the caller's contract.
    unsafe { dv_figure_show_live(f, title, 30, None, std::ptr::null_mut()) }
}

/// Like `dv_figure_show`, calling `update(figure, user)` up to `fps` times
/// a second for real-time data.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_show_live(
    f: *mut DvFigure,
    title: *const c_char,
    fps: u32,
    update: DvUpdate,
    user: *mut c_void,
) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let (d, title) = unsafe { (fig_mut(f)?, cstr(title)?) };
        #[cfg(feature = "window")]
        {
            // The window owns the figure while it runs; around each
            // callback the figure goes back into the `DvFigure`, so the
            // callback's plot and series handles work.
            let fp = f;
            let mut local = std::mem::replace(&mut d.fig, Figure::new(1, 1));
            let r = local.show_live(title, fps.max(1), |fig| match update {
                Some(u) => {
                    // SAFETY: `fp` is live for the whole call and not
                    // borrowed elsewhere while the callback runs.
                    unsafe {
                        std::mem::swap(fig, &mut (*fp).fig);
                        let redraw = u(fp, user) != 0;
                        std::mem::swap(fig, &mut (*fp).fig);
                        redraw
                    }
                }
                None => false,
            });
            // SAFETY: as above.
            unsafe { (*fp).fig = local };
            r.map_err(|e| e.to_string())?;
            Ok(1)
        }
        #[cfg(not(feature = "window"))]
        {
            let _ = (d, title, fps, update, user);
            Err("built without the `window` feature".into())
        }
    })
}

// ----- interaction (for embedding in native UIs) ---------------------------

/// Zooms the plot under `(x, y)` (pixels) by `factor` (< 1 zooms in);
/// 1 when a plot is there.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_zoom(f: *mut DvFigure, x: f64, y: f64, factor: f64) -> c_int {
    // SAFETY: the caller's contract.
    guard(0, || {
        Ok(unsafe { fig_mut(f) }?.fig.zoom_at(x, y, factor) as c_int)
    })
}

/// A drag at `(x, y)` by `(dx, dy)`: pans 2D plots, turns 3D ones.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_pan(
    f: *mut DvFigure,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
) -> c_int {
    // SAFETY: the caller's contract.
    guard(0, || {
        Ok(unsafe { fig_mut(f) }?.fig.pan_by(x, y, dx, dy) as c_int)
    })
}

/// Zooms to the box between two points.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_zoom_box(
    f: *mut DvFigure,
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        Ok(unsafe { fig_mut(f) }?.fig.zoom_to_box((ax, ay), (bx, by)) as c_int)
    })
}

/// Back to the first view.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_reset(f: *mut DvFigure) {
    guard((), || {
        // SAFETY: the caller's contract.
        unsafe { fig_mut(f) }?.fig.reset_view();
        Ok(())
    })
}

/// A click: legend entries hide or show their series (1 when changed).
#[no_mangle]
pub unsafe extern "C" fn dv_figure_click(f: *mut DvFigure, x: f64, y: f64) -> c_int {
    // SAFETY: the caller's contract.
    guard(0, || Ok(unsafe { fig_mut(f) }?.fig.click(x, y) as c_int))
}

/// Text describing the data point nearest `(x, y)` (owned by the figure
/// until the next call), with its position in `px`, `py`; null when none.
#[no_mangle]
pub unsafe extern "C" fn dv_figure_hover(
    f: *mut DvFigure,
    x: f64,
    y: f64,
    px: *mut f64,
    py: *mut f64,
) -> *const c_char {
    guard(std::ptr::null(), || {
        // SAFETY: the caller's contract.
        let d = unsafe { fig_mut(f) }?;
        let Some(h) = d.fig.hover(x, y) else {
            return Ok(std::ptr::null());
        };
        // SAFETY: null or writable.
        unsafe {
            if let Some(p) = px.as_mut() {
                *p = h.x;
            }
            if let Some(p) = py.as_mut() {
                *p = h.y;
            }
        }
        d.out.clear();
        d.out.extend_from_slice(h.text.as_bytes());
        d.out.push(0);
        Ok(d.out.as_ptr().cast())
    })
}

// ----- plots ---------------------------------------------------------------

macro_rules! text_setter {
    ($(#[$m:meta])* $name:ident, $method:ident) => {
        $(#[$m])*
        #[no_mangle]
        pub unsafe extern "C" fn $name(a: DvAxes, text: *const c_char) -> c_int {
            guard(0, || {
                // SAFETY: the caller's contract.
                let (ax, t) = unsafe { (axes_mut(a)?, cstr(text)?) };
                ax.$method(t);
                Ok(1)
            })
        }
    };
}

text_setter!(
    /// Title above the plot.
    dv_axes_title,
    title
);
text_setter!(
    /// x-axis label.
    dv_axes_xlabel,
    x_label
);
text_setter!(
    /// y-axis label.
    dv_axes_ylabel,
    y_label
);

/// x limits.
#[no_mangle]
pub unsafe extern "C" fn dv_axes_xlim(a: DvAxes, min: f64, max: f64) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        unsafe { axes_mut(a) }?.x_limits(min, max);
        Ok(1)
    })
}

/// y limits.
#[no_mangle]
pub unsafe extern "C" fn dv_axes_ylim(a: DvAxes, min: f64, max: f64) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        unsafe { axes_mut(a) }?.y_limits(min, max);
        Ok(1)
    })
}

/// Logarithmic x and/or y axis.
#[no_mangle]
pub unsafe extern "C" fn dv_axes_log(a: DvAxes, x: c_int, y: c_int) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let ax = unsafe { axes_mut(a) }?;
        if x != 0 {
            ax.x_scale(Scale::Log(10.0));
        }
        if y != 0 {
            ax.y_scale(Scale::Log(10.0));
        }
        Ok(1)
    })
}

/// Legend position: best, upper_right, upper_left, lower_left,
/// lower_right, outside_right, outside_top, none.
#[no_mangle]
pub unsafe extern "C" fn dv_axes_legend(a: DvAxes, position: *const c_char) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let (ax, p) = unsafe { (axes_mut(a)?, cstr(position)?) };
        ax.legend(spec::legend(p));
        Ok(1)
    })
}

/// Grid lines on or off.
#[no_mangle]
pub unsafe extern "C" fn dv_axes_grid(a: DvAxes, on: c_int) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        unsafe { axes_mut(a) }?.grid(on != 0);
        Ok(1)
    })
}

/// Any chart type of the JSON spec (one panel: `{"type": …, fields}`, or
/// `{"layers": […]}`), drawn into this plot; arrays named `"@name"` in
/// the spec come from `arrays` without JSON conversion.
#[no_mangle]
pub unsafe extern "C" fn dv_plot(
    a: DvAxes,
    json: *const c_char,
    arrays_: *const DvArray,
    n: usize,
) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let ax = unsafe { axes_mut(a) }?;
        let v: serde_json::Value =
            serde_json::from_str(unsafe { cstr(json) }?).map_err(|e| format!("JSON: {e}"))?;
        let arrays = unsafe { arrays(arrays_, n) }?;
        spec::with_arrays(&arrays, || spec::apply(ax, &v))?;
        Ok(1)
    })
}

// ----- series --------------------------------------------------------------

unsafe fn xy<'a>(x: *const f64, y: *const f64, n: usize) -> Result<(&'a [f64], &'a [f64]), String> {
    // SAFETY: the caller's contract.
    unsafe { Ok((slice(x, n)?, slice(y, n)?)) }
}

/// Line through `n` points (x may be null: 0, 1, 2, …).
#[no_mangle]
pub unsafe extern "C" fn dv_line(
    a: DvAxes,
    x: *const f64,
    y: *const f64,
    n: usize,
    st: *const DvStyle,
) -> DvSeries {
    guard(NO_SERIES, || {
        // SAFETY: the caller's contract.
        let (ax, s) = unsafe { (axes_mut(a)?, style(st)) };
        let y = unsafe { slice(y, n) }?;
        let l = if x.is_null() {
            ax.line((0..n).map(|i| i as f64), y.iter().copied())
        } else {
            ax.line(unsafe { slice(x, n) }?.iter().copied(), y.iter().copied())
        };
        line_style(l, s)?;
        Ok(series(a, ax))
    })
}

fn line_style(l: &mut Line, s: &DvStyle) -> Result<(), String> {
    // SAFETY: the style's strings are valid or null.
    if let Some(t) = unsafe { opt_cstr(s.label) } {
        l.label(t);
    }
    if s.color != 0 {
        l.color(color(s.color));
    }
    if s.width > 0.0 {
        l.width(s.width);
    }
    if s.dash != 0 {
        l.dash(dash(s.dash));
    }
    if let Some(m) = marker(s.marker) {
        l.marker(m);
    }
    if s.size > 0.0 {
        l.marker_size(s.size);
    }
    if s.opacity > 0.0 {
        l.fill_opacity(s.opacity as f32);
    }
    Ok(())
}

/// Filled area between the line and zero.
#[no_mangle]
pub unsafe extern "C" fn dv_area(
    a: DvAxes,
    x: *const f64,
    y: *const f64,
    n: usize,
    st: *const DvStyle,
) -> DvSeries {
    guard(NO_SERIES, || {
        // SAFETY: the caller's contract.
        let (ax, s) = unsafe { (axes_mut(a)?, style(st)) };
        let (x, y) = unsafe { xy(x, y, n) }?;
        let l = ax.area(x.iter().copied(), y.iter().copied());
        line_style(l, s)?;
        Ok(series(a, ax))
    })
}

/// Scatter plot; `values` (may be null) color the points through the
/// style's colormap.
#[no_mangle]
pub unsafe extern "C" fn dv_scatter(
    a: DvAxes,
    x: *const f64,
    y: *const f64,
    n: usize,
    values: *const f64,
    st: *const DvStyle,
) -> DvSeries {
    guard(NO_SERIES, || {
        // SAFETY: the caller's contract.
        let (ax, s) = unsafe { (axes_mut(a)?, style(st)) };
        let (x, y) = unsafe { xy(x, y, n) }?;
        let p: &mut Scatter = ax.scatter(x.iter().copied(), y.iter().copied());
        if let Some(t) = unsafe { opt_cstr(s.label) } {
            p.label(t);
        }
        if s.color != 0 {
            p.color(color(s.color));
        }
        if s.size > 0.0 {
            p.size(s.size);
        }
        if let Some(m) = marker(s.marker) {
            p.marker(m);
        }
        if s.opacity > 0.0 {
            p.opacity(s.opacity as f32);
        }
        if !values.is_null() {
            p.values(unsafe { slice(values, n) }?.iter().copied());
        }
        if let Some(c) = unsafe { opt_cstr(s.colormap) } {
            p.colormap(spec::colormap(c));
        }
        Ok(series(a, ax))
    })
}

/// Histogram with `bins` bins (0: automatic).
#[no_mangle]
pub unsafe extern "C" fn dv_hist(
    a: DvAxes,
    values: *const f64,
    n: usize,
    bins: u32,
    st: *const DvStyle,
) -> DvSeries {
    guard(NO_SERIES, || {
        // SAFETY: the caller's contract.
        let (ax, s) = unsafe { (axes_mut(a)?, style(st)) };
        let h = ax.hist(unsafe { slice(values, n) }?.iter().copied());
        if bins > 0 {
            h.bins(Bins::Count(bins as usize));
        }
        if let Some(t) = unsafe { opt_cstr(s.label) } {
            h.label(t);
        }
        if s.color != 0 {
            h.color(color(s.color));
        }
        if s.opacity > 0.0 {
            h.opacity(s.opacity as f32);
        }
        Ok(series(a, ax))
    })
}

/// Heatmap of a `rows` × `cols` matrix (row-major).
#[no_mangle]
pub unsafe extern "C" fn dv_heatmap(
    a: DvAxes,
    z: *const f64,
    rows: usize,
    cols: usize,
    st: *const DvStyle,
) -> DvSeries {
    guard(NO_SERIES, || {
        // SAFETY: the caller's contract.
        let (ax, s) = unsafe { (axes_mut(a)?, style(st)) };
        let z = unsafe { slice(z, rows * cols) }?;
        let h = ax.heatmap(z.chunks_exact(cols.max(1)).map(<[f64]>::to_vec).collect());
        if let Some(c) = unsafe { opt_cstr(s.colormap) } {
            h.colormap(spec::colormap(c));
        }
        Ok(series(a, ax))
    })
}

/// Density image of millions or billions of points (counted per pixel at
/// every zoom); `how`: 0 equalized histogram, 1 log, 2 linear.
#[no_mangle]
pub unsafe extern "C" fn dv_density(
    a: DvAxes,
    x: *const f64,
    y: *const f64,
    n: usize,
    how: c_int,
    st: *const DvStyle,
) -> DvSeries {
    guard(NO_SERIES, || {
        // SAFETY: the caller's contract.
        let (ax, s) = unsafe { (axes_mut(a)?, style(st)) };
        let (x, y) = unsafe { xy(x, y, n) }?;
        let d = ax.density(x.iter().copied(), y.iter().copied());
        d.scale(match how {
            1 => DensityScale::Log,
            2 => DensityScale::Linear,
            _ => DensityScale::EqHist,
        });
        if let Some(c) = unsafe { opt_cstr(s.colormap) } {
            d.colormap(spec::colormap(c));
        }
        Ok(series(a, ax))
    })
}

/// 3D points (`line` nonzero joins them into a 3D line).
#[no_mangle]
pub unsafe extern "C" fn dv_scatter3d(
    a: DvAxes,
    x: *const f64,
    y: *const f64,
    z: *const f64,
    n: usize,
    line: c_int,
    st: *const DvStyle,
) -> DvSeries {
    guard(NO_SERIES, || {
        // SAFETY: the caller's contract.
        let (ax, s) = unsafe { (axes_mut(a)?, style(st)) };
        let (x, y) = unsafe { xy(x, y, n) }?;
        let z = unsafe { slice(z, n) }?;
        let (x, y, z) = (x.iter().copied(), y.iter().copied(), z.iter().copied());
        let p = if line != 0 {
            ax.line3d(x, y, z)
        } else {
            ax.scatter3d(x, y, z)
        };
        if let Some(t) = unsafe { opt_cstr(s.label) } {
            p.label(t);
        }
        if s.color != 0 {
            p.color(color(s.color));
        }
        if s.size > 0.0 || s.width > 0.0 {
            p.size(if line != 0 && s.width > 0.0 {
                s.width
            } else {
                s.size
            });
        }
        if let Some(c) = unsafe { opt_cstr(s.colormap) } {
            p.colormap(spec::colormap(c));
        }
        Ok(series(a, ax))
    })
}

/// 3D surface of a `rows` × `cols` grid over the given x and y ranges.
#[no_mangle]
pub unsafe extern "C" fn dv_surface(
    a: DvAxes,
    z: *const f64,
    rows: usize,
    cols: usize,
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    st: *const DvStyle,
) -> DvSeries {
    guard(NO_SERIES, || {
        // SAFETY: the caller's contract.
        let (ax, s) = unsafe { (axes_mut(a)?, style(st)) };
        let z = unsafe { slice(z, rows * cols) }?;
        let m = z.chunks_exact(cols.max(1)).map(<[f64]>::to_vec).collect();
        let p = ax.surface((x0, x1), (y0, y1), m);
        if let Some(c) = unsafe { opt_cstr(s.colormap) } {
            p.colormap(spec::colormap(c));
        }
        if s.opacity > 0.0 {
            p.opacity(s.opacity as f32);
        }
        Ok(series(a, ax))
    })
}

/// Candlesticks (x is usually Unix seconds).
#[no_mangle]
pub unsafe extern "C" fn dv_candlestick(
    a: DvAxes,
    x: *const f64,
    open: *const f64,
    high: *const f64,
    low: *const f64,
    close: *const f64,
    n: usize,
) -> DvSeries {
    guard(NO_SERIES, || {
        // SAFETY: the caller's contract.
        let ax = unsafe { axes_mut(a) }?;
        let c = |p| unsafe { slice(p, n) }.map(|s| s.iter().copied());
        ax.candlestick(c(x)?, c(open)?, c(high)?, c(low)?, c(close)?);
        Ok(series(a, ax))
    })
}

/// Text at data coordinates.
#[no_mangle]
pub unsafe extern "C" fn dv_text(a: DvAxes, x: f64, y: f64, text: *const c_char) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let (ax, t) = unsafe { (axes_mut(a)?, cstr(text)?) };
        ax.text(x, y, t);
        Ok(1)
    })
}

/// Horizontal (`vertical` 0) or vertical reference line.
#[no_mangle]
pub unsafe extern "C" fn dv_refline(a: DvAxes, at: f64, vertical: c_int) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let ax = unsafe { axes_mut(a) }?;
        if vertical != 0 {
            ax.vline(at);
        } else {
            ax.hline(at);
        }
        Ok(1)
    })
}

// ----- live data -----------------------------------------------------------

unsafe fn series_axes<'a>(s: DvSeries) -> Result<&'a mut Axes, String> {
    // SAFETY: the caller's contract.
    unsafe {
        axes_mut(DvAxes {
            fig: s.fig,
            index: s.axes,
        })
    }
}

/// Replaces the data of a line or scatter series.
#[no_mangle]
pub unsafe extern "C" fn dv_series_set_data(
    s: DvSeries,
    x: *const f64,
    y: *const f64,
    n: usize,
) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let ax = unsafe { series_axes(s) }?;
        let (x, y) = unsafe { xy(x, y, n) }?;
        let (x, y) = (x.iter().copied(), y.iter().copied());
        let i = s.index as usize;
        if let Some(l) = ax.series_mut::<Line>(i) {
            l.set_data(x, y);
        } else if let Some(p) = ax.series_mut::<Scatter>(i) {
            p.set_data(x, y);
        } else {
            return Err("not a line or scatter series".into());
        }
        Ok(1)
    })
}

/// Appends a point to a line or scatter series, keeping at most `keep`
/// points (0: all) — a rolling window for streaming data.
#[no_mangle]
pub unsafe extern "C" fn dv_series_push(s: DvSeries, x: f64, y: f64, keep: usize) -> c_int {
    guard(0, || {
        // SAFETY: the caller's contract.
        let ax = unsafe { series_axes(s) }?;
        let i = s.index as usize;
        if let Some(l) = ax.series_mut::<Line>(i) {
            l.push(x, y);
            if keep > 0 {
                l.keep_last(keep);
            }
        } else if let Some(p) = ax.series_mut::<Scatter>(i) {
            p.push(x, y);
            if keep > 0 {
                p.keep_last(keep);
            }
        } else {
            return Err("not a line or scatter series".into());
        }
        Ok(1)
    })
}

/// The chart types of `dv_plot` / `dv_figure_from_json` as JSON
/// `[[type, fields, description], …]` (static string).
#[no_mangle]
pub extern "C" fn dv_chart_types() -> *const c_char {
    static TYPES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    TYPES
        .get_or_init(|| {
            let mut v = serde_json::to_vec(spec::CHART_TYPES).unwrap_or_default();
            v.push(0);
            v
        })
        .as_ptr()
        .cast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrays_reach_every_chart_type_without_json_numbers() {
        let f = dv_figure_new(400, 300);
        let ax = unsafe { dv_figure_axes(f, 0, 0) };
        let z: Vec<f64> = (0..12).map(f64::from).collect();
        let arrays = [DvArray {
            name: c"z".as_ptr(),
            data: z.as_ptr(),
            len: z.len(),
            cols: 4,
        }];
        let ok = unsafe {
            dv_plot(
                ax,
                cr#"{"type": "heatmap", "z": "@z"}"#.as_ptr(),
                arrays.as_ptr(),
                1,
            )
        };
        assert_eq!(ok, 1);
        // A matrix field needs `cols`.
        let flat = [DvArray {
            cols: 0,
            ..arrays[0]
        }];
        let bad = unsafe {
            dv_plot(
                ax,
                cr#"{"type": "heatmap", "z": "@z"}"#.as_ptr(),
                flat.as_ptr(),
                1,
            )
        };
        assert_eq!(bad, 0);
        let msg = unsafe { CStr::from_ptr(dv_last_error()) }.to_str().unwrap();
        assert!(msg.contains("matrix"), "{msg}");
        let mut n = 0;
        assert!(!unsafe { dv_figure_encode(f, c"png".as_ptr(), 1.0, &mut n) }.is_null());
        assert!(n > 100);
        unsafe { dv_figure_free(f) };
    }

    #[test]
    fn null_handles_fail_without_crashing() {
        let none = DvAxes {
            fig: std::ptr::null_mut(),
            index: 0,
        };
        assert_eq!(unsafe { dv_axes_title(none, c"x".as_ptr()) }, 0);
        assert!(unsafe {
            dv_line(
                none,
                std::ptr::null(),
                std::ptr::null(),
                0,
                std::ptr::null(),
            )
        }
        .fig
        .is_null());
        assert_eq!(unsafe { dv_series_push(NO_SERIES, 1.0, 1.0, 0) }, 0);
    }
}
