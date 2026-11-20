//! Animations: record a figure frame by frame to GIF, APNG or video.
//!
//! ```no_run
//! use dataviz::prelude::*;
//! use dataviz::plot::series::line::Line;
//! let mut fig = Figure::new(640, 360);
//! fig.axes().line(Vec::<f64>::new(), Vec::<f64>::new()).label("signal");
//! fig.axes().x_limits(0.0, 10.0).y_limits(-1.2, 1.2);
//! fig.record("wave.gif", 30.0, 120, |fig, i| {
//!     let t = i as f64 / 12.0;
//!     fig.axes().series_mut::<Line>(0).unwrap().push(t, t.sin());
//! })
//! .unwrap();
//! ```
//!
//! The format follows the file extension:
//!
//! - `.gif`: 256 colors per frame, exact when a frame has fewer colors
//!   (typical for charts), otherwise a weighted median-cut palette.
//! - `.png` / `.apng`: animated PNG, lossless with transparency.
//! - `.webp`: animated WebP, lossless with transparency, usually smaller
//!   than APNG and GIF (`formats` feature).
//! - `.mp4`, `.mov`, `.mkv` (H.264) and `.webm` (VP9): piped to the
//!   `ffmpeg` program, which must be on `PATH`.
//!
//! Only the rectangle that changed since the previous frame is stored, and
//! a frame equal to the previous one only extends how long that one is
//! shown, so recordings stay small and fast to write.

use super::figure::Figure;
use super::output::{Error, Pixels};
use super::raster::HashMap;
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Records frames into an animation file. See the [module documentation](self).
///
/// Frames are written when the next different frame arrives (its time
/// stamp gives the duration of the previous one); [`Recorder::finish`]
/// writes the last one and completes the file. Dropping the recorder
/// finishes it too, ignoring errors.
pub struct Recorder {
    sink: Sink,
    width: u32,
    height: u32,
    frame_time: Duration,
    fps: f64,
    /// Time stamp for the next [`Recorder::push`].
    clock: Duration,
    /// The frame before `pending` (empty until a frame was written).
    prev: Vec<u32>,
    /// The last pushed frame, written when a different one arrives.
    pending: Vec<u32>,
    pending_at: Duration,
    /// Frames written to the file.
    written: usize,
    /// Time written so far, in the sink's units.
    units: u64,
    pixels: Pixels,
    finished: bool,
}

enum Sink {
    /// Opened with the size of the first frame.
    Closed(Kind, PathBuf),
    Gif(Box<gif::Encoder<BufWriter<File>>>),
    Apng(Apng),
    #[cfg(feature = "formats")]
    WebP(WebP),
    Video(Video),
    Done,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Gif,
    Apng,
    WebP,
    Video,
}

impl Recorder {
    /// Creates a recorder writing `path`, for frames shown `fps` times per
    /// second (see [`Recorder::push`]). The format follows the extension;
    /// the size is taken from the first frame.
    pub fn create(path: impl AsRef<Path>, fps: f64) -> Result<Self, Error> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let kind = match ext.as_str() {
            "gif" => Kind::Gif,
            "png" | "apng" => Kind::Apng,
            "webp" => Kind::WebP,
            "mp4" | "mov" | "mkv" | "webm" => Kind::Video,
            other => return Err(Error::UnsupportedFormat(format!(".{other} (animation)"))),
        };
        let fps = if fps.is_finite() && fps > 0.0 {
            fps.min(1000.0)
        } else {
            30.0
        };
        Ok(Self {
            sink: Sink::Closed(kind, path.into()),
            width: 0,
            height: 0,
            frame_time: Duration::from_secs_f64(1.0 / fps),
            fps,
            clock: Duration::ZERO,
            prev: Vec::new(),
            pending: Vec::new(),
            pending_at: Duration::ZERO,
            written: 0,
            units: 0,
            pixels: Pixels::default(),
            finished: false,
        })
    }

    /// Renders the figure and adds it as the next frame.
    pub fn frame(&mut self, fig: &mut Figure) -> Result<(), Error> {
        let mut px = std::mem::take(&mut self.pixels);
        let r = fig.render_into(&mut px).and_then(|_| self.push(&px));
        self.pixels = px;
        r
    }

    /// Adds a frame, shown `1 / fps` seconds after the previous one pushed
    /// this way.
    pub fn push(&mut self, frame: &Pixels) -> Result<(), Error> {
        let at = self.clock;
        self.clock += self.frame_time;
        self.push_at(frame, at)
    }

    /// Adds a frame shown from `at` (time since the start of the recording)
    /// until the next frame; for recording at a varying rate, e.g. live
    /// data. Time stamps must not decrease.
    pub fn push_at(&mut self, frame: &Pixels, at: Duration) -> Result<(), Error> {
        if self.finished {
            return Err(Error::Render("the recording is finished".into()));
        }
        let n = frame.width as usize * frame.height as usize;
        if frame.rgba.len() < n * 4 || n == 0 {
            return Err(Error::Render("empty or incomplete frame".into()));
        }
        if self.pending.is_empty() {
            self.width = frame.width;
            self.height = frame.height;
        } else if (frame.width, frame.height) != (self.width, self.height) {
            return Err(Error::Render(format!(
                "frame size {}x{} differs from the first frame ({}x{})",
                frame.width, frame.height, self.width, self.height
            )));
        }
        self.clock = self.clock.max(at + self.frame_time);
        let px = frame.rgba[..n * 4].as_chunks::<4>().0;
        if self.pending.is_empty() {
            self.pending
                .extend(px.iter().map(|p| u32::from_ne_bytes(*p)));
            self.pending_at = at;
            return Ok(());
        }
        // An unchanged frame only extends the pending one.
        if px
            .iter()
            .zip(&self.pending)
            .all(|(p, q)| u32::from_ne_bytes(*p) == *q)
        {
            return Ok(());
        }
        self.write_pending(at)?;
        std::mem::swap(&mut self.prev, &mut self.pending);
        self.pending.clear();
        self.pending
            .extend(px.iter().map(|p| u32::from_ne_bytes(*p)));
        self.pending_at = at.max(self.pending_at);
        Ok(())
    }

    /// Frames written so far (equal frames in a row count once).
    pub fn frames(&self) -> usize {
        self.written + usize::from(!self.pending.is_empty() && !self.finished)
    }

    /// Writes the last frame (shown for one frame time) and completes the
    /// file.
    pub fn finish(mut self) -> Result<(), Error> {
        self.close()
    }

    fn close(&mut self) -> Result<(), Error> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        if !self.pending.is_empty() {
            // Repeated frames pushed after it extend the last one.
            let end = self.clock.max(self.pending_at + self.frame_time);
            self.write_pending(end)?;
        }
        match std::mem::replace(&mut self.sink, Sink::Done) {
            Sink::Gif(enc) => {
                let mut w = enc.into_inner().map_err(enc_err)?;
                w.flush()?;
            }
            Sink::Apng(a) => a.finish(self.written as u32)?,
            #[cfg(feature = "formats")]
            Sink::WebP(a) => a.finish()?,
            Sink::Video(v) => v.finish()?,
            Sink::Closed(..) | Sink::Done => {}
        }
        Ok(())
    }

    /// Writes the pending frame, shown until `end`.
    fn write_pending(&mut self, end: Duration) -> Result<(), Error> {
        if let Sink::Closed(kind, path) = &self.sink {
            self.sink = open(*kind, path, self.width, self.height, self.fps)?;
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let rect = if self.prev.is_empty() {
            Some((0, 0, w, h))
        } else {
            changed_rect(&self.prev, &self.pending, w, h)
        };
        let Some(rect) = rect else { return Ok(()) };
        // Durations are rounded on the running total, so they add up.
        let per_unit = match &self.sink {
            Sink::Gif(_) => 0.01,
            Sink::Apng(_) => 0.001,
            #[cfg(feature = "formats")]
            Sink::WebP(_) => 0.001,
            Sink::Video(v) => 1.0 / v.fps,
            _ => 1.0,
        };
        let end_units = (end.as_secs_f64() / per_unit).round() as u64;
        let units = end_units.saturating_sub(self.units);
        match &mut self.sink {
            Sink::Gif(enc) => {
                // Browsers show delays under 2/100 s as 1/10 s.
                let delay = units.max(2).min(u16::MAX as u64);
                let frame = gif_frame(&self.prev, &self.pending, w, rect, delay as u16);
                enc.write_frame(&frame).map_err(enc_err)?;
                self.units += delay;
            }
            Sink::Apng(a) => {
                let delay = units.clamp(1, u16::MAX as u64);
                a.frame(&self.prev, &self.pending, w, rect, delay as u16)?;
                self.units += delay;
            }
            #[cfg(feature = "formats")]
            Sink::WebP(a) => {
                let delay = units.clamp(1, 0xFF_FFFF);
                a.frame(&self.prev, &self.pending, w, h, rect, delay as u32)?;
                self.units += delay;
            }
            Sink::Video(v) => {
                v.frame(&self.pending, units)?;
                self.units += units;
            }
            _ => {}
        }
        self.written += 1;
        Ok(())
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

impl Figure {
    /// Records an animation of `frames` frames at `fps` frames per second
    /// into `path` (`.gif`, `.png` (APNG), `.mp4`, `.webm`, …; see
    /// [`crate::plot::anim`]): `update(fig, i)` changes the figure before
    /// frame `i` is rendered.
    pub fn record(
        &mut self,
        path: impl AsRef<Path>,
        fps: f64,
        frames: usize,
        mut update: impl FnMut(&mut Figure, usize),
    ) -> Result<(), Error> {
        let mut rec = Recorder::create(path, fps)?;
        for i in 0..frames {
            update(self, i);
            rec.frame(self)?;
        }
        rec.finish()
    }
}

fn enc_err(e: impl std::fmt::Display) -> Error {
    Error::Encode(e.to_string())
}

fn open(kind: Kind, path: &Path, w: u32, h: u32, fps: f64) -> Result<Sink, Error> {
    match kind {
        Kind::Gif => {
            if w > u16::MAX as u32 || h > u16::MAX as u32 {
                return Err(Error::Encode(
                    "GIF frames are at most 65535 pixels wide".into(),
                ));
            }
            let file = BufWriter::new(File::create(path)?);
            let mut enc = gif::Encoder::new(file, w as u16, h as u16, &[]).map_err(enc_err)?;
            enc.set_repeat(gif::Repeat::Infinite).map_err(enc_err)?;
            Ok(Sink::Gif(Box::new(enc)))
        }
        Kind::Apng => Ok(Sink::Apng(Apng::create(path, w, h)?)),
        #[cfg(feature = "formats")]
        Kind::WebP => Ok(Sink::WebP(WebP::create(path, w, h)?)),
        #[cfg(not(feature = "formats"))]
        Kind::WebP => Err(Error::UnsupportedFormat(
            ".webp animations need the `formats` cargo feature".into(),
        )),
        Kind::Video => Ok(Sink::Video(Video::spawn(path, w, h, fps)?)),
    }
}

/// Bounding box (x, y, w, h) of the pixels that differ.
fn changed_rect(a: &[u32], b: &[u32], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let rows = |y: usize| (&a[y * w..(y + 1) * w], &b[y * w..(y + 1) * w]);
    let y0 = (0..h).find(|&y| {
        let (p, q) = rows(y);
        p != q
    })?;
    let y1 = (y0..h)
        .rev()
        .find(|&y| {
            let (p, q) = rows(y);
            p != q
        })
        .unwrap_or(y0);
    let (mut x0, mut x1) = (w, 0);
    for y in y0..=y1 {
        let (p, q) = rows(y);
        if p == q {
            continue;
        }
        if let Some(i) = (0..x0.min(w)).find(|&i| p[i] != q[i]) {
            x0 = i;
        }
        if let Some(i) = (x1..w).rev().find(|&i| p[i] != q[i]) {
            x1 = i;
        }
    }
    let x0 = x0.min(x1);
    Some((x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

/// A pixel (RGBA bytes in a u32) composited on white, as RGB bytes.
#[inline]
fn on_white(p: u32) -> [u8; 3] {
    let [r, g, b, a] = p.to_ne_bytes();
    if a == 255 {
        return [r, g, b];
    }
    let a = a as u32;
    let c = |v: u8| ((v as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
    [c(r), c(g), c(b)]
}

#[inline]
fn rgb_key(c: [u8; 3]) -> u32 {
    (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32
}

/// One GIF frame for `rect` of `cur`; pixels equal to `prev` (when given)
/// are transparent.
fn gif_frame<'a>(
    prev: &[u32],
    cur: &[u32],
    w: usize,
    rect: (usize, usize, usize, usize),
    delay: u16,
) -> gif::Frame<'a> {
    let (rx, ry, rw, rh) = rect;
    let delta = !prev.is_empty();
    const SKIP: u32 = u32::MAX;
    // Colors of the rectangle row by row (SKIP = unchanged pixel); read
    // twice (count, then index) instead of being stored.
    let each_row = |f: &mut dyn FnMut(&mut dyn Iterator<Item = u32>)| {
        for y in ry..ry + rh {
            let row = y * w + rx..y * w + rx + rw;
            if delta {
                f(&mut cur[row.clone()].iter().zip(&prev[row]).map(|(c, p)| {
                    if c == p {
                        SKIP
                    } else {
                        rgb_key(on_white(*c))
                    }
                }));
            } else {
                f(&mut cur[row].iter().map(|c| rgb_key(on_white(*c))));
            }
        }
    };
    let mut counts: HashMap<u32, u32> = HashMap::default();
    let mut skipped = false;
    each_row(&mut |row| {
        let mut last = (SKIP, 0u32);
        for k in row {
            if k == last.0 {
                last.1 += 1;
                continue;
            }
            if last.0 != SKIP {
                *counts.entry(last.0).or_default() += last.1;
            }
            skipped |= k == SKIP;
            last = (k, 1);
        }
        if last.0 != SKIP {
            *counts.entry(last.0).or_default() += last.1;
        }
    });
    let transparent = delta && skipped;
    let max_colors = if transparent { 255 } else { 256 };
    let mut colors: Vec<(u32, u32)> = counts.into_iter().collect();
    let (palette, index) = if colors.len() <= max_colors {
        colors.sort_unstable();
        let index: HashMap<u32, u8> = colors
            .iter()
            .enumerate()
            .map(|(i, c)| (c.0, i as u8))
            .collect();
        let palette: Vec<[u8; 3]> = colors
            .iter()
            .map(|c| [(c.0 >> 16) as u8, (c.0 >> 8) as u8, c.0 as u8])
            .collect();
        (palette, index)
    } else {
        median_cut(&mut colors, max_colors)
    };
    let t = palette.len() as u8;
    let mut buf = Vec::with_capacity(rw * rh);
    each_row(&mut |row| {
        let mut last = (SKIP, t);
        for k in row {
            if k != last.0 {
                last = (k, if k == SKIP { t } else { index[&k] });
            }
            buf.push(last.1);
        }
    });
    let mut pal: Vec<u8> = palette.iter().flatten().copied().collect();
    if transparent {
        pal.extend([0, 0, 0]);
    }
    // The palette size must be a power of two (2..=256 entries).
    let entries = (pal.len() / 3).max(2).next_power_of_two();
    pal.resize(entries * 3, 0);
    gif::Frame {
        delay,
        dispose: gif::DisposalMethod::Keep,
        transparent: transparent.then_some(t),
        left: rx as u16,
        top: ry as u16,
        width: rw as u16,
        height: rh as u16,
        palette: Some(pal),
        buffer: Cow::Owned(buf),
        ..Default::default()
    }
}

/// Weighted median cut of `colors` ((0xRRGGBB, count)) into at most `k`
/// colors: palette and the palette index of every color.
fn median_cut(colors: &mut [(u32, u32)], k: usize) -> (Vec<[u8; 3]>, HashMap<u32, u8>) {
    let ch = |c: u32, i: usize| ((c >> (16 - 8 * i)) & 255) as u8;
    // Boxes are ranges of `colors`; (start, end, widest channel, score).
    let measure = |s: &[(u32, u32)]| -> (usize, u64) {
        let mut lo = [255u8; 3];
        let mut hi = [0u8; 3];
        let mut n = 0u64;
        for &(c, cnt) in s {
            for i in 0..3 {
                lo[i] = lo[i].min(ch(c, i));
                hi[i] = hi[i].max(ch(c, i));
            }
            n += cnt as u64;
        }
        // Green differences are the most visible, blue the least.
        let range = [
            (hi[0] - lo[0]) as u64 * 3,
            (hi[1] - lo[1]) as u64 * 4,
            (hi[2] - lo[2]) as u64 * 2,
        ];
        let axis = (0..3).max_by_key(|&i| range[i]).unwrap_or(0);
        // Wide boxes with many pixels are split first.
        (axis, range[axis] * range[axis] * (n as f64).sqrt() as u64)
    };
    let (axis, score) = measure(colors);
    let mut boxes = vec![(0usize, colors.len(), axis, score)];
    while boxes.len() < k {
        let Some((bi, _)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.1 - b.0 > 1 && b.3 > 0)
            .max_by_key(|(_, b)| b.3)
        else {
            break;
        };
        let (s, e, axis, _) = boxes[bi];
        let part = &mut colors[s..e];
        part.sort_unstable_by_key(|c| ch(c.0, axis));
        let total: u64 = part.iter().map(|c| c.1 as u64).sum();
        let mut acc = 0u64;
        let mut m = part.len() / 2;
        for (i, c) in part.iter().enumerate() {
            acc += c.1 as u64;
            if acc * 2 >= total {
                m = i + 1;
                break;
            }
        }
        let m = m.clamp(1, part.len() - 1);
        let (a, sa) = measure(&part[..m]);
        let (b, sb) = measure(&part[m..]);
        boxes[bi] = (s, s + m, a, sa);
        boxes.push((s + m, e, b, sb));
    }
    let mut palette = Vec::with_capacity(boxes.len());
    let mut index: HashMap<u32, u8> = HashMap::default();
    for (i, &(s, e, _, _)) in boxes.iter().enumerate() {
        let mut sum = [0u64; 3];
        let mut n = 0u64;
        for &(c, cnt) in &colors[s..e] {
            for (j, v) in sum.iter_mut().enumerate() {
                *v += ch(c, j) as u64 * cnt as u64;
            }
            n += cnt as u64;
            index.insert(c, i as u8);
        }
        let n = n.max(1);
        palette.push([
            ((sum[0] + n / 2) / n) as u8,
            ((sum[1] + n / 2) / n) as u8,
            ((sum[2] + n / 2) / n) as u8,
        ]);
    }
    (palette, index)
}

/// Animated WebP writer (RIFF `VP8X` + `ANIM` + one `ANMF` per frame):
/// each frame is the changed rectangle (on even coordinates, as `ANMF`
/// requires) encoded as lossless `VP8L`, replacing what was there.
#[cfg(feature = "formats")]
struct WebP {
    out: BufWriter<File>,
    rgba: Vec<u8>,
    bytes: u64,
}

#[cfg(feature = "formats")]
impl WebP {
    fn create(path: &Path, w: u32, h: u32) -> Result<Self, Error> {
        if w > 1 << 24 || h > 1 << 24 {
            return Err(Error::Encode(
                "WebP frames are at most 16 777 216 pixels wide".into(),
            ));
        }
        let mut out = BufWriter::new(File::create(path)?);
        // RIFF size is written by `finish`.
        out.write_all(b"RIFF\0\0\0\0WEBP")?;
        let mut vp8x = vec![0x10 | 0x02, 0, 0, 0];
        vp8x.extend(&(w - 1).to_le_bytes()[..3]);
        vp8x.extend(&(h - 1).to_le_bytes()[..3]);
        riff_chunk(&mut out, b"VP8X", &vp8x)?;
        // Transparent background, loop forever.
        riff_chunk(&mut out, b"ANIM", &[0, 0, 0, 0, 0, 0])?;
        Ok(Self {
            out,
            rgba: Vec::new(),
            bytes: 4 + 18 + 14,
        })
    }

    fn frame(
        &mut self,
        prev: &[u32],
        cur: &[u32],
        w: usize,
        h: usize,
        rect: (usize, usize, usize, usize),
        delay_ms: u32,
    ) -> Result<(), Error> {
        // ANMF offsets are stored halved: start on even pixels.
        let (mut rx, mut ry, mut rw, mut rh) = rect;
        if rx % 2 == 1 {
            rx -= 1;
            rw += 1;
        }
        if ry % 2 == 1 {
            ry -= 1;
            rh += 1;
        }
        let (rw, rh) = (rw.min(w - rx), rh.min(h - ry));
        // The rectangle's own pixels, not blended: decoders blend with
        // rounding (an opaque pixel can come out one level off).
        let _ = prev;
        self.rgba.clear();
        for y in ry..ry + rh {
            for &c in &cur[y * w + rx..y * w + rx + rw] {
                self.rgba.extend(c.to_ne_bytes());
            }
        }
        let mut file = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut file)
            .encode(
                &self.rgba,
                rw as u32,
                rh as u32,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(enc_err)?;
        // The VP8L chunk of the single-image file.
        let mut at = 12;
        let mut vp8l = None;
        while at + 8 <= file.len() {
            let size = u32::from_le_bytes([file[at + 4], file[at + 5], file[at + 6], file[at + 7]])
                as usize;
            let end = (at + 8 + size).min(file.len());
            if &file[at..at + 4] == b"VP8L" {
                vp8l = Some(&file[at..end]);
                break;
            }
            at = end + (size & 1);
        }
        let vp8l = vp8l.ok_or_else(|| Error::Encode("WebP: no VP8L data".into()))?;
        let mut anmf = Vec::with_capacity(16 + vp8l.len() + 1);
        anmf.extend(&((rx / 2) as u32).to_le_bytes()[..3]);
        anmf.extend(&((ry / 2) as u32).to_le_bytes()[..3]);
        anmf.extend(&((rw - 1) as u32).to_le_bytes()[..3]);
        anmf.extend(&((rh - 1) as u32).to_le_bytes()[..3]);
        anmf.extend(&delay_ms.to_le_bytes()[..3]);
        // Blending bit set = do not blend; no disposal.
        anmf.push(0x02);
        anmf.extend(vp8l);
        if vp8l.len() % 2 == 1 {
            anmf.push(0);
        }
        riff_chunk(&mut self.out, b"ANMF", &anmf)?;
        self.bytes += 8 + anmf.len().next_multiple_of(2) as u64;
        Ok(())
    }

    fn finish(mut self) -> Result<(), Error> {
        let size = u32::try_from(self.bytes)
            .map_err(|_| Error::Encode("WebP animations are at most 4 GB".into()))?;
        self.out.seek(SeekFrom::Start(4))?;
        self.out.write_all(&size.to_le_bytes())?;
        self.out.flush()?;
        Ok(())
    }
}

#[cfg(feature = "formats")]
fn riff_chunk(out: &mut impl Write, kind: &[u8; 4], data: &[u8]) -> Result<(), Error> {
    out.write_all(kind)?;
    out.write_all(&(data.len() as u32).to_le_bytes())?;
    out.write_all(data)?;
    if data.len() % 2 == 1 {
        out.write_all(&[0])?;
    }
    Ok(())
}

/// Animated PNG writer: frames are encoded as standalone PNGs and their
/// image data is moved into `fdAT` chunks; the frame count in `acTL` is
/// filled in by [`Apng::finish`].
struct Apng {
    out: BufWriter<File>,
    /// File offset of the `acTL` chunk.
    actl: u64,
    seq: u32,
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

impl Apng {
    fn create(path: &Path, w: u32, h: u32) -> Result<Self, Error> {
        let mut out = BufWriter::new(File::create(path)?);
        out.write_all(b"\x89PNG\r\n\x1a\n")?;
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend(w.to_be_bytes());
        ihdr.extend(h.to_be_bytes());
        ihdr.extend([8, 6, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr)?;
        let actl = 8 + 12 + 13;
        chunk(&mut out, b"acTL", &[0, 0, 0, 0, 0, 0, 0, 0])?;
        Ok(Self {
            out,
            actl,
            seq: 0,
            w,
            h,
            rgba: Vec::new(),
        })
    }

    fn frame(
        &mut self,
        prev: &[u32],
        cur: &[u32],
        w: usize,
        rect: (usize, usize, usize, usize),
        delay_ms: u16,
    ) -> Result<(), Error> {
        let (rx, ry, rw, rh) = rect;
        // Over-blending lets unchanged pixels be transparent (smaller
        // files); it needs the changed pixels to be opaque.
        let mut over = !prev.is_empty();
        self.rgba.clear();
        for y in ry..ry + rh {
            let row = y * w + rx..y * w + rx + rw;
            for (i, &c) in cur[row.clone()].iter().enumerate() {
                let bytes = c.to_ne_bytes();
                if over && bytes[3] != 255 && prev[row.start + i] != c {
                    over = false;
                }
                self.rgba.extend(bytes);
            }
        }
        if over {
            for y in 0..rh {
                let base = (ry + y) * w + rx;
                for x in 0..rw {
                    if prev[base + x] == cur[base + x] {
                        let at = (y * rw + x) * 4;
                        self.rgba[at..at + 4].fill(0);
                    }
                }
            }
        }
        let mut fctl = Vec::with_capacity(26);
        fctl.extend(self.seq.to_be_bytes());
        fctl.extend((rw as u32).to_be_bytes());
        fctl.extend((rh as u32).to_be_bytes());
        fctl.extend((rx as u32).to_be_bytes());
        fctl.extend((ry as u32).to_be_bytes());
        fctl.extend(delay_ms.to_be_bytes());
        fctl.extend(1000u16.to_be_bytes());
        fctl.extend([0, u8::from(over)]);
        chunk(&mut self.out, b"fcTL", &fctl)?;
        self.seq += 1;
        let data = png_idat(&self.rgba, rw as u32, rh as u32)?;
        if self.seq == 1 {
            chunk(&mut self.out, b"IDAT", &data)?;
        } else {
            let mut fdat = Vec::with_capacity(data.len() + 4);
            fdat.extend(self.seq.to_be_bytes());
            fdat.extend(&data);
            chunk(&mut self.out, b"fdAT", &fdat)?;
            self.seq += 1;
        }
        Ok(())
    }

    fn finish(mut self, frames: u32) -> Result<(), Error> {
        if self.seq == 0 {
            // No frame: a valid one-pixel transparent image.
            let (w, h) = (self.w as usize, self.h as usize);
            let empty = vec![0u32; w * h];
            self.frame(&[], &empty, w, (0, 0, w, h), 1)?;
        }
        chunk(&mut self.out, b"IEND", &[])?;
        let mut actl = Vec::with_capacity(8);
        actl.extend(frames.max(1).to_be_bytes());
        actl.extend(0u32.to_be_bytes());
        self.out.seek(SeekFrom::Start(self.actl))?;
        chunk(&mut self.out, b"acTL", &actl)?;
        self.out.flush()?;
        Ok(())
    }
}

/// zlib image data of an RGBA image (the concatenated `IDAT` payloads of a
/// PNG encoding of it).
fn png_idat(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, Error> {
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        enc.set_filter(png::Filter::Up);
        let mut wr = enc.write_header().map_err(enc_err)?;
        wr.write_image_data(rgba).map_err(enc_err)?;
        wr.finish().map_err(enc_err)?;
    }
    let mut data = Vec::new();
    let mut at = 8;
    while at + 12 <= png.len() {
        let len = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]) as usize;
        let kind = &png[at + 4..at + 8];
        if kind == b"IDAT" {
            data.extend_from_slice(&png[at + 8..at + 8 + len]);
        }
        at += 12 + len;
    }
    Ok(data)
}

fn chunk(out: &mut impl Write, kind: &[u8; 4], data: &[u8]) -> Result<(), Error> {
    out.write_all(&(data.len() as u32).to_be_bytes())?;
    out.write_all(kind)?;
    out.write_all(data)?;
    let crc = crc32(crc32(!0, kind), data);
    out.write_all(&(!crc).to_be_bytes())?;
    Ok(())
}

fn crc32(mut crc: u32, data: &[u8]) -> u32 {
    static TABLE: std::sync::LazyLock<[u32; 256]> = std::sync::LazyLock::new(|| {
        let mut t = [0u32; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
            *v = c;
        }
        t
    });
    for &b in data {
        crc = TABLE[((crc ^ b as u32) & 255) as usize] ^ (crc >> 8);
    }
    crc
}

/// Video through an `ffmpeg` process reading raw RGB frames.
struct Video {
    child: Child,
    fps: f64,
    rgb: Vec<u8>,
}

impl Video {
    fn spawn(path: &Path, w: u32, h: u32, fps: f64) -> Result<Self, Error> {
        let webm = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("webm"));
        let codec: &[&str] = if webm {
            &[
                "-c:v",
                "libvpx-vp9",
                "-crf",
                "30",
                "-b:v",
                "0",
                "-row-mt",
                "1",
            ]
        } else {
            &[
                "-c:v",
                "libx264",
                "-crf",
                "18",
                "-preset",
                "fast",
                "-movflags",
                "+faststart",
            ]
        };
        let child = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo"])
            .args(["-pix_fmt", "rgb24", "-s", &format!("{w}x{h}")])
            .args(["-r", &format!("{fps}"), "-i", "-"])
            // yuv420p (playable everywhere) needs even sizes.
            .args([
                "-vf",
                "pad=ceil(iw/2)*2:ceil(ih/2)*2:color=white",
                "-pix_fmt",
                "yuv420p",
            ])
            .args(codec)
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| {
                Error::Encode(if e.kind() == std::io::ErrorKind::NotFound {
                    "video output needs the ffmpeg program on PATH \
                     (or record .gif / .png instead)"
                        .to_string()
                } else {
                    format!("cannot start ffmpeg: {e}")
                })
            })?;
        Ok(Self {
            child,
            fps,
            rgb: Vec::new(),
        })
    }

    /// Writes `frame` `repeat` times (a constant frame rate).
    fn frame(&mut self, frame: &[u32], repeat: u64) -> Result<(), Error> {
        if repeat == 0 {
            return Ok(());
        }
        self.rgb.clear();
        self.rgb.extend(frame.iter().flat_map(|&p| on_white(p)));
        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| Error::Encode("ffmpeg closed its input".into()))?;
        for _ in 0..repeat {
            stdin.write_all(&self.rgb)?;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<(), Error> {
        drop(self.child.stdin.take());
        let status = self.child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::Encode(format!("ffmpeg failed ({status})")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::series::line::Line;

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("dataviz-anim-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn frames(n: usize) -> Vec<Pixels> {
        let mut fig = Figure::new(160, 100);
        fig.axes().line(Vec::<f64>::new(), Vec::<f64>::new());
        fig.axes().x_limits(0.0, 10.0).y_limits(-1.2, 1.2);
        (0..n)
            .map(|i| {
                let t = i as f64 * 0.5;
                fig.axes().series_mut::<Line>(0).unwrap().push(t, t.sin());
                fig.to_pixels().unwrap()
            })
            .collect()
    }

    #[test]
    fn changed_rect_bounds_the_difference() {
        let a = vec![0u32; 20];
        let mut b = a.clone();
        b[5 + 2] = 1;
        b[2 * 5 + 4] = 1;
        assert_eq!(changed_rect(&a, &b, 5, 4), Some((2, 1, 3, 2)));
        assert_eq!(changed_rect(&a, &a, 5, 4), None);
    }

    #[test]
    fn gif_round_trip() {
        use image::AnimationDecoder;
        let src = frames(8);
        let path = dir().join("a.gif");
        let mut rec = Recorder::create(&path, 20.0).unwrap();
        for f in &src {
            rec.push(f).unwrap();
        }
        // A repeated frame extends the last one.
        rec.push(&src[7]).unwrap();
        assert_eq!(rec.frames(), 8);
        rec.finish().unwrap();
        let dec = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
            File::open(&path).unwrap(),
        ))
        .unwrap();
        let out: Vec<_> = dec.into_frames().collect::<Result<_, _>>().unwrap();
        assert_eq!(out.len(), 8);
        let (n, d) = out[7].delay().numer_denom_ms();
        assert_eq!(n / d, 100);
        for (a, b) in src.iter().zip(&out) {
            let b = b.buffer();
            let mut worst = 0i32;
            for (p, q) in a.rgba.as_chunks::<4>().0.iter().zip(b.pixels()) {
                let p = on_white(u32::from_ne_bytes(*p));
                for c in 0..3 {
                    worst = worst.max((p[c] as i32 - q.0[c] as i32).abs());
                }
            }
            // Median cut keeps every color close.
            assert!(worst <= 24, "max channel error {worst}");
        }
    }

    #[test]
    fn apng_round_trip_is_lossless() {
        use image::AnimationDecoder;
        let src = frames(6);
        let path = dir().join("a.png");
        let mut rec = Recorder::create(&path, 10.0).unwrap();
        for f in &src {
            rec.push(f).unwrap();
        }
        rec.finish().unwrap();
        let dec = image::codecs::png::PngDecoder::new(std::io::BufReader::new(
            File::open(&path).unwrap(),
        ))
        .unwrap();
        assert!(dec.is_apng().unwrap());
        let out: Vec<_> = dec
            .apng()
            .unwrap()
            .into_frames()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(out.len(), 6);
        for (a, b) in src.iter().zip(&out) {
            assert_eq!(&a.rgba, b.buffer().as_raw());
        }
    }

    #[cfg(feature = "formats")]
    #[test]
    fn webp_round_trip_is_lossless() {
        use image::AnimationDecoder;
        let src = frames(6);
        let path = dir().join("a.webp");
        let mut rec = Recorder::create(&path, 10.0).unwrap();
        for f in &src {
            rec.push(f).unwrap();
        }
        rec.finish().unwrap();
        let dec = image::codecs::webp::WebPDecoder::new(std::io::BufReader::new(
            File::open(&path).unwrap(),
        ))
        .unwrap();
        assert!(dec.has_animation());
        let out: Vec<_> = dec.into_frames().collect::<Result<_, _>>().unwrap();
        assert_eq!(out.len(), 6);
        for (a, b) in src.iter().zip(&out) {
            assert_eq!(&a.rgba, b.buffer().as_raw());
        }
        assert_eq!(out[1].delay().numer_denom_ms(), (100, 1));
    }

    #[test]
    fn median_cut_limits_colors() {
        let mut colors: Vec<(u32, u32)> = (0..5000u32)
            .map(|i| (i * 3331 % 0xFFFFFF, 1 + i % 7))
            .collect();
        let (pal, index) = median_cut(&mut colors, 255);
        assert!(pal.len() <= 255);
        assert_eq!(index.len(), 5000);
    }

    #[test]
    fn unknown_extension_is_refused() {
        assert!(Recorder::create(dir().join("a.xyz"), 30.0).is_err());
    }
}
