#!/usr/bin/env python3
"""Prints the README performance section from dataviz-examples/benchmarks/results.json."""
import json
import os
import sys

path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "../../dataviz-examples/benchmarks/results.json")
r = json.load(open(path))


def ms(v):
    return "fails" if v is None else (f"{v / 1000:.2f} s" if v >= 1000 else (f"{v:.1f} ms" if v >= 10 else f"{v:.2f} ms"))


def bold(cells, vals):
    m = min(v for v in vals if v is not None)
    return [f"**{c}**" if v == m else c for c, v in zip(cells, vals)]


def label(k):
    sc, n = k.split(" ")
    n = int(n.replace(",", ""))
    names = {"line": "line", "bar": "bar", "scatter": "scatter", "hist": "histogram"}
    unit = "bars" if sc == "bar" else ("values" if sc == "hist" else "points")
    num = f"{n // 1_000_000} M" if n >= 1_000_000 and n % 1_000_000 == 0 else f"{n:,}".replace(",", " ")
    return f"{names[sc]}, {num} {unit}"


def ratio(a, b):
    return b / a if a and b else None


libs = ["dataviz", "plotters", "ruviz", "kuva", "charts-rs", "matplotlib"]
st, bd, rt, an = r["static"], r["bigdata"], r["realtime"], r["animation"]
mpl_static = [ratio(row["dataviz"]["ms"], row["matplotlib"]["ms"]) for row in st.values() if row.get("matplotlib")]
mpl_rt = ratio(rt["800x600 100,000"]["dataviz 1 thread"]["p50_ms"], rt["800x600 100,000"]["matplotlib"]["p50_ms"])
mem = ratio(bd["line 10,000,000"]["dataviz"]["extra_mb"], bd["line 10,000,000"]["matplotlib"]["extra_mb"])
L = ["## Performance\n",
     f"Measured against **matplotlib** (Python) and the Rust libraries plotters, ruviz, kuva and charts-rs: "
     f"the same charts, separate processes, medians ({r['machine']['cpu']}, {r['machine']['gpu']}, Linux). "
     "Full results, method and the code to rerun them: "
     "[dataviz-examples/benchmarks](https://github.com/dataviz-rs/dataviz-examples/tree/main/benchmarks).\n",
     f"> **Compared with matplotlib**: {min(mpl_static):.0f}–{max(mpl_static):.0f}× faster for charts saved to PNG, "
     f"{mpl_rt:.0f}× faster for a live 100 000-point chart, and {mem:.0f}× less memory for 10 M points.\n",
     "**Charts to an 800×600 PNG** (one thread, time per chart):\n",
     "| Chart | " + " | ".join(libs) + " |", "|---|" + "---:|" * len(libs)]
for k, row in st.items():
    vals = [row[n]["ms"] if row.get(n) else None for n in libs]
    L.append(f"| {label(k)} | " + " | ".join(bold([ms(v) for v in vals], vals)) + " |")
cols = ["dataviz", "dataviz all cores"] + libs[1:]
L += ["\n**Big data** (time / memory above the data itself):\n",
      "| Chart | " + " | ".join(cols) + " |", "|---|" + "---:|" * len(cols)]
for k, row in bd.items():
    vals = [row[n]["ms"] if row.get(n) else None for n in cols]
    cells = [f"{ms(row[n]['ms'])} / +{row[n]['extra_mb']:.0f} MB" if row.get(n) else "fails" for n in cols]
    L.append(f"| {label(k)} | " + " | ".join(bold(cells, vals)) + " |")
names = ["dataviz 1 thread", "dataviz all cores", "dataviz GPU", "plotters", "ruviz", "kuva", "matplotlib"]
L += ["\n**Live chart** (rolling window, one new point and a full redraw per frame; latency p50 / CPU per frame / memory of the process):\n",
      "| Window | " + " | ".join(names) + " |", "|---|" + "---:|" * len(names)]
for k, row in rt.items():
    vals = [row[n]["p50_ms"] if row.get(n) else None for n in names]
    cells = [f"{ms(row[n]['p50_ms'])} / {ms(row[n]['cpu_ms'])} / {row[n]['rss_mb']:.0f} MB" if row.get(n) else "–" for n in names]
    size, n = k.split(" ")
    L.append(f"| {n.replace(',', ' ')} points, {size.replace('x', '×')} | " + " | ".join(bold(cells, vals)) + " |")
al = ["dataviz", "plotters", "ruviz", "matplotlib"]
L += ["\n**Animated GIF** (800×600, 120 frames; time, file size):\n",
      "| Animation | " + " | ".join(al) + " |", "|---|" + "---:|" * len(al)]
for sc, lab in [("wave", "whole plot moves"), ("stream", "line grows on fixed axes")]:
    row = an[sc]
    vals = [row[l]["ms"] if row.get(l) else None for l in al]
    cells = [f"{ms(row[l]['ms'])}, {row[l]['kb']} KB" if row.get(l) else "–" for l in al]
    L.append(f"| {lab} | " + " | ".join(bold(cells, vals)) + " |")
td = r.get("threed")
if td:
    tl = [c for c in ["dataviz", "dataviz GPU", "plotters", "matplotlib", "PyVista"] if any(c in row for row in td.values())]
    L += ["\n**3D charts** (800×600 PNG; dataviz, plotters and matplotlib on one core, PyVista on the GPU; "
          "a rotation frame is the camera turned and the image drawn again):\n",
          "| Chart | " + " | ".join(tl) + " |", "|---|" + "---:|" * len(tl)]
    for k, row in td.items():
        vals = [row[c]["ms"] if row.get(c) else None for c in tl]
        cells = [(ms(row[c]["ms"]) + (f", {row[c]['kb']} KB" if "kb" in row[c] else "")) if row.get(c) else "–" for c in tl]
        L.append(f"| {k} | " + " | ".join(bold(cells, vals)) + " |")
L.append("""
What makes it fast: the scene is drawn straight into the output buffer by
dataviz's own anti-aliased rasterizer; dense lines become one pixel-column
envelope and dense scatter plots an occupancy grid, so time and memory stay
flat as the data grows; data passed to a series is not copied; glyphs and
markers are cached as bitmaps; histograms use SIMD binning. Animations store
only the changed part of each frame. On the web the same renderer compiled to
WebAssembly draws a 960×520 live chart with 2 × 3 000 points in about 1.7 ms.

In 3D dataviz is 35–52× faster than matplotlib for scatter plots, 5–23×
for surfaces and 4–9× for lines, molecules and rotation frames; it draws
a turning GIF 3.2× faster than plotters and 7.4× faster than PyVista and
is first (on the CPU or the GPU) for both surfaces and for scatter plots
up to a million points, with lighting, depth order and anti-aliasing.
plotters is faster for the long 3D line and the molecule: it draws
1-pixel lines and flat circles without depth sorting or lighting, while
dataviz shades every atom as a lit sphere. PyVista (VTK) keeps the scene
on the GPU and turns it in 0.6 ms per frame against dataviz's 2–5 ms
(still 200–500 frames per second), but needs 80–100 ms to save an image
and about 280 MB of memory.

Where it is not first: for a tiny live chart (1 000 points) on a single
thread plotters is 0.1–0.2 ms faster per frame; dataviz on all cores is
the fastest there. The GPU backend needs the least CPU time but ~240 MB of
driver memory and ~0.3 s to start, so the default stays on the CPU.""")
print("\n".join(L))
