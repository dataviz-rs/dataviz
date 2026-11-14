//! Font resolution and text measurement.
//!
//! The same font is used to measure text (for layout), to name the font
//! family in SVG output and to rasterize text, so labels never overflow or
//! overlap because of a metrics mismatch.

use ab_glyph::{Font, FontArc, FontVec, PxScale, ScaleFont};
use resvg::usvg::fontdb::{Database, Source};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// A resolved font: glyph metrics plus the font database used for rasterizing.
#[derive(Clone)]
pub(crate) struct FontSet {
    /// Family name written into SVG output.
    pub family: String,
    /// Metrics for measuring (None if no font is available at all).
    pub font: Option<FontArc>,
    /// Database used by the rasterizer (contains `family`).
    pub db: Arc<Database>,
}

impl FontSet {
    /// Resolves the font for an optional font file; cached per path.
    pub fn resolve(path: Option<&str>) -> Arc<FontSet> {
        static CACHE: OnceLock<Mutex<HashMap<Option<String>, Arc<FontSet>>>> = OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        let key = path.map(str::to_string);
        if let Some(f) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return f.clone();
        }
        let set = Arc::new(build(path));
        cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, set.clone());
        set
    }

    /// Width of `text` at `size` pixels.
    pub fn width(&self, text: &str, size: f64) -> f64 {
        match &self.font {
            Some(font) => {
                let sf = font.as_scaled(PxScale::from(size as f32));
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

    /// Height of a line of text at `size` pixels (ascent + descent).
    pub fn height(&self, size: f64) -> f64 {
        match &self.font {
            Some(font) => {
                let sf = font.as_scaled(PxScale::from(size as f32));
                (sf.ascent() - sf.descent()) as f64
            }
            None => size * 1.2,
        }
    }
}

const PREFERRED: [&str; 10] = [
    "DejaVu Sans",
    "Liberation Sans",
    "Arial",
    "Helvetica",
    "Helvetica Neue",
    "Noto Sans",
    "Segoe UI",
    "Roboto",
    "Ubuntu",
    "Cantarell",
];

fn system_db() -> Arc<Database> {
    static DB: OnceLock<Arc<Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
    .clone()
}

fn face_font(db: &Database, id: resvg::usvg::fontdb::ID) -> Option<FontArc> {
    db.with_face_data(id, |data, index| {
        FontVec::try_from_vec_and_index(data.to_vec(), index).ok()
    })
    .flatten()
    .map(FontArc::new)
}

fn build(path: Option<&str>) -> FontSet {
    let base = system_db();
    if let Some(path) = path {
        let mut db = (*base).clone();
        let before = db.len();
        db.load_font_file(path).ok();
        if db.len() > before {
            // Faces loaded from this file are the ones whose source is the path.
            let face = db.faces().find(|f| match &f.source {
                Source::File(p) => p.as_os_str() == std::ffi::OsStr::new(path),
                _ => false,
            });
            if let Some(face) = face {
                let family = face
                    .families
                    .first()
                    .map(|f| f.0.clone())
                    .unwrap_or_else(|| "sans-serif".into());
                let id = face.id;
                let font = face_font(&db, id);
                db.set_sans_serif_family(family.clone());
                return FontSet {
                    family,
                    font,
                    db: Arc::new(db),
                };
            }
        }
        eprintln!("dataviz: cannot load font {path:?}; using a system font");
    }

    use resvg::usvg::fontdb::{Style, Weight};
    let mut faces: Vec<_> = base
        .faces()
        .filter(|f| f.style == Style::Normal && f.weight == Weight::NORMAL && !f.monospaced)
        .map(|f| {
            let rank = f
                .families
                .iter()
                .filter_map(|(n, _)| PREFERRED.iter().position(|p| p == n))
                .min()
                .unwrap_or(PREFERRED.len());
            (rank, f.id, f.families.first().map(|x| x.0.clone()))
        })
        .collect();
    faces.sort_by_key(|f| f.0);
    for (_, id, family) in faces {
        if let (Some(font), Some(family)) = (face_font(&base, id), family) {
            let mut db = (*base).clone();
            db.set_sans_serif_family(family.clone());
            return FontSet {
                family,
                font: Some(font),
                db: Arc::new(db),
            };
        }
    }
    FontSet {
        family: "sans-serif".into(),
        font: None,
        db: base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_text() {
        let f = FontSet::resolve(None);
        let w1 = f.width("x", 10.0);
        let w2 = f.width("xxxx", 10.0);
        assert!(w1 > 0.0 && (w2 - 4.0 * w1).abs() < 1.0);
        assert!(f.width("x", 20.0) > w1 * 1.9);
        assert!(f.height(10.0) > 8.0);
        assert_eq!(f.width("", 10.0), 0.0);
    }

    #[test]
    fn missing_font_file_falls_back() {
        let f = FontSet::resolve(Some("/no/such/font.ttf"));
        assert_eq!(f.family, FontSet::resolve(None).family);
    }
}
