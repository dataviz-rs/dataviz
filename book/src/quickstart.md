# Quick start

A figure holds one or more plots (`Axes`); every chart is a method of an
axes, and options chain.

```rust
use dataviz::prelude::*;

let x: Vec<f64> = (0..100).map(|i| i as f64 / 10.0).collect();
let mut fig = Figure::new(900, 500);
fig.title("Two plots");
fig.grid(1, 2);

let ax = fig.at(0, 0);
ax.title("Lines").x_label("t [s]").y_label("value");
ax.line(&x, x.iter().map(|v| v.sin())).label("sin");
ax.line(&x, x.iter().map(|v| v.cos())).label("cos").dash(Dash::Dashed);

let ax = fig.at(0, 1);
ax.title("Bars");
ax.bar(["Q1", "Q2", "Q3", "Q4"], [3.0, 5.0, 4.0, 6.0]).label("2025");

fig.save("quick.png")?;      // .svg .pdf .jpg .webp .html too
fig.show("quick start")?;     // a window: zoom, pan, crosshair
# Ok::<(), dataviz::plot::Error>(())
```

- `fig.grid(rows, cols)` and `fig.at(row, col)` (or `fig.span(row, col,
  rowspan, colspan)`) lay out several plots; `fig.share_x(true)` links them.
- Themes: `fig.theme(Theme::dark())` (also `minimal`, `ggplot`, `seaborn`,
  `publication`, `solarized`).
- Text takes TeX math between dollars: `ax.y_label(r"$\sigma^2$ [m²]")`.

## Live data

```rust,ignore
let mut fig = Figure::new(900, 400);
fig.axes().line(Vec::<f64>::new(), Vec::<f64>::new()).label("signal");
let mut t = 0.0;
fig.show_live("live", 30, |fig| {
    t += 0.05;
    let line = fig.axes().series_mut::<dataviz::plot::series::line::Line>(0).unwrap();
    line.push(t, t.sin());
    line.keep_last(500);
    true // redraw
})?;
```
