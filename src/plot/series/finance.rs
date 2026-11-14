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

    fn data_points(&self, max: usize) -> Vec<(f64, f64)> {
        super::spread_points(&self.x, &self.close, max)
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

/// Rising / falling colors of the trading charts.
const UP: Color = Color::rgb(38, 166, 154);
const DOWN: Color = Color::rgb(239, 83, 80);

impl Axes {
    /// Volume profile: traded volume per price level as horizontal bars
    /// (price on y), the point of control (largest volume) highlighted and
    /// the value area (70 % of the volume around it) darker. Put it in a
    /// narrow plot next to the price chart with shared y.
    pub fn volume_profile(
        &mut self,
        price: impl IntoValues,
        volume: impl IntoValues,
        bins: usize,
    ) -> &mut super::bar::Bars {
        let (p, v) = (price.into_values(), volume.into_values());
        let (lo, hi) = p
            .iter()
            .filter(|x| x.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                (a.min(*x), b.max(*x))
            });
        let bins = bins.max(1);
        let mut vol = vec![0.0; bins];
        if lo.is_finite() && hi > lo {
            for (pi, vi) in p.iter().zip(&v) {
                if pi.is_finite() && vi.is_finite() {
                    let k = (((pi - lo) / (hi - lo)) * bins as f64)
                        .floor()
                        .clamp(0.0, bins as f64 - 1.0) as usize;
                    vol[k] += vi;
                }
            }
        }
        let step = if hi > lo {
            (hi - lo) / bins as f64
        } else {
            1.0
        };
        let centres: Vec<f64> = (0..bins).map(|k| lo + step * (k as f64 + 0.5)).collect();
        // Value area: grow from the point of control towards the larger side.
        let poc = (0..bins)
            .max_by(|a, b| vol[*a].total_cmp(&vol[*b]))
            .unwrap_or(0);
        let total: f64 = vol.iter().sum();
        let (mut a, mut b, mut acc) = (poc, poc, vol[poc]);
        while acc < 0.7 * total && (a > 0 || b + 1 < bins) {
            let down = if a > 0 { vol[a - 1] } else { -1.0 };
            let up = if b + 1 < bins { vol[b + 1] } else { -1.0 };
            if up >= down {
                b += 1;
                acc += up;
            } else {
                a -= 1;
                acc += down;
            }
        }
        let base = Color::rgb(66, 135, 245);
        let colors: Vec<Color> = (0..bins)
            .map(|k| {
                if k == poc {
                    Color::rgb(255, 152, 0)
                } else if k >= a && k <= b {
                    base.with_alpha(0.75)
                } else {
                    base.with_alpha(0.3)
                }
            })
            .collect();
        self.x.label.get_or_insert_with(|| "volume".into());
        let mut bars = super::bar::Bars::new(centres, vol, base);
        bars.horizontal = true;
        bars.width = step * 0.9;
        bars.colors = Some(colors);
        bars.label = Some(format!("POC {}", short(lo + step * (poc as f64 + 0.5))));
        self.add(bars)
    }

    /// Order book depth: cumulative bid size (green, left) and ask size (red,
    /// right) against price, as filled steps meeting at the spread.
    pub fn depth_chart(&mut self, bids: &[(f64, f64)], asks: &[(f64, f64)]) -> &mut Self {
        let mut b: Vec<(f64, f64)> = bids
            .iter()
            .copied()
            .filter(|(p, s)| p.is_finite() && s.is_finite())
            .collect();
        let mut a: Vec<(f64, f64)> = asks
            .iter()
            .copied()
            .filter(|(p, s)| p.is_finite() && s.is_finite())
            .collect();
        b.sort_by(|x, y| y.0.total_cmp(&x.0));
        a.sort_by(|x, y| x.0.total_cmp(&y.0));
        let cum = |side: &[(f64, f64)]| -> (Vec<f64>, Vec<f64>) {
            let mut t = 0.0;
            side.iter()
                .map(|(p, s)| {
                    t += s;
                    (*p, t)
                })
                .unzip()
        };
        let (bx, by) = cum(&b);
        let (ax_, ay) = cum(&a);
        for (x, y, c, name) in [(bx, by, UP, "bids"), (ax_, ay, DOWN, "asks")] {
            if x.is_empty() {
                continue;
            }
            let l = self.line(x, y);
            l.color(c)
                .width(1.5)
                .step(super::line::Step::Post)
                .label(name);
            l.fill = Some(0.0);
            l.fill_opacity = 0.25;
            l.decimate = false;
        }
        self.x.label.get_or_insert_with(|| "price".into());
        self.y.label.get_or_insert_with(|| "cumulative size".into());
        self
    }

    /// Ichimoku cloud: conversion (9) and base (26) lines, the cloud between
    /// leading spans A and B shifted 26 periods ahead (green when A is above
    /// B, red otherwise) and the lagging span. `x` must be evenly spaced.
    pub fn ichimoku(
        &mut self,
        x: impl IntoValues,
        high: impl IntoValues,
        low: impl IntoValues,
        close: impl IntoValues,
    ) -> &mut Self {
        let x = x.into_values();
        let [tenkan, kijun, a, b, lag] = crate::plot::stats::ichimoku(
            &high.into_values(),
            &low.into_values(),
            &close.into_values(),
        );
        let dx = if x.len() >= 2 {
            x[x.len() - 1] - x[x.len() - 2]
        } else {
            1.0
        };
        let mut xs = x.clone();
        while xs.len() < a.len() {
            let last = *xs.last().unwrap_or(&0.0);
            xs.push(last + dx);
        }
        // The cloud in runs of one color, joined at the crossings.
        let mut start = 0;
        while start < xs.len() {
            if !(a[start].is_finite() && b[start].is_finite()) {
                start += 1;
                continue;
            }
            let up = a[start] >= b[start];
            let mut end = start;
            while end + 1 < xs.len()
                && a[end + 1].is_finite()
                && b[end + 1].is_finite()
                && (a[end + 1] >= b[end + 1]) == up
            {
                end += 1;
            }
            let stop = (end + 2).min(xs.len());
            let (lo, hi): (Vec<f64>, Vec<f64>) = (start..stop)
                .map(|i| (a[i].min(b[i]), a[i].max(b[i])))
                .filter(|(l, h)| l.is_finite() && h.is_finite())
                .unzip();
            let n = lo.len();
            self.fill_between(&xs[start..start + n], lo, hi)
                .color(if up { UP } else { DOWN })
                .opacity(0.18);
            start = end + 1;
        }
        let lines: [(&[f64], Color, &str); 5] = [
            (&tenkan, Color::rgb(41, 98, 255), "conversion (9)"),
            (&kijun, Color::rgb(183, 28, 28), "base (26)"),
            (&a, UP.with_alpha(0.8), "span A"),
            (&b, DOWN.with_alpha(0.8), "span B"),
            (&lag, Color::rgb(130, 130, 130), "lagging"),
        ];
        for (y, c, name) in lines {
            let l = self.line(&xs, y);
            l.color(c).width(1.1).label(name);
            l.decimate = false;
        }
        self
    }

    /// Volume-weighted average price line.
    pub fn vwap(
        &mut self,
        x: impl IntoValues,
        high: impl IntoValues,
        low: impl IntoValues,
        close: impl IntoValues,
        volume: impl IntoValues,
    ) -> &mut super::line::Line {
        let v = crate::plot::stats::vwap(
            &high.into_values(),
            &low.into_values(),
            &close.into_values(),
            &volume.into_values(),
        );
        let l = self.line(x, v);
        l.color(Color::rgb(255, 152, 0)).width(1.6).label("VWAP");
        l
    }

    /// Stochastic oscillator %K and %D with the 20/80 levels (y 0–100).
    pub fn stochastic(
        &mut self,
        x: impl IntoValues,
        high: impl IntoValues,
        low: impl IntoValues,
        close: impl IntoValues,
        k: usize,
        d: usize,
    ) -> &mut super::line::Line {
        let x = x.into_values();
        let (pk, pd) = crate::plot::stats::stochastic(
            &high.into_values(),
            &low.into_values(),
            &close.into_values(),
            k,
            d,
        );
        let band = Color::rgb(126, 87, 194);
        self.hspan(20.0, 80.0).color(band).opacity(0.06);
        self.hline(80.0).color(band.with_alpha(0.5));
        self.hline(20.0).color(band.with_alpha(0.5));
        self.y_limits(0.0, 100.0);
        self.line(&x, pd)
            .color(Color::rgb(255, 109, 0))
            .width(1.2)
            .label(format!("%D ({d})"));
        let l = self.line(&x, pk);
        l.color(Color::rgb(41, 98, 255))
            .width(1.2)
            .label(format!("%K ({k})"));
        l
    }

    /// Average true range over `n` periods (volatility).
    pub fn atr(
        &mut self,
        x: impl IntoValues,
        high: impl IntoValues,
        low: impl IntoValues,
        close: impl IntoValues,
        n: usize,
    ) -> &mut super::line::Line {
        let a = crate::plot::stats::atr(
            &high.into_values(),
            &low.into_values(),
            &close.into_values(),
            n,
        );
        let l = self.line(x, a);
        l.color(Color::rgb(126, 87, 194))
            .width(1.4)
            .label(format!("ATR ({n})"));
        l
    }

    /// Drawdown of an equity curve in percent (red area below 0), with the
    /// maximum drawdown in the label.
    pub fn drawdown(
        &mut self,
        x: impl IntoValues,
        equity: impl IntoValues,
    ) -> &mut super::line::FillBetween {
        let x = x.into_values();
        let dd: Vec<f64> = crate::plot::stats::drawdown(&equity.into_values())
            .iter()
            .map(|v| v * 100.0)
            .collect();
        let max = dd.iter().copied().fold(0.0, f64::min);
        self.y.label.get_or_insert_with(|| "drawdown [%]".into());
        let zeros = vec![0.0; dd.len()];
        let f = self.fill_between(&x, dd, zeros);
        f.color(DOWN)
            .opacity(0.35)
            .label(format!("max drawdown {max:.1} %"));
        f
    }

    /// Renko chart: bricks of size `brick` from closing prices, drawn in
    /// order (time is not to scale on x).
    pub fn renko(&mut self, close: impl IntoValues, brick: f64) -> &mut Candles {
        let bricks = crate::plot::stats::renko(&close.into_values(), brick);
        let x: Vec<f64> = (0..bricks.len()).map(|i| i as f64).collect();
        let (o, c): (Vec<f64>, Vec<f64>) = bricks.iter().copied().unzip();
        let hi: Vec<f64> = bricks.iter().map(|(a, b)| a.max(*b)).collect();
        let lo: Vec<f64> = bricks.iter().map(|(a, b)| a.min(*b)).collect();
        self.x.label.get_or_insert_with(|| "brick".into());
        let k = self.candlestick(x, o, hi, lo, c);
        k.colors(UP, DOWN).width(0.95);
        k
    }

    /// A trading-day axis: bars are placed one after another, skipping
    /// weekends, holidays and nights, and the x-axis shows `times` (seconds
    /// since 1970) formatted with `format` (e.g. `"%d %b"`). Returns the x
    /// positions to plot the data at.
    pub fn trading_axis(&mut self, times: &[f64], format: &str) -> Vec<f64> {
        let labels: Vec<String> = times
            .iter()
            .map(|t| crate::plot::time::format_time(*t, format))
            .collect();
        // Few labels: one every k bars so that they do not crowd.
        let k = (times.len() / 8).max(1);
        let ticks: Vec<(f64, String)> = labels
            .iter()
            .enumerate()
            .filter(|(i, _)| i % k == 0)
            .map(|(i, l)| (i as f64, l.clone()))
            .collect();
        self.x.ticks = crate::plot::TickSpec::Labeled(ticks);
        (0..times.len()).map(|i| i as f64).collect()
    }
}
