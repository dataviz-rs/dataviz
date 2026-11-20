# dataviz for Python

Fast charts for Python, drawn by the [dataviz](https://github.com/dataviz-rs/dataviz)
Rust renderer: 5–22× faster than matplotlib for charts saved to PNG, with
120+ chart types — statistics, machine learning, finance, genomics, maps,
3D surfaces and molecules.

```sh
pip install dataviz-rs
```

The package is `dataviz-rs` on PyPI and is imported as `dataviz`.

```python
import numpy as np
import dataviz as dv

x = np.linspace(0, 10, 1_000_000)
fig = dv.Figure(900, 500, title="A million points")
ax = fig.axes()
ax.line(x, np.sin(x) + 0.1 * np.random.randn(x.size), label="signal")
ax.set(x_label="t [s]", y_label="value", legend="upper_right")
fig.save("signal.png")     # .png .svg .pdf .jpg .webp, .html (interactive)
fig                        # Jupyter shows it inline
```

NumPy arrays of any numeric type are read in place in one pass. Polars,
pyarrow and Arrow-backed pandas columns are read straight from their Arrow
buffers (the Arrow PyCapsule interface: no NumPy, no float64 copy in
between; missing values become gaps). Lists and anything else NumPy accepts
work too. 10 million rows from Polars take ~30 ms, 9–60× faster than
going through NumPy.

## Speed

Same Python process, 800×600 PNG, median of 6 runs on one core
([`benchmarks/bench_vs_matplotlib.py`](benchmarks/bench_vs_matplotlib.py); Intel Core Ultra 7 255HX):

| Chart | dataviz | matplotlib | faster |
|---|---:|---:|---:|
| line, 1 000 points | 1.3 ms | 28.9 ms | 21.7× |
| line, 100 000 points | 2.1 ms | 45.5 ms | 21.5× |
| line, 1 000 000 points | 18.8 ms | 88.2 ms | 4.7× |
| scatter, 10 000 points | 1.9 ms | 34.3 ms | 17.7× |
| scatter, 1 000 000 points | 49.2 ms | 539.9 ms | 11.0× |
| histogram, 1 000 000 values | 2.4 ms | 40.1 ms | 16.9× |
| 3D surface, 60 × 60 | 8.7 ms | 60.1 ms | 6.9× |

## Grammar of graphics (Altair / ggplot style)

`dv.plot` draws a chart from a DataFrame (pandas or Polars), a dict of
columns or a list of row dicts, a mark and encodings. Color groups,
legends, aggregation, histograms and small multiples follow from the
encodings. Numeric columns go to the renderer as buffers, not as JSON:
1 million rows take about 20 ms.

```python
dv.plot(penguins, "point", x="flipper", y="mass", color="species", size="bill")
dv.plot(penguins, "bar", x="island", y="mean(mass)", color="species", stack="grouped")
dv.plot(penguins, "bar", x={"field": "mass", "bin": 30}, color="species")   # histograms
dv.plot(sales, "area", x="date", y="sum(revenue)", color="region")
dv.plot(penguins, "point", x="bill", y="mass", color="sex", facet="species", columns=3)
fig.subplot(0, 1).encode(df, "boxplot", x="species", y="mass")              # into a plot
```

Marks: `point`, `line`, `area`, `bar`, `rect` (heatmap), `boxplot`.
Channels: `x`, `y`, `color`, `size`, `facet`. Each takes a column name,
Altair shorthand (`"mean(mass)"`, `"year:O"`, `"count()"`) or a dict
`{field, type, aggregate, bin, title, scale}`.

## matplotlib code, unchanged

```python
import dataviz.pyplot as plt     # instead of matplotlib.pyplot

fig, axs = plt.subplots(2, 2, figsize=(10, 7))
axs[0, 0].plot(x, np.sin(x), "r--", label="sin"); axs[0, 0].legend()
axs[0, 1].scatter(a, b, s=12, c=a * b, cmap="viridis")
axs[1, 0].hist(samples, bins=40); axs[1, 0].set_yscale("log")
axs[1, 1].bar(["a", "b", "c"], [3, 5, 2], color="tab:purple")
fig.suptitle("Report"); fig.savefig("report.png")
```

The common pyplot and `Axes` calls and their options work:

- **Drawing:** `plot` with format strings, `scatter`, `bar`, `barh`,
  `hist`, `fill_between`, `boxplot`, `violinplot`, `pie`, `imshow`,
  `axhline`, `axvline`, `text`
- **Axes:** `set_*` and the `xlabel`, `ylabel`, `xlim`, `ylim`,
  `xscale`, `yscale`, `title`, `suptitle`, `legend` and `grid` functions
- **Figures:** `subplots`, `subplot`, `figure(figsize=…)`, `savefig`,
  `show`, `style.use`

Unknown keyword arguments are ignored, so existing scripts keep running.
The 4-panel script above runs in 9 ms instead of 372 ms with
matplotlib, and a 1 million point line in 19 ms instead of 110 ms.

## Charts

| Method | Chart |
|---|---|
| `line`, `scatter`, `area`, `fill_between` | lines, points, bands |
| `bar`, `barh`, `hist` | bars (grouped by calling again), histograms |
| `boxplot`, `violin`, `swarm`, `raincloud`, `ridgeline`, `kde` | distributions of `{name: values}` |
| `heatmap`, `pie`, `candlestick` | matrices, parts of a whole, prices |
| `surface`, `scatter3d`, `line3d`, `molecule` | 3D (turn with `camera`, `rotate`) |
| `volume` | volume rendering of a 3D array `[z, y, x]` (composite or `mode="mip"`) |
| `hline`, `vline`, `text`, `set` | references, labels (TeX math), limits, log axes, legend |
| `chart(type, **fields)` | any of 40 types: sankey, chord, streamgraph, waffle, bullet, slope, pareto, SPC, ROC, calibration, confusion matrix, UpSet, volcano, phylogenetic tree, choropleth (also on a globe: `projection="orthographic"`, or equal-area `equalearth`, `mollweide`), arc diagram, edge bundling, DNA helix, … (`dv.chart_types()`) |

```python
fig = dv.Figure(1200, 450).grid(1, 3)
fig.subplot(0, 0).boxplot({"control": a, "treated": b}).set(title="Response")
fig.subplot(0, 1).chart("roc", labels=y_true, scores=y_score, name="model")
fig.subplot(0, 2).molecule("1ubq.pdb", style="cartoon").rotate("y", 30)
fig.save("report.png")
```

Themes: `dv.Figure(theme="high_contrast")` (black on white, larger text,
dark colors at least 4.5:1 against white), `dark`, `publication`, … HTML
output (`fig.save("chart.html")`) can be used from the keyboard and with
screen readers.

## Interactive in Jupyter

`fig.interactive()` shows the interactive page in the notebook: hover
readouts, zoom (wheel) and pan (drag), turning 3D plots, and linked
brushing — rows selected with Shift+drag in one plot are highlighted in all
of them. With [anywidget](https://anywidget.dev) installed (`pip install "dataviz-rs[widget]"`) it is a widget
whose `brushed` trait follows the selection, also kept by the figure:

```python
fig = dv.Figure(1100, 420).grid(1, 2).link_brushing()
fig.subplot(0, 0).scatter(df.price, df.volume)
fig.subplot(0, 1).scatter(df.price, df.returns)
w = fig.interactive()
w                                  # select points with Shift+drag
df.iloc[fig.brushed()]             # the selected rows
fig.save("selection.png")          # highlighted in saved images too
```

Without anywidget the same page is shown, without the link back to Python.
`fig` alone still displays a PNG (light notebooks, nbviewer, GitHub).

A whole figure can also come from a JSON spec (`dv.from_spec({...})`), the
format of the dataviz MCP server for AI assistants.

## License

MIT
