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


**Pure-Rust plotting and data visualization.** 40+ chart types, publication-quality
output to SVG, PNG, JPEG, WebP and PDF, interactive windows with zoom and hover,
live streaming data and terminal output, without Python, JavaScript or a browser.

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

    fig.save("signals.png")?;   // .svg .png .jpg .webp .bmp, .pdf with the `pdf` feature
    fig.show("Signals")?;       // interactive window: wheel zoom, drag pan, hover
    Ok(())
}
```

## **Chart types** (`dataviz::plot`)

| Category | Charts |
|---|---|
| Lines & areas | line, step, smooth spline, area, fill-between, stacked / 100 % area, function plot, sparkline |
| Points | scatter, colormapped scatter, bubble, 3D scatter |
| Bars | vertical, horizontal, grouped, stacked, percent, overlay, value labels, waterfall, Gantt, lollipop |
| Distributions | histogram (bin rules, density, cumulative, step), KDE, ECDF, box plot (notches), violin, strip (jitter), swarm (beeswarm), ridgeline, Q-Q plot, 2D histogram, hexbin |
| Uncertainty & fits | error bars (x/y, asymmetric), error bands, polynomial regression with R², moving average |
| Grids & fields | heatmap (annotated), correlation matrix, calendar heatmap, image, contour lines, filled contours, quiver, 3D surface/wireframe |
| Finance | candlestick, OHLC, Heikin-Ashi, Bollinger bands, RSI, MACD |
| Layouts | subplots with spans, shared and broken axes, facets (small multiples), pair plot (scatter matrix) |
| Parts of a whole | pie, donut, treemap, sunburst, funnel, gauge |
| Polar | polar line/scatter, radar (spider), rose / wind rose |
| Maps | world map, choropleth (ISO codes or names), points and great-circle routes on a map, GeoJSON shapes; Robinson, Natural Earth, equirectangular and Mercator projections |
| Relations & flows | sankey, network graph (force-directed), parallel coordinates |
| Annotations | text, arrows, reference lines, shaded spans, rectangles, SVG tooltips |

Run `cargo run --release --example plot_gallery -- out/` to render all of them, or see the [gallery](#gallery) below.

## **Gallery**

Every image links to its example in
[dataviz-examples](https://github.com/dataviz-rs/dataviz-examples) (code, `cargo run`, output).

<table>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-line"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-line/screenshots/line.png" alt="Line styles" width="100%"><br><sub>Line styles</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-markers-steps-smooth"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-markers-steps-smooth/screenshots/markers_steps_smooth.png" alt="Markers, steps and smoothing" width="100%"><br><sub>Markers, steps and smoothing</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-area-fill"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-area-fill/screenshots/area_fill.png" alt="Area and fill between" width="100%"><br><sub>Area and fill between</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-stacked-area"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-stacked-area/screenshots/stacked_area.png" alt="Stacked areas" width="100%"><br><sub>Stacked areas</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-scatter-bubble"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-scatter-bubble/screenshots/scatter_bubble.png" alt="Scatter and bubble charts" width="100%"><br><sub>Scatter and bubble charts</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-bars"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-bars/screenshots/bars.png" alt="Bar charts" width="100%"><br><sub>Bar charts</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-histograms"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-histograms/screenshots/histograms.png" alt="Histograms, KDE and ECDF" width="100%"><br><sub>Histograms, KDE and ECDF</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-box-violin"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-box-violin/screenshots/box_violin.png" alt="Box and violin plots" width="100%"><br><sub>Box and violin plots</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-errorbars-band"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-errorbars-band/screenshots/errorbars_band.png" alt="Error bars and confidence bands" width="100%"><br><sub>Error bars and confidence bands</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-heatmap"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-heatmap/screenshots/heatmap.png" alt="Heatmap (correlation matrix)" width="100%"><br><sub>Heatmap (correlation matrix)</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-hist2d-hexbin"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-hist2d-hexbin/screenshots/hist2d_hexbin.png" alt="2D histogram and hexbin" width="100%"><br><sub>2D histogram and hexbin</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-contour"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-contour/screenshots/contour.png" alt="Contour plots" width="100%"><br><sub>Contour plots</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-candlestick"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-candlestick/screenshots/candlestick.png" alt="Candlestick chart" width="100%"><br><sub>Candlestick chart</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-time-series"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-time-series/screenshots/time_series.png" alt="Time series" width="100%"><br><sub>Time series</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-log-scales"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-log-scales/screenshots/log_scales.png" alt="Logarithmic scales" width="100%"><br><sub>Logarithmic scales</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-formats"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-formats/screenshots/formats.png" alt="Tick formats" width="100%"><br><sub>Tick formats</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-stem-waterfall"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-stem-waterfall/screenshots/stem_waterfall.png" alt="Stem and waterfall charts" width="100%"><br><sub>Stem and waterfall charts</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-quiver-function"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-quiver-function/screenshots/quiver_function.png" alt="Vector field and function plots" width="100%"><br><sub>Vector field and function plots</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-gantt"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-gantt/screenshots/gantt.png" alt="Gantt chart" width="100%"><br><sub>Gantt chart</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-pie-donut"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-pie-donut/screenshots/pie_donut.png" alt="Pie and donut charts" width="100%"><br><sub>Pie and donut charts</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-polar-radar"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-polar-radar/screenshots/polar_radar.png" alt="Polar, radar and rose charts" width="100%"><br><sub>Polar, radar and rose charts</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-treemap-sunburst"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-treemap-sunburst/screenshots/treemap_sunburst.png" alt="Treemap and sunburst" width="100%"><br><sub>Treemap and sunburst</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-sankey"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-sankey/screenshots/sankey.png" alt="Sankey diagram" width="100%"><br><sub>Sankey diagram</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-funnel-gauge"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-funnel-gauge/screenshots/funnel_gauge.png" alt="Funnel and gauge" width="100%"><br><sub>Funnel and gauge</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-network"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-network/screenshots/network.png" alt="Network graph" width="100%"><br><sub>Network graph</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-parallel-calendar"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-parallel-calendar/screenshots/parallel_calendar.png" alt="Parallel coordinates and calendar heatmap" width="100%"><br><sub>Parallel coordinates and calendar heatmap</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-three-d"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-three-d/screenshots/three_d.png" alt="3D surface and trajectory" width="100%"><br><sub>3D surface and trajectory</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-subplots-shared"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-subplots-shared/screenshots/subplots_shared.png" alt="Subplots with a shared x-axis" width="100%"><br><sub>Subplots with a shared x-axis</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-annotations"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-annotations/screenshots/annotations.png" alt="Annotations" width="100%"><br><sub>Annotations</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-image"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-image/screenshots/image.png" alt="Image display" width="100%"><br><sub>Image display</sub></a></td></tr>
<tr><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-themes"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-themes/screenshots/theme_dark.png" alt="Themes (dark)" width="100%"><br><sub>Themes (dark)</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-themes"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-themes/screenshots/theme_ggplot.png" alt="Themes (ggplot)" width="100%"><br><sub>Themes (ggplot)</sub></a></td><td width="33%" align="center"><a href="https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-interactive"><img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-interactive/screenshots/interactive.png" alt="Interactive window" width="100%"><br><sub>Interactive window</sub></a></td></tr>
</table>

## **Highlights**

- **Axes**: linear, log, symlog, date/time and categorical scales; broken axes (`Axis::break_at`); secondary y-axis; subplots with spans and shared axes; nice ticks with minor ticks; tick formats (percent, SI, scientific, time, custom); automatic label thinning and rotation.
- **Styling**: 7 themes (light, dark, minimal, ggplot, seaborn, publication, solarized), palettes including the colour-blind safe Okabe–Ito, 14 colormaps with colorbars, 10 marker shapes, dash styles, legends in 10 positions, any TTF/OTF font.
- **Output**: SVG, PNG, JPEG, WebP, BMP, PDF (`pdf` feature), raw RGBA pixels, high-DPI scaling, terminal (`fig.print()`).
- **Finance**: candlestick, OHLC and Heikin-Ashi charts, moving averages, Bollinger bands, RSI and MACD; aligned stacked panels with a linked crosshair.
- **Statistics**: histograms, KDE, ECDF, box, violin, strip, swarm, ridgeline and Q-Q plots, regression lines; `fig.facet_wrap(..)` small multiples and `fig.pair_plot(..)` scatter matrices.
- **Any language**: missing characters fall back to system fonts (CJK, Thai, Indic, symbols), Arabic and Hebrew are laid out right to left and shaped, color emoji are drawn; Latin text keeps a fast path.
- **Math notation**: `$...$` in any text is typeset (TeX subset: `x^2`, `x_i`, `\frac{a}{b}`, `\sqrt{x}`, Greek letters, `\sum`, `\int`, accents, `\mathrm{..}`, …), e.g. `ax.y_label(r"$\sigma^2$ [$\mathrm{m^2\,s^{-2}}$]")`.
- **Animations**: `fig.record("wave.gif", 30.0, 120, |fig, i| ..)` or `Recorder` for GIF, lossless animated PNG and (with `ffmpeg` installed) MP4/WebM; only changed regions are stored, so recording is fast and files are small. In windows `V` records a GIF.
- **Interactive**: `fig.show(..)` (zoom, pan, box zoom, reset, hover, save, crosshair with axis values, linked cursor and zoom across plots sharing x, click a legend row to hide/show a series, range slider, Shift+drag to measure, Ctrl+drag to select, copy to the clipboard) and `fig.show_live(..)` for streaming data with rolling windows (`Line::push`, `Line::keep_since`); zoomed or panned plots keep their view while data arrives with the y-axis auto-scaled to the visible data (`A` toggles auto scale, `R` follows the data again); `plot::data::CsvLog` records the stream to CSV. See the [live dashboard](https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-streaming) and [Binance](https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-binance) examples.
- **Data**: any numeric slice/vector/iterator, CSV/TSV with dates (`plot::data::Table`), statistics helpers, custom chart types via the `Series` trait.
- **Features**: `window` (default, interactive windows via `minifb`), `pdf` (PDF output), `gpu` (GPU rasterization via `wgpu`). `default-features = false` builds headless.

## **Performance**

A built-in rasterizer draws the scene straight into the output buffer
(embedded DejaVu Sans, no system font scan, no copies of the data passed to
a series). Dense lines are drawn as pixel-column envelopes, dense opaque
scatter plots as an occupancy grid, so time and memory stay flat as the data
grows. Measured against plotters, ruviz, kuva and charts-rs (800×600 PNG,
Linux x86-64, single thread; `dataviz-bench`):

| Chart | dataviz | plotters | ruviz | kuva | charts-rs |
|---|---:|---:|---:|---:|---:|
| line, 1 000 points | **1.30 ms** | 1.41 ms | 3.58 ms | 4.39 ms | 11.4 ms |
| bar, 20 bars | **0.69 ms** | 0.71 ms | 4.41 ms | 3.04 ms | 1.79 ms |
| scatter, 100 000 points | **8.6 ms** | 16.6 ms | 16.7 ms | 14.7 ms | 657 ms |
| line, 10 M points | **94 ms** | 426 ms | 316 ms | 2.38 s | fails |
| scatter, 10 M points | **189 ms** | 1.27 s | 1.23 s | 1.36 s | fails |
| histogram, 100 M values | **241 ms** | 277 ms | 3.62 s | 1.53 s | 1.15 s |
| memory above the data (10 M points) | **+7–8 MB** | +11–314 MB | +199–389 MB | +177–788 MB | +161 MB |

### Backends: CPU, multi-core CPU, GPU

```rust
use dataviz::prelude::*;
let mut fig = Figure::new(800, 600);
fig.backend(Backend::Auto);                // default: CPU, all cores from 1 megapixel on
fig.backend(Backend::Cpu { threads: 1 });  // least CPU time on the CPU, lowest memory
fig.backend(Backend::Cpu { threads: 0 });  // all cores: lowest latency without a GPU
fig.backend(Backend::Gpu);                 // `gpu` feature: Vulkan / Metal / DX12 / OpenGL
```

Real-time line chart, rolling 100 000 points, one new point and a full
redraw per frame: latency p50 / CPU time per frame / peak RSS (RTX 5070 Ti
laptop GPU, 20-core CPU):

| Resolution | CPU 1 thread | CPU all cores | GPU | plotters |
|---|---|---|---|---|
| 800×600 | 2.1 ms / 2.1 ms / 11 MB | 1.9 ms / 4.7 ms / 13 MB | 1.5 ms / 1.3 ms / 241 MB | 5.5 ms / 5.5 ms / 16 MB |
| 1920×1080 | 5.5 ms / 5.4 ms / 18 MB | 3.3 ms / 14.8 ms / 20 MB | 2.5 ms / 2.3 ms / 248 MB | 8.7 ms / 8.7 ms / 21 MB |

The GPU backend needs the least CPU time and is fastest for large images
and many markers; it costs ~0.3 s of start-up (more if a laptop's discrete
GPU has to wake up) and ~230 MB of driver memory, so the default stays on
the CPU. Output of all backends matches within anti-aliasing differences
(`Backend::gpu_available()` tells whether a GPU can be used). Try
`cargo run --release --example plot_backends --features gpu`.

## **Installation**
```toml
[dependencies]
dataviz = "0.4"
# dataviz = { version = "0.4", features = ["pdf"] }
```

Requires Rust 1.89 or newer. On Linux the `window` feature needs the usual
windowing libraries (`libxkbcommon-dev`, `libwayland-dev`).

## **Examples**

[dataviz-examples](https://github.com/dataviz-rs/dataviz-examples) has a
runnable crate (code and output) for every chart type in the gallery above,
always on the latest dataviz release. In this repository,
`cargo run --release --example plot_gallery -- out/` renders all charts and
`cargo run --release --example plot_interactive` opens the interactive and
live windows.

## **Changelog**  
See [CHANGELOG.md](CHANGELOG.md).

## **License**  
This project is licensed under the **MIT**. See the `LICENSE` file for details.

---

## **Contributing**  
We welcome contributions to make **DataViz** even better!
- Visit DataViz repository: https://github.com/dataviz-rs/dataviz
- Report bugs and suggest features through GitHub Issues.  
- Submit pull requests for enhancements or fixes.

Let’s make data visualization in Rust easy and accessible!

---
