//! Trading charts: order-book liquidity heatmap, footprint (order flow)
//! candles, market profile (TPO), option payoff diagrams and implied
//! volatility surfaces.

use super::distribution::erfc;
use super::general::Shapes;
use super::grid::Heatmap;
use super::three_d::Mesh3d;
use super::{short, Context, Extent, Hit, Rect, Series};
use crate::plot::axes::Axes;
use crate::plot::axis::{AxisMap, TickSpec};
use crate::plot::color::{Color, Colormap};
use crate::plot::scene::{Anchor, Baseline, Stroke};
use crate::plot::style::{Dash, Marker};
use std::any::Any;

// ----- footprint -------------------------------------------------------------------

/// One bar of a footprint chart: OHLC and the volume traded at the bid and
/// at the ask on each price level.
#[derive(Clone, Debug, PartialEq)]
pub struct FootprintBar {
    /// Open.
    pub open: f64,
    /// High.
    pub high: f64,
    /// Low.
    pub low: f64,
    /// Close.
    pub close: f64,
    /// `(price, volume at bid, volume at ask)` per level.
    pub levels: Vec<(f64, f64, f64)>,
}

/// Footprint (order flow) chart: per bar and price level the volume sold at
/// the bid and bought at the ask, imbalances highlighted, the bar's point
/// of control outlined. Created by [`Axes::footprint`].
#[derive(Clone, Debug)]
pub struct Footprint {
    pub(crate) bars: Vec<FootprintBar>,
    pub(crate) tick: f64,
    pub(crate) imbalance: f64,
    pub(crate) up: Color,
    pub(crate) down: Color,
}

impl Footprint {
    /// Ratio of ask to bid volume (diagonal) counted as an imbalance
    /// (default 3).
    pub fn imbalance(&mut self, ratio: f64) -> &mut Self {
        self.imbalance = ratio.max(1.0);
        self
    }
}

impl Series for Footprint {
    fn extent(&self) -> Option<Extent> {
        let (lo, hi) = self
            .bars
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), k| {
                (a.min(k.low), b.max(k.high))
            });
        lo.is_finite().then(|| {
            Extent::point(-0.6, lo - self.tick)
                .include(self.bars.len() as f64 - 0.4, hi + self.tick)
        })
    }
    fn draw(&self, ctx: &mut Context) {
        // Sizes from the unclamped axis maps (0 may lie outside the view).
        let cell_h = (ctx.y.px(self.tick) - ctx.y.px(0.0)).abs();
        let col_w = (ctx.x.px(1.0) - ctx.x.px(0.0)).abs() * 0.6;
        // Text sized to the cells; left out when it would be unreadable.
        let widest = self
            .bars
            .iter()
            .flat_map(|b| b.levels.iter())
            .map(|l| format!("{} × {}", short(l.1), short(l.2)).len())
            .max()
            .unwrap_or(1) as f64;
        let ts = (ctx.theme.tick_size * 0.8)
            .min(cell_h * 0.9)
            .min(col_w / (widest * 0.55));
        let text_ok = ts >= 6.0;
        let vmax = self
            .bars
            .iter()
            .flat_map(|b| b.levels.iter())
            .map(|l| l.1 + l.2)
            .fold(0.0, f64::max)
            .max(1e-300);
        let bg = ctx.theme.plot_background;
        let mut cells: Vec<(Shapes, Color)> = Vec::new();
        let mut buy_imb = Shapes::default();
        let mut sell_imb = Shapes::default();
        let mut poc = Shapes::default();
        let mut labels = Vec::new();
        for (i, b) in self.bars.iter().enumerate() {
            let xc = i as f64;
            let (l, _) = ctx.point(xc - 0.18, 0.0);
            let (r, _) = ctx.point(xc + 0.42, 0.0);
            let mid = (l + r) / 2.0;
            let top = b.levels.iter().map(|lv| lv.1 + lv.2).fold(0.0, f64::max);
            for (k, &(p, bid, ask)) in b.levels.iter().enumerate() {
                let (_, ya) = ctx.point(xc, p + self.tick / 2.0);
                let (_, yb) = ctx.point(xc, p - self.tick / 2.0);
                // Cell shade by traded volume (8 steps, one path each).
                let step = (((bid + ask) / vmax).sqrt() * 7.0).round() as usize;
                let c = bg.mix(
                    if b.close >= b.open {
                        self.up
                    } else {
                        self.down
                    },
                    0.12 + 0.08 * step as f64,
                );
                match cells.iter_mut().find(|(_, cc)| *cc == c) {
                    Some((s, _)) => s.rect(l, ya, r, yb),
                    None => {
                        let mut s = Shapes::default();
                        s.rect(l, ya, r, yb);
                        cells.push((s, c));
                    }
                }
                // Diagonal imbalance: ask here against bid one level lower.
                let below = if k > 0 { b.levels[k - 1].1 } else { f64::NAN };
                if below.is_finite() && ask >= self.imbalance * below.max(1.0) {
                    buy_imb.rect(mid, ya, r, yb);
                }
                let above = b.levels.get(k + 1).map_or(f64::NAN, |x| x.2);
                if above.is_finite() && bid >= self.imbalance * above.max(1.0) {
                    sell_imb.rect(l, ya, mid, yb);
                }
                if bid + ask == top {
                    poc.line(&[(l, ya), (r, ya), (r, yb), (l, yb), (l, ya)]);
                }
                if text_ok {
                    labels.push((
                        mid,
                        (ya + yb) / 2.0,
                        format!("{} × {}", short(bid), short(ask)),
                    ));
                }
            }
            // Thin candle on the left of the column.
            let c = if b.close >= b.open {
                self.up
            } else {
                self.down
            };
            let (cx, hy) = ctx.point(xc - 0.3, b.high);
            let (_, ly) = ctx.point(xc - 0.3, b.low);
            ctx.polyline(&[(cx, hy), (cx, ly)], Stroke::new(c, 1.0));
            let (bl, oy) = ctx.point(xc - 0.36, b.open);
            let (br, cy) = ctx.point(xc - 0.24, b.close);
            ctx.rect(bl, oy, br, cy, Some(c), None);
        }
        for (s, c) in cells {
            s.fill(ctx, c);
        }
        buy_imb.fill(ctx, self.up.with_alpha(0.45));
        sell_imb.fill(ctx, self.down.with_alpha(0.45));
        poc.stroke(ctx, Stroke::new(ctx.theme.text, 1.2));
        for (x, y, t) in labels {
            ctx.text(
                x,
                y,
                t,
                ts,
                ctx.theme.text,
                Anchor::Middle,
                Baseline::Middle,
            );
        }
    }
    fn hit(&self, x: &AxisMap, y: &AxisMap, _r: Rect, px: f64, py: f64) -> Option<Hit> {
        let i = x.value(px).round();
        let b = self.bars.get(i as usize).filter(|_| i >= 0.0)?;
        let p = y.value(py);
        let lv = b
            .levels
            .iter()
            .min_by(|a, c| (a.0 - p).abs().total_cmp(&(c.0 - p).abs()))?;
        Some(Hit {
            px: x.px(i),
            py: y.px(lv.0),
            label: format!(
                "{}: bid {} × ask {} (delta {})",
                short(lv.0),
                short(lv.1),
                short(lv.2),
                short(lv.2 - lv.1)
            ),
            distance: 0.0,
        })
    }
    fn kind(&self) -> &'static str {
        "footprint"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- market profile ------------------------------------------------------------

/// Market profile (TPO chart): for each price level a letter for every
/// period that traded there; the point of control and the value area
/// (70 % of the letters) are highlighted. Created by
/// [`Axes::market_profile`].
#[derive(Clone, Debug)]
pub struct MarketProfile {
    /// Price levels (bottom up) and the period indices at each.
    pub(crate) levels: Vec<(f64, Vec<usize>)>,
    pub(crate) tick: f64,
    pub(crate) poc: usize,
    pub(crate) value_area: (usize, usize),
    pub(crate) colors: Vec<Color>,
}

/// Result of [`tpo`]: levels `(price, periods)`, index of the point of
/// control, value area index range.
pub type Tpo = (Vec<(f64, Vec<usize>)>, usize, (usize, usize));

/// TPO levels of periods `(low, high)`: price levels every `tick` with the
/// periods touching them, the point of control and the value area (index
/// range holding ~70 % of all letters).
pub fn tpo(periods: &[(f64, f64)], tick: f64) -> Tpo {
    let lo = periods
        .iter()
        .map(|p| p.0)
        .filter(|v| v.is_finite())
        .fold(f64::INFINITY, f64::min);
    let hi = periods
        .iter()
        .map(|p| p.1)
        .filter(|v| v.is_finite())
        .fold(f64::NEG_INFINITY, f64::max);
    if !(lo.is_finite() && hi.is_finite()) || tick <= 0.0 {
        return (Vec::new(), 0, (0, 0));
    }
    let n = ((hi - lo) / tick).round() as usize + 1;
    let levels: Vec<(f64, Vec<usize>)> = (0..n)
        .map(|k| {
            let p = (lo / tick).round() * tick + k as f64 * tick;
            let who = periods
                .iter()
                .enumerate()
                .filter(|(_, r)| r.0 <= p + tick * 0.5 && r.1 >= p - tick * 0.5)
                .map(|(i, _)| i)
                .collect();
            (p, who)
        })
        .collect();
    let count = |k: usize| levels[k].1.len();
    let poc = (0..n)
        .max_by(|a, b| count(*a).cmp(&count(*b)).then(b.cmp(a)))
        .unwrap_or(0);
    let total: usize = (0..n).map(count).sum();
    let (mut a, mut b) = (poc, poc);
    let mut sum = count(poc);
    while (sum as f64) < 0.7 * total as f64 && (a > 0 || b + 1 < n) {
        let up = if b + 1 < n { count(b + 1) } else { 0 };
        let down = if a > 0 { count(a - 1) } else { 0 };
        if (up >= down && b + 1 < n) || a == 0 {
            b += 1;
            sum += up;
        } else {
            a -= 1;
            sum += down;
        }
    }
    (levels, poc, (a, b))
}

fn tpo_letter(i: usize) -> char {
    let set = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    set[i % set.len()] as char
}

impl Series for MarketProfile {
    fn extent(&self) -> Option<Extent> {
        let w = self.levels.iter().map(|l| l.1.len()).max().unwrap_or(0) as f64;
        let (lo, hi) = (self.levels.first()?.0, self.levels.last()?.0);
        Some(Extent::point(-0.5, lo - self.tick).include(w + 0.5, hi + self.tick))
    }
    fn draw(&self, ctx: &mut Context) {
        let h = (ctx.y.px(self.tick) - ctx.y.px(0.0)).abs();
        let w = (ctx.x.px(1.0) - ctx.x.px(0.0)).abs();
        let size = (h * 0.85).min(w * 1.1).clamp(5.0, 16.0);
        // Value area band and point of control.
        let (a, b) = self.value_area;
        if let (Some(la), Some(lb)) = (self.levels.get(a), self.levels.get(b)) {
            let wmax = self.levels.iter().map(|l| l.1.len()).max().unwrap_or(0) as f64;
            let (x0, y0) = ctx.point(-0.5, la.0 - self.tick / 2.0);
            let (x1, y1) = ctx.point(wmax + 0.5, lb.0 + self.tick / 2.0);
            ctx.rect(
                x0,
                y0,
                x1,
                y1,
                Some(Color::rgb(70, 130, 180).with_alpha(0.12)),
                None,
            );
            let p = &self.levels[self.poc];
            let (x0, y0) = ctx.point(-0.5, p.0 - self.tick / 2.0);
            let (x1, y1) = ctx.point(p.1.len() as f64 - 0.5, p.0 + self.tick / 2.0);
            ctx.rect(
                x0,
                y0,
                x1,
                y1,
                Some(Color::rgb(230, 160, 30).with_alpha(0.35)),
                None,
            );
        }
        for (p, who) in &self.levels {
            for (k, &period) in who.iter().enumerate() {
                let (x, y) = ctx.point(k as f64, *p);
                let c = self.colors[period % self.colors.len()];
                ctx.text(
                    x,
                    y,
                    tpo_letter(period).to_string(),
                    size,
                    c,
                    Anchor::Middle,
                    Baseline::Middle,
                );
            }
        }
    }
    fn hit(&self, _x: &AxisMap, y: &AxisMap, _r: Rect, _px: f64, py: f64) -> Option<Hit> {
        let p = y.value(py);
        let (k, l) = self
            .levels
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 .0 - p).abs().total_cmp(&(b.1 .0 - p).abs()))?;
        let tag = if k == self.poc {
            " (POC)"
        } else if (self.value_area.0..=self.value_area.1).contains(&k) {
            " (value area)"
        } else {
            ""
        };
        Some(Hit {
            px: _r.x,
            py: y.px(l.0),
            label: format!("{}: {} TPO{}", short(l.0), l.1.len(), tag),
            distance: 0.0,
        })
    }
    fn kind(&self) -> &'static str {
        "market profile"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- options -----------------------------------------------------------------------

/// One leg of an option strategy.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum OptionLeg {
    /// Call: strike, premium paid (received when `qty` < 0), quantity.
    Call {
        /// Strike.
        strike: f64,
        /// Premium per contract.
        premium: f64,
        /// Contracts (negative = sold).
        qty: f64,
    },
    /// Put: strike, premium, quantity.
    Put {
        /// Strike.
        strike: f64,
        /// Premium per contract.
        premium: f64,
        /// Contracts (negative = sold).
        qty: f64,
    },
    /// The underlying bought (`qty` > 0) or sold at a price.
    Stock {
        /// Entry price.
        price: f64,
        /// Units (negative = short).
        qty: f64,
    },
}

fn norm_cdf(x: f64) -> f64 {
    0.5 * erfc(-x / std::f64::consts::SQRT_2)
}

/// Black–Scholes price of a call or put (`t` in years, `vol` and `rate`
/// annual).
pub fn black_scholes(call: bool, s: f64, k: f64, t: f64, vol: f64, rate: f64) -> f64 {
    if t <= 0.0 || vol <= 0.0 {
        return if call {
            (s - k).max(0.0)
        } else {
            (k - s).max(0.0)
        };
    }
    let d1 = ((s / k).ln() + (rate + vol * vol / 2.0) * t) / (vol * t.sqrt());
    let d2 = d1 - vol * t.sqrt();
    if call {
        s * norm_cdf(d1) - k * (-rate * t).exp() * norm_cdf(d2)
    } else {
        k * (-rate * t).exp() * norm_cdf(-d2) - s * norm_cdf(-d1)
    }
}

/// Profit of a strategy at underlying price `s`, `t` years before expiry
/// (0 = at expiry).
pub fn strategy_value(legs: &[OptionLeg], s: f64, t: f64, vol: f64, rate: f64) -> f64 {
    legs.iter()
        .map(|l| match *l {
            OptionLeg::Call {
                strike,
                premium,
                qty,
            } => qty * (black_scholes(true, s, strike, t, vol, rate) - premium),
            OptionLeg::Put {
                strike,
                premium,
                qty,
            } => qty * (black_scholes(false, s, strike, t, vol, rate) - premium),
            OptionLeg::Stock { price, qty } => qty * (s - price),
        })
        .sum()
}

impl Axes {
    /// Order-book liquidity heatmap: resting size `depth[time][level]` over
    /// a time range and a price range (log-scaled colors, so walls and thin
    /// books both show). Draw the traded price on top with
    /// [`Axes::line`].
    pub fn liquidity_heatmap(
        &mut self,
        time_range: (f64, f64),
        price_range: (f64, f64),
        depth: &[Vec<f64>],
    ) -> &mut Heatmap {
        // Field rows are y (price), columns x (time): transpose, log1p.
        let levels = depth.iter().map(|r| r.len()).max().unwrap_or(0);
        let z: Vec<Vec<f64>> = (0..levels)
            .map(|p| {
                depth
                    .iter()
                    .map(|r| {
                        r.get(p)
                            .copied()
                            .filter(|v| v.is_finite())
                            .unwrap_or(0.0)
                            .max(0.0)
                            .ln_1p()
                    })
                    .collect()
            })
            .collect();
        let h = self.field(time_range, price_range, z);
        h.colormap(Colormap::Inferno).colorbar("resting size (log)");
        h
    }

    /// Footprint (order flow) chart of bars with bid × ask volume per price
    /// level every `tick`.
    pub fn footprint(&mut self, bars: Vec<FootprintBar>, tick: f64) -> &mut Footprint {
        self.x.grid = Some(false);
        self.add(Footprint {
            bars,
            tick: tick.abs().max(1e-12),
            imbalance: 3.0,
            up: Color::rgb(38, 166, 91),
            down: Color::rgb(214, 48, 49),
        })
    }

    /// Market profile (TPO chart) of periods given as `(low, high)` (one
    /// letter each: A, B, C, …) on price levels every `tick`.
    pub fn market_profile(&mut self, periods: &[(f64, f64)], tick: f64) -> &mut MarketProfile {
        let (levels, poc, va) = tpo(periods, tick);
        self.x.ticks = TickSpec::None;
        self.x.grid = Some(false);
        let colors = (0..periods.len())
            .map(|i| {
                Colormap::Viridis
                    .at(0.05
                        + 0.85 * i as f64 / periods.len().max(2).saturating_sub(1).max(1) as f64)
            })
            .collect();
        self.add(MarketProfile {
            levels,
            tick,
            poc,
            value_area: va,
            colors,
        })
    }

    /// Option strategy payoff at expiry (profit green, loss red) over
    /// underlying prices `range`, with break-even points; when `days` > 0
    /// also the value `days` before expiry (Black–Scholes with `vol`).
    pub fn option_payoff(
        &mut self,
        legs: &[OptionLeg],
        range: (f64, f64),
        days: f64,
        vol: f64,
        rate: f64,
    ) -> &mut Self {
        let n = 400;
        let s: Vec<f64> = (0..=n)
            .map(|i| range.0 + (range.1 - range.0) * i as f64 / n as f64)
            .collect();
        let pay: Vec<f64> = s
            .iter()
            .map(|&x| strategy_value(legs, x, 0.0, vol, rate))
            .collect();
        let green = Color::rgb(38, 166, 91);
        let red = Color::rgb(214, 48, 49);
        self.fill_between(&s, pay.iter().map(|p| p.max(0.0)), vec![0.0; s.len()])
            .color(green)
            .opacity(0.18);
        self.fill_between(&s, pay.iter().map(|p| p.min(0.0)), vec![0.0; s.len()])
            .color(red)
            .opacity(0.18);
        self.hline(0.0).color(Color::rgb(90, 90, 90));
        if days > 0.0 {
            let t = days / 365.0;
            let now: Vec<f64> = s
                .iter()
                .map(|&x| strategy_value(legs, x, t, vol, rate))
                .collect();
            self.line(&s, now)
                .color(Color::rgb(31, 119, 180))
                .dash(Dash::Dashed)
                .width(1.8)
                .label(format!("{days:.0} days before expiry"));
        }
        self.line(&s, pay.clone())
            .color(Color::rgb(30, 30, 30))
            .width(2.4)
            .label("at expiry");
        // Break-even points: sign changes of the expiry payoff.
        let mut be = Vec::new();
        for i in 1..s.len() {
            if (pay[i - 1] < 0.0) != (pay[i] < 0.0) {
                let f = pay[i - 1] / (pay[i - 1] - pay[i]);
                be.push(s[i - 1] + f * (s[i] - s[i - 1]));
            }
        }
        if !be.is_empty() {
            self.scatter(be.clone(), vec![0.0; be.len()])
                .color(Color::rgb(30, 30, 30))
                .marker(Marker::Diamond)
                .size(9.0)
                .label("break-even");
            for b in &be {
                self.text(*b, 0.0, format!(" {}", short(*b)))
                    .anchor(Anchor::Start)
                    .size(11.0);
            }
        }
        self.x
            .label
            .get_or_insert_with(|| "underlying price at expiry".into());
        self.y.label.get_or_insert_with(|| "profit / loss".into());
        self
    }

    /// Implied volatility surface over strikes and maturities (any
    /// spacing): `iv[maturity][strike]`, colored by volatility.
    pub fn volatility_surface(
        &mut self,
        strikes: &[f64],
        maturities: &[f64],
        iv: &[Vec<f64>],
    ) -> &mut Mesh3d {
        let (nk, nm) = (strikes.len(), maturities.len().min(iv.len()));
        let mut v = Vec::with_capacity(nk * nm);
        let mut values = Vec::with_capacity(nk * nm);
        for (m, row) in iv.iter().enumerate().take(nm) {
            for (k, &s) in strikes.iter().enumerate() {
                let z = row.get(k).copied().unwrap_or(f64::NAN);
                v.push([s, maturities[m], z]);
                values.push(z);
            }
        }
        let mut faces = Vec::new();
        for m in 0..nm.saturating_sub(1) {
            for k in 0..nk.saturating_sub(1) {
                let id = |m: usize, k: usize| (m * nk + k) as u32;
                faces.push([id(m, k), id(m, k + 1), id(m + 1, k + 1), id(m + 1, k)]);
            }
        }
        self.x.label.get_or_insert_with(|| "strike".into());
        self.y.label.get_or_insert_with(|| "maturity".into());
        self.z_label("implied vol");
        let mesh = self.mesh3d(v, faces);
        mesh.values(values, Colormap::Plasma);
        mesh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn black_scholes_parity() {
        let (s, k, t, v, r) = (100.0, 95.0, 0.5, 0.25, 0.03);
        let c = black_scholes(true, s, k, t, v, r);
        let p = black_scholes(false, s, k, t, v, r);
        assert!((c - p - (s - k * (-r * t).exp())).abs() < 1e-6, "{c} {p}");
        // d1 = 0.4634, d2 = 0.2866 → C = 100·N(d1) − 95·e^(−0.015)·N(d2) ≈ 10.50.
        assert!((c - 10.50).abs() < 0.01, "{c}");
    }

    #[test]
    fn tpo_poc_and_value_area() {
        let periods = [(10.0, 12.0), (11.0, 13.0), (11.0, 12.0), (12.0, 14.0)];
        let (levels, poc, (a, b)) = tpo(&periods, 1.0);
        assert_eq!(levels.len(), 5);
        assert_eq!(levels[poc].0, 12.0);
        let total: usize = levels.iter().map(|l| l.1.len()).sum();
        let inside: usize = levels[a..=b].iter().map(|l| l.1.len()).sum();
        assert!(inside as f64 >= 0.7 * total as f64);
    }

    #[test]
    fn straddle_breakevens() {
        let legs = [
            OptionLeg::Call {
                strike: 100.0,
                premium: 5.0,
                qty: 1.0,
            },
            OptionLeg::Put {
                strike: 100.0,
                premium: 4.0,
                qty: 1.0,
            },
        ];
        assert!((strategy_value(&legs, 100.0, 0.0, 0.2, 0.0) + 9.0).abs() < 1e-12);
        assert!(strategy_value(&legs, 109.0, 0.0, 0.2, 0.0).abs() < 1e-12);
        let mut fig = Figure::new(600, 400);
        fig.axes()
            .option_payoff(&legs, (70.0, 130.0), 30.0, 0.25, 0.02);
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
    }

    #[test]
    fn all_render() {
        let mut fig = Figure::new(1200, 800);
        fig.grid(2, 2);
        fig.at(0, 0).liquidity_heatmap(
            (0.0, 10.0),
            (99.0, 101.0),
            &[vec![1.0, 5.0, 2.0], vec![3.0, 1.0, 0.0]],
        );
        fig.at(0, 1).footprint(
            vec![FootprintBar {
                open: 100.0,
                high: 101.0,
                low: 99.5,
                close: 100.5,
                levels: vec![
                    (99.5, 10.0, 3.0),
                    (100.0, 20.0, 40.0),
                    (100.5, 5.0, 30.0),
                    (101.0, 1.0, 2.0),
                ],
            }],
            0.5,
        );
        fig.at(1, 0)
            .market_profile(&[(10.0, 12.0), (11.0, 13.0)], 0.5);
        fig.at(1, 1).volatility_surface(
            &[90.0, 100.0, 110.0],
            &[0.1, 0.5],
            &[vec![0.3, 0.25, 0.28], vec![0.27, 0.24, 0.26]],
        );
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
    }
}
