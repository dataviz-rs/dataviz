# Python

```sh
pip install dataviz
```

```python
import numpy as np
import dataviz as dv

x = np.linspace(0, 10, 100_000)
fig = dv.Figure(900, 500, title="Signal", theme="light")
ax = fig.axes()
ax.line(x, np.sin(x), label="sin").set(x_label="t [s]", legend="upper_right")
fig.save("signal.png")       # .svg .pdf .html …
fig                          # Jupyter shows it inline
```

NumPy arrays are read in place; lists, pandas and Polars series work too.
Methods: `line`, `scatter`, `area`, `fill_between`, `bar`, `barh`, `hist`,
`boxplot`, `violin`, `swarm`, `raincloud`, `ridgeline`, `kde`, `heatmap`,
`pie`, `candlestick`, `surface`, `scatter3d`, `line3d`, `molecule`,
`hline`, `vline`, `text`, `set`, `camera`, `rotate` — and
`chart(type, **fields)` for all 40 spec types (`dv.chart_types()`).

```python
fig = dv.Figure(1200, 450).grid(1, 3)
fig.subplot(0, 0).boxplot({"control": a, "treated": b})
fig.subplot(0, 1).chart("roc", labels=y_true, scores=y_score)
fig.subplot(0, 2).molecule("1ubq.pdb", style="cartoon").rotate("y", 30)
```

Speed against matplotlib (same process, 800×600 PNG): 21× for a 1 000-point
line, 17× for a 1 M-value histogram, 11× for a 1 M-point scatter, 7× for a
3D surface.
