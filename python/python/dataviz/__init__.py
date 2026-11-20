"""dataviz: fast charts for Python, drawn by a Rust renderer.

    import numpy as np
    import dataviz as dv

    x = np.linspace(0, 10, 1000)
    fig = dv.Figure(900, 500, title="Signals")
    ax = fig.axes()
    ax.line(x, np.sin(x), label="sin")
    ax.scatter(x[::50], np.cos(x[::50]), label="cos samples", size=6)
    ax.set(x_label="t [s]", y_label="value")
    fig.save("signals.png")      # .png .svg .pdf .jpg .webp .html
    fig                          # in Jupyter: shown inline

Numbers can be NumPy arrays of any numeric type (read in place), Polars,
pyarrow or Arrow-backed pandas columns (read from their Arrow buffers;
nulls become gaps), lists — anything ``numpy.asarray`` accepts. Beyond the methods below,
``Axes.chart(type, **fields)`` draws any of 40 chart types (sankey, chord,
ROC, UpSet, volcano, phylogenetic trees, choropleth maps, DNA helices, …);
``dataviz.chart_types()`` lists them with their fields.
"""

from __future__ import annotations

import json as _json
import os as _os
import tempfile as _tempfile
from typing import Any, Iterable, Mapping, Optional, Sequence, Tuple, Union

from ._dataviz import Core as _Core
from ._dataviz import DensityGrid
from ._dataviz import __version__
from ._dataviz import chart_types as _chart_types

__all__ = ["Figure", "Axes", "DensityGrid", "chart_types", "from_spec", "plot", "__version__"]

Groups = Union[Mapping[str, Any], Iterable[Tuple[str, Any]]]


def chart_types() -> list:
    """The chart types of :meth:`Axes.chart`: ``(name, fields, description)``."""
    return _chart_types()


_BINARY = 64  # numeric arrays at least this long skip JSON


def _numeric(o: Any) -> bool:
    """A long numeric array (NumPy, pandas, Polars, pyarrow, array.array)."""
    if isinstance(o, (str, bytes, dict)) or not hasattr(o, "__len__"):
        return False
    dtype = getattr(o, "dtype", None)
    if dtype is None:
        return False
    try:
        if len(o) < _BINARY:
            return False
    except TypeError:
        return False
    kind = getattr(dtype, "kind", None)
    if kind is not None:
        return kind in "iufb"
    name = str(dtype).lower()
    return any(t in name for t in ("int", "float", "bool", "uint"))


def _split(o: Any, arrays: dict) -> Any:
    """The spec with long numeric arrays replaced by ``"@k"`` references
    into ``arrays`` (read from their buffers instead of as JSON text)."""
    if _numeric(o):
        name = str(len(arrays))
        arrays[name] = o
        return "@" + name
    if isinstance(o, Mapping):
        return {k: _split(v, arrays) for k, v in o.items()}
    if isinstance(o, (list, tuple)):
        return [_split(v, arrays) for v in o]
    return o


def _plain(o: Any) -> Any:
    """JSON-compatible values (NumPy arrays and scalars, pandas series)."""
    if hasattr(o, "tolist"):
        return o.tolist()
    if hasattr(o, "to_list"):
        return o.to_list()
    if isinstance(o, (set, tuple, range)):
        return list(o)
    if hasattr(o, "isoformat"):
        # datetime, date, pandas Timestamp.
        return o.isoformat()
    raise TypeError(f"cannot convert {type(o).__name__} to chart data")


class Axes:
    """One plot of a :class:`Figure` (get it with ``fig.axes()`` or
    ``fig.subplot(row, col)``). Every method returns the axes, so calls
    chain."""

    def __init__(self, fig: "Figure", cell: Optional[Tuple[int, int, int, int]]):
        self._fig = fig
        self._cell = cell

    @property
    def _core(self) -> _Core:
        return self._fig._core

    # ----- lines and points -------------------------------------------------

    def line(self, x, y=None, *, label=None, color=None, width=None, dash=None, marker=None) -> "Axes":
        """A line through ``(x, y)`` (or ``y`` against its index). ``dash``:
        "solid", "dashed"/"--", "dotted"/":", "dashdot"/"-."; ``marker``: "o",
        "s", "^", "D", "+", "x"."""
        if y is None:
            x, y = None, x
        self._core.xy(self._cell, "line", x, y, label, color, width, None, dash, marker, None)
        return self

    def scatter(self, x, y, *, label=None, color=None, size=None, marker=None, opacity=None,
                values=None, colormap=None, colorbar=None) -> "Axes":
        """Points at ``(x, y)``; ``size`` in pixels. Millions of points are
        fine. ``values`` color the points through ``colormap`` (default
        viridis) with a ``colorbar`` title."""
        self._core.xy(self._cell, "scatter", x, y, label, color, None, size, None, marker, opacity)
        if values is not None:
            self._core.color_by(self._cell, values, colormap, colorbar)
        return self

    def area(self, x, y=None, *, label=None, color=None) -> "Axes":
        """A line filled down to zero."""
        if y is None:
            x, y = None, x
        self._core.xy(self._cell, "area", x, y, label, color, None, None, None, None, None)
        return self

    def fill_between(self, x, y1, y2, *, label=None, color=None, opacity=None) -> "Axes":
        """The band between two curves (confidence intervals, ranges)."""
        self._core.fill_between(self._cell, x, y1, y2, label, color, opacity)
        return self

    # ----- categories and distributions ---------------------------------------

    def bar(self, categories, heights, *, label=None, color=None) -> "Axes":
        """Vertical bars; call again with the same categories for grouped bars."""
        self._core.bar(self._cell, categories, heights, False, label, color)
        return self

    def barh(self, categories, widths, *, label=None, color=None) -> "Axes":
        """Horizontal bars."""
        self._core.bar(self._cell, categories, widths, True, label, color)
        return self

    def hist(self, values, *, bins: Optional[int] = None, label=None, color=None, density: bool = False) -> "Axes":
        """Histogram (automatic bins unless ``bins`` is given)."""
        self._core.hist(self._cell, values, bins, label, color, density)
        return self

    def boxplot(self, groups: Groups) -> "Axes":
        """Box plots of ``{name: values}`` (or ``[(name, values), …]``)."""
        self._core.distribution(self._cell, "box", groups)
        return self

    def violin(self, groups: Groups) -> "Axes":
        """Violin plots of ``{name: values}``."""
        self._core.distribution(self._cell, "violin", groups)
        return self

    def swarm(self, groups: Groups) -> "Axes":
        """Beeswarm plots of ``{name: values}``."""
        self._core.distribution(self._cell, "swarm", groups)
        return self

    def raincloud(self, groups: Groups) -> "Axes":
        """Half violin, box and points per group."""
        self._core.distribution(self._cell, "raincloud", groups)
        return self

    def ridgeline(self, groups: Groups) -> "Axes":
        """Densities of the groups on stacked rows."""
        self._core.distribution(self._cell, "ridgeline", groups)
        return self

    def kde(self, groups: Groups) -> "Axes":
        """Kernel density curves of ``{name: values}``."""
        self._core.distribution(self._cell, "kde", groups)
        return self

    # ----- grids, parts, finance -------------------------------------------------

    def heatmap(self, z, *, x_labels=None, y_labels=None, colormap=None, annotate: bool = False,
                colorbar: Optional[str] = None, range: Optional[Tuple[float, float]] = None) -> "Axes":
        """Heatmap of a 2D array (rows top to bottom), e.g. a correlation matrix.
        ``colormap``: viridis, plasma, magma, inferno, cividis, turbo, coolwarm,
        rdbu, blues, reds, greens, ylgnbu, brbg, gray."""
        self._core.heatmap(self._cell, z, x_labels, y_labels, colormap, annotate, colorbar, range)
        return self

    def pie(self, labels, values, *, donut: Optional[float] = None) -> "Axes":
        """Pie chart; ``donut`` is the hole size (0..1)."""
        self._core.pie(self._cell, labels, values, donut)
        return self

    def candlestick(self, x, open, high, low, close) -> "Axes":
        """Candlesticks (x: numbers or timestamps in seconds)."""
        self._core.candlestick(self._cell, x, open, high, low, close)
        return self

    # ----- 3D ------------------------------------------------------------------------

    def surface(self, z, x_range: Tuple[float, float], y_range: Tuple[float, float], *, colormap=None) -> "Axes":
        """3D surface of ``z[row][col]`` over the ranges (turn it with
        :meth:`camera` / :meth:`rotate`)."""
        self._core.surface(self._cell, z, x_range, y_range, colormap)
        return self

    def density(self, x, y=None, *, how="eq_hist", colormap=None, colorbar=None, bin_pixels=None) -> "Axes":
        """A density image of many points (millions to billions): points are
        counted per screen pixel at every zoom and colored — the whole
        distribution where a scatter would be a blob. ``how``: "eq_hist"
        (histogram equalization, default), "log" or "linear". ``x`` can also
        be a :class:`DensityGrid` filled chunk by chunk (data larger than
        memory)."""
        if isinstance(x, DensityGrid):
            self._core.density_grid(self._cell, x, how, colormap, colorbar)
        else:
            self._core.density(self._cell, x, y, how, colormap, colorbar, bin_pixels)
        return self

    def volume(self, values, *, extent=None, colormap=None, opacity=None, threshold=None, mode="composite") -> "Axes":
        """Volume rendering of a 3D array ``values[z, y, x]`` (densities,
        CT/MRI scans, simulation fields) or of a file: NIfTI (``.nii``,
        ``.nii.gz``), VTK structured points (``.vtk``), MetaImage (``.mhd``,
        ``.mha``) or DICOM (a directory of slices or a multi-frame ``.dcm``),
        in its physical units. ``extent``: ((x0, x1), (y0, y1),
        (z0, z1)) in data units; ``opacity`` per grid cell (default 0.25);
        values below ``threshold`` (fraction of the range, default 0.1) are
        transparent; ``mode``: "composite" or "mip" (maximum intensity)."""
        if isinstance(values, (str, _os.PathLike)):
            # A scan or simulation file (.nii, .nii.gz, .vtk, .mhd, .mha,
            # .dcm) or a directory of DICOM slices.
            fields = {k: v for k, v in (("colormap", colormap), ("opacity", opacity),
                                        ("threshold", threshold)) if v is not None}
            if mode in ("mip", "max"):
                fields["mode"] = "mip"
            return self.chart("volume", path=_os.fspath(values), **fields)
        import numpy as _np

        a = _np.asarray(values)
        if a.ndim != 3:
            raise ValueError("volume: values must be a 3D array [z, y, x]")
        nz, ny, nx = a.shape
        self._core.volume(self._cell, _np.ascontiguousarray(a).ravel(), [nx, ny, nz],
                          extent, colormap, opacity, threshold, mode in ("mip", "max"))
        return self

    def scatter3d(self, x, y, z, *, values=None, colormap=None, size=None, color=None, label=None) -> "Axes":
        """3D points, colored by ``values`` (or z) with a colormap."""
        self._core.xyz(self._cell, x, y, z, False, values, colormap, size, color, label)
        return self

    def line3d(self, x, y, z, *, values=None, colormap=None, color=None, label=None) -> "Axes":
        """A 3D line (trajectory)."""
        self._core.xyz(self._cell, x, y, z, True, values, colormap, None, color, label)
        return self

    def molecule(self, pdb: str, *, style: Optional[str] = None, coloring: Optional[str] = None) -> "Axes":
        """A molecule from PDB text or a ``.pdb`` path. ``style``: cartoon,
        ball_and_stick, sticks, space_filling, backbone; ``coloring``:
        element, chain, rainbow."""
        if len(pdb) < 260 and _os.path.exists(pdb):
            with open(pdb) as f:
                pdb = f.read()
        self._core.molecule(self._cell, pdb, style, coloring)
        return self

    def camera(self, azimuth: float, elevation: float) -> "Axes":
        """View of a 3D plot from azimuth and elevation (degrees)."""
        self._core.camera(self._cell, azimuth, elevation)
        return self

    def rotate(self, axis: str, degrees: float) -> "Axes":
        """Turns a 3D plot around the "x", "y" or "z" axis."""
        self._core.rotate(self._cell, axis, degrees)
        return self

    # ----- annotations and options ---------------------------------------------------

    def hline(self, y: float, *, label=None, color=None) -> "Axes":
        """A horizontal reference line."""
        self._core.reference(self._cell, "h", y, label, color)
        return self

    def vline(self, x: float, *, label=None, color=None) -> "Axes":
        """A vertical reference line."""
        self._core.reference(self._cell, "v", x, label, color)
        return self

    def _drawing(self, d: dict) -> "Axes":
        self._core.chart_json(self._cell, _json.dumps({"layers": [], "drawings": [d]}, default=_plain))
        return self

    def trendline(self, start, end, *, ray: bool = False, color=None) -> "Axes":
        """A trend line between two ``(x, y)`` points (``ray=True``: continued
        to the plot edge). In a window: ``T`` and drag."""
        return self._drawing({"type": "ray" if ray else "trend", "from": list(start),
                              "to": list(end), "color": color})

    def price_level(self, y: float, *, color=None) -> "Axes":
        """A horizontal price level with its value tagged at the right edge
        (``L`` in a window)."""
        return self._drawing({"type": "level", "y": y, "color": color})

    def fib_retracement(self, start, end, *, color=None) -> "Axes":
        """Fibonacci retracement levels of the move from ``start`` to ``end``
        (``F`` in a window)."""
        return self._drawing({"type": "fib", "from": list(start), "to": list(end), "color": color})

    def text(self, x: float, y: float, text: str, *, size=None, color=None) -> "Axes":
        """Text at a data position (TeX math between $…$ works)."""
        self._core.text(self._cell, x, y, text, size, color)
        return self

    def set(self, *, title=None, x_label=None, y_label=None, z_label=None, x_limits=None, y_limits=None,
            x_log=None, y_log=None, legend=None, grid=None, equal_aspect=None) -> "Axes":
        """Titles, labels, limits, log axes, legend position ("best",
        "upper_right", "upper_left", "lower_left", "lower_right",
        "outside_right", "outside_bottom", "hidden"), grid, equal aspect."""
        self._core.set(self._cell, title, x_label, y_label, z_label, x_limits, y_limits, x_log, y_log,
                       legend, grid, equal_aspect)
        return self

    def chart(self, type: str, **fields: Any) -> "Axes":
        """Any of the 40 spec chart types, e.g.
        ``ax.chart("sankey", links=[["coal", "power", 30], ["power", "homes", 25]])``
        or ``ax.chart("roc", labels=y_true, scores=y_score)``. See
        :func:`chart_types` for the types and their fields."""
        arrays: dict = {}
        spec = _split({"type": type, **fields}, arrays)
        self._core.chart_json(self._cell, _json.dumps(spec, default=_plain), arrays)
        return self

    def encode(self, data: Any, mark: str = "point", **encoding: Any) -> "Axes":
        """Grammar of graphics into this plot: ``data`` (pandas or Polars
        DataFrame, ``{column: values}`` or a list of row dicts), a ``mark``
        (point, line, area, bar, rect, boxplot) and channels x, y, color,
        size, facet — see :func:`plot`."""
        spec, arrays = _grammar(data, mark, encoding)
        self._core.chart_json(self._cell, _json.dumps(spec, default=_plain), arrays)
        return self


class Figure:
    """A figure of one or more plots.

    ``Figure(width=900, height=600, title=None, theme=None)``; ``theme``:
    light, dark, minimal, ggplot, seaborn, publication, solarized,
    high_contrast.
    """

    def __init__(self, width: int = 900, height: int = 600, *, title: Optional[str] = None,
                 theme: Optional[str] = None, _core: Optional[_Core] = None):
        self._core = _core if _core is not None else _Core(int(width), int(height))
        if theme:
            self._core.theme(theme)
        if title:
            self._core.title(title)

    def axes(self) -> Axes:
        """The single plot of the figure."""
        return Axes(self, None)

    def grid(self, rows: int, cols: int) -> "Figure":
        """Lays the figure out as ``rows × cols`` plots (see :meth:`subplot`)."""
        self._core.grid(rows, cols)
        return self

    def subplot(self, row: int, col: int, rowspan: int = 1, colspan: int = 1) -> Axes:
        """The plot at ``(row, col)``, optionally spanning cells."""
        return Axes(self, (row, col, rowspan, colspan))

    def title(self, title: str) -> "Figure":
        self._core.title(title)
        return self

    def share_x(self, on: bool = True) -> "Figure":
        """Plots in a column share the x-axis (zoom and cursor linked)."""
        self._core.share_x(on)
        return self

    def resize(self, width: int, height: int) -> "Figure":
        self._core.resize(width, height)
        return self

    # ----- linked brushing -----------------------------------------------------------

    def link_brushing(self, on: bool = True) -> "Figure":
        """Links selections between plots: rows selected in one plot (Shift+drag
        in :meth:`interactive` / HTML output) are highlighted in every plot whose
        series have the same number of points."""
        self._core.link_brushing(on)
        return self

    def brush(self, rows: Iterable[int], n: int) -> "Figure":
        """Selects ``rows`` (indices) of the series with ``n`` points."""
        self._core.brush_rows(int(n), [int(r) for r in rows])
        return self

    def clear_brush(self) -> "Figure":
        self._core.clear_brush()
        return self

    def brushed(self) -> list:
        """Indices of the selected rows (e.g. ``df.iloc[fig.brushed()]``)."""
        return list(self._core.brushed())

    # ----- output -----------------------------------------------------------------

    def save(self, path: Union[str, "_os.PathLike[str]"]) -> None:
        """Saves by extension: .png .svg .pdf .jpg .webp .html (interactive)."""
        self._core.save(_os.fspath(path))

    def to_png(self) -> bytes:
        return bytes(self._core.encode("png"))

    def to_svg(self) -> str:
        return bytes(self._core.encode("svg")).decode()

    def to_html(self) -> str:
        """A self-contained interactive page (hover, crosshair, zoom)."""
        return bytes(self._core.encode("html")).decode()

    def to_array(self):
        """The rendered image as a ``(height, width, 4)`` uint8 NumPy array."""
        import numpy as np

        w, h, data = self._core.rgba()
        return np.frombuffer(data, dtype=np.uint8).reshape(h, w, 4)

    def alt_text(self) -> str:
        """A text description of the chart (accessibility, captions)."""
        return self._core.alt_text()

    def show(self) -> None:
        """Displays the figure: inline in Jupyter, otherwise in the system's
        image viewer."""
        try:
            from IPython import get_ipython
            from IPython.display import Image, display

            if get_ipython() is not None:
                display(Image(self.to_png()))
                return
        except ImportError:
            pass
        import webbrowser

        fd, path = _tempfile.mkstemp(suffix=".png", prefix="dataviz-")
        with _os.fdopen(fd, "wb") as f:
            f.write(self.to_png())
        webbrowser.open("file://" + path)

    def interactive(self, height: Optional[int] = None):
        """The interactive page in a notebook: hover readouts, zoom (wheel),
        pan (drag), turning 3D plots and linked brushing (Shift+drag).

        With `anywidget <https://anywidget.dev>`_ installed this is a widget
        whose ``brushed`` trait follows the rows selected in the browser (also
        stored in the figure: :meth:`brushed`, highlighted in saved images),
        with callbacks for dashboards::

            w = fig.interactive()
            w.on_click(lambda e: print(e["series"], e["index"], e["x"], e["y"]))
            w.on_view(lambda e: table.update(*e["x"]))   # after zoom / pan
            w.on_select(lambda rows: detail.show(rows))  # Shift+drag

        Without anywidget: a plain HTML display."""
        html = self.to_html()
        h = int(height) if height else self._core.size()[1] + 40
        try:
            import anywidget  # noqa: F401
        except ImportError:
            return _Page(_iframe(html, h))
        w = _widget_class()(html=html, height=h)

        def sync(change):
            rows, sel = change["new"].get("rows", 0), change["new"].get("sel", [])
            if sel:
                self._core.brush_rows(int(rows), [int(i) for i in sel])
            else:
                self._core.clear_brush()

        w.observe(sync, names="selection")
        return w

    def _repr_png_(self) -> bytes:
        return self.to_png()


class _Page:
    """HTML shown by Jupyter (``_repr_html_``) without extra packages."""

    def __init__(self, data: str):
        self.data = data

    def _repr_html_(self) -> str:
        return self.data


def _iframe(html: str, height: int) -> str:
    import html as _html

    src = _html.escape(html, quote=True)
    return f'<iframe style="width:100%;border:0" height="{height}" srcdoc="{src}"></iframe>'


_WIDGET = None


def _widget_class():
    """The anywidget class, created on first use (anywidget is optional)."""
    global _WIDGET
    if _WIDGET is None:
        import anywidget
        import traitlets

        class FigureWidget(anywidget.AnyWidget):
            _esm = """
export default { render({ model, el }) {
  const f = document.createElement('iframe');
  f.style.cssText = 'width:100%;border:0;display:block';
  const load = () => { f.style.height = model.get('height') + 'px'; f.srcdoc = model.get('html'); };
  load();
  el.appendChild(f);
  let n = 0;
  const on = e => {
    if (e.source !== f.contentWindow || !e.data || !e.data.dataviz) return;
    const d = e.data;
    if (d.dataviz === 'brush') {
      model.set('selection', { rows: d.rows, sel: d.sel });
      model.set('brushed', d.sel);
    } else {
      // click, hover, view: a numbered event (repeats still notify).
      model.set('event', Object.assign({ n: ++n }, d));
    }
    model.save_changes();
  };
  window.addEventListener('message', on);
  model.on('change:html', load);
  return () => window.removeEventListener('message', on);
} };"""
            html = traitlets.Unicode().tag(sync=True)
            height = traitlets.Int(500).tag(sync=True)
            brushed = traitlets.List(traitlets.Int()).tag(sync=True)
            selection = traitlets.Dict().tag(sync=True)
            event = traitlets.Dict().tag(sync=True)

            def _on(self, kind, fn):
                def handler(change):
                    e = dict(change["new"])
                    if e.get("dataviz") == kind:
                        e.pop("n", None)
                        e["type"] = e.pop("dataviz")
                        fn(e)
                self.observe(handler, names="event")
                return self

            def on_click(self, fn):
                """``fn(event)`` when a data point is clicked: ``{"type": "click",
                "plot", "series", "index", "x", "y"}`` (x and y as shown)."""
                return self._on("click", fn)

            def on_hover(self, fn):
                """``fn(event)`` when the mouse is over a data point (at most
                about 8 times a second); fields as :meth:`on_click`."""
                return self._on("hover", fn)

            def on_view(self, fn):
                """``fn(event)`` after zooming or panning: ``{"type": "view",
                "plot", "x": [min, max], "y": [min, max]}``."""
                return self._on("view", fn)

            def on_select(self, fn):
                """``fn(rows)`` when rows are selected with Shift+drag (linked
                brushing); an empty list when the selection is cleared."""
                self.observe(lambda c: fn(list(c["new"])), names="brushed")
                return self

        _WIDGET = FigureWidget
    return _WIDGET


def from_spec(spec: Union[Mapping[str, Any], str]) -> Figure:
    """A figure from a JSON spec (dict or string): ``{"title": …,
    "panels": [{"type": "line", "x": […], "y": […]}, …]}`` — the format of the
    dataviz MCP server. Long NumPy/pandas/Polars arrays in the dict are
    read from their buffers, not converted to JSON."""
    if isinstance(spec, str):
        return Figure(_core=_Core.from_json(spec))
    arrays: dict = {}
    text = _json.dumps(_split(spec, arrays), default=_plain)
    return Figure(_core=_Core.from_json(text, arrays))


# ----- grammar of graphics -----------------------------------------------------

_AGGREGATES = ("count", "sum", "mean", "average", "median", "min", "max")
_TYPES = {"Q": "quantitative", "N": "nominal", "O": "ordinal", "T": "temporal"}


def _channel(v: Any) -> Any:
    """A channel from a field name, Altair-style shorthand (``"mean(mass)"``,
    ``"species:N"``, ``"count()"``) or a dict."""
    if not isinstance(v, str):
        return v
    out: dict = {}
    name = v
    if ":" in name and name.rsplit(":", 1)[1] in _TYPES:
        name, t = name.rsplit(":", 1)
        out["type"] = _TYPES[t]
    if name.endswith(")") and "(" in name:
        agg, inner = name[:-1].split("(", 1)
        if agg.lower() in _AGGREGATES:
            out["aggregate"] = agg.lower()
            name = inner
    if name:
        out["field"] = name
    return out if len(out) > 1 or "field" not in out else name


def _columns(data: Any) -> Tuple[Any, set]:
    """``{column: values}`` (or rows as given) and the datetime columns."""
    import numpy as _np

    if isinstance(data, list):
        return data, set()
    if hasattr(data, "to_dict") and hasattr(data, "columns") and hasattr(data, "dtypes"):
        # pandas or Polars DataFrame.
        cols = {str(c): data[c] for c in data.columns}
    elif isinstance(data, Mapping):
        cols = dict(data)
    else:
        raise TypeError("data: a DataFrame, a dict of columns or a list of row dicts")
    out: dict = {}
    dates: set = set()
    for k, v in cols.items():
        dtype = getattr(v, "dtype", None)
        kind = getattr(dtype, "kind", None)
        name = str(dtype).lower() if dtype is not None else ""
        if kind == "M" or name.startswith("datetime"):
            # Seconds since 1970 (temporal axes).
            a = _np.asarray(v, dtype="datetime64[ns]")
            secs = a.astype("int64").astype("float64") / 1e9
            secs[_np.isnat(a)] = _np.nan
            out[k] = secs
            dates.add(k)
        elif _numeric(v) or (kind is not None and kind in "iufb"):
            out[k] = v
        elif hasattr(v, "dtype") or hasattr(v, "to_list") or hasattr(v, "tolist"):
            out[k] = [None if x is None else str(x) for x in _plain(v)]
        else:
            out[k] = list(v)
    return out, dates


def _grammar(data: Any, mark: str, encoding: Mapping[str, Any]) -> Tuple[dict, dict]:
    options = {k: encoding[k] for k in ("columns", "stack", "colormap", "title") if k in encoding}
    channels = {k: _channel(v) for k, v in encoding.items() if k not in options and v is not None}
    cols, dates = _columns(data)
    for k, ch in list(channels.items()):
        field = ch if isinstance(ch, str) else ch.get("field")
        if field in dates and (isinstance(ch, str) or "type" not in ch):
            channels[k] = {**({"field": ch} if isinstance(ch, str) else ch), "type": "temporal"}
    arrays: dict = {}
    if isinstance(cols, dict):
        # Numeric columns of any length go over as buffers.
        conv = {}
        for k, v in cols.items():
            if not isinstance(v, list) and _numeric_any(v):
                name = str(len(arrays))
                arrays[name] = v
                conv[k] = "@" + name
            else:
                conv[k] = v
        cols = conv
    spec = {"data": cols, "mark": mark, "encoding": channels, **options}
    return spec, arrays


def _numeric_any(v: Any) -> bool:
    kind = getattr(getattr(v, "dtype", None), "kind", None)
    if kind is not None:
        return kind in "iufb"
    return _numeric(v) or (str(getattr(v, "dtype", "")).lower().startswith(("int", "float", "uint")))


def plot(data: Any, mark: str = "point", *, width: int = 900, height: int = 600,
         theme: Optional[str] = None, **encoding: Any) -> Figure:
    """A chart from data, a mark and encodings (grammar of graphics, as
    Vega-Lite / Altair / ggplot2)::

        dv.plot(penguins, "point", x="flipper", y="mass", color="species")
        dv.plot(df, "bar", x="island", y="mean(mass)", color="species", stack="grouped")
        dv.plot(df, "bar", x={"field": "mass", "bin": 30})          # histogram
        dv.plot(df, "line", x="date", y="sum(sales)", color="region")
        dv.plot(df, "point", x="bill", y="mass", facet="species", columns=3)

    ``data``: a pandas or Polars DataFrame, ``{column: values}`` or a list
    of row dicts. ``mark``: point, line, area, bar, rect, boxplot. Channels
    x, y, color, size, facet take a column name, Altair shorthand
    (``"mean(mass)"``, ``"year:O"``, ``"count()"``) or a dict ``{field,
    type, aggregate, bin, title, scale: {"type": "log"}}``. Options:
    ``columns`` (facets per row), ``stack`` (stacked, grouped, percent),
    ``colormap``, ``title``.
    """
    spec, arrays = _grammar(data, mark, encoding)
    spec.update({"width": int(width), "height": int(height)})
    if theme:
        spec["theme"] = theme
    return Figure(_core=_Core.from_json(_json.dumps(spec, default=_plain), arrays))
