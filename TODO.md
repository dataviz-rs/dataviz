# dataviz roadmap: full plotting & data-visualization coverage

New, additive API in `dataviz::plot` (re-exported by `dataviz::prelude`).
The 0.1 API (`dataviz::figure::*`) stays unchanged and supported.

Architecture: every chart draws into a backend-independent `Scene`
(paths, shapes, text, images, clipping); the scene is written as SVG,
rasterized with resvg (anti-aliased PNG/JPEG/WebP/RGBA), converted to PDF,
shown in an interactive window, or drawn in the terminal.

Legend: `[x]` done · `[~]` partial · `[ ]` open

Status: all items implemented in 0.2.0 (see CHANGELOG); `examples/plot_gallery.rs`
renders every chart type and `tests/plot.rs` covers them with edge-case data.

## 1. Foundation
- [x] Scene primitives: path (line/polygon/curves), rect, circle, text (anchor, rotation), image, groups with clip & opacity
- [x] Color: RGBA, hex/named parsing, alpha, lighten/darken, interpolation
- [x] Text measurement with real font metrics (same font for SVG and raster)
- [x] Backends: SVG string/file, raster (PNG, JPEG, WebP, raw RGBA) via resvg, DPI/scale factor
- [x] PDF output (optional `pdf` feature)
- [x] Figure: size, background, title, padding, subplot grid (rows × cols, spans), shared axes
- [x] `prelude` module, builder API, `Result`-returning save functions

## 2. Axes, scales & ticks
- [x] Linear, logarithmic (any base), symmetric-log scales
- [x] Date/time axis (UTC timestamps, automatic second…year ticks, formats)
- [x] Categorical axis (string categories)
- [x] Automatic "nice" limits with margins; manual limits; inverted axes; equal aspect
- [x] Major & minor ticks, grid on/off per axis, custom tick positions/labels
- [x] Tick formatters: auto, fixed decimals, percent, SI prefixes (k, M, G), scientific, custom closure
- [x] Secondary (twin) y-axis
- [x] Axis labels, title, spines on/off, label overlap avoidance (thinning / rotation)

## 3. Styling
- [x] Line width, dash patterns (solid, dashed, dotted, dash-dot, custom), caps/joins, opacity
- [x] Markers: circle, square, diamond, triangle up/down, plus, cross, star, hexagon; size, fill, edge
- [x] Color cycles / categorical palettes (tab10, Set2, Okabe–Ito colour-blind safe, …)
- [x] Colormaps: viridis, plasma, inferno, magma, cividis, turbo, gray, coolwarm, RdBu, …; reversed
- [x] Colorbar
- [x] Themes: default, dark, minimal, ggplot-like, seaborn-like, publication
- [x] Legend: position (corners, outside), columns, frame, markers/lines/patches

## 4. Chart types (cartesian)
- [x] Line (straight, step pre/mid/post, smooth monotone spline)
- [x] Scatter / bubble (per-point size and color, colormap)
- [x] Area / fill-between / stacked area / 100 % stacked
- [x] Bar: vertical & horizontal, grouped, stacked, percent, negative values, value labels
- [x] Histogram: bin rules (fixed, Sturges, Scott, Freedman–Diaconis), density, cumulative, step outline
- [x] 2D histogram & hexbin
- [x] Box plot (with outliers, notches optional) & violin plot (KDE)
- [x] Error bars (x/y, asymmetric) & error bands
- [x] Candlestick & OHLC (financial)
- [x] Heatmap (matrix + colormap + annotations) & image display
- [x] Contour lines & filled contours (marching squares)
- [x] Stem plot, step plot, lollipop
- [x] Waterfall chart
- [x] Function plot (`y = f(x)` sampled adaptively)
- [x] Quiver (vector field)
- [x] Regression / trend line, moving average, KDE density line, ECDF
- [x] Gantt chart (bars on a time axis)
- [x] Sparklines (axis-less mini charts)

## 5. Non-cartesian charts
- [x] Pie & donut (labels, percentages, exploded slices)
- [x] Polar plot (line/scatter in r-θ) & polar bar / rose
- [x] Radar / spider chart
- [x] Gauge
- [x] Treemap (squarified)
- [x] Sunburst
- [x] Sankey diagram
- [x] Funnel chart
- [x] Network graph (force-directed layout)
- [x] Parallel coordinates
- [x] Calendar heatmap
- [x] 3D: surface/wireframe & 3D scatter (projected)

## 6. Annotations
- [x] Text, arrows, horizontal/vertical lines, shaded spans, rectangles
- [~] Data labels on bars (done) and points (via `Axes::text`)

## 7. Data & statistics helpers
- [x] Accept slices, iterators, `(x, y)` pairs, separate columns
- [x] CSV loading
- [x] Statistics: quantiles, KDE, linear/polynomial regression, moving average, ECDF
- [x] Large data: LTTB / min-max decimation for lines

## 8. Interactivity & output targets
- [x] Interactive window: pan, zoom (wheel & box), reset, hover tooltip, crosshair, save key
- [x] Real-time streaming (fixed-size rolling series)
- [x] SVG tooltips (`<title>`) for browsers
- [x] Terminal output (braille/ASCII)
- [x] Window support behind a default `window` feature (WASM / headless builds)

## 9. Quality
- [x] Tests for every chart type (SVG validity, raster checks, edge cases)
- [x] Gallery example rendering every chart type
- [x] README / docs / CHANGELOG
