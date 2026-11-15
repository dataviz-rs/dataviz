#!/usr/bin/env python3
"""Prints the README gallery (HTML table) from the dataviz-examples screenshots."""

RAW = "https://raw.githubusercontent.com/dataviz-rs/dataviz-examples/main"
TREE = "https://github.com/dataviz-rs/dataviz-examples/tree/main"

SECTIONS = [
    ("Live, interactive and animated", [
        ("plot-interactive", "interactive.gif", "Zoom, pan and range slider"),
        ("plot-streaming", "streaming.gif", "Live dashboard (streaming data)"),
        ("plot-animation", "animation.gif", "Animations (GIF, APNG, MP4)"),
        ("plot-binance", "binance.png", "Live Binance BTC dashboard"),
        ("plot-html", "html.png", "Interactive HTML (hover, crosshair)"),
        ("plot-wasm", "wasm.png", "In the browser (WebAssembly)"),
        ("plot-egui", "egui.png", "Inside an egui app"),
        ("plot-backends", "backends.png", "CPU, multi-core and GPU backends"),
        ("plot-terminal", "terminal.png", "Charts in the terminal"),
    ]),
    ("For data scientists, biologists, engineers and traders", [
        ("plot-data-science", "data_science.png", "Data science: ROC, confusion matrix, regression"),
        ("plot-data-science", "evaluation.png", "Models: SHAP, calibration, learning curve, UpSet"),
        ("plot-biology", "biology.png", "Biology: DNA logo, GWAS, volcano, UMAP, survival"),
        ("plot-biology", "clustermap.png", "Clustermap of gene expression"),
        ("plot-biology", "genomics.png", "Genomics: Circos, Hi-C, trees, oncoprint"),
        ("plot-biology-3d", "rotate.gif", "3D DNA and cell turning around x, y, z"),
        ("plot-biology-3d", "biology_3d.png", "3D: DNA, PDB molecules, cell, organoid, UMAP"),
        ("plot-biology-3d", "protein.png", "Protein cartoons, Ramachandran, isosurfaces"),
        ("plot-engineering", "engineering.png", "Engineering: spectrum, spectrogram, Nyquist, Smith"),
        ("plot-engineering", "bode.png", "Bode plot"),
        ("plot-engineering", "science.png", "Streamlines, ternary, SPC, Pareto, 3D waterfall"),
        ("plot-trading", "trading.png", "Trading: Ichimoku, VWAP, volume profile, depth"),
        ("plot-trading", "orderflow.png", "Order flow: liquidity, footprint, TPO, options"),
        ("plot-business", "business.png", "Business: bullet, slope, chord, streamgraph"),
    ]),
    ("Lines, points and bars", [
        ("plot-line", "line.png", "Line styles"),
        ("plot-markers-steps-smooth", "markers_steps_smooth.png", "Markers, steps and smoothing"),
        ("plot-area-fill", "area_fill.png", "Area and fill between"),
        ("plot-stacked-area", "stacked_area.png", "Stacked areas"),
        ("plot-scatter-bubble", "scatter_bubble.png", "Scatter and bubble charts"),
        ("plot-bars", "bars.png", "Bar charts"),
        ("plot-stem-waterfall", "stem_waterfall.png", "Stem and waterfall charts"),
        ("plot-errorbars-band", "errorbars_band.png", "Error bars and confidence bands"),
        ("plot-quiver-function", "quiver_function.png", "Vector field and function plots"),
    ]),
    ("Statistics", [
        ("plot-histograms", "histograms.png", "Histograms, KDE and ECDF"),
        ("plot-box-violin", "box_violin.png", "Box and violin plots"),
        ("plot-distributions", "distributions.png", "Strip, swarm, ridgeline, Q-Q"),
        ("plot-facets", "facets.png", "Facets (small multiples)"),
        ("plot-facets", "pair_plot.png", "Pair plot (scatter matrix)"),
        ("plot-hist2d-hexbin", "hist2d_hexbin.png", "2D histogram and hexbin"),
    ]),
    ("Finance and time", [
        ("plot-candlestick", "candlestick.png", "Candlestick chart"),
        ("plot-indicators", "indicators.png", "Bollinger, RSI, MACD"),
        ("plot-indicators", "heikin_ashi.png", "Heikin-Ashi candles"),
        ("plot-time-series", "time_series.png", "Time series"),
        ("plot-gantt", "gantt.png", "Gantt chart"),
        ("plot-parallel-calendar", "parallel_calendar.png", "Parallel coordinates and calendar"),
    ]),
    ("Grids, fields and 3D", [
        ("plot-heatmap", "heatmap.png", "Heatmap (correlation matrix)"),
        ("plot-contour", "contour.png", "Contour plots"),
        ("plot-image", "image.png", "Images"),
        ("plot-three-d", "three_d.png", "3D surface, Lorenz attractor, mesh"),
    ]),
    ("Maps", [
        ("plot-maps", "maps.png", "Choropleth and flight routes"),
        ("plot-maps", "regions.png", "States, flows, hexbin, OpenStreetMap basemap"),
        ("plot-maps", "projections.png", "Map projections"),
    ]),
    ("Parts, flows and polar", [
        ("plot-pie-donut", "pie_donut.png", "Pie and donut charts"),
        ("plot-treemap-sunburst", "treemap_sunburst.png", "Treemap and sunburst"),
        ("plot-funnel-gauge", "funnel_gauge.png", "Funnel and gauge"),
        ("plot-sankey", "sankey.png", "Sankey diagram"),
        ("plot-network", "network.png", "Network graph"),
        ("plot-polar-radar", "polar_radar.png", "Polar and radar charts"),
    ]),
    ("Axes, layout, text and style", [
        ("plot-subplots-shared", "subplots_shared.png", "Subplots with shared axes"),
        ("plot-log-scales", "log_scales.png", "Logarithmic scales"),
        ("plot-broken-axis", "broken_axis.png", "Broken axes"),
        ("plot-formats", "formats.png", "Tick formats"),
        ("plot-annotations", "annotations.png", "Annotations"),
        ("plot-math", "math.png", "Math notation (TeX)"),
        ("plot-international", "international.png", "Any language and emoji"),
        ("plot-accessibility", "risky.png", "Color vision simulation"),
        ("plot-themes", "theme_light.png", "Theme: light"),
        ("plot-themes", "theme_dark.png", "Theme: dark"),
        ("plot-themes", "theme_ggplot.png", "Theme: ggplot"),
        ("plot-themes", "theme_seaborn.png", "Theme: seaborn"),
        ("plot-themes", "theme_minimal.png", "Theme: minimal"),
        ("plot-themes", "theme_publication.png", "Theme: publication"),
        ("plot-themes", "theme_solarized.png", "Theme: solarized"),
    ]),
]


def cell(crate, image, title):
    return (f'<td width="33%" align="center"><a href="{TREE}/{crate}">'
            f'<img src="{RAW}/{crate}/screenshots/{image}" alt="{title}" width="100%"><br>'
            f'<sub>{title}</sub></a></td>')


out = []
for name, items in SECTIONS:
    out.append(f"### {name}\n\n<table>")
    for i in range(0, len(items), 3):
        row = "".join(cell(*it) for it in items[i:i + 3])
        out.append(f"<tr>{row}</tr>")
    out.append("</table>\n")
print("\n".join(out))
