//! Raster backend dispatch: CPU (single or multi-threaded) or GPU.

use super::output::{Backend, Error, Pixels};
use super::scene::Scene;
use super::text::FontSet;
use resvg::tiny_skia::Pixmap;
use std::cell::RefCell;

thread_local! {
    /// The pixel buffer of the last dropped figure on this thread. A new
    /// figure of the same size reuses it: its pages are already mapped, so
    /// the first render skips hundreds of page faults (~0.5 ms at 800×600).
    static POOL: RefCell<Option<Pixmap>> = const { RefCell::new(None) };
}

/// Keeps `pm` for the next figure on this thread.
pub(crate) fn recycle(pm: Pixmap) {
    POOL.with(|p| *p.borrow_mut() = Some(pm));
}

/// A pooled buffer of `w × h` pixels, if one is available.
fn pooled(w: u32, h: u32) -> Option<Pixmap> {
    POOL.with(|p| {
        let mut p = p.borrow_mut();
        p.as_ref()
            .is_some_and(|pm| pm.width() == w && pm.height() == h)
            .then(|| p.take())
            .flatten()
    })
}

/// Renders `scene` at `scale` into `pixmap` (premultiplied RGBA, resized if
/// needed) with `backend`.
pub(crate) fn render(
    scene: &Scene,
    fonts: &FontSet,
    scale: f64,
    backend: Backend,
    pixmap: &mut Option<Pixmap>,
) -> Result<(), Error> {
    if backend == Backend::Gpu {
        #[cfg(feature = "gpu")]
        return super::gpu::render(scene, fonts, scale, pixmap);
        #[cfg(not(feature = "gpu"))]
        return Err(Error::Render(
            "the GPU backend needs the `gpu` cargo feature".into(),
        ));
    }
    let s = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let w = (scene.width * s).round().max(1.0) as u32;
    let h = (scene.height * s).round().max(1.0) as u32;
    let threads = backend.cpu_threads(w, h);
    if pixmap.is_none() {
        *pixmap = pooled(w, h);
    }
    super::raster::render_pixmap_threads(scene, fonts, scale, pixmap, threads)
        .ok_or_else(|| Error::Render("empty image".into()))
}

/// Where a render ends up.
pub(crate) enum Out<'a> {
    /// Straight-alpha RGBA.
    Pixels(&'a mut Pixels),
    /// `0x00RRGGBB` composited on white (window frame buffers).
    Orgb(&'a mut Vec<u32>),
}

/// Renders into `out`; the CPU backends draw into `pixmap` first, the GPU
/// converts its read-back pixels directly.
pub(crate) fn render_to(
    scene: &Scene,
    fonts: &FontSet,
    scale: f64,
    backend: Backend,
    pixmap: &mut Option<Pixmap>,
    out: Out<'_>,
) -> Result<(), Error> {
    if backend == Backend::Gpu {
        #[cfg(feature = "gpu")]
        return super::gpu::render_out(scene, fonts, scale, out);
    }
    // CPU: draw straight into the output buffer (4 bytes per pixel in both
    // layouts) and convert it in place: no separate pixel buffer.
    let _ = pixmap;
    let (w, h) = super::raster::device_size(scene, scale);
    let threads = backend.cpu_threads(w, h);
    let n = w as usize * h as usize;
    match out {
        Out::Pixels(p) => {
            p.width = w;
            p.height = h;
            p.rgba.resize(n * 4, 0);
            super::raster::render_bytes(scene, fonts, scale, &mut p.rgba, w, h, threads);
            super::raster::demultiply_in_place(&mut p.rgba);
        }
        Out::Orgb(o) => {
            o.resize(n, 0);
            // SAFETY: a u32 slice viewed as its bytes (no alignment or
            // validity requirements for u8).
            let bytes =
                unsafe { std::slice::from_raw_parts_mut(o.as_mut_ptr().cast::<u8>(), n * 4) };
            super::raster::render_bytes(scene, fonts, scale, bytes, w, h, threads);
            super::raster::premul_to_0rgb_in_place(o, threads);
        }
    }
    Ok(())
}
