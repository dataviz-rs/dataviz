# dataviz roadmap: full plotting & data-visualization coverage

New, additive API in `dataviz::plot` (re-exported by `dataviz::prelude`).
The 0.1 API (`dataviz::figure::*`) stays unchanged and supported.

Architecture: every chart draws into a backend-independent `Scene`
(paths, shapes, text, images, clipping); the scene is written as SVG,
rasterized with resvg (anti-aliased PNG/JPEG/WebP/RGBA), converted to PDF,
shown in an interactive window, or drawn in the terminal.

Legend: `[x]` done · `[~]` partial · `[ ]` open

## 1. Foundation
- [ ] Scene primitives: path (line/polygon/curves), rect, circle, text (anchor, rotation), image, groups with clip & opacity
- [ ] Color: RGBA, hex/named parsing, alpha, lighten/darken, interpolation
- [ ] Text measurement with real font metrics (same font for SVG and raster)
- [ ] Backends: SVG string/file, raster (PNG, JPEG, WebP, raw RGBA) via resvg, DPI/scale factor
- [ ] PDF output (optional `pdf` feature)
- [ ] Figure: size, background, title, padding, subplot grid (rows × cols, spans), shared axes
- [ ] `prelude` module, builder API, `Result`-returning save functions

## 2. Axes, scales & ticks
- [ ] Linear, logarithmic (any base), symmetric-log scales
- [ ] Date/time axis (UTC timestamps, automatic second…year ticks, formats)
- [ ] Categorical axis (string categories)
- [ ] Automatic "nice" limits with margins; manual limits; inverted axes; equal aspect
- [ ] Major & minor ticks, grid on/off per axis, custom tick positions/labels
- [ ] Tick formatters: auto, fixed decimals, percent, SI prefixes (k, M, G), scientific, custom closure
- [ ] Secondary (twin) y-axis
- [ ] Axis labels, title, spines on/off, label overlap avoidance (thinning / rotation)

## 3. Styling
- [ ] Line width, dash patterns (solid, dashed, dotted, dash-dot, custom), caps/joins, opacity
- [ ] Markers: circle, square, diamond, triangle up/down, plus, cross, star, hexagon; size, fill, edge
- [ ] Color cycles / categorical palettes (tab10, Set2, Okabe–Ito colour-blind safe, …)
- [ ] Colormaps: viridis, plasma, inferno, magma, cividis, turbo, gray, coolwarm, RdBu, …; reversed
- [ ] Colorbar
- [ ] Themes: default, dark, minimal, ggplot-like, seaborn-like, publication
- [ ] Legend: position (corners, outside), columns, frame, markers/lines/patches

## 4. Chart types (cartesian)
- [ ] Line (straight, step pre/mid/post, smooth monotone spline)
- [ ] Scatter / bubble (per-point size and color, colormap)
- [ ] Area / fill-between / stacked area / 100 % stacked
- [ ] Bar: vertical & horizontal, grouped, stacked, percent, negative values, value labels
- [ ] Histogram: bin rules (fixed, Sturges, Scott, Freedman–Diaconis), density, cumulative, step outline
- [ ] 2D histogram & hexbin
- [ ] Box plot (with outliers, notches optional) & violin plot (KDE)
- [ ] Error bars (x/y, asymmetric) & error bands
- [ ] Candlestick & OHLC (financial)
- [ ] Heatmap (matrix + colormap + annotations) & image display
- [ ] Contour lines & filled contours (marching squares)
- [ ] Stem plot, step plot, lollipop
- [ ] Waterfall chart
- [ ] Function plot (`y = f(x)` sampled adaptively)
- [ ] Quiver (vector field)
- [ ] Regression / trend line, moving average, KDE density line, ECDF
- [ ] Gantt chart (bars on a time axis)
- [ ] Sparklines (axis-less mini charts)

## 5. Non-cartesian charts
- [ ] Pie & donut (labels, percentages, exploded slices)
- [ ] Polar plot (line/scatter in r-θ) & polar bar / rose
- [ ] Radar / spider chart
- [ ] Gauge
- [ ] Treemap (squarified)
- [ ] Sunburst
- [ ] Sankey diagram
- [ ] Funnel chart
- [ ] Network graph (force-directed layout)
- [ ] Parallel coordinates
- [ ] Calendar heatmap
- [ ] 3D: surface/wireframe & 3D scatter (projected)

## 6. Annotations
- [ ] Text, arrows, horizontal/vertical lines, shaded spans, rectangles
- [ ] Data labels on points/bars

## 7. Data & statistics helpers
- [ ] Accept slices, iterators, `(x, y)` pairs, separate columns
- [ ] CSV loading
- [ ] Statistics: quantiles, KDE, linear/polynomial regression, moving average, ECDF
- [ ] Large data: LTTB / min-max decimation for lines

## 8. Interactivity & output targets
- [ ] Interactive window: pan, zoom (wheel & box), reset, hover tooltip, crosshair, save key
- [ ] Real-time streaming (fixed-size rolling series)
- [ ] SVG tooltips (`<title>`) for browsers
- [ ] Terminal output (braille/ASCII)
- [ ] Window support behind a default `window` feature (WASM / headless builds)

## 9. Quality
- [ ] Tests for every chart type (SVG validity, raster checks, edge cases)
- [ ] Gallery example rendering every chart type
- [ ] README / docs / CHANGELOG
