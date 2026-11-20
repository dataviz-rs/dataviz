//! Terminal output: figures as Unicode half blocks (24-bit color) or braille
//! dots, for quick looks over SSH or in CI logs.

use super::figure::Figure;
use super::output::{Error, Pixels};

/// How a figure is drawn with characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TerminalMode {
    /// `▀` with 24-bit foreground/background colors: 1 × 2 pixels per cell.
    #[default]
    HalfBlock,
    /// Braille patterns: 2 × 4 dots per cell, colored by the dominant ink.
    Braille,
    /// Braille without colors (plain text, e.g. for logs).
    BrailleMono,
}

fn area_average(p: &Pixels, x0: f64, y0: f64, x1: f64, y1: f64) -> [f64; 3] {
    let (xa, xb) = (
        x0.floor() as u32,
        (x1.ceil() as u32).min(p.width).max(x0.floor() as u32 + 1),
    );
    let (ya, yb) = (
        y0.floor() as u32,
        (y1.ceil() as u32).min(p.height).max(y0.floor() as u32 + 1),
    );
    let mut acc = [0.0; 3];
    let mut n = 0.0;
    for y in ya..yb {
        for x in xa..xb {
            if let Some(px) = p.get(x, y) {
                let a = px[3] as f64 / 255.0;
                for c in 0..3 {
                    acc[c] += px[c] as f64 * a + 255.0 * (1.0 - a);
                }
                n += 1.0;
            }
        }
    }
    if n > 0.0 {
        acc.map(|v| v / n)
    } else {
        [255.0; 3]
    }
}

/// The color of an area for half blocks: thin lines and text keep their
/// own color (the pixel that differs most from the background `bg`, when
/// it differs clearly) instead of fading into the background as an
/// average would.
fn area_ink(p: &Pixels, x0: f64, y0: f64, x1: f64, y1: f64, bg: [f64; 3]) -> [f64; 3] {
    let avg = area_average(p, x0, y0, x1, y1);
    let (xa, xb) = (
        x0.floor() as u32,
        (x1.ceil() as u32).min(p.width).max(x0.floor() as u32 + 1),
    );
    let (ya, yb) = (
        y0.floor() as u32,
        (y1.ceil() as u32).min(p.height).max(y0.floor() as u32 + 1),
    );
    let mut best = (0.0, avg);
    for y in ya..yb {
        for x in xa..xb {
            if let Some(px) = p.get(x, y) {
                let a = px[3] as f64 / 255.0;
                let c = [0, 1, 2].map(|k| px[k] as f64 * a + 255.0 * (1.0 - a));
                let d = (0..3).map(|k| (c[k] - bg[k]).abs()).sum::<f64>();
                if d > best.0 {
                    best = (d, c);
                }
            }
        }
    }
    if best.0 > 90.0 {
        [0, 1, 2].map(|k| best.1[k] * 0.85 + avg[k] * 0.15)
    } else {
        avg
    }
}

/// Renders pixels into `cols` × `rows` character cells.
pub fn render_pixels(p: &Pixels, cols: usize, rows: usize, mode: TerminalMode) -> String {
    let (cols, rows) = (cols.max(1), rows.max(1));
    let mut out = String::new();
    match mode {
        TerminalMode::HalfBlock => {
            let (cw, ch) = (
                p.width as f64 / cols as f64,
                p.height as f64 / (rows * 2) as f64,
            );
            let bg = area_average(p, 0.0, 0.0, 2.0, 2.0);
            for r in 0..rows {
                for c in 0..cols {
                    let x0 = c as f64 * cw;
                    let top = area_ink(
                        p,
                        x0,
                        (2 * r) as f64 * ch,
                        x0 + cw,
                        (2 * r + 1) as f64 * ch,
                        bg,
                    );
                    let bot = area_ink(
                        p,
                        x0,
                        (2 * r + 1) as f64 * ch,
                        x0 + cw,
                        (2 * r + 2) as f64 * ch,
                        bg,
                    );
                    out.push_str(&format!(
                        "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
                        top[0] as u8,
                        top[1] as u8,
                        top[2] as u8,
                        bot[0] as u8,
                        bot[1] as u8,
                        bot[2] as u8
                    ));
                }
                out.push_str("\x1b[0m\n");
            }
        }
        TerminalMode::Braille | TerminalMode::BrailleMono => {
            let (cw, ch) = (
                p.width as f64 / (cols * 2) as f64,
                p.height as f64 / (rows * 4) as f64,
            );
            // Background = the most common light color: assume the corner.
            let bg = area_average(p, 0.0, 0.0, 2.0, 2.0);
            const BITS: [[u32; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];
            for r in 0..rows {
                for c in 0..cols {
                    let mut code = 0u32;
                    let mut ink = [0.0; 3];
                    let mut n = 0.0;
                    for (dy, row) in BITS.iter().enumerate() {
                        for (dx, bit) in row.iter().enumerate() {
                            let x0 = (c * 2 + dx) as f64 * cw;
                            let y0 = (r * 4 + dy) as f64 * ch;
                            let v = area_average(p, x0, y0, x0 + cw, y0 + ch);
                            let d = (0..3).map(|k| (v[k] - bg[k]).abs()).sum::<f64>();
                            if d > 60.0 {
                                code |= bit;
                                for k in 0..3 {
                                    ink[k] += v[k];
                                }
                                n += 1.0;
                            }
                        }
                    }
                    let ch = char::from_u32(0x2800 + code).unwrap_or(' ');
                    if mode == TerminalMode::Braille && n > 0.0 {
                        out.push_str(&format!(
                            "\x1b[38;2;{};{};{}m{ch}",
                            (ink[0] / n) as u8,
                            (ink[1] / n) as u8,
                            (ink[2] / n) as u8
                        ));
                    } else {
                        out.push(ch);
                    }
                }
                if mode == TerminalMode::Braille {
                    out.push_str("\x1b[0m");
                }
                out.push('\n');
            }
        }
    }
    out
}

impl Figure {
    /// The figure as text for a terminal of `cols` × `rows` characters.
    ///
    /// ```
    /// use dataviz::prelude::*;
    /// use dataviz::plot::terminal::TerminalMode;
    /// let mut fig = Figure::new(400, 200);
    /// fig.axes().line([0, 1, 2], [0.0, 1.0, 0.5]);
    /// let text = fig.to_terminal(60, 15, TerminalMode::BrailleMono).unwrap();
    /// assert_eq!(text.lines().count(), 15);
    /// ```
    pub fn to_terminal(
        &mut self,
        cols: usize,
        rows: usize,
        mode: TerminalMode,
    ) -> Result<String, Error> {
        let px = self.to_pixels()?;
        Ok(render_pixels(&px, cols, rows, mode))
    }

    /// Prints the figure to standard output, sized to the terminal (`COLUMNS`
    /// / `LINES` environment variables, default 100 × 30).
    pub fn print(&mut self) -> Result<(), Error> {
        let env = |k: &str, d: usize| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(d)
        };
        let cols = env("COLUMNS", 100);
        let rows = env("LINES", 30).saturating_sub(2).max(5);
        print!("{}", self.to_terminal(cols, rows, TerminalMode::HalfBlock)?);
        Ok(())
    }
}
