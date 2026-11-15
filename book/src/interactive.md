# Interactivity

## Windows

`fig.show("title")` opens a window; `fig.show_live("title", fps, update)`
redraws as data arrives.

| Mouse / key | Action |
|---|---|
| wheel, drag, right-drag | zoom at the cursor, pan, zoom to a box |
| `C`, `H` | crosshair with axis values (linked across plots), hover tooltip |
| click a legend row | hide or show the series |
| range slider | drag to pan, drag its edges to zoom |
| Shift+drag, Ctrl+drag | measure Δx, Δy and %, select a box (`fig.selection()`) |
| `A`, `R` | auto-scale y to the visible data, reset |
| `S`, Shift+S, Ctrl+C, `V` | save PNG, SVG/PDF, copy, record a GIF |
| 3D: drag, wheel, `X`/`Y`/`Z` | turn, zoom, rotate 15° around an axis (Shift: back) |

## HTML pages

`fig.save("chart.html")` writes one self-contained file: hover readouts,
a crosshair, linked cursors, legend toggling, zoom (mouse wheel at the
cursor; over an axis only that axis), pan (drag), reset (double click or
the Reset button), PNG download — and 3D plots that turn with the mouse.

## Linked brushing

With `fig.link_brushing(true)` a box selected in one plot (Ctrl+drag in a
window, Shift+drag in an HTML page) highlights the same rows in every plot:
row `i` of each scatter, line, bar or price series with the same number of
points is the same record. `fig.brushed()` returns the selected row
indices (for example to filter a table in your app), and
`fig.brush(plot, x_range, y_range)` or `fig.brush_rows(n, rows)` select
from code — the highlight then appears in PNG, SVG and PDF output too.

```rust
use dataviz::prelude::*;
let (price, volume, ret) = (vec![1.0, 2.0, 3.0], vec![5.0, 3.0, 4.0], vec![0.1, -0.2, 0.05]);
let mut fig = Figure::new(900, 400);
fig.grid(1, 2).link_brushing(true);
fig.at(0, 0).scatter(price.clone(), volume);
fig.at(0, 1).scatter(price, ret);
fig.save("linked.html")?;
# Ok::<(), dataviz::Error>(())
```

## Your own GUI or web page

The same interaction without a window: render into pixels and forward the
events.

```rust,ignore
fig.render_into(&mut pixels)?;            // RGBA for a texture
fig.zoom_at(x, y, 0.8);                   // wheel
fig.pan_by(x, y, dx, dy);                 // drag (turns 3D plots)
if let Some(h) = fig.hover(x, y) { show(h.text); }
fig.click(x, y);                          // legend rows
fig.rotate_view(plot, Axis3::Y, 15.0);
fig.reset_view();
```

Examples: [egui](https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-egui),
[WebAssembly](https://github.com/dataviz-rs/dataviz-examples/tree/main/plot-wasm)
([live demo](demo/index.html)).
