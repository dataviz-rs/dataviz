//! Font loading for the pixel renderer.
//!
//! Fonts used to be read from disk and parsed for every single text element,
//! and a missing font path panicked (`FigureConfig::default()` has none).
//! Fonts are now loaded once per path and cached; when no usable path is
//! configured, a sans-serif system font is used instead. If no font can be
//! found at all, text is skipped rather than panicking.

use ab_glyph::{FontArc, FontVec};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Loads (and caches) the font at `path`, falling back to a system font if
/// `path` is `None` or cannot be loaded.
pub(crate) fn load(path: Option<&str>) -> Option<FontArc> {
    match path {
        Some(p) => from_path(p).or_else(system),
        None => system(),
    }
}

fn from_path(path: &str) -> Option<FontArc> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<FontArc>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    cache
        .entry(path.to_string())
        .or_insert_with(|| {
            let font = std::fs::read(path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| FontArc::try_from_vec(bytes).map_err(|e| e.to_string()));
            match font {
                Ok(font) => Some(font),
                Err(e) => {
                    // Reported once per path; drawing continues with a fallback.
                    eprintln!("dataviz: cannot load font {path:?} ({e}); using a system font");
                    None
                }
            }
        })
        .clone()
}

/// A regular sans-serif system font, looked up once.
fn system() -> Option<FontArc> {
    static SYSTEM: OnceLock<Option<FontArc>> = OnceLock::new();
    SYSTEM.get_or_init(find_system_font).clone()
}

const PREFERRED: [&str; 8] = [
    "DejaVu Sans",
    "Liberation Sans",
    "Arial",
    "Helvetica",
    "Noto Sans",
    "Segoe UI",
    "Roboto",
    "Ubuntu",
];

/// Font database for rendering SVG text: system fonts plus `font_path`, with
/// the generic `sans-serif` family (used by the generated SVG) mapped to
/// `font_path`'s family or to an installed sans-serif font.
#[cfg(feature = "window")]
pub(crate) fn svg_font_database(font_path: Option<&str>) -> resvg::usvg::fontdb::Database {
    use resvg::usvg::fontdb::Database;

    let mut db = Database::new();
    db.load_system_fonts();
    let mut family = None;
    if let Some(path) = font_path {
        let before: std::collections::HashSet<_> = db.faces().map(|f| f.id).collect();
        if db.load_font_file(path).is_ok() {
            family = db
                .faces()
                .find(|f| !before.contains(&f.id))
                .and_then(|f| f.families.first())
                .map(|(name, _)| name.clone());
        }
    }
    let family = family.or_else(|| {
        PREFERRED
            .iter()
            .find(|name| {
                db.faces()
                    .any(|f| f.families.iter().any(|(n, _)| n == *name))
            })
            .map(|s| s.to_string())
    });
    if let Some(family) = family {
        db.set_sans_serif_family(family);
    }
    db
}

fn find_system_font() -> Option<FontArc> {
    use resvg::usvg::fontdb::{Database, Style, Weight};

    let mut db = Database::new();
    db.load_system_fonts();

    let regular = |f: &&resvg::usvg::fontdb::FaceInfo| {
        f.style == Style::Normal && f.weight == Weight::NORMAL && !f.monospaced
    };
    let rank = |f: &resvg::usvg::fontdb::FaceInfo| {
        f.families
            .iter()
            .filter_map(|(name, _)| PREFERRED.iter().position(|p| p == name))
            .min()
            .unwrap_or(PREFERRED.len())
    };

    let mut candidates: Vec<_> = db
        .faces()
        .filter(regular)
        .map(|f| (rank(f), f.id))
        .collect();
    candidates.sort_by_key(|(rank, _)| *rank);
    let all: Vec<_> = db.faces().map(|f| f.id).collect();
    candidates
        .into_iter()
        .map(|(_, id)| id)
        .chain(all)
        .find_map(|id| {
            db.with_face_data(id, |data, index| {
                FontVec::try_from_vec_and_index(data.to_vec(), index).ok()
            })
            .flatten()
        })
        .map(FontArc::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_falls_back_without_panicking() {
        // Whether a system font exists depends on the machine; the call
        // itself must not panic either way.
        let a = load(Some("/definitely/not/here.ttf"));
        let b = load(None);
        assert_eq!(a.is_some(), b.is_some());
    }
}
