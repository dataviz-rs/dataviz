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
