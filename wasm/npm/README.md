# dataviz-rs for JavaScript

Fast, good-looking charts from a JSON spec, drawn by the
[dataviz](https://github.com/dataviz-rs/dataviz) Rust renderer compiled to
WebAssembly: 120+ chart types (statistics, machine learning, finance,
genomics, maps, 3D surfaces and molecules), SVG, PNG, self-contained HTML or
a live, interactive `<canvas>`. Browsers, Node 18+, Deno and Bun; no
dependencies, no imports from the host.

```sh
npm install dataviz-rs
```

## In the browser

```html
<canvas id="c"></canvas>
<script type="module">
  import { init, Chart } from "https://cdn.jsdelivr.net/npm/dataviz-rs@0.1.0/index.js";
  await init();
  const chart = new Chart({
    width: 800, height: 400, title: "Signal",
    type: "line", x: [0, 1, 2, 3], y: [1, 3, 2, 4], label: "a",
  });
  chart.mount(document.getElementById("c"));
</script>
```

`mount` draws sharply on high-DPI screens and handles the wheel (zoom),
dragging (pan; turns 3D plots), Shift+drag (zoom to a box), double click
(reset), legend clicks (hide/show a series) and hover readouts. Call
`chart.update(spec)` with new data and the canvas redraws.

## In Node, Deno or Bun

```js
import { writeFileSync } from "node:fs";
import { init, Chart, svg, png } from "dataviz-rs";

await init();
writeFileSync("bars.svg", svg({ type: "bar", categories: ["a", "b"], values: [3, 5] }));
writeFileSync("hist.png", png({ type: "hist", values: [1, 2, 2, 3, 3, 3] }));

const chart = new Chart({
  width: 900, height: 600, title: "Two panels",
  panels: [
    { type: "scatter", x: [1, 2, 3], y: [2, 4, 3] },
    { type: "surface", z: [[0, 1], [1, 0]], colormap: "viridis" },
  ],
});
writeFileSync("page.html", chart.html()); // interactive, self-contained
chart.free();
```

## API

| | |
|---|---|
| `await init(source?)` | loads `dataviz.wasm` (next to `index.js`, or a URL / bytes / `WebAssembly.Module`) |
| `new Chart(spec)` | a chart; `spec` is an object or JSON string, typed arrays work |
| `.svg()` `.png()` `.html()` | exports |
| `.pixels(scale)` | RGBA `{width, height, data}` |
| `.mount(canvas, {interactive, tooltip})` | live canvas; returns a function that removes the handlers |
| `.update(spec)` `.resize(w, h)` | new data or size |
| `.zoom` `.pan` `.zoomBox` `.reset` `.click` `.hover` | interaction without `mount` |
| `.free()` | frees the chart now (otherwise on garbage collection) |
| `chartTypes()` | every chart type with its fields |
| `svg(spec)` `png(spec)` | one-shot exports |

The spec format is the one of the dataviz MCP server and Python bindings:
`{ width, height, title, theme, cols, panels: [{ type, ...fields }] }`, or
one panel's fields with `type` at the top level. `chartTypes()` lists the
types and their fields.

## Speed

The module is 3.8 MB (1.5 MB gzipped, fonts and text shaping included).
In V8 a frame of a 100 000-point line takes 10 ms, of a 10 000-point
scatter 1.8 ms, about 1.5× native single-threaded dataviz.

## License

MIT
