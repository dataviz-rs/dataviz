# dataviz for Claude Code

Charts for Claude: 120+ chart types — statistics, machine learning,
finance and order flow, genomics, maps, 3D surfaces and molecules — drawn
in milliseconds by the [dataviz](https://github.com/dataviz-rs/dataviz)
Rust library and returned to Claude as images, so it can look at a chart,
fix it and show it to you.

## Install

```text
/plugin marketplace add dataviz-rs/dataviz
/plugin install dataviz@dataviz
```

On first start the plugin installs its MCP server (`dataviz-mcp`, one small
binary): a prebuilt binary for Linux and macOS (x86_64, arm64), or
`cargo install dataviz-mcp` when Rust is installed. On Windows install it
with `cargo install dataviz-mcp` first; the plugin then uses the binary on
PATH. Nothing else is needed: no Python, no browser.

## Use

Ask in plain words — the `charts` skill tells Claude when and how to draw:

- "Plot the benchmark results in `bench.csv` and compare the two branches."
- "Show the correlation matrix of `data/features.parquet.json`."
- "Draw the ROC and calibration curves of these predictions."
- "Show ubiquitin (1UBQ) as a cartoon, rotated 90° around y, with its Ramachandran plot."

Or use the shortcuts:

```text
/dataviz:plot results/latency.csv
/dataviz:molecule 1BNA ball_and_stick
```

## Tools

| Tool | What it does |
|---|---|
| `plot` | A figure from a JSON spec: one or more panels of 40 types (line, scatter, bar, hist, box, violin, raincloud, heatmap, contour, pie, sankey, chord, streamgraph, bullet, pareto, SPC, candlestick, ROC, calibration, confusion matrix, UpSet, volcano, phylogenetic tree, choropleth, 3D surface/scatter, DNA helix, molecule, …) |
| `plot_file` | A chart of a CSV/TSV/JSON file: auto, line, scatter, bar, histogram, box, violin or correlation matrix |
| `molecule` | A PDB structure (by id, file or text) as cartoon, ball-and-stick, sticks, space-filling or backbone; optional Ramachandran plot |
| `chart_types` | The panel types and their fields |

Every tool can also save the chart (`output`: .png, .svg, .pdf, or .html
for an interactive page).

## Other MCP clients

The server works with any MCP client (Claude Desktop, other agents):

```sh
cargo install dataviz-mcp
```

```json
{ "mcpServers": { "dataviz": { "command": "dataviz-mcp" } } }
```

## License

MIT
