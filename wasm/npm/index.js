// dataviz for JavaScript: charts from a JSON spec, drawn by the dataviz
// Rust renderer compiled to WebAssembly. Works in browsers, Node, Deno and
// Bun; no dependencies.

let wasm = null;
let ready = null;
const enc = new TextEncoder();
const dec = new TextDecoder();

async function load(source) {
  if (source instanceof WebAssembly.Module) {
    return WebAssembly.instantiate(source, {});
  }
  if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
    return (await WebAssembly.instantiate(source, {})).instance;
  }
  const url = source ?? new URL("./dataviz.wasm", import.meta.url);
  if (typeof url === "object" && url.protocol === "file:") {
    // Node, Deno, Bun: read the file next to this module.
    const { readFile } = await import("node:fs/promises");
    return (await WebAssembly.instantiate(await readFile(url), {})).instance;
  }
  const res = fetch(url);
  if (WebAssembly.instantiateStreaming) {
    try {
      return (await WebAssembly.instantiateStreaming(res, {})).instance;
    } catch {
      // Served without the application/wasm type: fall back below.
    }
  }
  const bytes = await (await fetch(url)).arrayBuffer();
  return (await WebAssembly.instantiate(bytes, {})).instance;
}

/**
 * Loads the WebAssembly module (once). `source` is optional: a URL, the
 * module's bytes or a compiled `WebAssembly.Module`; by default
 * `dataviz.wasm` next to this file.
 */
export function init(source) {
  ready ??= load(source).then((i) => {
    wasm = i.exports;
  });
  return ready;
}

function need() {
  if (!wasm) throw new Error("dataviz: call `await init()` first");
  return wasm;
}

function bytes(ptr) {
  const w = need();
  if (!ptr) throw new Error(`dataviz: ${text(w.dv_error())}`);
  return new Uint8Array(w.memory.buffer, ptr, w.dv_out_len()).slice();
}

function text(ptr) {
  const w = need();
  return dec.decode(new Uint8Array(w.memory.buffer, ptr, w.dv_out_len()));
}

/** Arrays at least this long go to WebAssembly memory as binary f64. */
const BINARY = 64;

const isNumbers = (v) => {
  for (let i = 0; i < v.length; i++) if (typeof v[i] !== "number") return false;
  return true;
};

/**
 * The spec with long numeric arrays (typed or plain) replaced by `"@k"`
 * references into `arrays`, so they are copied once as binary instead of
 * going through JSON text; short typed arrays become plain arrays.
 */
function split(v, arrays) {
  if (ArrayBuffer.isView(v)) {
    if (v instanceof DataView) return v;
    const big = typeof v[0] === "bigint";
    if (v.length >= BINARY && !big) {
      arrays.push(v);
      return `@${arrays.length - 1}`;
    }
    return Array.from(v, Number);
  }
  if (Array.isArray(v)) {
    if (v.length === 0) return v;
    if (typeof v[0] !== "object" || v[0] === null) {
      if (v.length >= BINARY && typeof v[0] === "number" && isNumbers(v)) {
        arrays.push(v);
        return `@${arrays.length - 1}`;
      }
      return v;
    }
    return v.map((x) => split(x, arrays));
  }
  if (v && typeof v === "object") {
    const o = {};
    for (const k in v) o[k] = split(v[k], arrays);
    return o;
  }
  return v;
}

/**
 * Calls `f(ptr, len, desc, n)` with a spec copied into WebAssembly memory:
 * JSON text plus its binary arrays, described by `n` records of five u32
 * at `desc` (name pointer and length, data pointer, length, row length).
 */
function withSpec(spec, f) {
  const w = need();
  const arrays = [];
  const json =
    typeof spec === "string" ? spec : JSON.stringify(split(spec, arrays));
  const b = enc.encode(json);
  const names = arrays.map((_, i) => enc.encode(String(i)));
  // One block, 8-aligned: [arrays (f64)] [records (u32)] [names] [json].
  let data = 0;
  for (const a of arrays) data += a.length * 8;
  let text = b.length;
  for (const n of names) text += n.length;
  const size = 8 + data + arrays.length * 20 + text;
  const p = w.dv_alloc(size);
  try {
    const base = (p + 7) & ~7;
    const mem = w.memory.buffer;
    const f64 = new Float64Array(mem);
    const u8 = new Uint8Array(mem);
    const descAt = base + data;
    const desc = new Uint32Array(mem, descAt, arrays.length * 5);
    let at = base;
    let nameAt = descAt + arrays.length * 20;
    arrays.forEach((a, i) => {
      f64.set(a, at / 8);
      desc.set([nameAt, names[i].length, at, a.length, 0], i * 5);
      u8.set(names[i], nameAt);
      nameAt += names[i].length;
      at += a.length * 8;
    });
    u8.set(b, nameAt);
    return f(nameAt, b.length, descAt, arrays.length);
  } finally {
    w.dv_free(p, size);
  }
}

const registry =
  typeof FinalizationRegistry === "function"
    ? new FinalizationRegistry((h) => wasm?.dv_drop(h))
    : null;

/** A chart: build it from a spec, then export it or draw it on a canvas. */
export class Chart {
  /** `spec`: an object or JSON string (see `chartTypes()`). */
  constructor(spec) {
    const h = withSpec(spec, (p, n, d, k) => need().dv_new_with(p, n, d, k));
    if (!h) throw new Error(`dataviz: ${text(need().dv_error())}`);
    this.handle = h;
    registry?.register(this, h, this);
  }

  /** Replaces the data and options (the size stays). */
  update(spec) {
    if (!withSpec(spec, (p, n, d, k) => need().dv_update_with(this.handle, p, n, d, k))) {
      throw new Error(`dataviz: ${text(need().dv_error())}`);
    }
    this._redraw?.();
    return this;
  }

  /** Canvas size in CSS pixels. */
  get width() {
    return need().dv_width(this.handle);
  }

  get height() {
    return need().dv_height(this.handle);
  }

  resize(width, height) {
    need().dv_resize(this.handle, width, height);
    this._redraw?.();
    return this;
  }

  /** SVG document. */
  svg() {
    return dec.decode(bytes(need().dv_svg(this.handle)));
  }

  /** PNG file bytes. */
  png() {
    return bytes(need().dv_png(this.handle));
  }

  /** Self-contained interactive HTML page (zoom, pan, hover, 3D turning). */
  html() {
    return dec.decode(bytes(need().dv_html(this.handle)));
  }

  /**
   * RGBA pixels at `scale` device pixels per CSS pixel:
   * `{ width, height, data }` (a view valid until the next call).
   */
  pixels(scale = 1) {
    const w = need();
    const p = w.dv_render(this.handle, scale);
    if (!p) throw new Error(`dataviz: ${text(w.dv_error())}`);
    const width = w.dv_px_width(this.handle);
    const height = w.dv_px_height(this.handle);
    const data = new Uint8ClampedArray(w.memory.buffer, p, width * height * 4);
    return { width, height, data };
  }

  /** Zooms around `(x, y)` (CSS pixels); `factor` < 1 zooms in. */
  zoom(x, y, factor) {
    return !!need().dv_zoom(this.handle, x, y, factor);
  }

  /** A drag at `(x, y)` by `(dx, dy)`: pans 2D plots, turns 3D ones. */
  pan(x, y, dx, dy) {
    return !!need().dv_pan(this.handle, x, y, dx, dy);
  }

  /** Zooms to the box between two points. */
  zoomBox(ax, ay, bx, by) {
    return !!need().dv_zoom_box(this.handle, ax, ay, bx, by);
  }

  /** Back to the first view. */
  reset() {
    need().dv_reset(this.handle);
  }

  /** Legend rows hide or show their series; true when something changed. */
  click(x, y) {
    return !!need().dv_click(this.handle, x, y);
  }

  /** The data point nearest `(x, y)`: `{ plot, x, y, text }` or null. */
  hover(x, y) {
    const s = text(need().dv_hover(this.handle, x, y));
    return s ? JSON.parse(s) : null;
  }

  /**
   * Draws on a `<canvas>` (sharp on high-DPI screens) and, unless
   * `interactive: false`, handles the wheel (zoom), dragging (pan, turn
   * 3D), Shift+drag (zoom to a box), double click (reset), legend clicks
   * and hover readouts. Returns a function that removes the handlers.
   */
  mount(canvas, { interactive = true, tooltip = true } = {}) {
    const ctx = canvas.getContext("2d");
    const css = () => [this.width, this.height];
    let frame = 0;
    let tip = null;
    const draw = () => {
      frame = 0;
      const dpr = globalThis.devicePixelRatio || 1;
      const [w, h] = css();
      const px = this.pixels(dpr);
      if (canvas.width !== px.width || canvas.height !== px.height) {
        canvas.width = px.width;
        canvas.height = px.height;
      }
      canvas.style.width = `${w}px`;
      canvas.style.height = `${h}px`;
      ctx.putImageData(new ImageData(px.data.slice(), px.width, px.height), 0, 0);
      if (tip) {
        const s = dpr;
        ctx.save();
        ctx.scale(s, s);
        ctx.font = "12px system-ui, sans-serif";
        const lines = tip.text.split("\n");
        const tw = Math.max(...lines.map((l) => ctx.measureText(l).width)) + 12;
        const th = lines.length * 16 + 8;
        const x = Math.min(tip.x + 10, w - tw - 2);
        const y = Math.max(tip.y - th - 10, 2);
        ctx.fillStyle = "rgba(255,255,255,0.95)";
        ctx.strokeStyle = "rgba(0,0,0,0.25)";
        ctx.beginPath();
        ctx.roundRect?.(x, y, tw, th, 4) ?? ctx.rect(x, y, tw, th);
        ctx.fill();
        ctx.stroke();
        ctx.fillStyle = "#222";
        lines.forEach((l, i) => ctx.fillText(l, x + 6, y + 18 + i * 16));
        ctx.beginPath();
        ctx.arc(tip.x, tip.y, 4, 0, Math.PI * 2);
        ctx.stroke();
        ctx.restore();
      }
    };
    const redraw = () => {
      frame ||= requestAnimationFrame(draw);
    };
    this._redraw = redraw;
    draw();
    if (!interactive) return () => (this._redraw = null);

    const pos = (e) => {
      const r = canvas.getBoundingClientRect();
      return [e.clientX - r.left, e.clientY - r.top];
    };
    let drag = null;
    const on = [
      ["wheel", (e) => {
        const [x, y] = pos(e);
        if (this.zoom(x, y, e.deltaY < 0 ? 0.85 : 1 / 0.85)) {
          e.preventDefault();
          redraw();
        }
      }, { passive: false }],
      ["pointerdown", (e) => {
        const [x, y] = pos(e);
        drag = { x, y, x0: x, y0: y, box: e.shiftKey, moved: false };
        canvas.setPointerCapture(e.pointerId);
      }],
      ["pointermove", (e) => {
        const [x, y] = pos(e);
        if (drag) {
          drag.moved ||= Math.hypot(x - drag.x0, y - drag.y0) > 3;
          if (!drag.box && this.pan(drag.x, drag.y, x - drag.x, y - drag.y)) {
            drag.x = x;
            drag.y = y;
            tip = null;
            redraw();
          }
          return;
        }
        if (tooltip) {
          tip = this.hover(x, y);
          redraw();
        }
      }],
      ["pointerup", (e) => {
        const [x, y] = pos(e);
        if (drag?.box && drag.moved) this.zoomBox(drag.x0, drag.y0, x, y);
        else if (drag && !drag.moved) this.click(x, y);
        drag = null;
        redraw();
      }],
      ["pointerleave", () => {
        if (!drag && tip) {
          tip = null;
          redraw();
        }
      }],
      ["dblclick", () => {
        this.reset();
        redraw();
      }],
    ];
    for (const [t, f, o] of on) canvas.addEventListener(t, f, o);
    canvas.style.touchAction = "none";
    return () => {
      for (const [t, f, o] of on) canvas.removeEventListener(t, f, o);
      if (frame) cancelAnimationFrame(frame);
      this._redraw = null;
    };
  }

  /** Frees the chart now (otherwise when it is garbage collected). */
  free() {
    if (this.handle) {
      registry?.unregister(this);
      need().dv_drop(this.handle);
      this.handle = 0;
    }
  }
}

/** The chart types: `[{ type, fields, description }]`. */
export function chartTypes() {
  return JSON.parse(text(need().dv_chart_types())).map(([type, fields, description]) => ({
    type,
    fields,
    description,
  }));
}

/** One-shot helpers: a spec to an SVG string or PNG bytes. */
export function svg(spec) {
  const c = new Chart(spec);
  try {
    return c.svg();
  } finally {
    c.free();
  }
}

export function png(spec) {
  const c = new Chart(spec);
  try {
    return c.png();
  } finally {
    c.free();
  }
}
