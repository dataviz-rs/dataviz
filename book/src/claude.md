# Claude and MCP

The Claude Code plugin gives Claude all of these charts: it draws from your
data, CSV/JSON files and PDB structures, looks at the image, refines it and
shows it.

```text
/plugin marketplace add dataviz-rs/dataviz
/plugin install dataviz@dataviz
```

Ask *"plot `bench.csv` and compare the branches"*, *"show 1UBQ as a cartoon
with its Ramachandran plot"*, or use `/dataviz:plot` and `/dataviz:molecule`.

The plugin runs the `dataviz-mcp` server, which works with any MCP client:

```sh
cargo install dataviz-mcp
```

```json
{ "mcpServers": { "dataviz": { "command": "dataviz-mcp" } } }
```

Tools: `plot` (a figure from a JSON spec — 40 chart types, `layers`),
`plot_file`, `molecule`, `chart_types`. The spec format is the same as
`dataviz::plot::spec` (feature `spec`) and Python's `dataviz.from_spec`.
