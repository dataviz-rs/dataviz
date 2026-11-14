//! Font resolution and text measurement.
//!
//! By default the embedded DejaVu Sans is used, so no system font scan is
//! needed and output is identical on every machine. A font file can be set
//! with `Theme::font`. The same font measures text (layout), names the font
//! family in SVG output and is drawn by the rasterizer.
//!
//! Characters the font does not have (CJK, emoji, rare symbols, …) are drawn
//! with a system font that has them: the system fonts are scanned once, the
//! first time such a character appears. Right-to-left text (Arabic, Hebrew)
//! is put in visual order and, like other scripts that need it (Indic, Thai,
//! combining marks), shaped with HarfRust; other text takes a fast path
//! without shaping.

use ab_glyph::{Font, FontArc, FontRef, GlyphId, PxScale, ScaleFont};
use resvg::usvg::fontdb::{Database, Source};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// A font face that draws glyphs: the theme font or a system fallback.
/// Faces live for the whole program (they are few and cached), so their
/// address is a stable cache key.
pub(crate) struct Face {
    pub font: FontArc,
    data: &'static [u8],
    index: u32,
    upem: f32,
    units_to_em: f32,
    shaper: OnceLock<Option<harfrust::Font>>,
}

impl Face {
    fn load(data: &'static [u8], index: u32) -> Option<&'static Face> {
        let font = FontArc::new(FontRef::try_from_slice_and_index(data, index).ok()?);
        let upem = font.units_per_em().filter(|u| *u > 0.0).unwrap_or(2048.0);
        let units_to_em = units_to_em(&font);
        Some(Box::leak(Box::new(Face {
            font,
            data,
            index,
            upem,
            units_to_em,
            shaper: OnceLock::new(),
        })))
    }

    /// Cache key of the face's glyphs (even: the low bit marks italics).
    pub fn key(&'static self) -> usize {
        self as *const Face as usize
    }

    /// Pixels per font unit at `size` pixels per em.
    pub fn px_per_unit(&self, size: f64) -> f32 {
        size as f32 / self.upem
    }

    fn scale(&self, size: f64) -> PxScale {
        PxScale::from(size as f32 * self.units_to_em)
    }

    fn has(&self, c: char) -> bool {
        let id = self.font.glyph_id(c);
        id.0 != 0
            && (c.is_whitespace()
                || self.font.outline(id).is_some()
                || self.font.glyph_raster_image2(id, 128).is_some())
    }

    fn shaper(&self) -> Option<&harfrust::Font> {
        self.shaper
            .get_or_init(|| harfrust::Font::new(self.data, self.index))
            .as_ref()
    }
}

/// Whether `c` needs shaping (joining scripts, reordering, combining marks).
fn complex(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F
        | 0x0590..=0x08FF
        | 0x0900..=0x0DFF
        | 0x0E00..=0x0FFF
        | 0x1000..=0x109F
        | 0x1780..=0x18AF
        | 0x1A00..=0x1BFF
        | 0x1CD0..=0x1CFF
        | 0x200C..=0x200F
        | 0x202A..=0x202E
        | 0x2066..=0x2069
        | 0xA800..=0xABFF
        | 0xFB1D..=0xFDFF
        | 0xFE00..=0xFE0F
        | 0xFE20..=0xFE2F
        | 0xFE70..=0xFEFF
        | 0x10800..=0x10FFF
        | 0x11000..=0x11FFF
        | 0x1E800..=0x1EFFF
        | 0x1F3FB..=0x1F3FF
        | 0xE0100..=0xE01EF)
}

/// Whether `c` is a strong right-to-left character (or RTL mark).
fn rtl(c: char) -> bool {
    matches!(c as u32,
        0x0590..=0x08FF
        | 0xFB1D..=0xFDFF
        | 0xFE70..=0xFEFF
        | 0x10800..=0x10FFF
        | 0x1E800..=0x1EFFF
        | 0x200F
        | 0x202B
        | 0x202E
        | 0x2067)
}

/// System fonts for characters the theme font lacks, loaded on first use.
struct Fallback {
    db: Database,
    /// Faces in order of preference.
    order: Vec<resvg::usvg::fontdb::ID>,
    loaded: Mutex<HashMap<resvg::usvg::fontdb::ID, Option<&'static Face>>>,
}

/// Families tried first for missing characters, in order.
const PREFERRED: &[&str] = &[
    "Noto Sans",
    "DejaVu Sans",
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Noto Sans CJK KR",
    "Noto Sans CJK TC",
    "Noto Sans Arabic",
    "Noto Naskh Arabic",
    "Noto Sans Hebrew",
    "Noto Sans Devanagari",
    "Noto Sans Bengali",
    "Noto Sans Tamil",
    "Noto Sans Thai",
    "Noto Sans Math",
    "Noto Sans Symbols",
    "Noto Sans Symbols 2",
    "Noto Emoji",
    "Symbola",
    "Segoe UI",
    "Segoe UI Symbol",
    "Segoe UI Emoji",
    "Microsoft YaHei",
    "Yu Gothic",
    "Malgun Gothic",
    "Arial Unicode MS",
    "PingFang SC",
    "Hiragino Sans",
    "Apple SD Gothic Neo",
    "Apple Symbols",
    "Droid Sans Fallback",
    "WenQuanYi Micro Hei",
    "FreeSans",
    "FreeSerif",
    "Liberation Sans",
    "Arial",
];

impl Fallback {
    fn new() -> Self {
        let mut db = Database::new();
        db.load_system_fonts();
        let rank = |f: &resvg::usvg::fontdb::FaceInfo| {
            let fam = f.families.first().map_or("", |x| x.0.as_str());
            let pref = PREFERRED
                .iter()
                .position(|p| *p == fam)
                .unwrap_or(PREFERRED.len());
            let style = usize::from(f.style != resvg::usvg::fontdb::Style::Normal);
            let weight = (f.weight.0 as i32 - 400).unsigned_abs() as usize;
            (
                pref,
                style,
                usize::from(f.monospaced),
                weight,
                fam.to_string(),
            )
        };
        let mut faces: Vec<_> = db.faces().map(|f| (rank(f), f.id)).collect();
        faces.sort();
        Self {
            order: faces.into_iter().map(|(_, id)| id).collect(),
            db,
            loaded: Mutex::new(HashMap::new()),
        }
    }

    fn face(&self, id: resvg::usvg::fontdb::ID) -> Option<&'static Face> {
        let mut loaded = self.loaded.lock().unwrap_or_else(|e| e.into_inner());
        *loaded.entry(id).or_insert_with(|| {
            self.db
                .with_face_data(id, |data, index| {
                    // Kept for the rest of the program, like the face.
                    let data: &'static [u8] = Box::leak(data.to_vec().into_boxed_slice());
                    Face::load(data, index)
                })
                .flatten()
        })
    }

    /// The first face (in order of preference) that has `c`: faces whose
    /// family names the character's script come first, so that only a few
    /// font files are read.
    fn find(&self, c: char) -> Option<&'static Face> {
        let words = script_words(c);
        let named = |id: &resvg::usvg::fontdb::ID| {
            self.db.face(*id).is_some_and(|f| {
                f.families
                    .iter()
                    .any(|(name, _)| words.iter().any(|w| name.contains(w)))
            })
        };
        let first = self.order.iter().filter(|id| named(id));
        let rest = self.order.iter().filter(|id| !named(id));
        first.chain(rest).find_map(|&id| {
            // Cheap check on the face's character map before keeping it.
            let has = self
                .db
                .with_face_data(id, |data, index| ttf_has(data, index, c))
                .unwrap_or(false);
            if !has {
                return None;
            }
            self.face(id).filter(|f| f.has(c))
        })
    }
}

impl Fallback {
    /// The first color emoji face (by family name) that has `c`.
    fn find_emoji(&self, c: char) -> Option<&'static Face> {
        self.order
            .iter()
            .filter(|id| {
                self.db
                    .face(**id)
                    .is_some_and(|f| f.families.iter().any(|(n, _)| n.contains("Emoji")))
            })
            .find_map(|&id| {
                let has = self
                    .db
                    .with_face_data(id, |data, index| ttf_has(data, index, c))
                    .unwrap_or(false);
                if !has {
                    return None;
                }
                self.face(id).filter(|f| {
                    let g = f.font.glyph_id(c);
                    f.font.glyph_raster_image2(g, 128).is_some()
                })
            })
    }
}

/// Characters shown as (color) emoji by default.
fn emoji_default(c: char) -> bool {
    matches!(c as u32, 0x1F000..=0x1FAFF | 0x2705 | 0x274C | 0x2B50 | 0x2B55 | 0x231A..=0x231B | 0x23E9..=0x23F3)
}

/// Words in the family names of fonts likely to cover `c`'s script.
fn script_words(c: char) -> &'static [&'static str] {
    match c as u32 {
        0x0370..=0x03FF | 0x1F00..=0x1FFF => &["Greek", "Sans"],
        0x0400..=0x052F => &["Cyrillic", "Sans"],
        0x0530..=0x058F => &["Armenian"],
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => &["Hebrew"],
        0x0600..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => &["Arabic", "Naskh", "Kufi"],
        0x0900..=0x097F => &["Devanagari"],
        0x0980..=0x09FF => &["Bengali"],
        0x0A00..=0x0A7F => &["Gurmukhi"],
        0x0A80..=0x0AFF => &["Gujarati"],
        0x0B00..=0x0B7F => &["Oriya"],
        0x0B80..=0x0BFF => &["Tamil"],
        0x0C00..=0x0C7F => &["Telugu"],
        0x0C80..=0x0CFF => &["Kannada"],
        0x0D00..=0x0D7F => &["Malayalam"],
        0x0D80..=0x0DFF => &["Sinhala"],
        0x0E00..=0x0E7F => &["Thai"],
        0x0E80..=0x0EFF => &["Lao"],
        0x0F00..=0x0FFF => &["Tibetan"],
        0x1000..=0x109F => &["Myanmar"],
        0x10A0..=0x10FF => &["Georgian"],
        0x1200..=0x139F => &["Ethiopic"],
        0x1780..=0x17FF => &["Khmer"],
        0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF => {
            &["CJK", "Korean", "Gothic", "Fallback", "KR"]
        }
        0x2E80..=0x2FFF
        | 0x3000..=0x312F
        | 0x3190..=0x9FFF
        | 0xF900..=0xFAFF
        | 0x20000..=0x3FFFF => &[
            "CJK", "Fallback", "Hei", "Gothic", "Mincho", "Song", "Ming", "Kai",
        ],
        0x2190..=0x22FF | 0x27C0..=0x27FF | 0x2900..=0x2AFF | 0x1D400..=0x1D7FF => {
            &["Math", "Symbol", "STIX", "DejaVu"]
        }
        0x2300..=0x27BF | 0x2B00..=0x2BFF | 0x1F000..=0x1FAFF => &["Symbol", "Emoji", "Symbola"],
        _ => &["Sans"],
    }
}

/// Whether the font in `data` maps `c` to a glyph (without loading it).
fn ttf_has(data: &[u8], index: u32, c: char) -> bool {
    FontRef::try_from_slice_and_index(data, index).is_ok_and(|f| f.glyph_id(c).0 != 0)
}

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
    /// The theme font as a face.
    primary: Option<&'static Face>,
    /// System fonts for missing characters (scanned on first need).
    fallback: OnceLock<Fallback>,
    /// Face chosen for each character the primary font lacks.
    chars: RwLock<HashMap<char, Option<&'static Face>>>,
    /// Color emoji face for each emoji character.
    emoji: RwLock<HashMap<char, Option<&'static Face>>>,
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
        if self.primary.is_none() {
            // Average glyph width of a typical sans-serif font.
            return text.chars().count() as f64 * size * 0.55;
        }
        self.glyphs(text, size, |_, _, _, _| {}) as f64
    }

    /// The face that draws `c` (the theme font, else a system font that has
    /// it; None if no font has it).
    fn face_for(&self, c: char, emoji: bool) -> Option<&'static Face> {
        let primary = self.primary?;
        if emoji {
            if let Some(f) = self.emoji_face(c) {
                return Some(f);
            }
        }
        if primary.font.glyph_id(c).0 != 0 || c.is_control() {
            return Some(primary);
        }
        if let Some(f) = self.chars.read().unwrap_or_else(|e| e.into_inner()).get(&c) {
            return *f;
        }
        let found = self.fallback.get_or_init(Fallback::new).find(c);
        self.chars
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(c, found);
        found
    }

    /// A color emoji face for `c` (cached; None when the system has none).
    fn emoji_face(&self, c: char) -> Option<&'static Face> {
        if let Some(f) = self.emoji.read().unwrap_or_else(|e| e.into_inner()).get(&c) {
            return *f;
        }
        let found = self.fallback.get_or_init(Fallback::new).find_emoji(c);
        self.emoji
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(c, found);
        found
    }

    /// Lays out one line of `text` at `size` pixels: calls `f(face, glyph,
    /// x, y)` for every glyph (x along the baseline from the start, y down)
    /// in visual order and returns the advance width.
    pub fn glyphs(
        &self,
        text: &str,
        size: f64,
        mut f: impl FnMut(&'static Face, GlyphId, f32, f32),
    ) -> f32 {
        let Some(primary) = self.primary else {
            return 0.0;
        };
        // Fast path: every character in the theme font, no shaping needed.
        if text.chars().all(|c| {
            !complex(c) && !emoji_default(c) && (primary.font.glyph_id(c).0 != 0 || c.is_control())
        }) {
            return simple_run(primary, text, size, 0.0, &mut f);
        }
        let mut pen = 0.0;
        let mut each_run = |s: &str, right_to_left: bool, pen: &mut f32| {
            // Runs of characters drawn by the same face, in logical order.
            let mut runs: Vec<(Option<&'static Face>, usize, usize)> = Vec::new();
            let mut chars = s.char_indices().peekable();
            while let Some((i, c)) = chars.next() {
                // Emoji presentation: emoji blocks, or a character followed
                // by the emoji variation selector U+FE0F.
                let emoji = emoji_default(c) || chars.peek().is_some_and(|(_, n)| *n == '\u{FE0F}');
                // Marks and joiners stay with the preceding face.
                let face = if (c as u32 >= 0x0300 && c as u32 <= 0x036F)
                    || matches!(c as u32, 0x200C | 0x200D | 0xFE00..=0xFE0F | 0x1F3FB..=0x1F3FF)
                {
                    runs.last().map_or(Some(primary), |r| r.0)
                } else {
                    self.face_for(c, emoji)
                };
                match runs.last_mut() {
                    Some(r) if r.0.map(Face::key) == face.map(Face::key) => r.2 = i + c.len_utf8(),
                    _ => runs.push((face, i, i + c.len_utf8())),
                }
            }
            if right_to_left {
                runs.reverse();
            }
            for (face, a, b) in runs {
                let part = &s[a..b];
                let Some(face) = face else {
                    // No font has these characters: leave a gap.
                    *pen += part.chars().count() as f32 * size as f32 * 0.5;
                    continue;
                };
                *pen = if right_to_left || part.chars().any(complex) {
                    shape_run(face, part, size, right_to_left, *pen, &mut f)
                        .unwrap_or_else(|| simple_run(face, part, size, *pen, &mut f))
                } else {
                    simple_run(face, part, size, *pen, &mut f)
                };
            }
        };
        if text.chars().any(rtl) {
            let bidi = unicode_bidi::BidiInfo::new(text, None);
            for para in &bidi.paragraphs {
                let (levels, runs) = bidi.visual_runs(para, para.range.clone());
                for r in runs {
                    each_run(&text[r.clone()], levels[r.start].is_rtl(), &mut pen);
                }
            }
        } else {
            each_run(text, false, &mut pen);
        }
        pen
    }

    /// Ink extent of `text` at `size` pixels: (left, right, top, bottom)
    /// relative to the origin on the baseline, y down (None without ink).
    pub fn ink(&self, text: &str, size: f64) -> Option<(f64, f64, f64, f64)> {
        let mut b: Option<(f32, f32, f32, f32)> = None;
        self.glyphs(text, size, |face, id, x, y| {
            // Unrounded outline bounds (font units, y up).
            if let Some(o) = face.font.outline(id) {
                let r = o.bounds;
                let k = face.px_per_unit(size);
                // ab_glyph stores the top in `min.y`; order explicitly.
                let (y_lo, y_hi) = (r.min.y.min(r.max.y), r.min.y.max(r.max.y));
                let q = (x + r.min.x * k, x + r.max.x * k, y - y_hi * k, y - y_lo * k);
                b = Some(match b {
                    None => q,
                    Some(p) => (p.0.min(q.0), p.1.max(q.1), p.2.min(q.2), p.3.max(q.3)),
                });
            }
        });
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

/// Glyphs of `s` in `face` with kerning, no shaping.
fn simple_run(
    face: &'static Face,
    s: &str,
    size: f64,
    mut pen: f32,
    f: &mut impl FnMut(&'static Face, GlyphId, f32, f32),
) -> f32 {
    let sf = face.font.as_scaled(face.scale(size));
    let mut prev = None;
    for c in s.chars() {
        let id = sf.glyph_id(c);
        if let Some(p) = prev {
            pen += sf.kern(p, id);
        }
        f(face, id, pen, 0.0);
        pen += sf.h_advance(id);
        prev = Some(id);
    }
    pen
}

/// Glyphs of `s` in `face` shaped with HarfRust (None if the face cannot
/// be shaped).
fn shape_run(
    face: &'static Face,
    s: &str,
    size: f64,
    right_to_left: bool,
    mut pen: f32,
    f: &mut impl FnMut(&'static Face, GlyphId, f32, f32),
) -> Option<f32> {
    let font = face.shaper()?;
    let shaper = harfrust::ShaperFont::new(font);
    let mut buf = harfrust::Buffer::new();
    buf.push_str(s);
    buf.set_direction(if right_to_left {
        harfrust::Direction::RightToLeft
    } else {
        harfrust::Direction::LeftToRight
    });
    buf.guess_segment_properties();
    harfrust::shape(&shaper, &mut buf, harfrust::ShapeOptions::new()).ok()?;
    let k = face.px_per_unit(size);
    for (info, pos) in buf.glyph_infos().iter().zip(buf.glyph_positions()) {
        let id = GlyphId(u16::try_from(info.glyph_id).unwrap_or(0));
        f(
            face,
            id,
            pen + pos.x_offset as f32 * k,
            -(pos.y_offset as f32) * k,
        );
        pen += pos.x_advance as f32 * k;
    }
    Some(pen)
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
    let primary = Face::load(DEFAULT_FONT, 0);
    FontSet {
        family: DEFAULT_FAMILY.into(),
        font: primary.map(|f| f.font.clone()),
        path: None,
        db: OnceLock::new(),
        units_to_em: primary.map_or(1.0, |f| f.units_to_em),
        primary,
        fallback: OnceLock::new(),
        chars: RwLock::new(HashMap::new()),
        emoji: RwLock::new(HashMap::new()),
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
        // Theme fonts are cached for the rest of the program.
        let data: &'static [u8] = Box::leak(bytes.into_boxed_slice());
        let face = Face::load(data, 0)?;
        Some((family, face))
    });
    match loaded {
        Some((family, face)) => FontSet {
            family,
            font: Some(face.font.clone()),
            path: Some(path.to_string()),
            db: OnceLock::new(),
            units_to_em: face.units_to_em,
            primary: Some(face),
            fallback: OnceLock::new(),
            chars: RwLock::new(HashMap::new()),
            emoji: RwLock::new(HashMap::new()),
        },
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

    /// Glyphs of `text`: (face key, glyph id, x).
    fn glyphs(text: &str) -> Vec<(usize, u16, f32)> {
        let f = FontSet::resolve(None);
        let mut v = Vec::new();
        f.glyphs(text, 20.0, |face, id, x, _| v.push((face.key(), id.0, x)));
        v
    }

    #[test]
    fn latin_takes_the_primary_face() {
        let f = FontSet::resolve(None);
        let g = glyphs("Ağ ışık");
        assert_eq!(g.len(), 7);
        assert!(g.iter().all(|(k, _, _)| *k == f.primary.unwrap().key()));
        assert!(g.windows(2).all(|w| w[0].2 < w[1].2));
    }

    #[test]
    fn missing_characters_use_a_system_font() {
        let f = FontSet::resolve(None);
        let g = glyphs("a中b");
        assert_eq!(g.len(), 3);
        if g[1].1 == 0 || g[1].0 == g[0].0 {
            eprintln!("no system font with CJK; skipped");
            return;
        }
        assert!(f.width("中", 20.0) > 15.0);
    }

    #[test]
    fn right_to_left_text_is_reordered_and_shaped() {
        // Hebrew is in DejaVu Sans: the last character comes first.
        let f = FontSet::resolve(None);
        let font = &f.primary.unwrap().font;
        let g = glyphs("שלום");
        assert_eq!(g.len(), 4);
        assert_eq!(g[0].1, font.glyph_id('ם').0);
        assert_eq!(g[3].1, font.glyph_id('ש').0);
        assert!(g.windows(2).all(|w| w[0].2 < w[1].2));
        // Arabic letters join: shaping picks contextual forms, not the
        // isolated ones of the character map.
        let g = glyphs("سلام");
        let isolated: Vec<u16> = "سلام".chars().map(|c| font.glyph_id(c).0).collect();
        assert!(!g.is_empty());
        assert!(g.iter().any(|(_, id, _)| !isolated.contains(id)), "{g:?}");
        // Numbers inside right-to-left text keep their order.
        let g = glyphs("שלום 123");
        let digits: Vec<u16> = g
            .iter()
            .map(|x| x.1)
            .filter(|id| *id == font.glyph_id('1').0 || *id == font.glyph_id('3').0)
            .collect();
        assert_eq!(digits, vec![font.glyph_id('1').0, font.glyph_id('3').0]);
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
