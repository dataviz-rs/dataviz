//! Charts for signals and control engineering: FFT spectra, spectrograms,
//! Bode and Nyquist plots, and Smith charts.

use super::grid::Heatmap;
use super::line::Line;
use super::{Context, Extent, IntoValues, Series};
use crate::plot::axes::Axes;
use crate::plot::color::{Color, Colormap};
use crate::plot::figure::Figure;
use crate::plot::scale::Scale;
use crate::plot::scene::Stroke;
use crate::plot::style::{Dash, Marker};
use std::any::Any;
use std::f64::consts::PI;

/// A complex number as (real, imaginary).
pub type Complex = (f64, f64);

/// In-place iterative radix-2 FFT; `re.len()` must be a power of two.
fn fft_in_place(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0, 0.0);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                (cr, ci) = (cr * wr - ci * wi, cr * wi + ci * wr);
            }
        }
        len <<= 1;
    }
}

/// Discrete Fourier transform of a real signal (zero-padded to a power of
/// two).
pub fn fft(signal: &[f64]) -> Vec<Complex> {
    let n = signal.len().max(1).next_power_of_two();
    let mut re = signal.to_vec();
    re.resize(n, 0.0);
    let mut im = vec![0.0; n];
    fft_in_place(&mut re, &mut im);
    re.into_iter().zip(im).collect()
}

fn hann(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / (n.max(2) - 1) as f64).cos())
        .collect()
}

/// One-sided amplitude spectrum of a real signal sampled at
/// `sample_rate` Hz (Hann window): (frequencies in Hz, amplitudes in the
/// signal's units, so a sine of amplitude 1 shows as 1).
pub fn spectrum(signal: &[f64], sample_rate: f64) -> (Vec<f64>, Vec<f64>) {
    let n = signal.len();
    if n < 2 {
        return (Vec::new(), Vec::new());
    }
    let w = hann(n);
    let gain: f64 = w.iter().sum::<f64>() / 2.0;
    let windowed: Vec<f64> = signal.iter().zip(&w).map(|(s, w)| s * w).collect();
    let spec = fft(&windowed);
    let m = spec.len();
    let half = m / 2 + 1;
    let f: Vec<f64> = (0..half)
        .map(|k| k as f64 * sample_rate / m as f64)
        .collect();
    let a: Vec<f64> = spec[..half]
        .iter()
        .map(|(r, i)| r.hypot(*i) / gain)
        .collect();
    (f, a)
}

/// Short-time Fourier transform magnitudes in dB: `z[frequency][frame]`,
/// window `nfft` (a power of two), step `hop` samples; frame times are at
/// window centres.
pub fn spectrogram_db(signal: &[f64], nfft: usize, hop: usize) -> Vec<Vec<f64>> {
    let nfft = nfft.max(8).next_power_of_two();
    let hop = hop.max(1);
    let w = hann(nfft);
    let gain: f64 = w.iter().sum::<f64>() / 2.0;
    let frames = if signal.len() >= nfft {
        (signal.len() - nfft) / hop + 1
    } else {
        0
    };
    let bins = nfft / 2 + 1;
    let mut z = vec![vec![f64::NAN; frames]; bins];
    let (mut re, mut im) = (vec![0.0; nfft], vec![0.0; nfft]);
    for f in 0..frames {
        for (i, (r, x)) in re
            .iter_mut()
            .zip(&signal[f * hop..f * hop + nfft])
            .enumerate()
        {
            *r = x * w[i];
        }
        im.iter_mut().for_each(|v| *v = 0.0);
        fft_in_place(&mut re, &mut im);
        for (k, row) in z.iter_mut().enumerate() {
            let a = re[k].hypot(im[k]) / gain;
            row[f] = 20.0 * a.max(1e-12).log10();
        }
    }
    z
}

/// Frequency response of a transfer function H(s) = num(s) / den(s)
/// (coefficients from the highest power down, like `[1.0, 0.0]` = s) at
/// angular frequencies `w` (rad/s).
pub fn freq_response(num: &[f64], den: &[f64], w: &[f64]) -> Vec<Complex> {
    let eval = |c: &[f64], s: Complex| -> Complex {
        c.iter().fold((0.0, 0.0), |(ar, ai), &k| {
            (ar * s.0 - ai * s.1 + k, ar * s.1 + ai * s.0)
        })
    };
    w.iter()
        .map(|&wv| {
            let s = (0.0, wv);
            let (nr, ni) = eval(num, s);
            let (dr, di) = eval(den, s);
            let d = dr * dr + di * di;
            ((nr * dr + ni * di) / d, (ni * dr - nr * di) / d)
        })
        .collect()
}

/// `n` logarithmically spaced values from `a` to `b` (both > 0).
pub fn logspace(a: f64, b: f64, n: usize) -> Vec<f64> {
    let (la, lb) = (a.ln(), b.ln());
    (0..n.max(2))
        .map(|i| (la + (lb - la) * i as f64 / (n.max(2) - 1) as f64).exp())
        .collect()
}

/// A Smith chart grid (constant resistance circles and reactance arcs).
/// Created by [`Axes::smith_chart`].
#[derive(Clone, Debug)]
pub struct SmithGrid {
    pub(crate) color: Color,
}

/// The reflection coefficient Γ = (z − 1) / (z + 1) of a normalized
/// impedance z.
pub fn gamma(z: Complex) -> Complex {
    let (a, b) = (z.0 - 1.0, z.1);
    let (c, d) = (z.0 + 1.0, z.1);
    let den = c * c + d * d;
    ((a * c + b * d) / den, (b * c - a * d) / den)
}

impl Series for SmithGrid {
    fn extent(&self) -> Option<Extent> {
        let mut e = Extent::point(-1.0, -1.0).include(1.0, 1.0);
        e.sticky_x_min = false;
        Some(e)
    }

    fn draw(&self, ctx: &mut Context) {
        let minor = Stroke::new(self.color.with_alpha(0.35 * self.color.a), 0.8);
        let major = Stroke::new(self.color, 1.2);
        let pts = |f: &dyn Fn(f64) -> Complex, ctx: &Context| -> Vec<(f64, f64)> {
            (0..=240)
                .map(|i| {
                    let t = -PI / 2.0 + PI * (i as f64 + 0.5) / 241.0;
                    let g = gamma(f(t));
                    ctx.point(g.0, g.1)
                })
                .collect()
        };
        // Constant resistance circles (x runs over all reactances).
        for r in [0.2, 0.5, 1.0, 2.0, 5.0] {
            let p = pts(&|t| (r, t.tan() * 4.0), ctx);
            ctx.polyline(&p, minor.clone());
        }
        // Constant reactance arcs (r from 0 to ∞).
        for x in [0.2, 0.5, 1.0, 2.0, 5.0, -0.2, -0.5, -1.0, -2.0, -5.0] {
            let p: Vec<(f64, f64)> = (0..=200)
                .map(|i| {
                    let r = (i as f64 / 200.0 * PI / 2.0 * 0.999).tan() * 3.0;
                    let g = gamma((r, x));
                    ctx.point(g.0, g.1)
                })
                .collect();
            ctx.polyline(&p, minor.clone());
        }
        // Outer circle (r = 0) and the real axis.
        let outer: Vec<(f64, f64)> = (0..=360)
            .map(|i| {
                let a = i as f64 * PI / 180.0;
                ctx.point(a.cos(), a.sin())
            })
            .collect();
        ctx.polyline(&outer, major.clone());
        ctx.polyline(&[ctx.point(-1.0, 0.0), ctx.point(1.0, 0.0)], minor);
        // Resistance labels on the real axis.
        let size = ctx.theme.annotation_size * 0.9;
        let color = ctx.theme.text.with_alpha(0.7);
        for r in [0.2_f64, 0.5, 1.0, 2.0, 5.0] {
            let g = gamma((r, 0.0));
            let (x, y) = ctx.point(g.0, g.1);
            ctx.text(
                x - 2.0,
                y - 3.0,
                crate::plot::series::short(r),
                size,
                color,
                crate::plot::scene::Anchor::End,
                crate::plot::scene::Baseline::Bottom,
            );
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Unwraps phases in degrees (no jumps over ±180°).
fn unwrap_deg(p: &mut [f64]) {
    for i in 1..p.len() {
        let d = p[i] - p[i - 1];
        let k = (d / 360.0).round();
        if k != 0.0 {
            for v in &mut p[i..] {
                *v -= k * 360.0;
            }
        }
    }
}

impl Axes {
    /// Amplitude spectrum of a signal sampled at `sample_rate` Hz (FFT
    /// with a Hann window): frequency \[Hz\] against amplitude.
    pub fn spectrum(&mut self, signal: impl IntoValues, sample_rate: f64) -> &mut Line {
        let (f, a) = spectrum(&signal.into_values(), sample_rate);
        self.x.label.get_or_insert_with(|| "frequency [Hz]".into());
        self.y.label.get_or_insert_with(|| "amplitude".into());
        let l = self.line(f, a);
        l.decimate = true;
        l
    }

    /// Spectrogram of a signal sampled at `sample_rate` Hz: time against
    /// frequency, colored by power in dB (STFT with a Hann window of `nfft`
    /// samples every `hop` samples).
    pub fn spectrogram(
        &mut self,
        signal: impl IntoValues,
        sample_rate: f64,
        nfft: usize,
        hop: usize,
    ) -> &mut Heatmap {
        let s = signal.into_values();
        let nfft = nfft.max(8).next_power_of_two();
        let hop = hop.max(1);
        let z = spectrogram_db(&s, nfft, hop);
        let frames = z.first().map_or(0, |r| r.len()).max(1);
        // Cell edges: frames centred on their window, bins on their frequency.
        let dt = hop as f64 / sample_rate;
        let t0 = nfft as f64 / 2.0 / sample_rate - dt / 2.0;
        let df = sample_rate / nfft as f64;
        let top = z
            .iter()
            .flatten()
            .copied()
            .filter(|v| v.is_finite())
            .fold(f64::NEG_INFINITY, f64::max);
        self.x.label.get_or_insert_with(|| "time [s]".into());
        self.y.label.get_or_insert_with(|| "frequency [Hz]".into());
        self.x.grid = Some(false);
        self.y.grid = Some(false);
        let h = self.field(
            (t0, t0 + frames as f64 * dt),
            (-df / 2.0, sample_rate / 2.0 + df / 2.0),
            z,
        );
        // 80 dB of dynamic range below the peak.
        h.colormap(Colormap::Magma)
            .colorbar("power [dB]")
            .smooth(true);
        if top.is_finite() {
            h.range(top - 80.0, top);
        }
        h
    }

    /// Nyquist plot of a frequency response: Im against Re for positive
    /// frequencies (solid) and their mirror (dashed), with the critical
    /// point −1.
    pub fn nyquist(&mut self, response: &[Complex]) -> &mut Line {
        let color = self.next_color();
        let (re, im): (Vec<f64>, Vec<f64>) = response.iter().copied().unzip();
        self.line(re.clone(), im.iter().map(|v| -v))
            .color(color.with_alpha(0.6))
            .dash(Dash::Dashed)
            .width(1.2);
        self.scatter([-1.0], [0.0])
            .marker(Marker::Plus)
            .size(12.0)
            .color(Color::rgb(214, 39, 40))
            .label("−1");
        self.x.label.get_or_insert_with(|| "Re".into());
        self.y.label.get_or_insert_with(|| "Im".into());
        self.equal_aspect = true;
        let l = self.line(re, im);
        l.color(color).width(2.0);
        l.decimate = false;
        l
    }

    /// A Smith chart (normalized impedance grid); draw impedances on it with
    /// [`Axes::smith`].
    pub fn smith_chart(&mut self) -> &mut SmithGrid {
        self.x.hidden();
        self.y.hidden();
        self.x.grid = Some(false);
        self.y.grid = Some(false);
        self.equal_aspect = true;
        self.frame = false;
        self.x.margin = 0.03;
        self.y.margin = 0.03;
        self.add(SmithGrid {
            color: Color::rgb(110, 120, 130),
        })
    }

    /// Normalized impedances z = Z / Z₀ on a Smith chart (adds the chart
    /// when the plot has none).
    pub fn smith(&mut self, z: &[Complex]) -> &mut Line {
        if !self.series.iter().any(|s| s.kind() == "smith") {
            self.smith_chart();
        }
        let (gx, gy): (Vec<f64>, Vec<f64>) = z.iter().map(|&v| gamma(v)).unzip();
        let l = self.line(gx, gy);
        l.width(2.0);
        l.decimate = false;
        l
    }
}

impl Figure {
    /// Bode plot of a frequency response at angular frequencies `w`
    /// (rad/s): magnitude in dB above, unwrapped phase in degrees below,
    /// log frequency axis shared. Call again to add more systems.
    pub fn bode(&mut self, name: &str, w: &[f64], response: &[Complex]) -> &mut Self {
        let mag: Vec<f64> = response
            .iter()
            .map(|(r, i)| 20.0 * r.hypot(*i).max(1e-300).log10())
            .collect();
        let mut phase: Vec<f64> = response
            .iter()
            .map(|(r, i)| i.atan2(*r).to_degrees())
            .collect();
        unwrap_deg(&mut phase);
        self.grid(2, 1).share_x(true);
        let ax = self.at(0, 0);
        let color = ax.next_color();
        ax.x.scale = Scale::Log(10.0);
        ax.y.label.get_or_insert_with(|| "magnitude [dB]".into());
        ax.x.tick_labels(false);
        let l = ax.line(w, mag);
        l.color(color).width(2.0).label(name);
        l.decimate = false;
        ax.legend(crate::plot::LegendPosition::UpperRight);
        let ax = self.at(1, 0);
        ax.next_color();
        ax.x.scale = Scale::Log(10.0);
        ax.x.label.get_or_insert_with(|| "frequency [rad/s]".into());
        ax.y.label.get_or_insert_with(|| "phase [°]".into());
        // Phase ticks at multiples of 45° (90° over wide ranges).
        let (lo, hi) = phase
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        if lo.is_finite() {
            let step = if hi - lo > 360.0 { 90.0 } else { 45.0 };
            let mut at: Vec<f64> = ((lo / step).floor() as i64..=(hi / step).ceil() as i64)
                .map(|k| k as f64 * step)
                .collect();
            // Keep the ticks of systems drawn before.
            if let crate::plot::TickSpec::At(old) = &ax.y.ticks {
                at.extend(old.iter().copied());
            }
            at.sort_by(f64::total_cmp);
            at.dedup();
            let step = if at.last().unwrap_or(&0.0) - at.first().unwrap_or(&0.0) > 360.0 {
                90.0
            } else {
                45.0
            };
            at.retain(|v| (v / step).round() * step == *v);
            ax.y.ticks(crate::plot::TickSpec::At(at));
        }
        let l = ax.line(w, phase);
        l.color(color).width(2.0);
        l.decimate = false;
        self
    }
}

impl SmithGrid {
    /// Grid color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_finds_a_sine() {
        let fs = 1000.0;
        let s: Vec<f64> = (0..1024)
            .map(|i| 2.0 * (2.0 * PI * 125.0 * i as f64 / fs).sin())
            .collect();
        let (f, a) = spectrum(&s, fs);
        let k = a
            .iter()
            .enumerate()
            .max_by(|x, y| x.1.total_cmp(y.1))
            .unwrap()
            .0;
        assert!((f[k] - 125.0).abs() < 1.0, "{}", f[k]);
        assert!((a[k] - 2.0).abs() < 0.05, "{}", a[k]);
        // Parseval-style check on a delta.
        let d = fft(&[1.0, 0.0, 0.0, 0.0]);
        assert!(d
            .iter()
            .all(|(r, i)| (r - 1.0).abs() < 1e-12 && i.abs() < 1e-12));
    }

    #[test]
    fn transfer_functions_and_smith() {
        // First-order low-pass 1/(s+1): -3 dB and -45° at 1 rad/s.
        let h = freq_response(&[1.0], &[1.0, 1.0], &[1.0]);
        let mag = 20.0 * h[0].0.hypot(h[0].1).log10();
        assert!((mag + 3.0103).abs() < 1e-3);
        assert!((h[0].1.atan2(h[0].0).to_degrees() + 45.0).abs() < 1e-9);
        assert_eq!(gamma((1.0, 0.0)), (0.0, 0.0));
        let g = gamma((0.0, 1.0));
        assert!((g.0.hypot(g.1) - 1.0).abs() < 1e-12);
        let mut p = vec![170.0, -170.0, -150.0];
        unwrap_deg(&mut p);
        assert_eq!(p, vec![170.0, 190.0, 210.0]);
    }

    #[test]
    fn charts_render() {
        let fs = 8000.0;
        let s: Vec<f64> = (0..16000)
            .map(|i| {
                let t = i as f64 / fs;
                (2.0 * PI * (200.0 + 300.0 * t) * t).sin()
            })
            .collect();
        let mut fig = Figure::new(600, 600);
        fig.grid(2, 2);
        fig.at(0, 0).spectrum(&s, fs);
        fig.at(0, 1).spectrogram(&s, fs, 256, 64);
        let w = logspace(0.01, 100.0, 200);
        let h = freq_response(&[1.0], &[1.0, 0.5, 1.0], &w);
        fig.at(1, 0).nyquist(&h);
        fig.at(1, 1).smith(&[(0.5, 0.5), (1.0, 0.0), (2.0, -1.0)]);
        fig.to_pixels().unwrap();
        let mut b = Figure::new(600, 500);
        b.bode("H", &w, &h);
        b.to_pixels().unwrap();
    }
}
