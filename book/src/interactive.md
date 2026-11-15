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
