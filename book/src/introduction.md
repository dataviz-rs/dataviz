# dataviz

**Fast plotting for Rust and Python.** 120+ chart types — statistics,
machine learning, finance and order flow, genomics, maps, business charts,
3D surfaces, molecules and isosurfaces — drawn by one anti-aliased renderer
that needs no system libraries, no Python and no browser.

- **Fast and light:** 5–22× faster than matplotlib from Python, the least CPU
  and memory of the Rust plotting crates (see [Performance](performance.md)).
- **Interactive:** windows with zoom, pan, crosshair, linked plots and live
  data; HTML pages with hover and 3D rotation; the same interaction inside
  egui or WebAssembly apps.
- **Everywhere:** PNG, SVG, PDF, JPEG, WebP, interactive HTML, animated
  GIF/APNG/MP4, the terminal, raw pixels; [Python](python.md); a
  [Claude Code plugin](claude.md).

```rust
use dataviz::prelude::*;

let x: Vec<f64> = (0..200).map(|i| i as f64 / 20.0).collect();
let mut fig = Figure::new(800, 450);
fig.axes()
    .title("Signals")
    .line(&x, x.iter().map(|v| v.sin()))
    .label("sin");
fig.save("signals.png")?;
# Ok::<(), dataviz::plot::Error>(())
```

This guide shows how to install it, give it your data, choose a chart, make
it interactive and get it out — and the [chart catalog](catalog/index.md)
shows every chart type with its code. Try it in the browser:
**[live demo](demo/index.html)** (WebAssembly).
