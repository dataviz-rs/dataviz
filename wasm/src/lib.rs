//! dataviz for JavaScript: the WebAssembly module of the `dataviz-rs` npm
//! package (`npm/` holds the JavaScript side).
//!
//! Plain C ABI, no imports and no wasm-bindgen: JavaScript writes a JSON
//! spec into memory from [`dv_alloc`], gets a chart handle from [`dv_new`]
//! and reads results (pixels, SVG, PNG, HTML, hover text) from the pointer
//! a call returns, [`dv_out_len`] bytes long. Handles are small integers;
//! 0 means failure ([`dv_error`] has the message).

use dataviz::plot::spec;
use dataviz::prelude::*;
use std::cell::RefCell;

struct Chart {
    fig: Figure,
    px: Pixels,
}

#[derive(Default)]
struct State {
    charts: Vec<Option<Chart>>,
    out: Vec<u8>,
    error: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Runs `f` on chart `h`; `None` for an unknown handle.
fn chart<R>(h: u32, f: impl FnOnce(&mut Chart, &mut Vec<u8>) -> R) -> Option<R> {
    state(|s| {
        let State { charts, out, .. } = s;
        let c = charts.get_mut((h as usize).checked_sub(1)?)?.as_mut()?;
        Some(f(c, out))
    })
}

/// Puts bytes into the output buffer; returns its address.
fn emit(out: &mut Vec<u8>, bytes: &[u8]) -> *const u8 {
    out.clear();
    out.extend_from_slice(bytes);
    out.as_ptr()
}

fn fail(msg: String) -> u32 {
    state(|s| s.error = msg);
    0
}

/// # Safety
/// `ptr..ptr + len` must be memory from [`dv_alloc`] holding UTF-8.
unsafe fn text<'a>(ptr: *const u8, len: usize) -> Result<&'a str, String> {
    if ptr.is_null() {
        return Ok("");
    }
    // SAFETY: the caller's contract.
    let b = unsafe { std::slice::from_raw_parts(ptr, len) };
    std::str::from_utf8(b).map_err(|e| e.to_string())
}

fn figure(json: &str) -> Result<Figure, String> {
    figure_with(json, &[])
}

fn figure_with(json: &str, arrays: &[spec::Array<'_>]) -> Result<Figure, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("JSON: {e}"))?;
    spec::with_arrays(arrays, || spec::figure(&v)).map_err(|e| e.to_string())
}

/// Arrays described by `n` records of five u32 at `desc`: name pointer,
/// name length, data pointer (f64, aligned), length, row length.
///
/// # Safety
/// The records and what they point to must be valid memory.
unsafe fn arrays<'a>(desc: *const u32, n: usize) -> Result<Vec<spec::Array<'a>>, String> {
    if n == 0 {
        return Ok(Vec::new());
    }
    // SAFETY: the caller's contract.
    let d = unsafe { std::slice::from_raw_parts(desc, n * 5) };
    d.as_chunks::<5>()
        .0
        .iter()
        .map(|r| {
            // SAFETY: the caller's contract.
            let name = unsafe { text(r[0] as usize as *const u8, r[1] as usize) }?;
            let p = r[2] as usize as *const f64;
            if !(p as usize).is_multiple_of(8) {
                return Err("array data must be 8-byte aligned".into());
            }
            // SAFETY: the caller's contract.
            let data = unsafe { std::slice::from_raw_parts(p, r[3] as usize) };
            Ok(spec::Array {
                name,
                data,
                cols: r[4] as usize,
            })
        })
        .collect()
}

/// Like [`dv_new`], with `n` binary arrays (see [`arrays`]) that the spec
/// names as `"@name"`.
///
/// # Safety
/// As [`dv_new`]; `desc` holds `n` valid records.
#[no_mangle]
pub unsafe extern "C" fn dv_new_with(
    ptr: *const u8,
    len: usize,
    desc: *const u32,
    n: usize,
) -> u32 {
    // SAFETY: the caller's contract.
    let fig = match unsafe { text(ptr, len) }
        .and_then(|j| figure_with(j, &unsafe { arrays(desc, n) }?))
    {
        Ok(f) => f,
        Err(e) => return fail(e),
    };
    add(fig)
}

/// Like [`dv_update`], with binary arrays as [`dv_new_with`].
///
/// # Safety
/// As [`dv_new_with`].
#[no_mangle]
pub unsafe extern "C" fn dv_update_with(
    h: u32,
    ptr: *const u8,
    len: usize,
    desc: *const u32,
    n: usize,
) -> u32 {
    // SAFETY: the caller's contract.
    let fig = match unsafe { text(ptr, len) }
        .and_then(|j| figure_with(j, &unsafe { arrays(desc, n) }?))
    {
        Ok(f) => f,
        Err(e) => return fail(e),
    };
    replace(h, fig)
}

fn add(fig: Figure) -> u32 {
    state(|s| {
        let c = Some(Chart {
            fig,
            px: Pixels::default(),
        });
        match s.charts.iter().position(Option::is_none) {
            Some(i) => {
                s.charts[i] = c;
                i as u32 + 1
            }
            None => {
                s.charts.push(c);
                s.charts.len() as u32
            }
        }
    })
}

fn replace(h: u32, fig: Figure) -> u32 {
    chart(h, |c, _| {
        let (w, hh) = c.fig.size();
        c.fig = fig;
        c.fig.resize(w as u32, hh as u32);
        1
    })
    .unwrap_or(0)
}

/// `n` bytes for JavaScript to write into (free with [`dv_free`]).
#[no_mangle]
pub extern "C" fn dv_alloc(n: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(n.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Frees memory from [`dv_alloc`].
///
/// # Safety
/// `ptr` and `n` must come from one [`dv_alloc`] call.
#[no_mangle]
pub unsafe extern "C" fn dv_free(ptr: *mut u8, n: usize) {
    if !ptr.is_null() {
        // SAFETY: allocated by `dv_alloc` with capacity `n.max(1)`.
        drop(unsafe { Vec::from_raw_parts(ptr, 0, n.max(1)) });
    }
}

/// A chart from a JSON spec (see `chartTypes()`); 0 on error.
///
/// # Safety
/// `ptr..ptr + len` must be memory from [`dv_alloc`] holding UTF-8.
#[no_mangle]
pub unsafe extern "C" fn dv_new(ptr: *const u8, len: usize) -> u32 {
    // SAFETY: the caller's contract.
    let fig = match unsafe { text(ptr, len) }.and_then(figure) {
        Ok(f) => f,
        Err(e) => return fail(e),
    };
    add(fig)
}

/// Replaces the chart's spec, keeping its size; 0 on error.
///
/// # Safety
/// As [`dv_new`].
#[no_mangle]
pub unsafe extern "C" fn dv_update(h: u32, ptr: *const u8, len: usize) -> u32 {
    // SAFETY: the caller's contract.
    let fig = match unsafe { text(ptr, len) }.and_then(figure) {
        Ok(f) => f,
        Err(e) => return fail(e),
    };
    replace(h, fig)
}

/// Frees a chart.
#[no_mangle]
pub extern "C" fn dv_drop(h: u32) {
    state(|s| {
        if let Some(c) = (h as usize)
            .checked_sub(1)
            .and_then(|i| s.charts.get_mut(i))
        {
            *c = None;
        }
    });
}

/// The message of the last failure (UTF-8, [`dv_out_len`] bytes).
#[no_mangle]
pub extern "C" fn dv_error() -> *const u8 {
    state(|s| {
        let State { out, error, .. } = s;
        emit(out, error.as_bytes())
    })
}

/// Length of the last result.
#[no_mangle]
pub extern "C" fn dv_out_len() -> usize {
    state(|s| s.out.len())
}

/// Canvas size of a chart in CSS pixels (width; height with [`dv_height`]).
#[no_mangle]
pub extern "C" fn dv_width(h: u32) -> f64 {
    chart(h, |c, _| c.fig.size().0).unwrap_or(0.0)
}

#[no_mangle]
pub extern "C" fn dv_height(h: u32) -> f64 {
    chart(h, |c, _| c.fig.size().1).unwrap_or(0.0)
}

/// Sets the canvas size (CSS pixels).
#[no_mangle]
pub extern "C" fn dv_resize(h: u32, width: u32, height: u32) {
    chart(h, |c, _| {
        c.fig.resize(width.max(1), height.max(1));
    });
}

/// Renders straight-alpha RGBA at `scale` device pixels per CSS pixel;
/// the size is [`dv_px_width`] × [`dv_px_height`]. Null on error.
#[no_mangle]
pub extern "C" fn dv_render(h: u32, scale: f64) -> *const u8 {
    let r = chart(h, |c, _| {
        c.fig.scale(scale);
        c.fig.render_into(&mut c.px).map(|_| c.px.rgba.as_ptr())
    });
    match r {
        Some(Ok(p)) => p,
        Some(Err(e)) => {
            fail(e.to_string());
            std::ptr::null()
        }
        None => std::ptr::null(),
    }
}

/// Width of the last rendered pixels.
#[no_mangle]
pub extern "C" fn dv_px_width(h: u32) -> u32 {
    chart(h, |c, _| c.px.width).unwrap_or(0)
}

/// Height of the last rendered pixels.
#[no_mangle]
pub extern "C" fn dv_px_height(h: u32) -> u32 {
    chart(h, |c, _| c.px.height).unwrap_or(0)
}

fn encode(h: u32, format: Format) -> *const u8 {
    let r = chart(h, |c, out| {
        c.fig.scale(1.0);
        c.fig.encode(format).map(|b| emit(out, &b))
    });
    match r {
        Some(Ok(p)) => p,
        Some(Err(e)) => {
            fail(e.to_string());
            std::ptr::null()
        }
        None => std::ptr::null(),
    }
}

/// SVG document (UTF-8).
#[no_mangle]
pub extern "C" fn dv_svg(h: u32) -> *const u8 {
    encode(h, Format::Svg)
}

/// PNG file.
#[no_mangle]
pub extern "C" fn dv_png(h: u32) -> *const u8 {
    encode(h, Format::Png)
}

/// Self-contained interactive HTML page (UTF-8).
#[no_mangle]
pub extern "C" fn dv_html(h: u32) -> *const u8 {
    encode(h, Format::Html)
}

/// Zooms the plot under `(x, y)` (CSS pixels) by `factor` (< 1 zooms in;
/// 3D plots move the camera); 1 when a plot is there.
#[no_mangle]
pub extern "C" fn dv_zoom(h: u32, x: f64, y: f64, factor: f64) -> u32 {
    chart(h, |c, _| c.fig.zoom_at(x, y, factor) as u32).unwrap_or(0)
}

/// Drags the plot under `(x, y)` by `(dx, dy)`: pans 2D plots, turns 3D.
#[no_mangle]
pub extern "C" fn dv_pan(h: u32, x: f64, y: f64, dx: f64, dy: f64) -> u32 {
    chart(h, |c, _| c.fig.pan_by(x, y, dx, dy) as u32).unwrap_or(0)
}

/// Zooms to the box between two points.
#[no_mangle]
pub extern "C" fn dv_zoom_box(h: u32, ax: f64, ay: f64, bx: f64, by: f64) -> u32 {
    chart(h, |c, _| c.fig.zoom_to_box((ax, ay), (bx, by)) as u32).unwrap_or(0)
}

/// Back to the first view.
#[no_mangle]
pub extern "C" fn dv_reset(h: u32) {
    chart(h, |c, _| c.fig.reset_view());
}

/// A click: legend rows hide or show their series (1 when changed).
#[no_mangle]
pub extern "C" fn dv_click(h: u32, x: f64, y: f64) -> u32 {
    chart(h, |c, _| c.fig.click(x, y) as u32).unwrap_or(0)
}

/// The nearest data point as JSON `{"plot","x","y","text"}`; empty (length
/// 0) when there is none.
#[no_mangle]
pub extern "C" fn dv_hover(h: u32, x: f64, y: f64) -> *const u8 {
    chart(h, |c, out| match c.fig.hover(x, y) {
        Some(hv) => {
            let j = serde_json::json!({"plot": hv.plot, "x": hv.x, "y": hv.y, "text": hv.text});
            emit(out, j.to_string().as_bytes())
        }
        None => emit(out, b""),
    })
    .unwrap_or(std::ptr::null())
}

/// The chart types of the spec as JSON `[[type, fields, description], …]`.
#[no_mangle]
pub extern "C" fn dv_chart_types() -> *const u8 {
    let j = serde_json::to_string(spec::CHART_TYPES).unwrap_or_default();
    state(|s| emit(&mut s.out, j.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(json: &str) -> u32 {
        let p = dv_alloc(json.len());
        // SAFETY: `p` has room for `json`.
        unsafe { std::ptr::copy_nonoverlapping(json.as_ptr(), p, json.len()) };
        let h = unsafe { dv_new(p, json.len()) };
        unsafe { dv_free(p, json.len()) };
        h
    }

    fn out(p: *const u8) -> Vec<u8> {
        // SAFETY: results are `dv_out_len` bytes at `p`.
        unsafe { std::slice::from_raw_parts(p, dv_out_len()) }.to_vec()
    }

    #[test]
    fn spec_to_pixels_svg_png_and_interaction() {
        let h = call(
            r#"{"width": 400, "height": 300, "type": "line", "x": [1, 2, 3], "y": [2, 1, 3], "label": "a"}"#,
        );
        assert!(h > 0);
        assert!(!dv_render(h, 2.0).is_null());
        assert_eq!((dv_px_width(h), dv_px_height(h)), (800, 600));
        assert!(out(dv_svg(h)).starts_with(b"<svg"));
        assert!(out(dv_png(h)).starts_with(b"\x89PNG"));
        assert!(dv_zoom(h, 200.0, 150.0, 0.5) == 1);
        dv_reset(h);
        dv_drop(h);
        assert!(dv_svg(h).is_null());
        assert_eq!(call("{"), 0);
        assert!(String::from_utf8(out(dv_error()))
            .unwrap()
            .starts_with("JSON"));
    }
}
