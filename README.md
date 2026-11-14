# **DataViz** 🚀

<a href="https://crates.io/crates/dataviz">
    <img style="display: inline!important" src="https://img.shields.io/crates/v/dataviz.svg"></img>
</a>
<a href="https://docs.rs/dataviz">
    <img style="display: inline!important" src="https://docs.rs/dataviz/badge.svg"></img>
</a>
<a href="https://docs.rs/dataviz">
    <img style="display: inline!important" src="https://img.shields.io/crates/d/dataviz"></img>
</a>

**Fast, pure-Rust plotting: 50+ chart types, interactive windows, live
data, animations, maps, math and every language — with the least CPU and
memory of the Rust plotting libraries.**

<table>
<tr>
<td width="33%"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-interactive/screenshots/interactive.gif" alt="Zoom, pan and range slider" width="100%"></td>
<td width="33%"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-streaming/screenshots/streaming.gif" alt="Live dashboard" width="100%"></td>
<td width="33%"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-animation/screenshots/animation.gif" alt="Animation" width="100%"></td>
</tr>
<tr>
<td width="33%"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-maps/screenshots/maps.png" alt="Maps" width="100%"></td>
<td width="33%"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-indicators/screenshots/indicators.png" alt="Finance" width="100%"></td>
<td width="33%"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-math/screenshots/math.png" alt="Math notation" width="100%"></td>
</tr>
</table>

## Why dataviz

- **Fast and lean.** Its own rasterizer draws straight into the output
  buffer: a 10-million-point line renders in about 0.1 s with a few MB of
  extra memory, a live chart redraws in 1–2 ms. 8–50× faster than
  matplotlib, and faster and lighter than plotters, ruviz, kuva and
  charts-rs in nearly every measurement ([numbers below](#performance)).
  Optional multi-core and GPU backends.
- **Everything in one crate.** 50+ chart types — lines, bars, statistics,
  finance with indicators, heatmaps, contours, 3D, maps, sankey, networks —
  with themes, subplots, shared and broken axes, legends and colorbars.
- **Interactive and live.** `fig.show()` opens a window with zoom, pan, a
  crosshair with axis values, hover readouts, legend toggling, a range
  slider, measurement and selection; `fig.show_live()` streams data with
  rolling windows and records it to CSV or a GIF.
- **Every output.** PNG, SVG, PDF, JPEG, WebP, interactive HTML (one file,
  no internet needed), animated GIF/PNG and MP4, the terminal, raw pixels
  for your own GUI, and the browser via WebAssembly.
- **Text done right.** TeX math (`$\sigma^2 = \frac{1}{N}\sum x_i^2$`) in
  every label, any language (CJK, Arabic and Hebrew right to left, Hindi,
  Thai) and color emoji.
- **Accessible.** Generated text descriptions for screen readers, color
  blindness simulation and warnings for colors that look alike.
- **Pure Rust, few dependencies.** No Python, JavaScript, browser,
  Cairo or system fonts needed (a font is embedded); builds headless and
  for `wasm32`.

## Quick start

```toml
[dependencies]
dataviz = "0.15"
```

```rust no_run
use dataviz::prelude::*;

fn main() -> Result<(), Error> {
    let x: Vec<f64> = (0..200).map(|i| i as f64 / 20.0).collect();

    let mut fig = Figure::new(900, 500);
    let ax = fig.axes();
    ax.title("Signals").x_label("time [s]").y_label("amplitude");
    ax.line(&x, x.iter().map(|t| t.sin())).label("sin");
    ax.line(&x, x.iter().map(|t| t.cos())).label("cos").dash(Dash::Dashed);
    ax.scatter([1.6, 4.7, 7.9], [1.0, -1.0, 1.0]).label("peaks").marker(Marker::Star).size(12.0);

    fig.save("signals.png")?;   // .svg .pdf .html .jpg .webp .bmp
    fig.show("Signals")?;       // interactive window
    Ok(())
}
```

Every chart type has a runnable example with code and output in
[dataviz-examples](https://github.com/dataviz-rs/dataviz-examples) — see
the [gallery](#gallery).

## Features

### Interactive windows and live data

```rust ignore
fig.show("title")?;                          // static data
fig.show_live("title", 30, |fig| { /* push new points */ true })?;
```

| Mouse / key | Action |
|---|---|
| wheel, drag, right-drag | zoom around the cursor, pan, zoom to a box |
| crosshair (`C`) | cursor values on the axes and every series' value at that x; linked across plots sharing x |
| hover (`H`) | nearest data point |
| click a legend row | hide or show the series |
| range slider | drag to pan, drag its edges to zoom |
| Shift+drag / Ctrl+drag | measure Δx, Δy and % / select a box (`fig.selection()`) |
| `A`, `R` | auto scale y to the visible data, follow live data again |
| `S`, Shift+S, Ctrl+C, `V` | save PNG, SVG/PDF, copy to the clipboard, record a GIF |

Live charts keep the view you zoomed to while data arrives (y follows the
visible data), stream with `Line::push` / `keep_last` / `keep_since`, and
`plot::data::CsvLog` records the stream to CSV. The same zoom, pan, hover
and click work in your own GUI or web page through
`fig.zoom_at`, `pan_by`, `hover`, `click` ([egui](https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-egui),
[WebAssembly](https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-wasm)).

### Chart types

| Category | Charts |
|---|---|
| Lines & areas | line, step, smooth spline, area, fill-between, stacked / 100 % area, function plot, sparkline |
| Points | scatter, colormapped scatter, bubble, 3D scatter |
| Bars | vertical, horizontal, grouped, stacked, percent, overlay, value labels, waterfall, Gantt, lollipop |
| Distributions | histogram (bin rules, density, cumulative, step), KDE, ECDF, box (notches), violin, strip, swarm, ridgeline, Q-Q, 2D histogram, hexbin |
| Uncertainty & fits | error bars, error bands, polynomial regression with R², moving averages |
| Finance | candlestick, OHLC, Heikin-Ashi, Bollinger bands, RSI, MACD |
| Grids & fields | heatmap, correlation matrix, calendar heatmap, image, contour lines and fills, quiver, 3D surface/wireframe |
| Maps | built-in world map, choropleth by ISO code or name, points and great-circle routes, GeoJSON shapes; Robinson, Natural Earth, equirectangular, Mercator |
| Parts of a whole | pie, donut, treemap, sunburst, funnel, gauge |
| Polar | polar line/scatter, radar, rose / wind rose |
| Relations & flows | sankey, network graph (force-directed), parallel coordinates |
| Layouts | subplots with spans, shared axes, broken axes, secondary y-axis, facets (small multiples), pair plot, aligned panels |
| Annotations | text, arrows, reference lines, shaded spans, rectangles |

### Statistics and finance

```rust ignore
ax.violin([("A", &a), ("B", &b)]);
ax.swarm([("A", &a), ("B", &b)]);
ax.ridgeline(months);                      // (name, values) per row
ax.qq(&values);                            // against the normal distribution
ax.regression(&x, &y, 2);                  // label shows the equation and R²
fig.facet_wrap(groups, 3, |ax, data| { ax.line(&t, data); });
fig.pair_plot([("length", &l), ("weight", &w), ("age", &a)]);

ax.candlestick(&t, &open, &high, &low, &close);
ax.bollinger(&t, &close, 20, 2.0);
rsi_panel.rsi(&t, &close, 14);
macd_panel.macd(&t, &close, 12, 26, 9);
```

### Maps

```rust ignore
use dataviz::plot::geo::Projection;
ax.map(Projection::Robinson);
ax.choropleth([("DEU", 83.2), ("TUR", 85.3), ("JP", 124.5), ("Brazil", 203.1)])
    .colorbar("population [M]");
ax.geo_line([28.98, -74.0], [41.01, 40.7]);    // great circle Istanbul → New York
```

Country outlines with ISO codes, continent, population and GDP are
included (Natural Earth, public domain); `geo::read_geojson` reads other
shapes.

### Text: math and any language

```rust ignore
ax.y_label(r"$\sigma^2$ [$\mathrm{m^2\,s^{-2}}$]");
ax.text(1.0, 2.0, r"$\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}$");
fig.title("Ventes · 全球销售 · المبيعات · 🚀");
```

TeX math (scripts, fractions, roots, Greek letters, operators, accents) is
typeset in PNG, SVG, PDF and on the GPU alike. Characters missing from the
embedded font come from system fonts; right-to-left text is reordered and
shaped; color emoji are drawn.

### Output

```rust ignore
fig.save("chart.png")?;            // also .svg .pdf .jpg .webp .bmp
fig.save("chart.html")?;           // interactive, self-contained page
fig.record("wave.gif", 30.0, 120, |fig, i| { /* change the figure */ })?; // .gif .png (APNG) .mp4 .webm
fig.print()?;                      // terminal (colored half blocks or Braille)
fig.render_into(&mut pixels)?;     // RGBA pixels for your own window or texture
```

### Accessibility

```rust ignore
let text = fig.alt_text();          // "Sales. Line chart; quarter from …; Series: north (3 to 5); …"
for w in fig.color_warnings() { println!("{w}"); }   // e.g. two series look alike with deuteranopia
pixels.simulate(Cvd::Deuteranopia); // see the chart as people with color blindness do
```

SVG, PDF and HTML output carry the title and description for screen
readers.

### Backends

```rust ignore
fig.backend(Backend::Auto);                // default: CPU, all cores from 1 megapixel on
fig.backend(Backend::Cpu { threads: 1 });  // least CPU time and memory
fig.backend(Backend::Cpu { threads: 0 });  // all cores: lowest latency without a GPU
fig.backend(Backend::Gpu);                 // `gpu` feature: Vulkan / Metal / DX12 / OpenGL
```

## Performance

Measured against **matplotlib** (Python) and the Rust libraries plotters, ruviz, kuva and charts-rs: the same charts, separate processes, medians (Intel(R) Core(TM) Ultra 7 255HX, NVIDIA GeForce RTX 5070 Ti Laptop GPU, Linux). Full results, method and the code to rerun them: [dataviz-examples/benchmarks](https://github.com/dataviz-rs/dataviz-examples/tree/main/benchmarks).

> **Compared with matplotlib**: 8–51× faster for charts saved to PNG, 29× faster for a live 100 000-point chart, and 151× less memory for 10 M points.

**Charts to an 800×600 PNG** (one thread, time per chart):

| Chart | dataviz | plotters | ruviz | kuva | charts-rs | matplotlib |
|---|---:|---:|---:|---:|---:|---:|
| line, 1 000 points | **1.62 ms** | 3.76 ms | 3.97 ms | 11.3 ms | 11.4 ms | 29.5 ms |
| bar, 20 bars | **0.81 ms** | 0.83 ms | 3.98 ms | 3.64 ms | 2.14 ms | 41.6 ms |
| scatter, 10 000 points | **2.25 ms** | 2.90 ms | 4.38 ms | 8.71 ms | 76.6 ms | 37.5 ms |
| scatter, 100 000 points | **10.1 ms** | 16.7 ms | 16.6 ms | 17.4 ms | 776.4 ms | 80.7 ms |
| line, 100 000 points | **2.13 ms** | 5.99 ms | 8.22 ms | 28.4 ms | 831.8 ms | 49.8 ms |
| histogram, 1 M values | **1.99 ms** | 2.77 ms | 29.7 ms | 15.8 ms | 12.4 ms | 49.7 ms |

**Big data** (time / memory above the data itself):

| Chart | dataviz | dataviz all cores | plotters | ruviz | kuva | charts-rs | matplotlib |
|---|---:|---:|---:|---:|---:|---:|---:|
| line, 10 M points | 122.3 ms / +8 MB | **99.6 ms / +8 MB** | 486.1 ms / +315 MB | 329.5 ms / +389 MB | 2.71 s / +787 MB | fails | 565.7 ms / +1162 MB |
| scatter, 10 M points | 222.1 ms / +9 MB | **176.5 ms / +9 MB** | 1.53 s / +12 MB | 1.23 s / +379 MB | 1.55 s / +788 MB | fails | 5.11 s / +910 MB |
| histogram, 100 M values | 242.3 ms / +7 MB | **237.6 ms / +7 MB** | 260.6 ms / +12 MB | 3.58 s / +1915 MB | 1.62 s / +1551 MB | 1.21 s / +1532 MB | 520.0 ms / +13 MB |

**Live chart** (rolling window, one new point and a full redraw per frame; latency p50 / CPU per frame / memory of the process):

| Window | dataviz 1 thread | dataviz all cores | dataviz GPU | plotters | ruviz | kuva | matplotlib |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 000 points, 800×600 | 0.74 ms / 0.73 ms / 8 MB | **0.52 ms / 2.53 ms / 9 MB** | 0.83 ms / 0.43 ms / 238 MB | 0.63 ms / 0.63 ms / 14 MB | 5.26 ms / 5.27 ms / 11 MB | 5.21 ms / 5.20 ms / 27 MB | 9.98 ms / 9.97 ms / 74 MB |
| 100 000 points, 800×600 | 2.06 ms / 2.03 ms / 12 MB | 1.81 ms / 4.50 ms / 14 MB | **1.76 ms / 1.30 ms / 242 MB** | 5.39 ms / 5.43 ms / 16 MB | 30.8 ms / 30.8 ms / 17 MB | 45.6 ms / 45.8 ms / 36 MB | 60.0 ms / 60.2 ms / 113 MB |
| 1 000 points, 1920×1080 | 1.60 ms / 1.63 ms / 16 MB | **0.77 ms / 5.83 ms / 18 MB** | 1.71 ms / 1.37 ms / 244 MB | 1.44 ms / 1.43 ms / 18 MB | 14.9 ms / 14.9 ms / 18 MB | 17.3 ms / 17.3 ms / 33 MB | 12.0 ms / 12.0 ms / 80 MB |
| 100 000 points, 1920×1080 | 5.15 ms / 5.17 ms / 19 MB | 3.14 ms / 14.1 ms / 21 MB | **2.80 ms / 2.40 ms / 249 MB** | 8.15 ms / 8.17 ms / 21 MB | 119.5 ms / 119.5 ms / 28 MB | 111.8 ms / 112.1 ms / 42 MB | 135.8 ms / 135.8 ms / 177 MB |

**Animated GIF** (800×600, 120 frames; time, file size):

| Animation | dataviz | plotters | ruviz | matplotlib |
|---|---:|---:|---:|---:|
| whole plot moves | **252.0 ms, 1001 KB** | 1.08 s, 3227 KB | 2.93 s, 1741 KB | 5.90 s, 1119 KB |
| line grows on fixed axes | **81.0 ms, 38 KB** | 2.38 s, 3257 KB | 2.93 s, 1701 KB | 6.10 s, 420 KB |

What makes it fast: the scene is drawn straight into the output buffer by
dataviz's own anti-aliased rasterizer; dense lines become one pixel-column
envelope and dense scatter plots an occupancy grid, so time and memory stay
flat as the data grows; data passed to a series is not copied; glyphs and
markers are cached as bitmaps; histograms use SIMD binning. Animations store
only the changed part of each frame. On the web the same renderer compiled to
WebAssembly draws a 960×520 live chart with 2 × 3 000 points in about 1.7 ms.

Where it is not first: for a tiny live chart (1 000 points) on a single
thread plotters is about 0.1 ms faster per frame; dataviz on all cores is
the fastest there. The GPU backend needs the least CPU time but ~240 MB of
driver memory and ~0.3 s to start, so the default stays on the CPU.

## Gallery

Every image links to its example in
[dataviz-examples](https://github.com/dataviz-rs/dataviz-examples) (code,
`cargo run`, output).

### Live, interactive and animated

<table>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-interactive"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-interactive/screenshots/interactive.gif" alt="Zoom, pan and range slider" width="100%"><br><sub>Zoom, pan and range slider</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-streaming"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-streaming/screenshots/streaming.gif" alt="Live dashboard (streaming data)" width="100%"><br><sub>Live dashboard (streaming data)</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-animation"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-animation/screenshots/animation.gif" alt="Animations (GIF, APNG, MP4)" width="100%"><br><sub>Animations (GIF, APNG, MP4)</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-binance"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-binance/screenshots/binance.png" alt="Live Binance BTC dashboard" width="100%"><br><sub>Live Binance BTC dashboard</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-html"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-html/screenshots/html.png" alt="Interactive HTML (hover, crosshair)" width="100%"><br><sub>Interactive HTML (hover, crosshair)</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-wasm"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-wasm/screenshots/wasm.png" alt="In the browser (WebAssembly)" width="100%"><br><sub>In the browser (WebAssembly)</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-egui"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-egui/screenshots/egui.png" alt="Inside an egui app" width="100%"><br><sub>Inside an egui app</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-backends"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-backends/screenshots/backends.png" alt="CPU, multi-core and GPU backends" width="100%"><br><sub>CPU, multi-core and GPU backends</sub></a></td></tr>
</table>

### Lines, points and bars

<table>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-line"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-line/screenshots/line.png" alt="Line styles" width="100%"><br><sub>Line styles</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-markers-steps-smooth"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-markers-steps-smooth/screenshots/markers_steps_smooth.png" alt="Markers, steps and smoothing" width="100%"><br><sub>Markers, steps and smoothing</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-area-fill"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-area-fill/screenshots/area_fill.png" alt="Area and fill between" width="100%"><br><sub>Area and fill between</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-stacked-area"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-stacked-area/screenshots/stacked_area.png" alt="Stacked areas" width="100%"><br><sub>Stacked areas</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-scatter-bubble"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-scatter-bubble/screenshots/scatter_bubble.png" alt="Scatter and bubble charts" width="100%"><br><sub>Scatter and bubble charts</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-bars"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-bars/screenshots/bars.png" alt="Bar charts" width="100%"><br><sub>Bar charts</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-stem-waterfall"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-stem-waterfall/screenshots/stem_waterfall.png" alt="Stem and waterfall charts" width="100%"><br><sub>Stem and waterfall charts</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-errorbars-band"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-errorbars-band/screenshots/errorbars_band.png" alt="Error bars and confidence bands" width="100%"><br><sub>Error bars and confidence bands</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-quiver-function"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-quiver-function/screenshots/quiver_function.png" alt="Vector field and function plots" width="100%"><br><sub>Vector field and function plots</sub></a></td></tr>
</table>

### Statistics

<table>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-histograms"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-histograms/screenshots/histograms.png" alt="Histograms, KDE and ECDF" width="100%"><br><sub>Histograms, KDE and ECDF</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-box-violin"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-box-violin/screenshots/box_violin.png" alt="Box and violin plots" width="100%"><br><sub>Box and violin plots</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-distributions"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-distributions/screenshots/distributions.png" alt="Strip, swarm, ridgeline, Q-Q" width="100%"><br><sub>Strip, swarm, ridgeline, Q-Q</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-facets"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-facets/screenshots/facets.png" alt="Facets (small multiples)" width="100%"><br><sub>Facets (small multiples)</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-facets"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-facets/screenshots/pair_plot.png" alt="Pair plot (scatter matrix)" width="100%"><br><sub>Pair plot (scatter matrix)</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-hist2d-hexbin"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-hist2d-hexbin/screenshots/hist2d_hexbin.png" alt="2D histogram and hexbin" width="100%"><br><sub>2D histogram and hexbin</sub></a></td></tr>
</table>

### Finance and time

<table>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-candlestick"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-candlestick/screenshots/candlestick.png" alt="Candlestick chart" width="100%"><br><sub>Candlestick chart</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-indicators"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-indicators/screenshots/indicators.png" alt="Bollinger, RSI, MACD" width="100%"><br><sub>Bollinger, RSI, MACD</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-indicators"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-indicators/screenshots/heikin_ashi.png" alt="Heikin-Ashi candles" width="100%"><br><sub>Heikin-Ashi candles</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-time-series"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-time-series/screenshots/time_series.png" alt="Time series" width="100%"><br><sub>Time series</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-gantt"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-gantt/screenshots/gantt.png" alt="Gantt chart" width="100%"><br><sub>Gantt chart</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-parallel-calendar"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-parallel-calendar/screenshots/parallel_calendar.png" alt="Parallel coordinates and calendar" width="100%"><br><sub>Parallel coordinates and calendar</sub></a></td></tr>
</table>

### Grids, fields and 3D

<table>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-heatmap"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-heatmap/screenshots/heatmap.png" alt="Heatmap (correlation matrix)" width="100%"><br><sub>Heatmap (correlation matrix)</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-contour"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-contour/screenshots/contour.png" alt="Contour plots" width="100%"><br><sub>Contour plots</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-image"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-image/screenshots/image.png" alt="Images" width="100%"><br><sub>Images</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-three-d"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-three-d/screenshots/three_d.png" alt="3D surface and scatter" width="100%"><br><sub>3D surface and scatter</sub></a></td></tr>
</table>

### Maps

<table>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-maps"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-maps/screenshots/maps.png" alt="Choropleth and flight routes" width="100%"><br><sub>Choropleth and flight routes</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-maps"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-maps/screenshots/projections.png" alt="Map projections" width="100%"><br><sub>Map projections</sub></a></td></tr>
</table>

### Parts, flows and polar

<table>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-pie-donut"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-pie-donut/screenshots/pie_donut.png" alt="Pie and donut charts" width="100%"><br><sub>Pie and donut charts</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-treemap-sunburst"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-treemap-sunburst/screenshots/treemap_sunburst.png" alt="Treemap and sunburst" width="100%"><br><sub>Treemap and sunburst</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-funnel-gauge"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-funnel-gauge/screenshots/funnel_gauge.png" alt="Funnel and gauge" width="100%"><br><sub>Funnel and gauge</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-sankey"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-sankey/screenshots/sankey.png" alt="Sankey diagram" width="100%"><br><sub>Sankey diagram</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-network"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-network/screenshots/network.png" alt="Network graph" width="100%"><br><sub>Network graph</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-polar-radar"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-polar-radar/screenshots/polar_radar.png" alt="Polar and radar charts" width="100%"><br><sub>Polar and radar charts</sub></a></td></tr>
</table>

### Axes, layout, text and style

<table>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-subplots-shared"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-subplots-shared/screenshots/subplots_shared.png" alt="Subplots with shared axes" width="100%"><br><sub>Subplots with shared axes</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-log-scales"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-log-scales/screenshots/log_scales.png" alt="Logarithmic scales" width="100%"><br><sub>Logarithmic scales</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-broken-axis"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-broken-axis/screenshots/broken_axis.png" alt="Broken axes" width="100%"><br><sub>Broken axes</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-formats"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-formats/screenshots/formats.png" alt="Tick formats" width="100%"><br><sub>Tick formats</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-annotations"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-annotations/screenshots/annotations.png" alt="Annotations" width="100%"><br><sub>Annotations</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-math"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-math/screenshots/math.png" alt="Math notation (TeX)" width="100%"><br><sub>Math notation (TeX)</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-international"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-international/screenshots/international.png" alt="Any language and emoji" width="100%"><br><sub>Any language and emoji</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-accessibility"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-accessibility/screenshots/risky.png" alt="Color vision simulation" width="100%"><br><sub>Color vision simulation</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-themes"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-themes/screenshots/theme_light.png" alt="Theme: light" width="100%"><br><sub>Theme: light</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-themes"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-themes/screenshots/theme_dark.png" alt="Theme: dark" width="100%"><br><sub>Theme: dark</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-themes"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-themes/screenshots/theme_ggplot.png" alt="Theme: ggplot" width="100%"><br><sub>Theme: ggplot</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-themes"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-themes/screenshots/theme_seaborn.png" alt="Theme: seaborn" width="100%"><br><sub>Theme: seaborn</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-themes"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-themes/screenshots/theme_minimal.png" alt="Theme: minimal" width="100%"><br><sub>Theme: minimal</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-themes"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-themes/screenshots/theme_publication.png" alt="Theme: publication" width="100%"><br><sub>Theme: publication</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-themes"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-themes/screenshots/theme_solarized.png" alt="Theme: solarized" width="100%"><br><sub>Theme: solarized</sub></a></td></tr>
</table>

## Installation

```toml
[dependencies]
dataviz = "0.15"
# dataviz = { version = "0.15", features = ["pdf", "gpu"] }
# dataviz = { version = "0.15", default-features = false }   # headless / wasm32
```

| Feature | Default | What it adds |
|---|---|---|
| `window` | yes | interactive windows (`minifb`) |
| `pdf` | no | PDF output |
| `gpu` | no | GPU rasterization (`wgpu`) |

Requires Rust 1.89 or newer. On Linux the `window` feature needs the usual
windowing libraries (`libxkbcommon-dev`, `libwayland-dev`).

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## License

MIT; see `LICENSE`. The embedded DejaVu Sans font has its own license
(`assets/DejaVuSans-LICENSE.txt`); the world map is Natural Earth (public
domain).

## Contributing

Bug reports, ideas and pull requests are welcome at
<https://github.com/dataviz-rs/dataviz>.
