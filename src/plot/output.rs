//! Rendering a [`Scene`] to files and pixel buffers.

use super::scene::Scene;
use super::text::FontSet;
use resvg::{tiny_skia, usvg};
use std::fmt;
use std::path::Path;
use std::sync::Arc;

/// Errors from rendering or saving a figure.
#[derive(Debug)]
pub enum Error {
    /// Reading or writing a file failed.
    Io(std::io::Error),
    /// The generated SVG could not be rendered (should not happen).
    Render(String),
    /// Image encoding failed.
    Encode(String),
    /// The file extension / format is not supported.
    UnsupportedFormat(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {e}"),
            Error::Render(e) => write!(f, "render error: {e}"),
            Error::Encode(e) => write!(f, "encoding error: {e}"),
            Error::UnsupportedFormat(e) => write!(f, "unsupported format: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Output file formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    /// Scalable vector graphics.
    Svg,
    /// Lossless raster.
    Png,
    /// Lossy raster (no transparency; composited on white).
    Jpeg,
    /// WebP (lossless).
    WebP,
    /// Windows bitmap.
    Bmp,
    /// Portable Document Format (requires the `pdf` feature).
    Pdf,
}

impl Format {
    /// Format from a file extension (case-insensitive).
    pub fn from_path(path: &Path) -> Result<Self, Error> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        Ok(match ext.as_str() {
            "svg" => Format::Svg,
            "png" => Format::Png,
            "jpg" | "jpeg" => Format::Jpeg,
            "webp" => Format::WebP,
            "bmp" => Format::Bmp,
            "pdf" => Format::Pdf,
            other => return Err(Error::UnsupportedFormat(format!(".{other}"))),
        })
    }
}

/// An RGBA image (straight alpha), the result of rasterizing a figure.
#[derive(Clone, Debug, PartialEq)]
pub struct Pixels {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes, row-major RGBA.
    pub rgba: Vec<u8>,
}

impl Pixels {
    /// The pixel at `(x, y)`.
    pub fn get(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        Some([
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ])
    }

    /// `0x00RRGGBB` pixels composited on white (the layout `minifb` expects).
    pub fn to_0rgb(&self) -> Vec<u32> {
        self.rgba
            .chunks_exact(4)
            .map(|p| {
                let a = p[3] as u32;
                let c = |v: u8| (v as u32 * a + 255 * (255 - a)) / 255;
                c(p[0]) << 16 | c(p[1]) << 8 | c(p[2])
            })
            .collect()
    }

    /// Encodes the image (PNG, JPEG, WebP or BMP).
    pub fn encode(&self, format: Format) -> Result<Vec<u8>, Error> {
        use image::{ImageBuffer, ImageFormat, Rgb, Rgba};
        let mut out = std::io::Cursor::new(Vec::new());
        let rgba: ImageBuffer<Rgba<u8>, &[u8]> =
            ImageBuffer::from_raw(self.width, self.height, &self.rgba[..])
                .ok_or_else(|| Error::Encode("buffer size mismatch".into()))?;
        let enc = |e: image::ImageError| Error::Encode(e.to_string());
        match format {
            Format::Png => rgba.write_to(&mut out, ImageFormat::Png).map_err(enc)?,
            Format::WebP => rgba.write_to(&mut out, ImageFormat::WebP).map_err(enc)?,
            Format::Jpeg | Format::Bmp => {
                // No alpha channel: composite on white.
                let rgb: ImageBuffer<Rgb<u8>, Vec<u8>> =
                    ImageBuffer::from_fn(self.width, self.height, |x, y| {
                        let p = rgba.get_pixel(x, y).0;
                        let a = p[3] as u32;
                        let c = |v: u8| ((v as u32 * a + 255 * (255 - a)) / 255) as u8;
                        Rgb([c(p[0]), c(p[1]), c(p[2])])
                    });
                let f = if format == Format::Jpeg {
                    ImageFormat::Jpeg
                } else {
                    ImageFormat::Bmp
                };
                rgb.write_to(&mut out, f).map_err(enc)?
            }
            other => {
                return Err(Error::UnsupportedFormat(format!(
                    "{other:?} is not a raster format"
                )))
            }
        }
        Ok(out.into_inner())
    }
}

/// Rasterizes a scene at `scale` (1.0 = one pixel per unit, 2.0 for
/// high-DPI output).
pub(crate) fn rasterize(scene: &Scene, fonts: &FontSet, scale: f64) -> Result<Pixels, Error> {
    let svg = scene.to_svg();
    rasterize_svg(&svg, fonts, scale)
}

pub(crate) fn usvg_options(fonts: &FontSet) -> usvg::Options<'static> {
    usvg::Options {
        fontdb: Arc::clone(&fonts.db),
        font_family: fonts.family.clone(),
        ..usvg::Options::default()
    }
}

pub(crate) fn rasterize_svg(svg: &str, fonts: &FontSet, scale: f64) -> Result<Pixels, Error> {
    let opt = usvg_options(fonts);
    let tree = usvg::Tree::from_str(svg, &opt).map_err(|e| Error::Render(e.to_string()))?;
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let size = tree.size();
    let w = (size.width() as f64 * scale).round().max(1.0) as u32;
    let h = (size.height() as f64 * scale).round().max(1.0) as u32;
    let mut pixmap =
        tiny_skia::Pixmap::new(w, h).ok_or_else(|| Error::Render("empty image".into()))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale as f32, scale as f32),
        &mut pixmap.as_mut(),
    );
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for p in pixmap.pixels() {
        let c = p.demultiply();
        rgba.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok(Pixels {
        width: w,
        height: h,
        rgba,
    })
}

#[cfg(feature = "pdf")]
pub(crate) fn to_pdf(svg: &str, fonts: &FontSet) -> Result<Vec<u8>, Error> {
    // svg2pdf bundles its own usvg version, so it gets its own font database.
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use svg2pdf::usvg as pusvg;
    static DBS: OnceLock<Mutex<HashMap<String, Arc<pusvg::fontdb::Database>>>> = OnceLock::new();
    let db = {
        let mut map = DBS
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.entry(fonts.family.clone())
            .or_insert_with(|| {
                let mut db = pusvg::fontdb::Database::new();
                db.load_system_fonts();
                // Fonts loaded from files (custom theme fonts).
                for face in fonts.db.faces() {
                    if let resvg::usvg::fontdb::Source::File(path) = &face.source {
                        if face.families.iter().any(|f| f.0 == fonts.family) {
                            let _ = db.load_font_file(path);
                        }
                    }
                }
                db.set_sans_serif_family(fonts.family.clone());
                Arc::new(db)
            })
            .clone()
    };
    let opt = pusvg::Options {
        fontdb: db,
        font_family: fonts.family.clone(),
        ..pusvg::Options::default()
    };
    let tree = pusvg::Tree::from_str(svg, &opt).map_err(|e| Error::Render(e.to_string()))?;
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|e| Error::Encode(e.to_string()))
}

/// Writes `scene` to `path`, choosing the format from the extension.
pub(crate) fn save(scene: &Scene, fonts: &FontSet, path: &Path, scale: f64) -> Result<(), Error> {
    let format = Format::from_path(path)?;
    let bytes = encode(scene, fonts, format, scale)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Encodes `scene` in `format`.
pub(crate) fn encode(
    scene: &Scene,
    fonts: &FontSet,
    format: Format,
    scale: f64,
) -> Result<Vec<u8>, Error> {
    match format {
        Format::Svg => Ok(scene.to_svg().into_bytes()),
        Format::Pdf => {
            #[cfg(feature = "pdf")]
            {
                to_pdf(&scene.to_svg(), fonts)
            }
            #[cfg(not(feature = "pdf"))]
            {
                Err(Error::UnsupportedFormat(
                    "PDF output needs the `pdf` cargo feature".into(),
                ))
            }
        }
        raster => rasterize(scene, fonts, scale)?.encode(raster),
    }
}
