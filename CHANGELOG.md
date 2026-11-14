# Changelog

## 0.10.0

### Added
- **Broken axes**: `Axis::break_at(from, to)` leaves a range out of an axis
  (several breaks allowed): the break is marked with a gap and `//` on the
  spines, ticks are placed per visible segment, margins follow the visible
  span and lines do not jump over a break.
- **Technical indicators**: `Axes::bollinger`, `Axes::rsi` (with 30/70
  levels), `Axes::macd` (lines and a green/red histogram) and
  `Axes::heikin_ashi` candles; `stats::sma`, `ema`, `bollinger`, `rsi`,
  `macd`, `heikin_ashi`.

### Changed
- Plots in a grid line up: plots starting in the same column share their
  left edge, plots ending in the same column their right edge, and likewise
  for rows (stacked price/volume/indicator panels, facets).

## 0.9.0

### Added
- `Axes::strip` (jittered points per category), `Axes::swarm` and
  `Axes::swarm_horizontal` (beeswarm: points side by side without
  overlap), returning `Strip` (`size`, `spread`, `colors`, `opacity`,
  `marker`, `edge`).
- `Axes::ridgeline`: density curves per group on overlapping rows
  (`Ridgeline::overlap`, `colors`, `opacity`, `edge`).
- `Axes::qq`: Q-Q plot against the normal distribution with a reference
  line through the quartiles; `series::distribution::normal_quantile`.
- `Figure::facet_wrap(groups, cols, draw)`: small multiples with shared
  ranges and tick labels on the outer plots.
- `Figure::pair_plot(columns)`: scatter matrix with histograms on the
  diagonal.
- `Axis::tick_labels(on)`: hides tick labels but keeps their space, so the
  plots of a grid line up.

### Changed
- KDE (violin, ridgeline, `Axes::kde`) of more than 4096 values uses linear
  binning and a truncated kernel: O(n) instead of O(n × points).
- Category axes extend to data reaching beyond the first or last category.

## 0.8.0

### Added
- **Font fallback**: characters the theme font lacks (CJK, Thai, Indic,
  symbols, …) are drawn with a system font that has them. The system fonts
  are scanned once, the first time such a character appears (about 25 ms),
  and fonts named after the character's script are tried first.
- **Right-to-left and complex scripts**: Arabic and Hebrew are put in visual
  order (Unicode bidi) and, like Indic scripts, Thai and combining marks,
  shaped with HarfRust (joined Arabic letters, conjuncts, mark placement).
  Other text keeps the fast path without shaping.
- **Color emoji** from bitmap emoji fonts (e.g. Noto Color Emoji) on CPU and
  GPU; emoji characters and characters followed by U+FE0F prefer them.

### Changed
- New direct dependencies `harfrust` and `unicode-bidi` (already used through
  resvg/usvg). Latin text renders as fast as before.

## 0.7.0

### Added
- **Math notation** in every text (titles, axis labels, legends, tick
  formats, annotations, tooltips): `$...$` holds a TeX subset (see
  `plot::math`): super- and subscripts, `\frac`, `\sqrt`, accents
  (`\bar \hat \vec \dot \tilde`, `\overline`), Greek letters, operators
  and relations with TeX spacing, big operators (`\sum \int \prod`),
  function names (`\sin \log \lim` …), `\mathrm \mathbf \mathit \text`,
  `\mathbb \mathcal \mathfrak`, spacing commands; `\$` is a dollar sign.
  It is laid out into ordinary text runs and lines, so CPU, GPU, SVG and
  PDF output all show it; letters are italic (synthesized oblique).
- `TextNode::italic`.

### Changed
- `FontSet` measures math text as laid out; window tooltips size their
  boxes with real text widths.
- The `plot_interactive` example requires the `window` feature.

## 0.6.0

### Added
- **Animations** (`plot::anim`): `Figure::record(path, fps, frames, update)`
  and `Recorder` (`create`, `frame`, `push`, `push_at` for varying rates,
  `finish`) write
  - GIF: exact palette when a frame has at most 256 colors (typical for
    charts), otherwise a weighted median-cut palette;
  - animated PNG (`.png`/`.apng`): lossless, with transparency;
  - MP4/MOV/MKV (H.264) and WebM (VP9) through the `ffmpeg` program when it
    is on `PATH`.
  Only the changed rectangle of each frame is stored and repeated frames
  extend the previous one. 800×600, 120 frames: a moving sine wave takes
  0.25 s and 1.0 MB (plotters 1.1 s / 3.2 MB, ruviz 3.7 s / 1.7 MB), a
  growing line 0.08 s and 38 KB (plotters 2.4 s / 3.2 MB, ruviz 3.7 s /
  1.7 MB).
- Windows: `V` starts and stops recording a GIF of the plots (encoded on a
  background thread; frames are dropped rather than slowing the window).
- `Recorder` is in the prelude.

### Changed
- New direct dependencies `gif` and `png` (already used through `image`).

## 0.5.1

### Added
- **Linked zoom and pan** in windows: zooming or panning a plot moves the
  plots that showed (about) the same x range with it.

### Changed
- Plots count as showing the same x range (linked cursor, linked zoom) when
  their x scales are of the same kind and the ranges overlap by at least
  90 %, so e.g. candlesticks and volume bars of the same times are linked
  although their margins differ slightly.

## 0.5.0

### Added
- **Crosshair** in windows (`C` toggles, on by default): dashed lines at the
  mouse with its x and y values on the axes (time axes as date/time,
  categories by name), and the values of every series at that x with a dot
  on each point.
- **Linked cursor**: plots showing the same x range (`Figure::share_x`, or
  equal limits) show a cursor line and their series' values at the same x.
- **Legend toggle**: clicking a legend row hides or shows its series (the
  row is dimmed; hidden series are not drawn and do not count for the
  limits). `Axes::set_visible(index, on)` and `Axes::is_visible(index)` do
  the same from code.

### Changed
- Hover hit tests on x-sorted lines, areas, candlesticks and bars use a
  binary search around the cursor: 1M-point lines go from ~10 ms to
  0.3 ms (crosshair) / 1.5 ms (nearest point) per mouse move.
- Window overlays are drawn directly over a copy of the frame instead of
  rasterizing a full transparent layer and blending it (~0.4 ms at 900×600).
- `Layout` has a crate-private field (legend hit areas).

## 0.4.2

### Added
- **Auto scale**: `Axes::auto_scale_y(true)` fits the y-axes to the data
  inside the visible x range (zoom, rolling windows); `Series::y_range_in`
  (default: whole extent) is implemented by lines, areas, scatter,
  candlesticks and bars. In `show_live` windows it is on by default: zooming
  and panning move along x while y follows the visible data; `A` toggles it
  (off = the y range is held too).
- `plot::data::CsvLog`: records live data to CSV as it arrives (buffered,
  flushed every second and on drop, appends to existing files; timestamps
  readable by `Table::time_column`) and `plot::data::format_timestamp`.

### Fixed
- `Figure::show_live`: a plot the user zoomed or panned keeps that view
  while new data arrives (also when the update rebuilds the figure); a hint
  is shown, `R` returns to following the data.
- Windows can be moved and resized on GNOME Wayland: on Linux the window
  uses X11 (XWayland) when available, so the desktop draws a title bar
  (`DATAVIZ_WAYLAND=1` keeps Wayland).

## 0.4.1

### Added
- `Line::keep_since(x_min)`: rolling time window for streaming data.
- `Histogram::set_values` and `Histogram::values`: re-bin live data.
- `Pixels` is in the prelude and implements `Default` (buffer for
  `Figure::render_into`).
- New example in dataviz-examples: `plot-streaming`, a live dashboard fed by
  a producer thread (simulated sensors, CPU/memory, stdin).

## 0.4.0

### Removed
- The classic 0.1 API (`dataviz::figure`: `FigureConfig`, `PixelCanvas`,
  `SvgCanvas`, `Quadrant1Graph`, `CartesianGraph`, `GroupBarChart`, …,
  `Winop`) and its `imageproc` dependency. Everything it did is covered by
  `dataviz::plot` (`use dataviz::prelude::*;`); see the README and
  [dataviz-examples](https://github.com/dataviz-rs/dataviz-examples).

### Changed
- README and crate documentation describe only the `plot` API.

## 0.3.0

Performance: faster rendering with less CPU and memory; the only new
(optional) dependency is `wgpu` for the `gpu` feature.

### Breaking
- `plot::scene::Node` is `#[non_exhaustive]` and has two new variants,
  `Instances` (one marker template at many positions) and `MarkerGrid`
  (markers at the set cells of an occupancy grid). Code matching on `Node`
  needs a wildcard arm.

### Added
- **Backends**: `Figure::backend(Backend)` chooses where raster output is
  computed: `Backend::Auto` (default; CPU, all cores from 1 megapixel on),
  `Backend::Cpu { threads }` (0 = all cores) or `Backend::Gpu`.
- **Multi-core CPU rendering**: the image is split into horizontal bands
  drawn in parallel (4K frame: 50 ms → 9 ms); pixel conversions are
  parallel too.
- **GPU rendering** (`gpu` feature, wgpu on Vulkan/Metal/DX12/OpenGL):
  analytic anti-aliased circles and rectangles, glyph atlas, stencil-and-cover
  fills with 4× MSAA, analytic clipping, conversion to the output format on
  the GPU. Real-time 4K frame: 5.9 ms with 4.8 ms CPU time (CPU backends:
  9.3 ms with 80 ms CPU time on all cores). `Backend::gpu_available()`.
  Group opacity is applied per element on the GPU.
- `examples/plot_backends.rs` compares the backends.
- **Large data**: lines are decimated while they are mapped (no buffer of
  every point: 50 M points 1.35 s → 1.02 s); dense opaque scatter plots skip
  markers that would paint an already painted quarter-pixel position or lie
  outside the plot (10 M points 1.63 s → 0.40 s, +1.3 GB → +0.33 GB RAM);
  histogram min/max and counting in vectorizable multi-lane loops. The GPU
  backend requests the adapter's largest buffers, reports geometry above the
  limit as an error and never uses a software (CPU) adapter.
- **Markers** are one template plus 8-byte positions (`Node::Instances`),
  drawn from cached quarter-pixel sprites (CPU) or instanced (GPU); SVG uses
  `<defs>`/`<use>`. Dense opaque scatter plots become an occupancy grid
  (`Node::MarkerGrid`): memory no longer grows with the number of points.
- **No data copies**: series keep the vectors they are given (pass `Vec`s by
  value to avoid any copy).
- **Rendering into the output buffer**: `render_0rgb`/`render_into` draw
  straight into the caller's buffer and convert in place (one 4-byte-per-pixel
  buffer instead of two).
- **Faster primitives**: rectilinear fills (column envelopes) as rectangles,
  steep line segments walked by column, exact touched-pixel spans for line
  blending, constant-alpha row blending, AVX2 histogram binning/min-max and
  pixel conversions (run-time detected, scalar fallback), PNG with the "Up"
  filter written straight from the render buffer, reused buffers between
  figures on a thread.
- `Figure::render_into(&mut Pixels)` and `Figure::render_0rgb(&mut Vec<u32>)`
  render into reused buffers (real-time redraws without allocations);
  `Pixels::write_0rgb`.

### Changed
- **Direct rasterizer**: PNG/JPEG/WebP/BMP/RGBA output and windows draw the
  scene straight into pixels instead of serializing SVG and parsing it with
  resvg. Analytic clipping, cached glyph bitmaps, marker sprites at
  quarter-pixel offsets, own anti-aliased rectangle, polygon (signed-area)
  and polyline (distance coverage) fillers.
- **Embedded font**: DejaVu Sans is built in (Bitstream Vera license,
  `assets/`); no system font scan at start-up (cold start 35 ms → 4 ms) and
  identical output on every machine. `Theme::font` still loads any TTF/OTF.
- Dense lines are drawn as per-pixel-column envelopes (100 k points:
  25 ms → 2 ms for the line).
- Histograms: binning without sorting (O(n) quartiles), direct bin index for
  equal-width bins, results cached between renders.
- SVG numbers are written without allocations (scatter 100 k SVG 3× faster).
- Window redraws reuse the frame buffer.

Real-time benchmark (800×600, full redraw per frame, p50): 1 k points
7.2 → 1.6 ms, 10 k 14.4 → 1.5 ms, 100 k 30.3 → 2.3 ms; peak RSS
15.6–21.6 MB → 8–15 MB.

### Fixed
- `--no-default-features` builds without dead-code warnings.

## 0.2.0

A complete plotting library in the new `dataviz::plot` module (with
`dataviz::prelude`). The 0.1 API (`dataviz::figure::*`) is unchanged and
still supported (`cargo semver-checks`: no breaking change).

### Added
- **Architecture**: every chart draws into a backend-independent scene that
  is written as **SVG**, rasterized with resvg to anti-aliased **PNG, JPEG,
  WebP, BMP** or raw RGBA (any DPI scale), converted to **PDF** (`pdf`
  feature), shown in an **interactive window** (`window` feature, on by
  default) or printed in the **terminal** (24-bit half blocks or braille).
- **Figures**: subplot grids with spans, shared axes, titles, themes, custom
  fonts (system sans-serif by default, same metrics for layout, SVG and
  raster).
- **Axes**: linear, log (any base), symmetric-log, date/time and categorical
  scales; automatic nice limits with margins and sticky edges; manual limits,
  inverted axes, equal aspect, secondary y-axis; major/minor ticks, explicit
  or labelled ticks, tick formats (auto, decimals, percent, SI, scientific,
  time, prefix/suffix, custom closure), automatic label thinning/rotation.
- **Styling**: line width, dash styles, markers (10 shapes), opacity,
  palettes (Tab10, Okabe–Ito, Set2, Dark2, Tab20, Neon, Muted), 14 colormaps
  with colorbars, CSS color names/hex/rgba parsing, 7 themes (light, dark,
  minimal, ggplot, seaborn, publication, solarized), legends in 10 positions
  with columns.
- **Chart types**: line (steps, monotone smoothing, min/max decimation for
  huge data), area, fill-between, stacked and 100 % stacked areas, scatter
  (colormapped, bubble), bars (vertical/horizontal, grouped, stacked,
  percent, overlay, value labels, rounded), histogram (bin rules, density,
  cumulative, step), 2D histogram, hexbin, box plot (notches, means,
  outliers), violin, error bars and bands, KDE, ECDF, polynomial regression,
  moving average, heatmap (annotated), image display, contour lines and
  filled contours, candlestick and OHLC, stem/lollipop, waterfall, quiver,
  function plots, Gantt, sparklines, pie/donut, polar line/scatter, radar,
  rose/polar bar, gauge, funnel, treemap, sunburst, sankey, network graph,
  parallel coordinates, calendar heatmap, 3D surface/wireframe, 3D scatter
  and line.
- **Annotations**: text, arrows, horizontal/vertical lines, shaded spans,
  rectangles; SVG tooltips (`<title>`).
- **Interactivity**: `Figure::show` with wheel zoom, drag pan, box zoom,
  reset, hover tooltips and PNG export; `Figure::show_live` for streaming
  data (`Line::push` / `keep_last`).
- **Data & statistics**: generic numeric inputs (slices, vectors, ranges,
  iterators of any number type), CSV/TSV loading with dates
  (`plot::data::Table`), statistics helpers (quantiles, box stats, KDE,
  histogram bin rules, regression, moving average, ECDF, LTTB).
- Custom chart types by implementing `plot::Series`.

### Changed
- `minifb` is now optional behind the default `window` feature
  (`default-features = false` builds headless / WASM-friendly).

## 0.1.11

### Fixed
- Data lying on the edge of the plot (e.g. a constant series, or the
  maximum value) or on an axis was hidden under the frame/axis lines in PNG
  output. Frame and axis lines are now drawn below the data in both
  renderers.

### Changed
- Value axes use round tick values: the axis limits are widened to a round
  step (1, 1.5, 2, 2.5, 3, 4, 5, 6, 8 × 10ⁿ) while keeping exactly
  `num_axis_ticks` intervals, e.g. `0, 30, …, 240` instead of
  `0, 27.375, …, 219`. Cartesian graphs stay symmetric around zero. Hover
  uses the same limits.

## 0.1.10

Backward compatible with 0.1.9: `cargo semver-checks` passes, and every
example in [dataviz-examples](https://github.com/dataviz-rs/dataviz-examples)
builds and runs unchanged. Rendering is now shared by all charts and both
canvases, so PNG output, SVG output and hover hit-testing use the same
layout.

### Fixed
- **SVG ignored `FigureConfig`** ([#6]): tick count, grid counts, axis, grid
  and title colours and font sizes were hard-coded (10 or 20 ticks,
  `lightgray`, `black`, 12 px). All SVG charts now use the configuration,
  like the PNG renderer.
- **Quadrant 1 SVG always drew black circles on every data point** ([#5]),
  hiding the line colour on dense data. Points are no longer marked by
  default; see `draw_svg_with_markers` below.
- **Dashed and dotted lines looked solid on dense data** (follow-up to [#1]):
  the dash pattern restarted at every data point. Lines are now drawn as one
  polyline whose pattern runs continuously (`<polyline>` in SVG). Dotted SVG
  lines are drawn as round dots.
- **The PNG grid used the grid "counts" as pixel spacing**, drawing a line
  every 10 px; it now divides the plot into `num_grid_*` intervals.
- **Histogram counted values in the wrong bins**: values were binned with the
  bin width at the time they were added, and the first value and every new
  maximum were dropped. Counts are now recomputed from the data; every finite
  value is counted once. The SVG x-axis labels started at `max` instead of
  `min`. A histogram created by `FigureFactory` had 0 bins and panicked.
- **Panics**: scatter, area and quadrant charts with negative values
  (`attempt to subtract with overflow`), any chart without data (NaN/∞
  ranges), single values or all-equal values (division by zero), histograms
  with fewer than two bins, margins larger than the canvas, and lines or
  points outside the canvas (index overflow). Drawing now clips to the canvas.
- **Out-of-canvas pixels wrapped around** to the next row in `draw_pixel`.
- **`FigureConfig::default()` panicked** with "Font path is not set" when
  drawing a PNG, and an unreadable font path panicked. A system sans-serif
  font is used instead (or text is skipped if there is none). Fonts are now
  loaded once instead of once per text element, and text is drawn without
  copying the whole image twice per label.
- **Malformed SVG**: titles, labels and legend entries were not XML-escaped
  (`&` or `<` produced an unreadable file), and `SvgCanvas::clear` removed
  the document header.
- **Cartesian SVG swapped the axis labels.** Histogram PNG did too.
- **Area chart SVG ignored `alpha`** (always 0.5); the pixel area could not
  be drawn below a negative baseline and divided by zero on vertical steps.
- **Grouped bar charts** truncated categories to `u32` (2.5 merged into 2,
  negative values became 0) and their SVG had no axis labels.
- **Pie chart**: a single 100 % slice was invisible in SVG, PNG and SVG
  drew slices in opposite directions, and hover reported the mirrored slice.
- **Hover** used a different coordinate mapping than drawing for area,
  cartesian, scatter and grouped bar charts (the latter assumed a fixed range
  of 10), so tooltips pointed at the wrong data; the histogram hover panicked
  with fewer than two bins.
- **`Winop::display_svg`** loaded the font *path* as font data and rendered
  transparent areas black; the display loops ran at 100 % CPU, `fps = 0`
  panicked, and the `H` (hints) key did nothing.
- Overlapping axis labels are thinned out automatically.

### Added
- `Quadrant1Graph::draw_with_markers` / `draw_svg_with_markers` and the same
  on `CartesianGraph`: mark every data point with a `ScatterDotType` in the
  dataset colour.
- `SvgCanvas::draw_polyline_rgb_styled`.
- `PixelCanvas::try_save_as_image` (returns an error instead of panicking)
  and `PixelCanvas::get_pixel`.
- `Histogram::recalculate`.
- `Debug`/`PartialEq` (and `Clone` for datasets) on the public data types.
- `examples/gallery.rs`, integration tests for every chart and every issue.

### Changed
- Dependencies: `imageproc` 0.27, `resvg` 0.50, `minifb` 0.29, `ab_glyph`
  0.2.32; the unused `rusttype` dependency was removed. Minimum Rust version
  is 1.89 (`rust-version` is now declared).
- Tick labels use as many decimals as the tick step needs (`600`, `0.25`,
  `1.57`) instead of a fixed `.1`/`.2`.
- Legends wrap onto several rows when they do not fit.

[#1]: https://github.com/dataviz-rs/dataviz/issues/1
[#5]: https://github.com/dataviz-rs/dataviz/issues/5
[#6]: https://github.com/dataviz-rs/dataviz/issues/6
