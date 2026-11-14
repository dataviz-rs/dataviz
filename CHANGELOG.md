# Changelog

## Unreleased

Performance: faster rendering with less CPU and memory, no new dependencies.

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
