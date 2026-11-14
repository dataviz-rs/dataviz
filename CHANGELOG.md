# Changelog

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
