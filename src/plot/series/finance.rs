//! Financial charts: candlestick and OHLC.

use super::Rect;
use super::{short, Context, Extent, Hit, IntoValues, LegendEntry, LegendSymbol, Series};
use crate::plot::axes::Axes;
use crate::plot::axis::AxisMap;
use crate::plot::color::Color;
use crate::plot::scene::Stroke;
use std::any::Any;

/// Candlestick or OHLC bars. Created by [`Axes::candlestick`] / [`Axes::ohlc`].
#[derive(Clone, Debug)]
pub struct Candles {
    pub(crate) x: Vec<f64>,
    pub(crate) open: Vec<f64>,
    pub(crate) high: Vec<f64>,
    pub(crate) low: Vec<f64>,
    pub(crate) close: Vec<f64>,
    pub(crate) up: Color,
    pub(crate) down: Color,
    pub(crate) width: f64,
    pub(crate) ohlc: bool,
    pub(crate) label: Option<String>,
}

impl Candles {
    /// Colors for rising and falling periods.
    pub fn colors(&mut self, up: impl Into<Color>, down: impl Into<Color>) -> &mut Self {
        self.up = up.into();
        self.down = down.into();
        self
    }
    /// Body width as a fraction of the spacing between x values (default 0.7).
    pub fn width(&mut self, w: f64) -> &mut Self {
        self.width = w.clamp(0.0, 1.0);
        self
    }
    /// Legend label.
    pub fn label(&mut self, label: impl Into<String>) -> &mut Self {
        self.label = Some(label.into());
        self
    }

    fn len(&self) -> usize {
        self.x
            .len()
            .min(self.open.len())
            .min(self.high.len())
            .min(self.low.len())
            .min(self.close.len())
    }

    fn spacing(&self) -> f64 {
        let mut xs: Vec<f64> = self.x.iter().copied().filter(|v| v.is_finite()).collect();
        xs.sort_by(f64::total_cmp);
        let s = xs
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|d| *d > 0.0)
            .fold(f64::INFINITY, f64::min);
        if s.is_finite() {
            s
        } else {
            1.0
        }
    }
}

impl Series for Candles {
    fn y_range_in(&self, x0: f64, x1: f64) -> Option<Option<(f64, f64)>> {
        Some(super::y_range_where(&self.x, x0, x1, |i| {
            (self.low[i], self.high[i])
        }))
    }
    fn extent(&self) -> Option<Extent> {
        let half = self.spacing() * self.width / 2.0;
        let mut e: Option<Extent> = None;
        for i in 0..self.len() {
            let p = Extent::point(self.x[i] - half, self.low[i])
                .include(self.x[i] + half, self.high[i]);
            if p.x_min.is_finite() && p.y_min.is_finite() {
                e = Some(e.map_or(p, |e| e.union(p)));
            }
        }
        e
    }

    fn draw(&self, ctx: &mut Context) {
        let half = self.spacing() * self.width / 2.0;
        for i in 0..self.len() {
            let (x, o, h, l, c) = (
                self.x[i],
                self.open[i],
                self.high[i],
                self.low[i],
                self.close[i],
            );
            if ![x, o, h, l, c].iter().all(|v| v.is_finite()) {
                continue;
            }
            let color = if c >= o { self.up } else { self.down };
            let s = Stroke::new(color, 1.2);
            if self.ohlc {
                ctx.polyline(&[ctx.point(x, l), ctx.point(x, h)], s.clone());
                ctx.polyline(&[ctx.point(x - half, o), ctx.point(x, o)], s.clone());
                ctx.polyline(&[ctx.point(x, c), ctx.point(x + half, c)], s);
            } else {
                ctx.polyline(&[ctx.point(x, l), ctx.point(x, o.min(c))], s.clone());
                ctx.polyline(&[ctx.point(x, o.max(c)), ctx.point(x, h)], s.clone());
                let (x0, y0) = ctx.point(x - half, o);
                let (x1, y1) = ctx.point(x + half, c);
                let y1 = if (y1 - y0).abs() < 1.0 { y0 + 1.0 } else { y1 };
                ctx.rect(x0, y0, x1, y1, Some(color), Some(s));
            }
        }
    }

    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.up,
                    edge: Some(self.down),
                },
            })
            .collect()
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let n = self.len();
        super::x_window(&self.x[..n], 0.0, x, px, 40.0)
            .unwrap_or(0..n)
            .filter(|i| self.x[*i].is_finite() && self.close[*i].is_finite())
            .map(|i| {
                let (cx, cy) = (x.px(self.x[i]), y.px(self.close[i]));
                Hit {
                    px: cx,
                    py: cy,
                    label: format!(
                        "O {} H {} L {} C {}",
                        short(self.open[i]),
                        short(self.high[i]),
                        short(self.low[i]),
                        short(self.close[i])
                    ),
                    distance: (cx - px).abs() + (cy - py).abs() * 0.01,
                }
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// Candlestick chart (x is usually a time; see [`crate::plot::time`]).
    pub fn candlestick(
        &mut self,
        x: impl IntoValues,
        open: impl IntoValues,
        high: impl IntoValues,
        low: impl IntoValues,
        close: impl IntoValues,
    ) -> &mut Candles {
        self.add(Candles {
            x: x.into_values(),
            open: open.into_values(),
            high: high.into_values(),
            low: low.into_values(),
            close: close.into_values(),
            up: Color::rgb(38, 166, 91),
            down: Color::rgb(220, 53, 69),
            width: 0.7,
            ohlc: false,
            label: None,
        })
    }

    /// OHLC bar chart.
    pub fn ohlc(
        &mut self,
        x: impl IntoValues,
        open: impl IntoValues,
        high: impl IntoValues,
        low: impl IntoValues,
        close: impl IntoValues,
    ) -> &mut Candles {
        let c = self.candlestick(x, open, high, low, close);
        c.ohlc = true;
        c
    }
}

impl Axes {
    /// Heikin-Ashi candles (smoothed candles that show trends more clearly)
    /// computed from OHLC data.
    pub fn heikin_ashi(
        &mut self,
        x: impl IntoValues,
        open: impl IntoValues,
        high: impl IntoValues,
        low: impl IntoValues,
        close: impl IntoValues,
    ) -> &mut Candles {
        let (o, h, l, c) = crate::plot::stats::heikin_ashi(
            &open.into_values(),
            &high.into_values(),
            &low.into_values(),
            &close.into_values(),
        );
        self.candlestick(x, o, h, l, c)
    }

    /// Bollinger bands over `n` periods at `k` standard deviations: a
    /// shaded band with its edges and the moving average in the middle.
    /// Returns the band.
    pub fn bollinger(
        &mut self,
        x: impl IntoValues,
        close: impl IntoValues,
        n: usize,
        k: f64,
    ) -> &mut super::line::FillBetween {
        let x = x.into_values();
        let (mid, up, lo) = crate::plot::stats::bollinger(&close.into_values(), n, k);
        let c = Color::rgb(41, 98, 255);
        self.line(&x, &mid)
            .color(c.with_alpha(0.9))
            .width(1.0)
            .dash(crate::plot::style::Dash::Dashed);
        self.line(&x, &up).color(c.with_alpha(0.7)).width(1.0);
        self.line(&x, &lo).color(c.with_alpha(0.7)).width(1.0);
        let band = self.fill_between(&x, &lo, &up);
        band.color(c).opacity(0.1).label(format!("BB({n}, {k})"));
        band
    }

    /// Relative strength index over `n` periods (0–100) with the 30/70
    /// levels; sets the y range to 0–100. Returns the RSI line.
    pub fn rsi(
        &mut self,
        x: impl IntoValues,
        close: impl IntoValues,
        n: usize,
    ) -> &mut super::line::Line {
        let r = crate::plot::stats::rsi(&close.into_values(), n);
        let band = Color::rgb(126, 87, 194);
        self.hspan(30.0, 70.0).color(band).opacity(0.08);
        self.hline(70.0).color(band.with_alpha(0.6));
        self.hline(30.0).color(band.with_alpha(0.6));
        self.y_limits(0.0, 100.0);
        let l = self.line(x, r);
        l.color(band).width(1.4).label(format!("RSI({n})"));
        l
    }

    /// MACD (`fast`, `slow`, `signal`; usually 12, 26, 9): the MACD and
    /// signal lines and their difference as green/red bars. Returns the
    /// MACD line.
    pub fn macd(
        &mut self,
        x: impl IntoValues,
        close: impl IntoValues,
        fast: usize,
        slow: usize,
        signal: usize,
    ) -> &mut super::line::Line {
        let x = x.into_values();
        let (line, sig, hist) = crate::plot::stats::macd(&close.into_values(), fast, slow, signal);
        let spacing = x
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .filter(|d| *d > 0.0)
            .fold(f64::INFINITY, f64::min);
        let colors: Vec<Color> = hist
            .iter()
            .map(|h| {
                if *h >= 0.0 {
                    Color::rgb(38, 166, 154).with_alpha(0.7)
                } else {
                    Color::rgb(239, 83, 80).with_alpha(0.7)
                }
            })
            .collect();
        let bars = self.bar_xy(
            &x,
            hist.iter()
                .map(|h| if h.is_finite() { *h } else { 0.0 })
                .collect::<Vec<_>>(),
        );
        bars.color(Color::rgb(38, 166, 154))
            .colors(colors)
            .label("histogram");
        if spacing.is_finite() {
            bars.width(spacing * 0.7);
        }
        self.line(&x, &sig)
            .color(Color::rgb(255, 109, 0))
            .width(1.3)
            .label(format!("signal({signal})"));
        let l = self.line(&x, &line);
        l.color(Color::rgb(41, 98, 255))
            .width(1.3)
            .label(format!("MACD({fast}, {slow})"));
        l
    }
}
