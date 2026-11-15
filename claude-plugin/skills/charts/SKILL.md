---
name: charts
description: Use when the user wants data visualized (a chart, plot, graph, dashboard, figure, map, molecule) or when results — benchmark numbers, logs, CSV/JSON data, model metrics, prices, genomic data — would be clearer as a picture. Draws with the dataviz MCP tools and returns images to inspect.
---

# Charts with dataviz

The `dataviz` MCP server draws charts in milliseconds and returns them as
PNG images that you can look at. Use it instead of writing plotting
scripts when the goal is to see or show data.

## Tools

- `plot` — a figure from a JSON spec: `panels` (laid out in a grid), each
  with a `type` and its data. Call `chart_types` once to see all 40 types
  and their fields.
- `plot_file` — a chart of a CSV/TSV/JSON file without loading it yourself
  (`kind`: auto, line, scatter, bar, hist, box, violin, correlation).
- `molecule` — a protein/DNA structure from the PDB (`pdb_id`), a file or
  text; cartoon for proteins, ball-and-stick for small molecules;
  `ramachandran: true` adds the dihedral plot.
- Every tool takes `title`, `width`, `height`, `theme` and `output` (save to
  .png/.svg/.pdf, or .html for an interactive page).

## How to work

1. Pick the chart from the question: trend over time → line; compare
   categories → bar (barh for long names); distribution → hist, box,
   violin, raincloud; relationship → scatter, regression; composition →
   pie (few parts), waffle, streamgraph; correlation matrix → heatmap or
   `plot_file` with `kind: correlation`; flows → sankey, chord; model
   quality → roc, calibration, confusion; prices → candlestick; countries →
   choropleth; 3D data → surface, scatter3d; structures → molecule.
2. Put several related views in one call (`panels`), not many calls.
3. Look at the returned image. Fix what is wrong (overlapping labels, a
   bad range → `x_limits`/`y_limits`, skewed data → `x_log`/`y_log`, wrong
   chart type) and call again.
4. When the user wants to keep the chart, pass `output` with a path in
   their project and tell them where it is.
5. Describe what the chart shows in one or two sentences; the text part of
   the tool result summarizes axes and ranges.

## Spec examples

```json
{"title": "Latency", "panels": [
  {"type": "line", "series": [{"name": "p50", "x": [1,2,3], "y": [12,11,13]},
                              {"name": "p99", "x": [1,2,3], "y": [40,38,55]}],
   "x_label": "release", "y_label": "ms"},
  {"type": "box", "groups": {"eu": [12, 14, 13, 30], "us": [18, 17, 21, 25]}}
]}
```

```json
{"panels": [{"type": "roc", "labels": [true, false, true], "scores": [0.9, 0.3, 0.6], "name": "model A"}]}
```

```json
{"panels": [{"type": "surface", "z": [[0, 1], [1, 3]], "x_range": [0, 1], "y_range": [0, 1],
             "camera": [-50, 25], "rotate": {"z": 30}}]}
```

Data fields accept numbers, numeric strings and null (a gap). Keep data in
the call small enough to read (thousands of points are fine; for large
files use `plot_file`).
