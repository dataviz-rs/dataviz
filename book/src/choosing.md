# Choosing a chart

| You want to show | Chart | Method |
|---|---|---|
| a trend over time | line, area, step | `line`, `area`, `line(…).step(…)` |
| many series over time | small multiples, horizon | `facet_wrap`, `horizon` |
| values per category | bars (horizontal for long names) | `bar`, `barh` |
| change between two times | slope, dumbbell | `slope`, `dumbbell` |
| rankings over time | bump chart | `bump` |
| a distribution | histogram, KDE, ECDF | `hist`, `kde`, `ecdf` |
| distributions of groups | box, violin, raincloud, swarm, ridgeline | `boxplot`, `violin`, `raincloud`, `swarm`, `ridgeline` |
| relationship of two variables | scatter, regression, density | `scatter`, `regplot`, `density_contour`, `hexbin` |
| many variables at once | pair plot, correlation matrix, parallel coordinates | `pair_plot`, `heatmap`, `parallel_coordinates` |
| parts of a whole | pie (few parts), waffle, treemap, sunburst, marimekko | `pie`, `waffle`, `treemap`, `sunburst`, `marimekko` |
| flows | sankey, chord, flow map | `sankey`, `chord`, `geo_flows` |
| performance against targets | bullet, gauge, Pareto | `bullet`, `gauge`, `pareto` |
| a process in control | SPC control chart | `control_chart` |
| model quality | ROC, PR, calibration, confusion matrix, SHAP | `roc_curve`, `pr_curve`, `calibration_curve`, `confusion_matrix`, `shap_summary` |
| prices | candlestick, Heikin-Ashi, Renko, footprint, market profile | `candlestick`, `heikin_ashi`, `renko`, `footprint`, `market_profile` |
| countries or regions | choropleth, region map, hexbin map | `choropleth`, `region_map`, `geo_hexbin` |
| genomics | Manhattan, volcano, Circos, Hi-C, oncoprint | `manhattan`, `volcano`, `circos`, `hic`, `oncoprint` |
| a function of two variables | heatmap, contours, 3D surface | `heatmap`, `contour`, `surface` |
| structures | DNA helix, molecule, isosurface | `dna_helix`, `molecule`, `isosurface` |

Each has its code and picture in the [chart catalog](catalog/index.md).
