"""dataviz vs matplotlib from Python: the same charts, 800×600 PNG in memory.

    pip install dataviz-rs matplotlib
    taskset -c 2 python benchmarks/bench_vs_matplotlib.py   # one core
"""
import time, io, statistics
import numpy as np
import dataviz as dv
import matplotlib
matplotlib.use("Agg")
matplotlib.rcParams["agg.path.chunksize"] = 10000  # needed for 1M-point lines
import matplotlib.pyplot as plt

rng = np.random.default_rng(1)
def med(f, n=7):
    ts = []
    for _ in range(n):
        t = time.perf_counter(); f(); ts.append((time.perf_counter() - t) * 1000)
    return statistics.median(ts[1:])

cases = {}
for n in [1_000, 100_000, 1_000_000]:
    x = np.linspace(0, 100, n); y = np.sin(x * 0.3) * 10 + rng.random(n)
    def d(x=x, y=y): f = dv.Figure(800, 600); a = f.axes(); a.line(x, y, label="line"); a.set(title="line", x_label="x", y_label="y"); f.to_png()
    def m(x=x, y=y):
        fig = plt.figure(figsize=(8, 6), dpi=100); ax = fig.add_subplot(); ax.plot(x, y, label="line"); ax.set_title("line"); ax.set_xlabel("x"); ax.set_ylabel("y")
        b = io.BytesIO(); fig.savefig(b, format="png"); plt.close(fig)
    cases[f"line {n:,}"] = (d, m)
for n in [10_000, 1_000_000]:
    x = rng.standard_normal(n); y = x * 0.6 + rng.standard_normal(n) * 0.5
    def d(x=x, y=y): f = dv.Figure(800, 600); f.axes().scatter(x, y, size=4); f.to_png()
    def m(x=x, y=y):
        fig = plt.figure(figsize=(8, 6), dpi=100); ax = fig.add_subplot(); ax.scatter(x, y, s=(4*72/100)**2)
        b = io.BytesIO(); fig.savefig(b, format="png"); plt.close(fig)
    cases[f"scatter {n:,}"] = (d, m)
v = rng.standard_normal(1_000_000)
def d(): f = dv.Figure(800, 600); f.axes().hist(v, bins=50); f.to_png()
def m():
    fig = plt.figure(figsize=(8, 6), dpi=100); ax = fig.add_subplot(); ax.hist(v, bins=50)
    b = io.BytesIO(); fig.savefig(b, format="png"); plt.close(fig)
cases["hist 1,000,000"] = (d, m)
g = np.linspace(-8, 8, 60); X, Y = np.meshgrid(g, g); Z = np.sin(np.hypot(X, Y))
def d(): f = dv.Figure(800, 600); f.axes().surface(Z, (-8, 8), (-8, 8)); f.to_png()
def m():
    fig = plt.figure(figsize=(8, 6), dpi=100); ax = fig.add_subplot(projection="3d"); ax.plot_surface(X, Y, Z, cmap="viridis", rcount=60, ccount=60)
    b = io.BytesIO(); fig.savefig(b, format="png"); plt.close(fig)
cases["3D surface 60²"] = (d, m)
print(f"{'chart':18} {'dataviz':>10} {'matplotlib':>11} {'speed-up':>9}")
for k, (d, m) in cases.items():
    a, b = med(d), med(m, 5)
    print(f"{k:18} {a:8.1f} ms {b:9.1f} ms {b/a:8.1f}×")
