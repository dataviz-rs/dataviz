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
<a href="https://pypi.org/project/dataviz-rs/">
    <img style="display: inline!important" src="https://img.shields.io/pypi/v/dataviz-rs.svg"></img>
</a>
<a href="https://www.npmjs.com/package/dataviz-rs">
    <img style="display: inline!important" src="https://img.shields.io/npm/v/dataviz-rs.svg"></img>
</a>
<a href="https://github.com/dataviz-rs/dataviz/stargazers">
    <img style="display: inline!important" src="https://img.shields.io/github/stars/dataviz-rs/dataviz?style=social"></img>
</a>
<a href="https://github.com/dataviz-rs/dataviz/actions/workflows/rust.yml">
    <img style="display: inline!important" src="https://github.com/dataviz-rs/dataviz/actions/workflows/rust.yml/badge.svg"></img>
</a>

**Fast plotting for Rust, Python, JavaScript and C/C++: 120+ chart types,
interactive windows, live data, 3D, maps and a grammar of graphics.
Renders charts 8–50× faster than matplotlib in [these benchmarks](#performance)
(with a much smaller API: see [differences](#differences-from-matplotlib)),
and faster than the other Rust plotting libraries in nearly every
measurement.**

```rust ignore
let mut fig = Figure::new(900, 500);
fig.axes().line(&x, &y).label("signal");
fig.save("signal.png")?;          // or fig.show() for a live, zoomable window
```

```python
import dataviz.pyplot as plt      # the common matplotlib calls, not all of them
```

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

**[Guide and chart catalog](https://dataviz-rs.github.io/dataviz/)** ·
**[Live demo in the browser](https://dataviz-rs.github.io/dataviz/demo/)** ·
[API docs](https://docs.rs/dataviz) · [Examples](https://github.com/dataviz-rs/dataviz-examples)

*Built with heavy AI assistance (Claude Code); see [how it's made](#how-its-made).*

## Why dataviz

- **Fast and lean.** Its own rasterizer draws straight into the output
  buffer: a 10-million-point line renders in about 0.1 s with a few MB of
  extra memory, a live chart redraws in 1–2 ms. 4–50× faster than
  matplotlib for typical charts (warm runs, imports excluded) (2× for a 100-million-value histogram,
  78× for animations), and faster and lighter than plotters, ruviz, kuva and
  charts-rs in nearly every measurement ([numbers below](#performance)).
  Optional multi-core and GPU backends. An idle window uses no CPU (it
  only sends frames that changed).
- **Everything in one crate.** 120+ chart types — lines, bars, statistics,
  heatmaps, contours, 3D, maps, sankey, networks — plus ready-made charts
  for **data scientists** (ROC/PR curves, confusion matrices, regression
  bands, density contours, dendrograms), **biologists and clinicians**
  (DNA sequence logos, gene tracks, Manhattan, volcano, clustermaps,
  single-cell UMAPs, dose-response, Kaplan-Meier, forest plots),
  **engineers** (FFT spectra, spectrograms, Bode, Nyquist, Smith charts)
  and **traders** (candlesticks,
  Ichimoku, VWAP, RSI, MACD, volume profile, order book depth, drawdown,
  Renko), with themes, subplots, shared and broken axes.
- **Interactive and live.** `fig.show()` opens a window with zoom, pan, a
  crosshair with axis values, hover readouts, legend toggling, a range
  slider, measurement, selection and linked brushing across plots; `fig.show_live()` streams data with
  rolling windows and records it to CSV or a GIF.
- **Every output.** PNG, SVG, PDF, JPEG, WebP, interactive HTML (one file,
  no internet needed: hover, zoom, pan, rotating 3D), animated GIF/PNG/WebP and MP4, the terminal, raw pixels
  for your own GUI, and the browser via WebAssembly.
- **Text done right.** TeX math (`$\sigma^2 = \frac{1}{N}\sum x_i^2$`) in
  every label, any language (CJK, Arabic and Hebrew right to left, Hindi,
  Thai) and color emoji.
- **Accessible.** Generated text descriptions for screen readers, color
  blindness simulation and warnings for colors that look alike.
- **From any language.** Rust, Python (with a matplotlib-compatible
  `dataviz.pyplot` and an Altair-style `dv.plot`), JavaScript/npm (React,
  Svelte), C and C++ (CMake; prebuilt for Linux, macOS, Windows, iOS and
  Android), and Claude through the MCP server. A grammar of graphics (data,
  mark, encodings, facets) works in all of them.
- **Pure Rust, few dependencies.** No Python, JavaScript, browser,
  Cairo or system fonts needed (a font is embedded); builds headless and
  for `wasm32`.

## Quick start

```toml
[dependencies]
dataviz = "1"
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

## Your data, as it is

Any iterable of numbers works — `Vec`, slices, ranges, iterators and arrays
from other crates, without conversion or features:

```rust ignore
ax.line(&x, &y);                                   // Vec<f64>, &[f32], i64, …
ax.line(&array1, &array1.mapv(f64::sin));          // ndarray
ax.line(&times, polars_df.column("price")?.f64()?); // Polars (nulls become gaps)
ax.scatter(arrow_x.values(), &arrow_y);            // Arrow Float64Array
let t = dataviz::plot::data::Table::read("data.csv")?;   // CSV/TSV, quoted fields
ax.line(t.time_column("date").unwrap(), t.column("close").unwrap());
```

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
| 3D: drag, wheel, `X`/`Y`/`Z` | turn the scene, zoom, rotate 15° around that axis (Shift = reverse) |
| `T`, `L`, `F`, Backspace | draw a trend line, a price level, a Fibonacci retracement; take back the last drawing |

Live charts keep the view you zoomed to while data arrives (y follows the
visible data), stream with `Line::push` / `keep_last` / `keep_since`, and
`plot::data::CsvLog` records the stream to CSV. The same zoom, pan, hover
and click work in your own GUI or web page through
`fig.zoom_at`, `pan_by`, `hover`, `click` ([egui](https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-egui),
[WebAssembly](https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-wasm)).

### Grammar of graphics

Charts can also be described the way Vega-Lite, ggplot2 and Altair do it:
data, a mark (point, line, area, bar, rect, boxplot) and encodings. The
color groups, legends, aggregation, histograms, stacking and small
multiples follow from the encodings. This works from Rust, from JSON
specs, from Python, JavaScript and C, and through the MCP server:

```rust ignore
use dataviz::plot::grammar::{Chart, Data, Field, Aggregate, Mark};
let data = Data::new().text("species", &species).num("mass", &mass).text("island", &island);
Chart::new(data, Mark::Bar)
    .x("island")
    .y(Field::new("mass").aggregate(Aggregate::Mean))
    .color("species")
    .draw(&mut fig)?;
```

```json
{"data": [{"species": "Adelie", "mass": 3750, "island": "Biscoe"}, …],
 "mark": "point", "encoding": {"x": "flipper", "y": "mass", "color": "species", "facet": "island"}}
```

### Chart types

| Category | Charts |
|---|---|
| Lines & areas | line, step, smooth spline, area, fill-between, stacked / 100 % area, streamgraph, horizon chart, function plot, sparkline |
| Points | scatter, colormapped scatter, bubble |
| Bars | vertical, horizontal, grouped, stacked, percent, overlay, value labels, waterfall, Gantt, lollipop, bullet, dumbbell, Pareto |
| Distributions | histogram (bin rules, density, cumulative, step), KDE, ECDF, box (notches), violin, raincloud, strip, swarm, ridgeline, Q-Q, 2D histogram, hexbin |
| Uncertainty & fits | error bars, error bands, polynomial regression with R², moving averages |
| Machine learning | ROC curve (AUC), precision-recall curve, confusion matrix, SHAP summary, calibration curve, learning curve, cumulative gain and lift, regression with confidence band, LOESS, 2D density contours, dendrogram, UpSet plot |
| Biology & medicine | 3D DNA double helix, molecules from PDB/XYZ (ball-and-stick, sticks, space-filling, backbone, cartoon), Ramachandran plot, isosurfaces of volumes, 3D cells and organoids, 3D UMAP, Circos, Hi-C contact map, coverage track, phylogenetic trees (Newick; rectangular, circular), oncoprint, mutation lollipop, DNA/protein sequence logo, gene track, Manhattan plot, p-value Q-Q, volcano plot, MA plot, clustermap, single-cell embedding (UMAP/t-SNE), dose-response (IC50), Kaplan-Meier survival, forest plot |
| Signals & control | FFT spectrum, spectrogram, 3D waterfall, Bode plot, Nyquist plot, Smith chart, streamlines, SPC control chart |
| Finance & trading | candlestick, OHLC, Heikin-Ashi, Renko, footprint (order flow), market profile (TPO), liquidity heatmap, option payoff (Black–Scholes), implied volatility surface, Bollinger bands, Ichimoku cloud, VWAP, RSI, MACD, Stochastic, ATR, volume profile, order book depth, drawdown, trading-day axis |
| Grids & fields | heatmap, correlation matrix, calendar heatmap, image, contour lines and fills, quiver |
| 3D | surface, wireframe, scatter, line, spheres, meshes, ellipsoids, isosurfaces, vectors, bars, text; one shared scene per plot with depth sorting, lighting, perspective and free rotation around x, y and z |
| Maps | built-in world map and the states/provinces of every country, choropleth by ISO code or name, points, great-circle routes and flow maps, hexbin density maps, web-map tile basemaps (OpenStreetMap), GeoJSON shapes; Robinson, Natural Earth, Equal Earth, Mollweide, equirectangular, Mercator, orthographic globe, Albers and Lambert conic, stereographic, azimuthal equal-area |
| Parts of a whole | pie, donut, treemap, sunburst, waffle, marimekko (mosaic), funnel, gauge |
| Polar | polar line/scatter, radar, rose / wind rose |
| Relations & flows | sankey, chord diagram, network graph (force-directed), parallel coordinates, slope chart, bump chart, ternary diagram |
| Layouts | subplots with spans, shared axes, broken axes, secondary y-axis, facets (small multiples), pair plot, aligned panels |
| Annotations | text, arrows, reference lines, shaded spans, rectangles |

### For data scientists

```rust ignore
ax.roc_curve("gradient boosting", &labels, &scores);    // legend shows the AUC
ax.pr_curve("gradient boosting", &labels, &scores);     // and the average precision
ax.confusion_matrix(["cat", "dog", "bird"], &actual, &predicted, true);
ax.regplot(&x, &y);                                     // fit, 95 % band and R²
ax.density_contour(&x, &y);                             // 2D KDE, millions of points
ax.dendrogram(&names, &features);                       // hierarchical clustering
ax.shap_summary(&features, &shap, &values, 10);         // what drives the model
ax.calibration_curve("model", &labels, &probs, 10);     // Brier score in the legend
ax.learning_curve(&sizes, &train, &validation);
ax.gain_curve("model", &labels, &scores);               // and lift_curve
ax.raincloud([("A", &a), ("B", &b)]);
ax.upset(&sets, 12);                                    // set intersections
```

### For biologists and clinicians

```rust ignore
ax.sequence_logo(&aligned_sequences);                   // DNA, RNA or protein motif
ax.gene_track(&genes);                                  // exons, introns, strand
ax.manhattan(&chromosome, &position, &p_values);        // GWAS
ax.volcano(&genes, &log2_fc, &p_values, 1.0, 1e-3, 10); // top 10 genes named
fig.clustermap(&genes, &samples, &expression);          // clustered heatmap
ax.embedding(&umap_x, &umap_y, &cell_types);            // single-cell clusters
ax.dose_response(&concentration, &viability);           // 4PL fit and IC50
ax.kaplan_meier([("control", t1, e1), ("treated", t2, e2)]); // log-rank p
ax.forest_plot(&studies, &hr, &low, &high, 1.0);        // meta-analysis
ax.circos(&chromosomes).track(&chr, &pos, &cn).links(&sv);
ax.hic(matrix, start, bin).domains(&tads);              // contact map as a triangle
ax.coverage(&pos, &depth).min_depth(20.0);
ax.phylo_tree(newick);                                  // or phylo_tree_circular
ax.oncoprint(&genes, samples, &alterations);
ax.mutation_lollipop(393.0, &domains, &mutations);
ax.molecule(Molecule::from_pdb(&pdb)).style(MolStyle::Cartoon);
ax.ramachandran(&phi, &psi);                            // from molecule.phi_psi()
ax.isosurface(&volume, [nx, ny, nz], bounds, level);    // microscopy, CT, densities
```

### 3D: rotate around x, y and z

<img src="https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main/plot-biology-3d/screenshots/rotate.gif" alt="DNA and a cell turning around the z, x and y axes" width="100%">

```rust ignore
ax.dna_helix("ATGCGTATAAAAGCC").highlight(5..11, "#ffd400", Some("TATA box"));
ax.molecule(Molecule::from_pdb(&pdb)).style(MolStyle::Backbone).coloring(MolColor::Rainbow);
ax.ellipsoid([0.0, 0.0, 0.0], [10.0, 8.0, 6.0]).opacity(0.16);   // a cell membrane
ax.spheres3d(&x, &y, &z, [1.0]).values(&expression, Colormap::Viridis);
ax.embedding3d(&umap1, &umap2, &umap3, &cell_types);
ax.surface_fn(|x, y| (x * y).sin(), (-3.0, 3.0), (-3.0, 3.0), 60);
ax.rotate(Axis3::X, 30.0).roll(10.0).perspective(0.4);       // or ax.camera(azimuth, elevation)
```

All 3D series of a plot share one scene, sorted by depth, lit from the
upper left and optionally faded with distance (`depth_cue`). The camera is
a rotation matrix, so it turns freely around any axis. In a window: drag
to turn, wheel to zoom, `X`/`Y`/`Z` rotate by 15° (Shift+key the other
way), `R` resets, hover names the atom, base pair or point. A frame of the
1BNA DNA structure (486 atoms, ball-and-stick, shaded) takes about 8 ms on
one core, a 10 000-point 3D scatter 3 ms; a turning 90-frame GIF of a
surface 0.9 s (3D benchmarks in the Performance section).

### For engineers

```rust ignore
use dataviz::plot::series::signal::{freq_response, logspace};
ax.spectrum(&signal, 8000.0);                           // FFT amplitude spectrum
ax.spectrogram(&signal, 8000.0, 512, 64);               // STFT power in dB
let w = logspace(0.01, 100.0, 400);
fig.bode("plant", &w, &freq_response(&[1.0], &[1.0, 0.4, 1.0], &w));
ax.nyquist(&freq_response(&[1.0], &[1.0, 0.4, 1.0], &w));
ax.smith(&impedances);                                  // normalized z on a Smith chart
ax.streamplot(|x, y| (u(x, y), v(x, y)), (-3.0, 3.0), (-2.0, 2.0), 28);
ax.ternary(&a, &b, &c).names("sand", "silt", "clay");
ax.waterfall3d(&freq, &time, spectra);
ax.control_chart(&measurements);                        // ±3σ, Western Electric rules
ax.pareto(&causes, &counts);
ax.quiver3d(points, vectors);
```

### For traders

```rust ignore
let x = ax.trading_axis(&times, "%d %b");               // no weekend gaps
ax.candlestick(&x, &open, &high, &low, &close);
ax.ichimoku(&x, &high, &low, &close);
ax.vwap(&x, &high, &low, &close, &volume);
profile.volume_profile(&close, &volume, 24);            // POC and value area
osc.stochastic(&x, &high, &low, &close, 14, 3);
book.depth_chart(&bids, &asks);
dd.drawdown(&x, &equity);                               // max drawdown in the legend
heat.liquidity_heatmap((t0, t1), (p0, p1), &depth);     // order book over time
of.footprint(bars, 0.25);                               // bid × ask per level
tpo.market_profile(&ranges, 1.0);                       // POC and value area
opt.option_payoff(&legs, (80.0, 120.0), 30.0, 0.22, 0.03);
vol.volatility_surface(&strikes, &maturities, &iv);
```

### Business and editorial

```rust ignore
ax.bullet(&kpis, &values, &targets, &ranges);
ax.dumbbell(&names, &before, &after);
ax.slope(&names, &before, &after, "2020", "2025");
ax.bump(&teams, &seasons, &ranks);
ax.waffle(&parts, &shares);
ax.marimekko(&columns, &rows, &values);
ax.chord(&groups, &flows);
ax.streamgraph(&x, &layers);
ax.horizon(&x, &rows);
```

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
ax.region_map("TUR", [("TR-34", 15.7), ("Ankara", 5.8)]);   // states/provinces (feature `regions`)
ax.geo_flows(&flows, &names);                   // flow map, width by value
ax.geo_hexbin(&lon, &lat);                      // density of millions of points
ax.basemap(lon, lat, 12, tile_dir("tiles"), "© OpenStreetMap contributors");
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
fig.save("chart.html")?;           // interactive page; 3D plots turn with the mouse
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

{{PERFORMANCE}}

### Beyond Rust plotting libraries

Each of these was measured on the same machine, against the most common
tool for the job:

| Task | dataviz | Compared with |
|---|---:|---:|
| Density image of 100 M points (Datashader style), Rust / Python | **70 ms / 81 ms** | Datashader 372 ms |
| Density image of 10 M points, Python | **39 ms** | Datashader 73 ms |
| 4-panel script through `dataviz.pyplot` (lines, scatter, log histogram, bars) | **3.3 ms** | matplotlib 94 ms |
| 1 M point line through `dataviz.pyplot` | **17 ms** | matplotlib 190 ms |
| 10 M rows from Polars / pyarrow into a chart | **59 ms** | 508 ms / 2.86 s via NumPy |
| 1 M point chart built in JavaScript (npm, Float64Array) | **3 ms** | 214 ms in 0.18 (JSON) |
| 1 M point chart built in C/C++ with binary arrays | **4–7 ms** | 166 ms through JSON text |
| 3D point cloud, 1 M colormapped points, CPU | **73 ms** | 3.3 s in 0.18 |
| Idle chart window, CPU use | **0.5 %** | 7 % in 0.18 |

The `pyplot` rows are the median of warm runs in one process, with the
imports excluded and both libraries saving the same PNG
([script](https://github.com/dataviz-rs/dataviz-examples/blob/main/benchmarks/pyplot_bench.py)). The first call costs more: 7 ms against matplotlib's
326 ms, and the imports take about 60 ms against 230 ms. matplotlib does
far more than these scripts use, so these numbers compare the common
case, not the whole library.

## Gallery

Every image links to its example in
[dataviz-examples](https://github.com/dataviz-rs/dataviz-examples) (code,
`cargo run`, output).

{{GALLERY}}

## JavaScript

The same renderer in WebAssembly ([`wasm/npm`](wasm/npm), npm package
`dataviz-rs`): charts from a JSON spec in the browser, Node, Deno and Bun;
SVG, PNG, HTML or a live, interactive canvas.

```js
import { init, Chart } from "dataviz-rs";
await init();
new Chart({ type: "line", x: [0, 1, 2], y: [1, 3, 2] }).mount(canvas);
```

## C and C++

A C library with a header-only C++17 wrapper ([`capi/`](capi)) for Linux,
macOS and Windows, plus static libraries for iOS and Android. It covers
every chart type and all outputs, raw pixels for your own UI,
interactive windows and live data. Large arrays are passed as pointers,
never as JSON text. With CMake:

```cmake
add_subdirectory(dataviz/capi dataviz)
target_link_libraries(app PRIVATE dataviz::dataviz)
```

```cpp
#include "dataviz.hpp"
dataviz::Figure fig(900, 500);
fig.axes().line(t, v, dataviz::Style().label("signal"));
fig.save("signal.png");
```

## Python

The same renderer for Python ([`python/`](python)): NumPy, pandas and
Polars data, Jupyter display, 5–22× faster than matplotlib. Wheels for
Linux, macOS and Windows (Python 3.9+) are on
[PyPI](https://pypi.org/project/dataviz-rs/):

```sh
pip install dataviz-rs        # imported as `dataviz`
```

```python
import numpy as np, dataviz as dv
x = np.linspace(0, 10, 100_000)
fig = dv.Figure(900, 500, title="Signal")
fig.axes().line(x, np.sin(x), label="sin").set(x_label="t [s]")
fig.save("signal.png")
```

## Differences from matplotlib

`dataviz.pyplot` covers the common calls of matplotlib's pyplot and `Axes`
API (listed in the [Python README](python#matplotlib-code-unchanged)), not
all of matplotlib. Keep using matplotlib when you need:

- **Calls that are missing:** `annotate`, `errorbar`, `contour`, `step`,
  `stackplot`, `hexbin`, `colorbar`, `twinx`, `set_xticks`,
  `tick_params`, and the artist, patch and transform APIs. (`dv.Figure`
  has contours, through `chart("contour", …)`, and colorbars, through
  `colorbar=` on heatmaps and colored scatters, but under its own names.)
- **Fine control:** `rcParams`, tick locators and formatters, custom
  projections, GridSpec and `sharex`/`sharey` in `subplots`.
- **Every argument honored:** unknown keyword arguments are *ignored*,
  not rejected, so a script keeps running but may look different (for
  example `hist(bins=[…edges…])`, `bar(width=…)` or `savefig(dpi=…)`).
  The docstrings say what each call supports.
- **Embedding in Qt, Tk or GTK applications** through matplotlib's
  backends, and the many libraries built on matplotlib (seaborn, pandas
  `.plot()`).

## Use it from Claude

The [Claude Code plugin](claude-plugin) gives Claude all of these charts:
it draws from your data, CSV/JSON files and PDB structures, looks at the
image and refines it.

```text
/plugin marketplace add dataviz-rs/dataviz
/plugin install dataviz@dataviz
```

Then ask: *"plot `bench.csv` and compare the branches"*, *"show 1UBQ as a
cartoon with its Ramachandran plot"*, or use `/dataviz:plot` and
`/dataviz:molecule`. Other MCP clients use the server directly:
`cargo install dataviz-mcp` ([dataviz-mcp](mcp)).

## Installation

```toml
[dependencies]
dataviz = "1"
# dataviz = { version = "1", features = ["pdf", "gpu"] }
# dataviz = { version = "1", default-features = false }   # headless / wasm32
```

| Feature | Default | What it adds |
|---|---|---|
| `window` | yes | interactive windows (`minifb`) |
| `pdf` | no | PDF output |
| `gpu` | no | GPU rasterization (`wgpu`) |

Requires Rust 1.89 or newer. On Linux the `window` feature needs the usual
windowing libraries (`libxkbcommon-dev`, `libwayland-dev`).

## Stability

dataviz follows [semantic versioning](https://semver.org) from 1.0 on:
code written for 1.0 keeps compiling and working with every 1.x release.

- **Covered:**
  - the public Rust API, with its cargo features (they are kept);
  - the JSON spec format (fields keep their meaning; new fields and chart
    types are added);
  - the C API in `dataviz.h` (functions keep their signatures; new ones
    are added).
- **New things come in minor versions (1.1, 1.2, …).** Enums that grow
  (chart kinds, projections, colormaps, markers, …) are
  `#[non_exhaustive]`, so a match on them needs a `_` arm.
- **Fixes come in patch versions.**
- **Not covered:**
  - the exact pixels of a rendering (anti-aliasing, layout and tick
    choices can improve);
  - items marked `#[doc(hidden)]`;
  - performance characteristics.
- **Minimum Rust version:** raised only in minor versions, and noted in
  the changelog.

The Python (`dataviz-rs` on PyPI) and npm (`dataviz-rs`) packages have
their own versions.

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## License

MIT; see `LICENSE`. The embedded DejaVu Sans font has its own license
(`assets/DejaVuSans-LICENSE.txt`); the world map is Natural Earth (public
domain).

## How it's made

dataviz is developed with heavy AI assistance (Claude Code). The author
decides the design and the scope, reviews the changes and runs the tests,
fuzzing and benchmarks; the benchmark code is public so the numbers can be
checked. Bug reports about anything that looks wrong are very welcome.

## Contributing

Bug reports, ideas and pull requests are welcome at
<https://github.com/dataviz-rs/dataviz>. Issues labelled
[good first issue](https://github.com/dataviz-rs/dataviz/labels/good%20first%20issue)
are a good place to start.

If dataviz saves you time, a ⭐ on GitHub helps others find it.
