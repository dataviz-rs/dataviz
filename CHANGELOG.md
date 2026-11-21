# Changelog

## Unreleased

### Added
- `Axes::group_bars`: places bar series with the same categories and
  orientation side by side.

- dataviz-mcp 0.1.1: tool descriptions say when to use each tool, what it
  reads, writes and downloads, and how errors look; every parameter is
  described, and the tools carry MCP annotations (read-only, network).
  A Dockerfile and `glama.json` for the Glama listing.

### Fixed
- Python: calling `bar` or `barh` again with the same categories draws
  grouped bars, as documented, instead of drawing them on top of each
  other (also in `dataviz.pyplot`).

## 1.0.0

dataviz 1.0: the API is stable, and 1.x releases follow semantic
versioning (see "Stability" in the README).

### Changed
- Public enums that will grow are `#[non_exhaustive]`: `Projection`,
  `Colormap`, `Palette`, `Marker`, `Dash`, `Format`, `Error`, `Scale`,
  `Bins`, `LegendPosition`, `BarMode`, `Mark`, `Aggregate`, `Kind`,
  `VolumeMode`, `DensityScale` and others. Matches on them need a `_`
  arm, and new variants in 1.x are not breaking changes.

## 0.19.1

### Fixed
- 3D scatter plots of a million same-colored points on the CPU took 24 s
  instead of 0.15 s in 0.19.0. Culling hidden points split the runs of
  batched sprites, and instance sprites were then found by a linear
  search. Runs now skip culled points, sprites are looked up in a hash
  map, and points that are already batched (mostly one look) are not
  culled.
- C API: the shared library carries its name (SONAME on Linux and
  Android, `@rpath` install name on macOS and iOS), so programs find it
  through their rpath instead of the build path they linked from.

## 0.19.0

### Changed
- Volume rendering is lit from its value gradient: surfaces inside the
  volume get diffuse shading and a highlight from the viewer's upper
  left. This costs about 25 % more time; `Volume::shading(false)` turns
  it off.
- GPU back end: `DATAVIZ_GPU_POWER=low` picks the integrated GPU. It
  opens in ~90 ms instead of ~260 ms and draws less power, but its frames
  are slower than a discrete GPU's (21 ms against 2.8 ms for a 100 k
  point line here). By default the fastest GPU is used.
- 3D point clouds on the CPU are 13–45× faster:
  - opaque points hidden behind a nearer point of at least the same size
    are dropped before drawing (exact: the same quarter-pixel coverage);
  - small filled circles are stamped from a coverage mask cached by size,
    tinted per point, instead of a sprite per color. A per-color sprite
    used to be found by a linear search, so colormapped points spent
    their time there.

  For 1 M colormapped points a frame takes 73 ms instead of 3.3 s, and
  for 100 k points 12 ms instead of 210 ms.
- Colormapped 2D scatter plots of more than 20 000 opaque points draw
  only the last marker per quarter-pixel cell (exact). For 1 M points:
  128 ms instead of 256 ms.
- Windows send a frame only when it changed (plus twice a second):
  an idle window showing a chart uses 0.2–0.7 % of a core instead of 7 %.
  The RGBA copy for overlays is made only while an overlay is drawn.
- Axes left with one or two ticks after rounding the step up (a range of
  22 with room for 4 ticks used to show only 40 and 50) take the next
  smaller nice step.
- npm package 1.52 → 0.91 MB gzipped: new default cargo features `formats`
  (JPEG/WebP/BMP), `full-font` and `shaping` (complex scripts) are off in
  the WebAssembly build, which embeds a 235 KB font subset (Latin, Greek,
  Cyrillic, math, symbols).
- npm: numeric arrays of 64+ items (typed or plain) go into WebAssembly
  memory as binary f64 instead of JSON text. A chart of 1 M points builds
  in 3 ms instead of 214 ms (Float64Array) or 164 ms (plain arrays).
- npm: React component (`dataviz-rs/react`) and a Svelte action
  (`dataviz-rs/svelte`).

### Added
- Map projections:
  - Albers equal-area conic and Lambert conformal conic for country and
    continent maps (`Projection::USA`, `Projection::EUROPE` presets);
  - stereographic and Lambert azimuthal equal-area for polar and
    regional maps;
  - `Axes::map_bounds` frames a longitude/latitude window. In the spec:
    `projection: usa | europe | albers | lambert | stereographic |
    azimuthal` and `bounds`.

  Conic maps cut shapes at the meridian opposite their centre, and long
  edges (along the poles) are drawn as curves.
- Callbacks for dashboards:
  - HTML output posts `click`, `hover` (throttled) and `view` (after zoom
    or pan) messages to the embedding page.
  - Python widgets get `on_click`, `on_hover`, `on_view` and
    `on_select`.
  - npm `mount(canvas, {onClick, onHover, onView})`, React
    `onPointClick` / `onPointHover` / `onView`, Svelte `pointclick` /
    `pointhover` / `view` events.
- Chart drawing tools, as in trading platforms:
  - `Axes::trendline`, `Axes::ray`, `Axes::price_level` (value tag at the
    right edge) and `Axes::fib_retracement` (0–100 % levels with prices
    and shaded bands);
  - in windows: `T`, `L` or `F`, then drag or click on the chart, with a
    live preview; Backspace takes back the last drawing;
  - spec field `drawings: [{type: trend|ray|fib|level, …}]` for every
    panel, and Python `Axes.trendline`, `price_level` and
    `fib_retracement`.

  Drawings stay at their data positions when you zoom or pan.
- Animated WebP (`Recorder` / `Figure::record` to `.webp`): lossless, with transparency,
  storing only the changed rectangle per frame. For a 150-frame
  640×360 chart: 73 KB in 40 ms (APNG 139 KB, GIF 41 KB with at most 256
  colors).
- Volume files (`plot::volume_file::VolumeData`, `Axes::volume_data`,
  spec `volume` with `path`, Python `Axes.volume("scan.nii.gz")`):
  - NIfTI-1 and NIfTI-2 (`.nii`, `.nii.gz`; either byte order, every
    data type, intensity scaling; for 4D files the first volume);
  - VTK legacy structured points (ASCII or binary);
  - MetaImage (`.mhd` + raw, `.mha`, zlib-compressed);
  - DICOM (`plot::dicom`): a directory of CT/MRI slices, sorted along the
    slice normal and stacked with the rescale applied (Hounsfield units),
    or a multi-frame `.dcm`. Uncompressed transfer syntaxes (implicit
    and explicit VR, little and big endian) are read; compressed ones get
    a clear error.

  Voxel spacing and origin are kept, so scans keep their physical
  proportions. `VolumeData::to_nifti` writes a NIfTI file, and
  `Axes::volume_f32` takes `f32` data without conversion. New default
  feature `gzip`.
- Grammar of graphics (`plot::grammar`, spec `mark` + `encoding`, as in
  Vega-Lite and ggplot2):
  - data as columns, rows or `{"values": rows}`;
  - marks: point, line, area, bar, rect and boxplot;
  - channels: x, y, color, size and facet, each with a type,
    aggregation (count, sum, mean, median, min, max), binning, a title
    and a log scale;
  - stacked, grouped or percent bars.

  Color groups keep the same colors in every facet, and the figure gets
  one legend. Histograms of all groups share their bin edges. 1 M rows in
  3 color groups build in 22 ms.
- C and C++ API (`capi/`): `dataviz.h` and a header-only C++17 wrapper
  `dataviz.hpp`, CMake support (shared or static), tested on Linux, macOS
  and Windows; static libraries for iOS and Android. It has direct
  functions for lines, scatter, histograms, heatmaps, density, 3D,
  surfaces and candlesticks, plus `dv_plot` for every chart type of the
  JSON spec. Raw RGBA rendering and interaction calls let you embed charts
  in your own UI. Live windows take a per-frame callback.
- Spec: fields `"@name"` take arrays handed over in memory
  (`spec::with_arrays`), never as JSON numbers. For a 1 M point line this
  takes 4 ms instead of 166 ms. Object keys keep their order (e.g. the
  groups of a violin plot), and line, scatter and bar series accept
  `label` as well as `name`.
- Density images of millions to billions of points (`Axes::density`,
  Python `Axes.density`, spec `density_map`), as Datashader: points counted
  per screen pixel for the visible range (exact at every zoom), in
  parallel, colored by histogram equalization (or log/linear).
  `DensityGrid` counts chunks one after another for data larger than
  memory. 100 M points to PNG: 70 ms (Datashader: 372 ms); from Python,
  NumPy arrays are counted in place (100 M points: 81 ms).

### Fixed
- Edge bundling with large group numbers (e.g. 1e308) no longer
  overflows; group numbers are made dense.

### Python
- 0.4.0:
  - `dv.plot(data, mark, x=, y=, color=, size=, facet=)` and
    `Axes.encode`: grammar of graphics from pandas or Polars DataFrames,
    dicts or row lists, with Altair shorthand (`"mean(mass)"`,
    `"year:O"`).
  - `dataviz.pyplot`: matplotlib-style `plot` (format strings),
    `scatter`, `subplots`, `savefig`, … as a drop-in import.
  - Long NumPy, pandas and Polars arrays in `from_spec` and `Axes.chart`
    are read from their buffers instead of through JSON (1 M points:
    2 ms).
  - `Axes.scatter(values=, colormap=, colorbar=)`.
  - Fixed: a histogram or bars on a log axis were empty (bars from 0 now
    reach past the axis start and are clipped).
- 0.3.0: `Axes.volume(array)`, the globe and equal-area projections,
  arc diagrams and edge bundling through `chart`, `theme="high_contrast"`.

## 0.18.0

### Changed
- GPU back end: 3D lines, markers, spheres and bonds join the retained
  GPU mesh (one instanced quad each, depth tested, shaded and depth-cued
  in the shader like the CPU scene; coverage written as MSAA samples so
  edges never hide what is behind them). A 100 000-point 3D line
  15.0 → 4.5 ms, a million-point 3D scatter 128 → 41 ms; turning them
  sends only the camera.

### Added
- Maps: equal-area `Projection::EqualEarth` and `Projection::Mollweide`;
  a globe (`Axes::globe(lon, lat)`, `Projection::Orthographic`) with the
  far side cut away along the rim, turned by `Axes::globe_center` (a
  spinning globe is a `record` loop). Spec: `projection: "equalearth" |
  "mollweide" | "orthographic"` with `center_lon`, `center_lat`.
- Networks: `Axes::arc_diagram` (nodes on a line, edges as arcs, groups
  side by side) and `Axes::edge_bundling` (hierarchical edge bundling:
  nodes on a circle by group, edges as B-splines through the hierarchy
  with a `tension`); both in the spec (`arc_diagram`, `edge_bundling`
  with node names or indices). 500 nodes and 3000 edges in 37 ms.
- Volume rendering (`Axes::volume`, Python `Axes.volume(array)`, spec
  `volume`): rays through a 3D grid with trilinear sampling, colormap
  transfer function and a transparency `threshold`; composite (emission
  and absorption) or maximum intensity projection; perspective-correct
  with the 3D camera, multi-threaded, empty bricks skipped. A 64³ grid in
  24 ms, 128³ in 72 ms at 800×600.

### Python
- Columns from Polars, pyarrow, DuckDB and Arrow-backed pandas are read
  from their Arrow buffers (Arrow PyCapsule interface; nulls become
  gaps), NumPy arrays of every numeric type in one pass: `ax.line(x, y)`
  with 10 M rows from Polars 508 → 59 ms, pyarrow 2856 → 59 ms, NumPy
  float32/int64 435 → 55 ms.

### JavaScript
- `dataviz-rs` npm package (`wasm/`): charts from a JSON spec in the
  browser, Node, Deno and Bun — SVG, PNG, HTML, RGBA pixels or a live
  canvas with zoom, pan, 3D turning, box zoom, legend clicks and hover
  readouts. WebAssembly with SIMD128, no imports, 1.5 MB gzipped; about
  1.5× native single-threaded speed.

### Fixed
- Spec: a single panel given inline no longer shows its title twice.
- Paths with infinite or NaN points (e.g. GeoJSON coordinates beyond
  f64) no longer panic the rasterizer: such points become gaps, and
  paths reaching far outside the canvas are clamped. Found by the new
  fuzz tests.

### Accessibility
- `Theme::high_contrast()` (spec `"high_contrast"`): black on white,
  heavier lines and frame, larger text, dark Okabe–Ito colors each at
  least 4.5:1 against white.
- HTML output can be used from the keyboard: the chart takes focus;
  ←/→ walk the data points (Shift: 10), ↑/↓ the series, Page Up/Down the
  plots, Home/End jump, + − 0 zoom and reset, Enter hides or shows a
  series, Esc clears. Every step is announced to screen readers (an
  `aria-live` region).

### Quality
- More golden images: globe, Equal Earth, arc diagram, edge bundling,
  volume, 3D points and lines, a molecule, the high-contrast theme.
- `tests/fuzz.rs`: deterministic mutation fuzzing of the PDB, XYZ,
  Newick, CSV, GeoJSON and color readers and of JSON specs (every chart
  type), on stable Rust in every `cargo test`; `DATAVIZ_FUZZ=<n>` for
  longer runs.

## 0.17.0

### Changed
- GPU back end: 3D plots of opaque faces (surfaces, meshes,
  isosurfaces) stay on the GPU as meshes — uploaded once, then only the
  camera is sent; projection, perspective, lighting and depth cueing run in
  the shaders with a depth buffer. A rotation frame of a 200² surface:
  34 ms on one CPU core, 0.5 ms on the GPU (was 5.7 ms for 120²). Scenes
  with lines, points, text, edges or translucent faces keep the CPU path.
- Faster, smoother 3D molecules: bonds and other tubes are one lit
  cylinder per segment (`Node::Tube`, shading table per direction)
  instead of two or three stroked bands; shaded spheres reuse one
  color-independent shading per size. Molecule rotation frames
  2.8 → 2.1 ms on one core.

- GPU back end: tubes (molecule bonds) are drawn in one pass with the
  CPU's lighting — a 1BNA rotation frame 3.7 → 0.7 ms (faster than the
  CPU now). `save`/`encode` to PNG and window frames keep 3D meshes on
  the GPU as well (200² surface PNG 22.8 → 6.7 ms). The GPU opens with
  Vulkan/Metal/DX12 first (OpenGL as fallback), without validation in
  release builds, and starts opening in the background as soon as
  `Backend::Gpu` is chosen: first image ~400 → ~275 ms.
- Python package published on PyPI as `dataviz-rs` (`pip install
  dataviz-rs`, imported as `dataviz`).

### Added
- Linked brushing (`Figure::link_brushing`, `brush`, `brush_rows`,
  `brushed`, `clear_brush`): a box selected in one plot (Ctrl+drag in a
  window, Shift+drag in HTML) highlights the same rows in every plot;
  `Series::row_points` for custom series.
- HTML output: zoom (wheel at the cursor, over an axis only that axis),
  pan (drag) and reset (double click, Reset button) of 2D plots; ticks,
  labels and grid are redrawn, markers keep their size.
- Python: `Figure.interactive()` shows the interactive page in Jupyter
  (hover, zoom, pan, 3D, brushing); with anywidget (`dataviz-rs[widget]`)
  it is a widget whose `brushed` trait follows the selection;
  `Figure.link_brushing`, `brush`, `brushed`, `clear_brush`.

## 0.16.0

### Added
- **Data science** (`series::ml`): `Axes::roc_curve` (AUC in the legend,
  chance diagonal), `Axes::pr_curve` (average precision, baseline),
  `Axes::confusion_matrix` (counts or row fractions, annotated),
  `Axes::regplot` (least-squares line with a 95 % confidence band and R²),
  `Axes::loess`, `Axes::density_contour` (2D KDE of any number of points,
  binned) and `Axes::dendrogram` (average-linkage clustering); functions
  `ml::roc`, `precision_recall`, `confusion`, `loess`.
- **Engineering** (`series::signal`): `Axes::spectrum` (FFT amplitude
  spectrum, Hann window), `Axes::spectrogram` (STFT in dB),
  `Figure::bode` (magnitude and unwrapped phase on a log frequency axis;
  several systems), `Axes::nyquist`, `Axes::smith_chart` and `Axes::smith`;
  functions `signal::fft`, `spectrum`, `spectrogram_db`, `freq_response`,
  `logspace`, `gamma`.
- **Trading**: `Axes::volume_profile` (point of control and value area),
  `Axes::depth_chart` (order book), `Axes::ichimoku` (cloud),
  `Axes::vwap`, `Axes::stochastic`, `Axes::atr`, `Axes::drawdown`,
  `Axes::renko` and `Axes::trading_axis` (bars one after another, skipping
  weekends and nights, with date labels); `stats::vwap`, `stochastic`,
  `atr`, `ichimoku`, `drawdown`, `renko`.
- **Biology, genomics and medicine** (`series::bio`):
  `Axes::sequence_logo` (DNA/RNA/protein motifs, letters stretched to
  their information content in bits), `Axes::gene_track` (exons, introns,
  strand), `Axes::manhattan` (GWAS with genome-wide and suggestive
  thresholds), `Axes::qq_pvalues` (with λ), `Axes::volcano` (labels of the
  top genes, kept apart), `Axes::ma_plot`, `Figure::clustermap` (rows and
  columns clustered, dendrograms on both sides), `Axes::embedding`
  (UMAP/t-SNE cells colored and named by cluster), `Axes::dose_response`
  (four-parameter logistic fit, IC50), `Axes::kaplan_meier` (95 % bands,
  censoring marks, log-rank p) and `Axes::forest_plot` (pooled diamond);
  functions `bio::logo_heights`, `kaplan_meier`, `logrank`, `fit_4pl`.
- **3D scenes**: all 3D series of a plot share one scene (one box, one
  camera, one depth order) with lighting (diffuse and specular), shaded
  spheres and tubes, optional perspective (`Axes::perspective`) and depth
  cueing (`Axes::depth_cue`). The camera (`Camera`) is a rotation matrix:
  `Axes::rotate(Axis3, deg)` turns around the x, y or z axis,
  `Axes::roll`, `Axes::camera(azimuth, elevation)`, `Axes::set_camera`,
  `Camera::principal` (longest extent across the screen). In a window
  dragging turns the scene, the wheel zooms toward the cursor and `X`/`Y`/`Z` rotate by 15°
  (Shift+key the other way); `Figure::rotate_view`, `pan_by` and `zoom_at`
  do the same in GUI and web apps; `R` / `reset_view` restore the camera.
  New 3D series: `Axes::spheres3d` (radii in data units),
  `Axes::mesh3d` / `trisurf` (smooth-shaded, translucent),
  `Axes::ellipsoid`, `Axes::text3d`; `Axes::z_label`, `z_limits`,
  `box3d`, `lighting`; `Points3d::values`, `label`, `shaded`;
  `Surface::edges`, `opacity`. 3D scenes stay inside their plot area when
  zoomed. Custom series draw 3D through
  `Series::bounds3d`, `draw3d` and `hit3d` (`Scene3d`).
- **3D biology** (`series::bio3d`): `Axes::dna_helix` (B-DNA double helix
  with base pairs colored by base, highlighted regions, RNA pairs A–U),
  `Axes::molecule` with `Molecule::from_pdb` / `from_xyz` (bonds from
  covalent radii in linear time; ball-and-stick, sticks, space-filling and
  backbone styles; colors by element, chain or along the chain; water
  hidden), `Axes::embedding3d` (single-cell clusters on a 3D UMAP).
- **Business and editorial charts** (`series::general`): `Axes::bullet`
  (each row in % of its own range), `dumbbell`, `slope`, `bump`,
  `waffle`, `marimekko`, `chord` (d3 layout), `streamgraph` (wiggle
  baseline) and `horizon`.
- **Model evaluation** (`series::evaluation`): `Axes::shap_summary`
  (beeswarm, colors by feature-value rank), `calibration_curve` (Brier
  score), `learning_curve`, `gain_curve`, `lift_curve`, `raincloud`,
  `upset`; functions `calibration`, `gain`, `intersections`.
- **Engineering** (`series::engineering`): `Axes::streamplot` (evenly
  spaced streamlines, colors by speed, log scale over wide ranges),
  `ternary`, `waterfall3d`, `control_chart` (σ from the moving range,
  Western Electric rules 1 and 2), `pareto`, `quiver3d`, `bar3d` (category
  axes in 3D); functions `streamlines`, `control_violations`.
- **Trading** (`series::trading`): `Axes::liquidity_heatmap`,
  `footprint` (`FootprintBar`; imbalances, point of control),
  `market_profile` (TPO, POC, value area), `option_payoff` (`OptionLeg`;
  at expiry and before it), `volatility_surface`; functions
  `black_scholes`, `strategy_value`, `tpo`.
- **Genomics** (`series::genome`): `Axes::circos` (data ring, links),
  `hic` (triangle map, TADs), `coverage`, `phylo_tree` /
  `phylo_tree_circular` (`parse_newick`), `ramachandran`, `oncoprint`
  (`Alteration`), `mutation_lollipop`.
- **Proteins and volumes**: `MolStyle::Cartoon` (helix ribbons, strand
  arrows; `Molecule::secondary` from the PDB HELIX/SHEET records, else from
  the Cα geometry), `Molecule::phi_psi`; `Axes::isosurface` /
  `isosurface_fn` (marching tetrahedra, shared vertices, outward faces;
  `isosurface_mesh`); `Scene3d::tube_line`, `arrow`.
- **Claude Code plugin and MCP server**: `dataviz-mcp` (crate in `mcp/`)
  serves the tools `plot` (40 panel types from a JSON spec), `plot_file`
  (CSV/TSV/JSON), `molecule` (PDB id, file or text) and `chart_types`,
  returning PNG images and text summaries; `claude-plugin/` packages it with
  skills (`charts`, `/dataviz:plot`, `/dataviz:molecule`) and a launcher
  that installs the server on first use; the repository is a plugin
  marketplace (`/plugin marketplace add dataviz-rs/dataviz`).
- `Annotate::background`: a box behind text annotations (readable over
  dense data).
- `Figure::resize(width, height)` (figures embedded in resizable panels).

- Data from other crates without conversion: `Option` values (nulls of
  Arrow and Polars arrays) become gaps, booleans count as 1/0; ndarray
  arrays work as they are (tested).
- Text descriptions (`Figure::alt_text`) of 3D plots: molecules (atoms,
  residues, chains, helices, strands, ligands), DNA (length, GC content,
  marked regions), surfaces, meshes, 3D scatter; 3D axis ranges;
  `Series::describe` for custom series.
- **Python bindings** (`python/`, package `dataviz`, PyO3 + maturin):
  `Figure` / `Axes` with NumPy arrays read in place (lists, pandas, Polars
  via `numpy.asarray`), Jupyter display, PNG/SVG/PDF/HTML/arrays, 3D
  camera, and `Axes.chart(type, …)` for all 40 spec types; wheels workflow.
- **`plot::spec`** (feature `spec`): figures from JSON specs — 40 chart
  types, `layers` of several types on one axes; shared by the MCP server and
  Python. New spec type `radar`.
- **Maps** (`plot::geo_more`): states and provinces of every country
  (feature `regions`, Natural Earth 1:10m admin-1, 0.8 MB compressed;
  `regions(country)`, `Region::area_km2`, `Axes::region_map` by ISO 3166-2
  code or name), `Axes::geo_flows` (flow maps along great circles, width
  and color by value, arrows), `Axes::geo_hexbin` (density maps),
  `Axes::basemap` (web-map tiles from any source, e.g. `tile_dir`;
  `tile_range`). `Anchor` and `Baseline` in the prelude.
- **Interactive 3D in HTML**: `fig.save("x.html")` / `Figure::to_html`
  turns 3D plots in the browser — drag to rotate, wheel to zoom, double
  click to reset — with the same lighting, perspective, depth order, box
  and ticks as the Rust renderer (shaded spheres as radial gradients); the
  scene is exported once in normalized coordinates and drawn on a canvas.
- **Guide** (`book/`, mdBook): installation, quick start, data input,
  choosing a chart, a chart catalog generated from the examples
  (`tools/book.py`), interactivity, 3D, maps, output, performance, Python
  and Claude; published with the WebAssembly demo on GitHub Pages.
- Visual regression tests (`tests/golden.rs`) and CI on Windows and macOS.

### Changed
- Faster 3D: primitives sorted with a radix sort, same-looking spheres
  drawn from one cached sprite, 3D lines drawn in pieces, surface grids
  projected once; small polygons and short strokes take the rasterizer's
  fast paths (any path length). Shaded spheres (`Node::Sphere`) are lit
  per pixel in one pass instead of stacked circles (radial gradients in
  SVG); strokes up to 1.25 px use a Wu line rasterizer; touching tube
  segments of the same look are batched. 3D frames got 2–4× faster on one
  core (1 M-point 3D scatter 464 → 142 ms, 200² surface 77 → 31 ms, 1BNA
  molecule 18 → 4.5 ms, 100 000-point 3D line 28 → 9.7 ms).
- GPU backend: convex fills (3D faces) are batched triangle lists instead
  of one stencil-and-cover pass each, and shaded spheres are drawn
  analytically; rotation frames 2.5–3× faster (120² surface
  18.4 → 5.7 ms).
- 3D: the camera, axis labels (`x_label`, `y_label`, `z_label`) and
  limits belong to the axes instead of each 3D series
  (`Surface::camera` / `labels` and `Points3d::camera` / `labels` are
  gone); 3D scatter points are shaded spheres; `AxesKind::ThreeD`.
- Terminal half blocks keep the color of thin lines and text (the most
  distinct pixel of each half cell) instead of averaging them into the
  background, so lines stay vivid.
- With `Axes::auto_scale_y` on, `zoom_at`, `pan_by` and `zoom_to_box`
  only change x; the y-axes keep fitting the visible data.

### Fixed
- `Figure::alt_text`: sentences start with a capital letter.

## 0.15.0

### Added
- **Accessibility** (`plot::a11y`): SVG, PDF and HTML output carry an
  accessible name (`<title>`, the figure title) and description (`<desc>`,
  `role="img"`): `Figure::alt_text()` describes the plots, axes, ranges and
  series; `Figure::description(text)` sets your own.
- **Color vision checks**: `Cvd` (protanopia, deuteranopia, tritanopia,
  achromatopsia; Machado et al. 2009), `Color::simulate`,
  `Pixels::simulate` (see a chart as others do), `Color::oklab`,
  `Color::difference` and `Figure::color_warnings()` (series whose colors
  are hard to tell apart, normally or with a color vision deficiency).
- `Series::kind` (chart type name for descriptions); `Scene::title` and
  `Scene::description`. `Cvd` is in the prelude.

## 0.14.0

### Added
- **Interaction without a window** (`plot::interact`), for GUI toolkits,
  WebAssembly pages and custom windows that show the figure's pixels:
  `Figure::zoom_at`, `pan_by`, `zoom_to_box`, `reset_view`, `hover`
  (`Hover` with text and position), `click` (legend rows) and `plot_at`,
  all in canvas pixels of the last render.
- **WebAssembly**: builds for `wasm32-unknown-unknown` with
  `default-features = false` (one thread in the browser); see the
  `plot-wasm` example (canvas, streaming data, zoom/pan/hover in Rust, no
  wasm-bindgen).

### Changed
- `image` is used with only the formats dataviz writes (PNG, JPEG, WebP,
  BMP): no AVIF encoder or rayon in the dependency tree, faster builds,
  smaller binaries and no wasm-bindgen imports on wasm32.

## 0.13.1

### Added
- HTML output: linked cursor across plots showing the same x range (as in
  windows).

## 0.13.0

### Added
- **Interactive HTML**: `fig.save("chart.html")` / `Figure::to_html()` write
  one self-contained page (no network access, no external scripts): the
  figure as SVG plus a small script with a crosshair and axis values, a
  tooltip with every series' value at the cursor's x, clicking legend rows
  to hide series, and a "save as PNG" button. `Format::Html`.
- `Series::name` and `Series::data_points` (hover data; implemented by
  lines, scatter, bars and candlesticks).
- `Node::Group::id`: series are drawn in groups with ids (`p<plot>-s<k>`)
  in SVG output.

## 0.12.0

### Added
- **Range slider**: `Axes::range_slider(true)` draws all data in small below
  the plot with the shown x range highlighted (also in static output); in a
  window it is dragged to pan, at its edges to zoom, and a click moves the
  view there.
- Windows:
  - Shift+drag **measures** between two points: Δx (durations on time
    axes), Δy and the change in percent;
  - Ctrl+drag **selects a box**, read with `Figure::selection()` (e.g. in
    the `show_live` update; `Selection::contains`);
  - Shift+S saves SVG (and PDF with the `pdf` feature), Ctrl+C copies the
    plots to the clipboard as PNG (wl-copy/xclip on Linux, the system
    clipboard on macOS and Windows).

## 0.11.0

### Added
- **Maps** (`plot::geo`): a built-in world map (Natural Earth 1:110m
  countries, public domain, 53 KB) with names, ISO codes, continent,
  population and GDP (`geo::world`, `geo::country`); projections
  Robinson, Natural Earth, equirectangular and Mercator;
  `Axes::map(projection)`, `Axes::world_map`, `Axes::choropleth` (values
  by ISO code or name, linear or log colors, colorbar, graticule, country
  tooltips), `Axes::geo_scatter`, `Axes::geo_line` (great circles, split at
  the antimeridian), and `geo::read_geojson` + `Axes::geo_shapes` for other
  shapes (regions, provinces).
- `Axes::frame(on)`: shows or hides the spines.

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
