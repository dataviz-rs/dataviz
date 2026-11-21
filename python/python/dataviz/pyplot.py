"""matplotlib-style interface: existing pyplot code runs on dataviz with one
changed import::

    import dataviz.pyplot as plt

    plt.figure(figsize=(8, 4))
    plt.plot(x, np.sin(x), "r--", label="sin")
    plt.scatter(x, y, s=10, c="tab:blue")
    plt.xlabel("t"); plt.ylabel("v"); plt.title("Signals"); plt.legend()
    plt.savefig("signals.png")
    plt.show()

``fig, axs = plt.subplots(2, 2)`` returns axes with the ``set_*``,
``plot``, ``scatter``, ``bar``, ``hist``, ``imshow`` … methods of
matplotlib's object API. Covered: the common 2D plots and their usual
options; unknown keyword arguments are ignored rather than rejected, so
scripts keep running. Sizes follow matplotlib (``figsize`` in inches at
``dpi`` 100).
"""

from __future__ import annotations

import re as _re
from typing import Any, Optional

from . import Figure as _Figure

__all__ = [
    "figure", "subplots", "subplot", "gcf", "gca", "plot", "scatter", "bar", "barh", "hist",
    "fill_between", "boxplot", "violinplot", "pie", "imshow", "axhline", "axvline", "text",
    "title", "suptitle", "xlabel", "ylabel", "xlim", "ylim", "xscale", "yscale", "legend",
    "grid", "savefig", "show", "close", "clf", "tight_layout", "style",
]

_COLORS = {
    "b": "#1f77b4", "g": "#2ca02c", "r": "#d62728", "c": "#17becf", "m": "#9467bd",
    "y": "#bcbd22", "k": "#000000", "w": "#ffffff",
    "tab:blue": "#1f77b4", "tab:orange": "#ff7f0e", "tab:green": "#2ca02c",
    "tab:red": "#d62728", "tab:purple": "#9467bd", "tab:brown": "#8c564b",
    "tab:pink": "#e377c2", "tab:gray": "#7f7f7f", "tab:grey": "#7f7f7f",
    "tab:olive": "#bcbd22", "tab:cyan": "#17becf",
}
_DASHES = {"-": None, "--": "dashed", ":": "dotted", "-.": "dashdot",
           "solid": None, "dashed": "dashed", "dotted": "dotted", "dashdot": "dashdot"}
_MARKERS = {"o": "o", "s": "s", "^": "^", "v": "v", "D": "D", "d": "D", "+": "+",
            "x": "x", "*": "*", "h": "h", "H": "h", "p": "p", ".": "o"}
_FMT = _re.compile(r"^(?P<m1>[os^vDd+x*hHp.])?(?P<ls>--|-\.|-|:)?(?P<m2>[os^vDd+x*hHp.])?(?P<c>[bgrcmykw])?$")


def _color(c: Any) -> Optional[str]:
    if c is None:
        return None
    if isinstance(c, str):
        if c.startswith("C") and c[1:].isdigit():
            pal = list(_COLORS.values())[8:]
            return pal[int(c[1:]) % len(pal)]
        return _COLORS.get(c, c)
    if isinstance(c, (tuple, list)) and len(c) in (3, 4) and all(isinstance(v, (int, float)) for v in c):
        r, g, b = (int(round(float(v) * 255)) for v in c[:3])
        return f"#{r:02x}{g:02x}{b:02x}"
    return None


def _fmt(fmt: str) -> dict:
    """matplotlib format strings: ``"r--o"``, ``"k:"``, ``"o"``, …"""
    out: dict = {}
    # Color may come first ("r--") or last ("--r").
    color = None
    if fmt and fmt[0] in "bgrcmykw":
        color, fmt = fmt[0], fmt[1:]
    m = _FMT.match(fmt)
    if not m:
        return {"color": _color(color)} if color else {}
    marker = m.group("m1") or m.group("m2")
    color = color or m.group("c")
    if color:
        out["color"] = _color(color)
    if m.group("ls"):
        out["dash"] = _DASHES[m.group("ls")]
    if marker:
        out["marker"] = _MARKERS[marker]
        if not m.group("ls"):
            out["only_markers"] = True
    return out


class Axes:
    """matplotlib-like axes around a :class:`dataviz.Axes`."""

    def __init__(self, fig: "FigureWrapper", ax):
        self.figure = fig
        self._ax = ax
        self._legend = False

    # ----- plots -----------------------------------------------------------

    def plot(self, *args, label=None, color=None, c=None, linewidth=None, lw=None,
             linestyle=None, ls=None, marker=None, markersize=None, **_ignored):
        """``plot(y)``, ``plot(x, y)``, ``plot(x, y, fmt)``, also several
        ``x, y[, fmt]`` groups."""
        groups = []
        rest = list(args)
        while rest:
            if len(rest) >= 2 and not isinstance(rest[1], str):
                x, y, rest = rest[0], rest[1], rest[2:]
            else:
                x, y, rest = None, rest[0], rest[1:]
            f = rest.pop(0) if rest and isinstance(rest[0], str) else ""
            groups.append((x, y, f))
        for x, y, f in groups:
            st = _fmt(f)
            col = _color(color or c) or st.get("color")
            dash = _DASHES.get(linestyle or ls or "", None) if (linestyle or ls) else st.get("dash")
            mk = _MARKERS.get(marker, marker) if marker else st.get("marker")
            if st.get("only_markers") and not (linestyle or ls):
                if x is None:
                    import numpy as _np
                    x = _np.arange(len(y))
                self._ax.scatter(x, y, label=label, color=col, size=markersize or 6, marker=mk)
            else:
                self._ax.line(x, y, label=label, color=col, width=linewidth or lw, dash=dash, marker=mk)
        return []

    def scatter(self, x, y, s=None, c=None, marker=None, alpha=None, label=None, cmap=None,
                color=None, **_ignored):
        """Scatter plot of ``y`` against ``x``.

        ``s`` is the marker area in points² as in matplotlib (an array uses its
        mean size). A numeric ``c`` with one value per point is colored through
        ``cmap``; a single color string or RGB tuple colors every point.
        Supported: ``s``, ``c``, ``color``, ``marker``, ``alpha``, ``label``,
        ``cmap``. Other keyword arguments (``edgecolors``, ``norm``, …) are
        ignored.
        """
        import numpy as _np

        size = None
        if s is not None:
            # matplotlib's s is an area in points² (1 pt = dpi/72 px).
            k = self.figure.dpi / 72.0
            size = k * (float(_np.sqrt(_np.mean(s))) if _np.ndim(s) else float(_np.sqrt(s)))
        mk = _MARKERS.get(marker, marker) if marker else None
        if c is not None and not isinstance(c, str) and _np.ndim(c) == 1 and len(c) == len(x) \
                and _np.issubdtype(_np.asarray(c).dtype, _np.number):
            self._ax.scatter(x, y, label=label, size=size, marker=mk, opacity=alpha,
                             values=c, colormap=cmap)
            return None
        self._ax.scatter(x, y, label=label, color=_color(color or c), size=size,
                         marker=mk, opacity=alpha)
        return None

    def bar(self, x, height, width=None, label=None, color=None, **_ignored):
        """Vertical bars of ``height`` at the categories ``x``.

        Calling it again with the same categories draws grouped bars.
        Supported: ``label``, ``color``. ``width``, ``bottom`` and ``align`` are
        ignored.
        """
        self._ax.bar([str(v) for v in _list(x)], height, label=label, color=_color(color))

    def barh(self, y, width, label=None, color=None, **_ignored):
        """Horizontal bars of ``width`` at the categories ``y``.

        Supported: ``label``, ``color``. Other keyword arguments are ignored.
        """
        self._ax.barh([str(v) for v in _list(y)], width, label=label, color=_color(color))

    def hist(self, x, bins=None, density=False, label=None, color=None, alpha=None, **_ignored):
        """Histogram of the values ``x``.

        ``bins`` must be an integer; an array of bin edges is ignored and the
        bins are chosen automatically. Supported: ``bins``, ``density``,
        ``label``, ``color``. ``alpha``, ``histtype``, ``weights`` and
        ``cumulative`` are ignored.
        """
        b = bins if isinstance(bins, int) else None
        self._ax.hist(x, bins=b, label=label, color=_color(color), density=bool(density))

    def fill_between(self, x, y1, y2=0, alpha=None, label=None, color=None, **_ignored):
        """Fills the area between ``y1`` and ``y2`` (a number or an array).

        Supported: ``alpha``, ``label``, ``color``. ``where`` and
        ``interpolate`` are ignored.
        """
        import numpy as _np

        y2 = _np.broadcast_to(_np.asarray(y2, float), _np.shape(y1))
        self._ax.fill_between(x, y1, y2, label=label, color=_color(color), opacity=alpha)

    def boxplot(self, x, labels=None, tick_labels=None, **_ignored):
        """Box plots of one array, a list of arrays or the columns of a 2D array.

        ``labels`` (or ``tick_labels``) names the boxes. Other keyword arguments
        (``notch``, ``vert``, ``whis``, …) are ignored.
        """
        self._ax.boxplot(_groups(x, labels or tick_labels))

    def violinplot(self, dataset, **_ignored):
        """Violin plots of one array, a list of arrays or the columns of a 2D
        array. Keyword arguments (``positions``, ``widths``, …) are ignored.
        """
        self._ax.violin(_groups(dataset, None))

    def pie(self, x, labels=None, **_ignored):
        """Pie chart of the values ``x``, named by ``labels`` (default ``1``,
        ``2``, …). ``autopct``, ``explode`` and ``startangle`` are ignored.
        """
        labels = labels or [str(i + 1) for i in range(len(_list(x)))]
        self._ax.pie(labels, x)

    def imshow(self, z, cmap=None, **_ignored):
        """Draws the 2D array ``z`` as a heatmap colored through ``cmap``.

        RGB(A) images, ``extent``, ``origin`` and ``interpolation`` are not
        supported.
        """
        self._ax.heatmap(z, colormap=cmap)

    def axhline(self, y=0, color=None, label=None, **_ignored):
        """Horizontal reference line at ``y`` across the plot."""
        self._ax.hline(y, label=label, color=_color(color))

    def axvline(self, x=0, color=None, label=None, **_ignored):
        """Vertical reference line at ``x`` across the plot."""
        self._ax.vline(x, label=label, color=_color(color))

    def text(self, x, y, s, fontsize=None, color=None, **_ignored):
        """Text at the data position ``(x, y)``; ``s`` may contain TeX math
        between ``$`` signs. Supported: ``fontsize``, ``color``.
        """
        self._ax.text(x, y, s, size=fontsize, color=_color(color))

    # ----- options ---------------------------------------------------------

    def set_title(self, t, **_ignored):
        """Sets the title of these axes."""
        self._ax.set(title=t)

    def set_xlabel(self, t, **_ignored):
        """Sets the x-axis label."""
        self._ax.set(x_label=t)

    def set_ylabel(self, t, **_ignored):
        """Sets the y-axis label."""
        self._ax.set(y_label=t)

    def set_xlim(self, left=None, right=None, **_ignored):
        """Sets the x-axis limits: ``set_xlim(left, right)`` or
        ``set_xlim((left, right))``; ``None`` keeps a side automatic.
        """
        if isinstance(left, (tuple, list)):
            left, right = left
        self._ax.set(x_limits=(left, right))

    def set_ylim(self, bottom=None, top=None, **_ignored):
        """Sets the y-axis limits: ``set_ylim(bottom, top)`` or
        ``set_ylim((bottom, top))``; ``None`` keeps a side automatic.
        """
        if isinstance(bottom, (tuple, list)):
            bottom, top = bottom
        self._ax.set(y_limits=(bottom, top))

    def set_xscale(self, s, **_ignored):
        """``"log"`` makes the x-axis logarithmic; any other value (including
        ``"symlog"`` and ``"logit"``) makes it linear.
        """
        self._ax.set(x_log=(s == "log"))

    def set_yscale(self, s, **_ignored):
        """``"log"`` makes the y-axis logarithmic; any other value (including
        ``"symlog"`` and ``"logit"``) makes it linear.
        """
        self._ax.set(y_log=(s == "log"))

    def set(self, title=None, xlabel=None, ylabel=None, xlim=None, ylim=None, xscale=None,
            yscale=None, **_ignored):
        """Sets several properties at once: ``title``, ``xlabel``, ``ylabel``,
        ``xlim``, ``ylim``, ``xscale``, ``yscale``. Other names are ignored.
        """
        self._ax.set(title=title, x_label=xlabel, y_label=ylabel, x_limits=xlim, y_limits=ylim,
                     x_log=(xscale == "log") if xscale else None,
                     y_log=(yscale == "log") if yscale else None)

    def legend(self, loc=None, **_ignored):
        """Shows the legend of the labeled series.

        ``loc`` may be ``"upper right"``, ``"upper left"``, ``"lower left"`` or
        ``"lower right"``; anything else places it automatically.
        """
        pos = {"upper right": "upper_right", "upper left": "upper_left",
               "lower left": "lower_left", "lower right": "lower_right"}.get(loc or "", "best")
        self._ax.set(legend=pos)

    def grid(self, visible=True, **_ignored):
        """Shows (``True``) or hides (``False``) the grid lines."""
        self._ax.set(grid=bool(visible))


class FigureWrapper:
    """matplotlib-like figure around a :class:`dataviz.Figure`."""

    def __init__(self, figsize=(6.4, 4.8), dpi=100):
        w, h = figsize
        self.dpi = dpi
        self.fig = _Figure(int(w * dpi), int(h * dpi), theme=style._theme)
        self._axes: dict = {}
        self._shape = (1, 1)
        self._current: Optional[Axes] = None

    def add_subplot(self, nrows=1, ncols=1, index=1, **_ignored) -> Axes:
        """Adds or returns the axes at ``index`` (1-based) of an
        ``nrows`` × ``ncols`` grid; ``add_subplot(221)`` works too.
        """
        if isinstance(nrows, int) and nrows > 100:
            nrows, ncols, index = nrows // 100, (nrows // 10) % 10, nrows % 10
        if (nrows, ncols) != (1, 1):
            self.fig.grid(nrows, ncols)
        self._shape = (nrows, ncols)
        r, c = divmod(index - 1, ncols)
        key = (r, c)
        if key not in self._axes:
            inner = self.fig.subplot(r, c) if (nrows, ncols) != (1, 1) else self.fig.axes()
            self._axes[key] = Axes(self, inner)
        self._current = self._axes[key]
        return self._current

    def gca(self) -> Axes:
        """The current axes, created when there is none."""
        return self._current or self.add_subplot(1, 1, 1)

    def suptitle(self, t, **_ignored):
        """Sets the title of the whole figure."""
        self.fig.title(t)

    def savefig(self, path, dpi=None, **_ignored):
        """Saves the figure; the format follows the extension (``.png``,
        ``.svg``, ``.pdf``, ``.jpg``, ``.webp``, ``.html``, …).

        ``dpi``, ``bbox_inches`` and ``transparent`` are ignored: the image size
        is ``figsize`` × the figure's ``dpi``.
        """
        self.fig.save(path)

    def show(self):
        """Opens the figure in an interactive window."""
        self.fig.show()

    def tight_layout(self, **_ignored):
        """Does nothing: dataviz always lays out labels and titles so they fit."""
        pass

    def _repr_png_(self):
        return self.fig._repr_png_()


def _list(x) -> list:
    return x.tolist() if hasattr(x, "tolist") else list(x)


def _groups(x, labels):
    import numpy as _np

    a = x
    if hasattr(x, "ndim") and getattr(x, "ndim", 1) == 2:
        a = [x[:, i] for i in range(x.shape[1])]
    elif not isinstance(x, (list, tuple)) or (len(x) and _np.ndim(x[0]) == 0):
        a = [x]
    names = labels or [str(i + 1) for i in range(len(a))]
    return [(str(n), _np.asarray(v, float)) for n, v in zip(names, a)]


# ----- state machine ----------------------------------------------------------

_figs: list = []


def figure(num=None, figsize=(6.4, 4.8), dpi=100, **_ignored) -> FigureWrapper:
    """Creates a new figure and makes it current.

    ``figsize`` is in inches at ``dpi`` (default 100), as in matplotlib, so
    ``figsize=(8, 4)`` gives an 800 × 400 image. ``num`` is ignored.
    """
    f = FigureWrapper(figsize, dpi)
    _figs.append(f)
    return f


def gcf() -> FigureWrapper:
    """The current figure, created when there is none."""
    return _figs[-1] if _figs else figure()


def gca() -> Axes:
    """The current axes of the current figure."""
    return gcf().gca()


def subplot(*args, **kwargs) -> Axes:
    """Selects (and creates) a subplot of the current figure:
    ``subplot(2, 2, 1)`` or ``subplot(221)``.
    """
    if len(args) == 1 and isinstance(args[0], int):
        return gcf().add_subplot(args[0])
    return gcf().add_subplot(*args)


class _AxesArray(list):
    """Axes of ``subplots`` indexable as ``axs[i, j]`` and ``axs[i][j]``."""

    def __getitem__(self, i):
        if isinstance(i, tuple):
            r, c = i
            return list.__getitem__(self, r)[c]
        return list.__getitem__(self, i)

    def flat_list(self):
        out = []
        for row in self:
            out.extend(row if isinstance(row, list) else [row])
        return out

    @property
    def flat(self):
        return iter(self.flat_list())

    def ravel(self):
        return self.flat_list()

    flatten = ravel


def subplots(nrows=1, ncols=1, figsize=None, dpi=100, squeeze=True, **_ignored):
    """Creates a figure with an ``nrows`` × ``ncols`` grid of axes.

    Returns ``(fig, ax)`` for one plot, otherwise ``(fig, axs)`` where
    ``axs`` is indexed as ``axs[i]`` (one row or column) or ``axs[i, j]``
    and has ``flat`` and ``ravel()``. ``sharex``, ``sharey`` and
    ``gridspec_kw`` are ignored.
    """
    f = figure(figsize=figsize or (6.4 * max(1, ncols) ** 0.5 * 1.2 if ncols > 1 else 6.4,
                                    4.8 * max(1, nrows) ** 0.5 * 1.2 if nrows > 1 else 4.8), dpi=dpi)
    axes = [[f.add_subplot(nrows, ncols, r * ncols + c + 1) for c in range(ncols)]
            for r in range(nrows)]
    if squeeze and nrows == 1 and ncols == 1:
        return f, axes[0][0]
    if squeeze and (nrows == 1 or ncols == 1):
        return f, _AxesArray([a for row in axes for a in row])
    return f, _AxesArray(axes)


def _forward(name):
    def call(*args, **kwargs):
        return getattr(gca(), name)(*args, **kwargs)
    call.__name__ = name
    call.__doc__ = f"``{name}`` on the current axes."
    return call


plot = _forward("plot")
scatter = _forward("scatter")
bar = _forward("bar")
barh = _forward("barh")
hist = _forward("hist")
fill_between = _forward("fill_between")
boxplot = _forward("boxplot")
violinplot = _forward("violinplot")
pie = _forward("pie")
imshow = _forward("imshow")
axhline = _forward("axhline")
axvline = _forward("axvline")
text = _forward("text")
legend = _forward("legend")
grid = _forward("grid")


def title(t, **kw):
    """Sets the title of the current axes."""
    gca().set_title(t, **kw)


def xlabel(t, **kw):
    """Sets the x-axis label of the current axes."""
    gca().set_xlabel(t, **kw)


def ylabel(t, **kw):
    """Sets the y-axis label of the current axes."""
    gca().set_ylabel(t, **kw)


def xlim(*args, **kw):
    """Sets the x-axis limits of the current axes (see :meth:`Axes.set_xlim`)."""
    gca().set_xlim(*args, **kw)


def ylim(*args, **kw):
    """Sets the y-axis limits of the current axes (see :meth:`Axes.set_ylim`)."""
    gca().set_ylim(*args, **kw)


def xscale(s, **kw):
    """Sets the x-axis scale of the current axes (see :meth:`Axes.set_xscale`)."""
    gca().set_xscale(s)


def yscale(s, **kw):
    """Sets the y-axis scale of the current axes (see :meth:`Axes.set_yscale`)."""
    gca().set_yscale(s)


def suptitle(t, **kw):
    """Sets the title of the current figure."""
    gcf().suptitle(t)


def savefig(path, **kw):
    """Saves the current figure (see :meth:`FigureWrapper.savefig`)."""
    gcf().savefig(path, **kw)


def tight_layout(**kw):
    """Does nothing: dataviz always lays out labels and titles so they fit."""
    pass


def show(**kw):
    """Shows the open figures (inline in Jupyter, else a window) and
    closes them."""
    try:
        from IPython import get_ipython
        from IPython.display import display

        if get_ipython() is not None:
            for f in _figs:
                display(f.fig)
            _figs.clear()
            return
    except ImportError:
        pass
    for f in _figs:
        f.fig.show()
    _figs.clear()


def close(fig=None):
    """Forgets a figure: the given one, or all of them with no argument or
    ``"all"``.
    """
    if fig is None or fig == "all":
        _figs.clear()
    elif fig in _figs:
        _figs.remove(fig)


def clf():
    """Replaces the current figure with an empty one."""
    if _figs:
        _figs[-1] = FigureWrapper()


class _Style:  # noqa: D101
    """``plt.style.use(name)`` picks the theme of new figures (ggplot,
    seaborn, dark_background, …)."""

    available = ["default", "ggplot", "seaborn", "dark_background", "classic", "fivethirtyeight"]
    _map = {"ggplot": "ggplot", "seaborn": "seaborn", "seaborn-v0_8": "seaborn",
            "dark_background": "dark", "fivethirtyeight": "minimal", "classic": "light",
            "default": None}
    _theme: Optional[str] = None

    def use(self, name):
        name = name if isinstance(name, str) else "default"
        self._theme = self._map.get(name, self._map.get(name.split("-")[0]))


style = _Style()
