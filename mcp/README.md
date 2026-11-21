# dataviz-mcp

A [Model Context Protocol](https://modelcontextprotocol.io) server that
gives AI assistants charts: 120+ chart types — statistics, machine
learning, finance, genomics, maps, 3D surfaces and molecules — drawn in
milliseconds by [dataviz](https://crates.io/crates/dataviz) and returned as
PNG images (plus a text summary), so the assistant can look at a chart,
refine it and show it.

One small binary, stdio transport, no Python or browser.

## Claude Code

Install the plugin (it installs this server on first use):

```text
/plugin marketplace add dataviz-rs/dataviz
/plugin install dataviz@dataviz
```

## Any MCP client

```sh
cargo install dataviz-mcp
```

```json
{ "mcpServers": { "dataviz": { "command": "dataviz-mcp" } } }
```

## Docker

```sh
docker build -t dataviz-mcp mcp/
docker run -i --rm -v "$PWD:/data" dataviz-mcp   # stdio; saved charts land in the mounted folder
```

## Tools

| Tool | What it does |
|---|---|
| `plot` | A figure from a JSON spec: `panels`, each with a `type` (44 types: line, scatter, area, bar, barh, hist, kde, box, violin, swarm, raincloud, ridgeline, regression, heatmap, contour, pie, radar, funnel, waffle, sankey, chord, streamgraph, bullet, dumbbell, slope, pareto, control_chart, candlestick, roc, calibration, confusion, upset, volcano, phylo_tree, choropleth, surface, scatter3d, line3d, dna_helix, molecule) and options (titles, labels, limits, log axes, legend, 3D camera and rotation) |
| `plot_file` | A chart of a CSV/TSV/JSON file: auto, line, scatter, bar, hist, box, violin or correlation matrix |
| `molecule` | A PDB structure (`pdb_id` downloaded from RCSB, `pdb_path` or `pdb` text) as cartoon, ball-and-stick, sticks, space-filling or backbone, optionally with its Ramachandran plot |
| `chart_types` | The panel types and their fields |

Every tool takes `title`, `width`, `height`, `theme` and `output` (also save
as .png, .svg, .pdf, or .html for an interactive page).

```json
{"title": "Latency", "panels": [
  {"type": "line", "series": [{"name": "p50", "x": [1, 2, 3], "y": [12, 11, 13]}]},
  {"type": "box", "groups": {"eu": [12, 14, 13, 30], "us": [18, 17, 21, 25]}}
]}
```

## License

MIT
