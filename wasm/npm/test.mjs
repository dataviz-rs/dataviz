import { test } from "node:test";
import assert from "node:assert/strict";
import { init, Chart, chartTypes, svg, png } from "./index.js";

await init();

test("spec to SVG, PNG, HTML and pixels", () => {
  const c = new Chart({ width: 400, height: 300, type: "line", x: [1, 2, 3], y: [2, 1, 3], label: "a" });
  assert.equal(c.width, 400);
  assert.match(c.svg(), /^<svg/);
  assert.deepEqual([...c.png().slice(0, 4)], [0x89, 0x50, 0x4e, 0x47]);
  assert.match(c.html(), /<html/i);
  const px = c.pixels(2);
  assert.equal(px.width, 800);
  assert.equal(px.data.length, 800 * 600 * 4);
  c.free();
});

test("interaction", () => {
  const c = new Chart({ width: 400, height: 300, type: "scatter", x: [1, 2, 3], y: [3, 1, 2] });
  c.pixels();
  assert.equal(c.zoom(200, 150, 0.5), true);
  assert.equal(c.pan(200, 150, 10, 0), true);
  c.reset();
  c.pixels();
  const h = c.hover(200, 150);
  assert.ok(h === null || typeof h.text === "string");
  c.update({ type: "bar", categories: ["a", "b"], values: [1, 2] });
  assert.equal(c.width, 400);
  c.free();
});

test("typed arrays", () => {
  const c = new Chart({ type: "line", x: new Float64Array([1, 2, 3]), y: new Int16Array([3, 1, 2]) });
  assert.match(c.svg(), /<svg/);
  c.free();
});

test("errors and helpers", () => {
  assert.throws(() => new Chart("{"), /JSON/);
  assert.throws(() => new Chart({ type: "nope" }), /dataviz/);
  assert.ok(chartTypes().length > 30);
  assert.match(svg({ type: "pie", labels: ["a", "b"], values: [1, 2] }), /<svg/);
  assert.ok(png({ type: "hist", values: [1, 2, 2, 3] }).length > 100);
});

test("long arrays go over as binary (typed and plain) in every field kind", () => {
  const n = 5000;
  const x = new Float64Array(n).map((_, i) => i);
  const y = Array.from(x, (v) => Math.sin(v / 100));
  const labels = Array.from(x, (v) => (v % 3 === 0 ? 1 : 0));
  const scores = new Float32Array(n).map((_, i) => (i % 3 === 0 ? 0.7 : 0.3) + (i % 7) / 50);
  const ids = Array.from({ length: 100 }, (_, i) => i);
  for (const spec of [
    { type: "line", x, y, name: "binary" },
    { type: "scatter", x: Int32Array.from(x), y },
    { type: "roc", labels, scores },
    { type: "upset", sets: { a: ids, b: ids.slice(50) } },
    { type: "violin", groups: { a: y, b: x } },
    { type: "heatmap", z: [Float64Array.of(1, 2), Float64Array.of(3, 4)] },
  ]) {
    const c = new Chart({ width: 300, height: 200, ...spec });
    assert.match(c.svg(), /^<svg/, spec.type);
    c.free();
  }
  const c = new Chart({ type: "line", x, y });
  c.update({ type: "line", x, y: x });
  c.free();
});
