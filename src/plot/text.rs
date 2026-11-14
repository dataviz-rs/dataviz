//! Font resolution and text measurement.
//!
//! By default the embedded DejaVu Sans is used, so no system font scan is
//! needed and output is identical on every machine. A font file can be set
//! with `Theme::font`. The same font measures text (layout), names the font
//! family in SVG output and is drawn by the rasterizer.

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use resvg::usvg::fontdb::{Database, Source};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The embedded default font.
pub(crate) const DEFAULT_FONT: &[u8] = include_bytes!("../../assets/DejaVuSans.ttf");
/// Family name of [`DEFAULT_FONT`].
pub(crate) const DEFAULT_FAMILY: &str = "DejaVu Sans";

/// A resolved font.
pub(crate) struct FontSet {
    /// Family name written into SVG output.
    pub family: String,
    /// Glyphs and metrics.
    pub font: Option<FontArc>,
    /// Font file (None = embedded font).
    path: Option<String>,
    /// Font database for the resvg reference renderer (tests), on demand.
    #[cfg_attr(not(test), allow(dead_code))]
    db: OnceLock<Arc<Database>>,
    /// Scale factor from font units to pixels per pixel of em size.
    units_to_em: f32,
}

impl FontSet {
    /// Resolves the font for an optional font file; cached per path.
    pub fn resolve(path: Option<&str>) -> Arc<FontSet> {
        static CACHE: OnceLock<Mutex<HashMap<Option<String>, Arc<FontSet>>>> = OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        let key = path.map(str::to_string);
        let mut map = cache.lock().unwrap_or_else(|e| e.into_inner());
        map.entry(key)
            .or_insert_with(|| Arc::new(build(path)))
            .clone()
    }

    /// PxScale whose em size is `size` pixels (SVG `font-size` semantics).
    pub fn scale(&self, size: f64) -> PxScale {
        PxScale::from((size as f32) * self.units_to_em)
    }

    /// Width of `text` at `size` pixels (math in `$...$` is laid out, see
    /// [`crate::plot::math`]).
    pub fn width(&self, text: &str, size: f64) -> f64 {
        if super::math::has_math(text) {
            return super::math::layout(self, text, size, false).width;
        }
        self.width_plain(text, size)
    }

    /// Width of `text` without math layout.
    pub fn width_plain(&self, text: &str, size: f64) -> f64 {
        match &self.font {
            Some(font) => {
                let sf = font.as_scaled(self.scale(size));
                let mut w = 0.0f32;
                let mut prev = None;
                for c in text.chars() {
                    let id = sf.glyph_id(c);
                    if let Some(p) = prev {
                        w += sf.kern(p, id);
                    }
                    w += sf.h_advance(id);
                    prev = Some(id);
                }
                w as f64
            }
            // Average glyph width of a typical sans-serif font.
            None => text.chars().count() as f64 * size * 0.55,
        }
    }

    /// Ink extent of `text` at `size` pixels: (left, right, top, bottom)
    /// relative to the origin on the baseline, y down (None without ink).
    pub fn ink(&self, text: &str, size: f64) -> Option<(f64, f64, f64, f64)> {
        let font = self.font.as_ref()?;
        let sf = font.as_scaled(self.scale(size));
        let mut pen = 0.0f32;
        let mut prev = None;
        let mut b: Option<(f32, f32, f32, f32)> = None;
        for c in text.chars() {
            let id = sf.glyph_id(c);
            if let Some(p) = prev {
                pen += sf.kern(p, id);
            }
            // Unrounded outline bounds (font units, y up).
            if let Some(o) = font.outline(id) {
                let r = o.bounds;
                let (hx, vy) = (sf.h_scale_factor(), sf.v_scale_factor());
                // ab_glyph stores the top in `min.y`; order explicitly.
                let (y_lo, y_hi) = (r.min.y.min(r.max.y), r.min.y.max(r.max.y));
                let q = (
                    pen + r.min.x * hx,
                    pen + r.max.x * hx,
                    -y_hi * vy,
                    -y_lo * vy,
                );
                b = Some(match b {
                    None => q,
                    Some(p) => (p.0.min(q.0), p.1.max(q.1), p.2.min(q.2), p.3.max(q.3)),
                });
            }
            pen += sf.h_advance(id);
            prev = Some(id);
        }
        b.map(|q| (q.0 as f64, q.1 as f64, q.2 as f64, q.3 as f64))
    }

    /// Height of a line of text at `size` pixels (ascent + descent).
    pub fn height(&self, size: f64) -> f64 {
        match &self.font {
            Some(font) => {
                let sf = font.as_scaled(self.scale(size));
                (sf.ascent() - sf.descent()) as f64
            }
            None => size * 1.2,
        }
    }

    /// Font database (system fonts + this font) for resvg.
    #[cfg(test)]
    pub fn db(&self) -> Arc<Database> {
        self.db
            .get_or_init(|| {
                let mut db = Database::new();
                db.load_system_fonts();
                match &self.path {
                    Some(p) => {
                        let _ = db.load_font_file(p);
                    }
                    None => db.load_font_data(DEFAULT_FONT.to_vec()),
                }
                db.set_sans_serif_family(self.family.clone());
                Arc::new(db)
            })
            .clone()
    }

    /// The font file (None for the embedded font).
    #[allow(dead_code)]
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }
}

fn units_to_em(font: &FontArc) -> f32 {
    // ab_glyph's PxScale is the ascent-descent height; convert so that the
    // scale is the em size like SVG font-size.
    match font.units_per_em() {
        Some(upem) if upem > 0.0 => font.height_unscaled() / upem,
        _ => 1.0,
    }
}

fn embedded() -> FontSet {
    let font = FontArc::try_from_slice(DEFAULT_FONT).ok();
    let k = font.as_ref().map_or(1.0, units_to_em);
    FontSet {
        family: DEFAULT_FAMILY.into(),
        font,
        path: None,
        db: OnceLock::new(),
        units_to_em: k,
    }
}

fn build(path: Option<&str>) -> FontSet {
    let Some(path) = path else { return embedded() };
    let loaded = std::fs::read(path).ok().and_then(|bytes| {
        // Family name from the font's name table (no system scan).
        let mut db = Database::new();
        db.load_font_data(bytes.clone());
        let family = db
            .faces()
            .find(|f| matches!(f.source, Source::Binary(_)))
            .and_then(|f| f.families.first().map(|x| x.0.clone()))?;
        let font = FontArc::try_from_vec(bytes).ok()?;
        Some((family, font))
    });
    match loaded {
        Some((family, font)) => {
            let k = units_to_em(&font);
            FontSet {
                family,
                font: Some(font),
                path: Some(path.to_string()),
                db: OnceLock::new(),
                units_to_em: k,
            }
        }
        None => {
            eprintln!("dataviz: cannot load font {path:?}; using the embedded font");
            embedded()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_text() {
        let f = FontSet::resolve(None);
        assert_eq!(f.family, DEFAULT_FAMILY);
        let w1 = f.width("x", 10.0);
        let w2 = f.width("xxxx", 10.0);
        assert!(w1 > 0.0 && (w2 - 4.0 * w1).abs() < 1.0);
        assert!(f.width("x", 20.0) > w1 * 1.9);
        assert!(f.height(10.0) > 10.0);
        assert_eq!(f.width("", 10.0), 0.0);
        // DejaVu Sans "x" advance is 1212/2048 em.
        assert!((f.width("x", 20.0) - 20.0 * 1212.0 / 2048.0).abs() < 0.2);
    }

    #[test]
    fn missing_font_file_falls_back() {
        let f = FontSet::resolve(Some("/no/such/font.ttf"));
        assert_eq!(f.family, DEFAULT_FAMILY);
    }

    #[test]
    fn font_file_family() {
        let p = "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf";
        if std::path::Path::new(p).exists() {
            assert_eq!(FontSet::resolve(Some(p)).family, "DejaVu Serif");
        }
    }
}
