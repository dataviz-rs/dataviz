import json
import math
import os

import numpy as np
import pytest

import dataviz as dv


def png_ok(fig):
    data = fig.to_png()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    return data


def test_methods_draw():
    x = np.linspace(0, 10, 200)
    fig = dv.Figure(1200, 900, title="All methods", theme="light").grid(3, 4)
    fig.subplot(0, 0).line(x, np.sin(x), label="sin", dash="--", marker="o").set(legend="upper_right")
    fig.subplot(0, 1).scatter(x, np.cos(x), size=4, opacity=0.5).area(x, np.abs(np.sin(x)))
    fig.subplot(0, 2).fill_between(x, np.sin(x) - 0.2, np.sin(x) + 0.2).hline(0.0, label="zero").vline(5.0)
    fig.subplot(0, 3).bar(["a", "b", "c"], [3, 5, 2], label="2024").bar(["a", "b", "c"], [4, 4, 3], label="2025")
    fig.subplot(1, 0).barh(["x", "y"], [1, 2]).hist(np.random.default_rng(1).normal(size=500), bins=20)
    groups = {"a": np.random.default_rng(2).normal(size=100), "b": [1, 2, 3, 4, 5]}
    fig.subplot(1, 1).boxplot(groups)
    fig.subplot(1, 2).violin(groups)
    fig.subplot(1, 3).raincloud(list(groups.items()))
    fig.subplot(2, 0).heatmap(np.arange(12).reshape(3, 4), annotate=True, colormap="rdbu", colorbar="v")
    fig.subplot(2, 1).pie(["p", "q"], [1, 2], donut=0.5)
    fig.subplot(2, 2).surface(np.outer(np.sin(x[:20]), np.cos(x[:20])), (0, 1), (0, 1), colormap="plasma").camera(-40, 25).rotate("z", 20)
    fig.subplot(2, 3).scatter3d(np.arange(10), np.arange(10) ** 0.5, np.sin(np.arange(10)), values=np.arange(10), colormap="turbo")
    png_ok(fig)
    assert "line" in fig.alt_text().lower()


def test_inputs_lists_ranges_and_non_contiguous_arrays():
    a = np.arange(20.0)[::2]  # not contiguous
    fig = dv.Figure(400, 300)
    fig.axes().line(range(10), a).scatter([1, 2, 3], (3, 1, 2)).line(np.arange(5, dtype=np.int32))
    png_ok(fig)


def test_every_chart_type():
    cases = {
        "sankey": dict(links=[["a", "b", 3], ["a", "c", 1]]),
        "chord": dict(labels=["a", "b"], matrix=np.array([[0, 2], [1, 0]])),
        "streamgraph": dict(x=[0, 1, 2], layers={"a": [1, 2, 1], "b": [2, 1, 2]}),
        "waffle": dict(labels=["a", "b"], values=[60, 40]),
        "bullet": dict(labels=["k"], values=[5], targets=[6], ranges=[[3, 6, 9]]),
        "roc": dict(labels=np.array([True, False, True, False]), scores=np.array([0.9, 0.2, 0.7, 0.4])),
        "upset": dict(sets={"A": ["1", "2"], "B": ["2", "3"]}),
        "volcano": dict(names=["g1", "g2"], log2fc=[2, -1], pvalues=[0.001, 0.5]),
        "phylo_tree": dict(newick="((A:1,B:2):1,C:3);", circular=True),
        "choropleth": dict(values={"DEU": 1, "TUR": 2}),
        "dna_helix": dict(sequence="ATGCGT"),
        "control_chart": dict(values=np.array([1, 2, 1.5, 9, 1.2])),
        "radar": dict(labels=["a", "b", "c"], series={"x": [1, 2, 3], "y": [3, 2, 1]}),
    }
    names = {t[0] for t in dv.chart_types()}
    assert len(names) >= 40
    for kind, fields in cases.items():
        assert kind in names
        fig = dv.Figure(400, 300)
        fig.axes().chart(kind, **fields)
        png_ok(fig)


def test_errors_are_value_errors():
    with pytest.raises(ValueError, match="unknown type"):
        dv.Figure().axes().chart("nope")
    with pytest.raises(ValueError):
        dv.Figure().axes().rotate("w", 10)


def test_outputs(tmp_path):
    fig = dv.Figure(300, 200)
    fig.axes().line([1, 2, 3], [3, 1, 2])
    for ext in ["png", "svg", "pdf", "html"]:
        p = tmp_path / f"c.{ext}"
        fig.save(p)
        assert p.stat().st_size > 100
    assert fig.to_svg().lstrip().startswith("<svg") or "<svg" in fig.to_svg()[:200]
    assert "<html" in fig.to_html().lower()
    arr = fig.to_array()
    assert arr.shape == (200, 300, 4) and arr.dtype == np.uint8
    assert fig._repr_png_()[:4] == b"\x89PNG"


def test_from_spec():
    fig = dv.from_spec({"title": "Spec", "panels": [
        {"type": "bar", "categories": ["a", "b"], "values": np.array([1, 2])},
        {"layers": [{"type": "scatter", "x": [1, 2], "y": [2, 1]}, {"type": "line", "x": [1, 2], "y": [1, 2]}]},
    ]})
    png_ok(fig)
    assert json.loads(json.dumps({"ok": True}))["ok"]


def test_molecule_from_text():
    pdb = (
        "ATOM      1  CA  ALA A   1      11.639   6.071  -5.147  1.00  0.00           C\n"
        "ATOM      2  CA  GLY A   2      15.264   6.410  -4.153  1.00  0.00           C\n"
    )
    fig = dv.Figure(300, 300)
    fig.axes().molecule(pdb, style="backbone")
    png_ok(fig)
    here = os.path.dirname(__file__)
    ubq = os.path.join(here, "..", "..", "..", "dataviz-examples", "plot-biology-3d", "data", "1UBQ.pdb")
    if os.path.exists(ubq):
        fig = dv.Figure(400, 400)
        fig.axes().molecule(ubq, style="cartoon", coloring="rainbow").rotate("y", 30)
        png_ok(fig)


def test_linked_brushing_and_interactive_page():
    x = np.arange(20.0)
    fig = dv.Figure(600, 300).grid(1, 2).link_brushing()
    fig.subplot(0, 0).scatter(x, x ** 2)
    fig.subplot(0, 1).scatter(x, -x)
    fig.brush([3, 4, 99], n=20)
    assert fig.brushed() == [3, 4]
    html = fig.to_html()
    assert '"brush":{"rows":20,"sel":[3,4]}' in html and "postMessage" in html
    assert 'id="p1-brush"' in fig.to_svg()
    fig.clear_brush()
    assert fig.brushed() == []
    page = fig.interactive()
    # anywidget is optional: a widget, or a plain HTML display without it.
    assert hasattr(page, "html") or "srcdoc" in page.data


def _same(got, want):
    return len(got) == len(want) and all(
        (math.isnan(a) and math.isnan(b)) or a == b for a, b in zip(got, want)
    )


def test_numeric_numpy_types_in_one_pass():
    from dataviz._dataviz import _values

    assert _values(np.array([1.5, 2, 3], np.float32)) == [1.5, 2.0, 3.0]
    assert _values(np.array([1, -2], np.int64)) == [1.0, -2.0]
    assert _values(np.arange(10, dtype=np.uint8)[::3]) == [0.0, 3.0, 6.0, 9.0]


def test_arrow_columns_with_nulls_and_chunks():
    pl = pytest.importorskip("polars")
    pa = pytest.importorskip("pyarrow")
    from dataviz._dataviz import _values

    nan = float("nan")
    assert _same(_values(pl.Series([1.0, None, 3.0])), [1.0, nan, 3.0])
    chunked = pl.concat([pl.Series([1.0, 2.0]), pl.Series([3.0])], rechunk=False)
    assert _values(chunked) == [1.0, 2.0, 3.0]
    assert _values(pl.Series(range(10)).slice(3, 4)) == [3.0, 4.0, 5.0, 6.0]
    assert _values(pl.Series([True, False])) == [1.0, 0.0]
    assert _same(_values(pa.array([1, None, 5], pa.int16())), [1.0, nan, 5.0])
    assert _same(_values(pa.chunked_array([[1.0], [None, 4.0]])), [1.0, nan, 4.0])
    fig = dv.Figure(300, 200)
    fig.axes().line(pl.Series([1.0, 2.0, 3.0]), pa.array([3, 1, 2]))
    png_ok(fig)


def test_volume_globe_and_networks():
    z, y, x = np.mgrid[-1:1:20j, -1:1:20j, -1:1:20j]
    fig = dv.Figure(500, 400)
    fig.axes().volume(np.exp(-(x * x + y * y + z * z) * 4), mode="mip")
    png_ok(fig)
    fig = dv.Figure(500, 400)
    fig.axes().chart("choropleth", values={"TUR": 85, "BRA": 214}, projection="orthographic",
                     center_lon=30, center_lat=20)
    png_ok(fig)
    fig = dv.Figure(500, 400)
    fig.axes().chart("edge_bundling", nodes=["a", "b", "c"], groups=[0, 0, 1],
                     edges=[["a", "c"], ["b", "c", 2]])
    png_ok(fig)
