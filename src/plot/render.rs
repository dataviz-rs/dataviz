//! Raster backend dispatch: CPU (single or multi-threaded) or GPU.

use super::output::{Backend, Error, Pixels};
use super::scene::Scene;
use super::text::FontSet;
use resvg::tiny_skia::Pixmap;

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
    render(scene, fonts, scale, backend, pixmap)?;
    let pm = pixmap
        .as_ref()
        .ok_or_else(|| Error::Render("empty image".into()))?;
    let threads = backend.cpu_threads(pm.width(), pm.height());
    match out {
        Out::Pixels(p) => super::raster::to_pixels(pm, p, threads),
        Out::Orgb(o) => super::raster::to_0rgb(pm, o, threads),
    }
    Ok(())
}
