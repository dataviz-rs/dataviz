# Installation

## Rust

```toml
[dependencies]
dataviz = "1"
```

Optional features:

| Feature | Adds | Default |
|---|---|---|
| `window` | interactive windows (`fig.show`, `fig.show_live`) | on |
| `pdf` | PDF output | off |
| `gpu` | GPU rasterizer (wgpu: Vulkan, Metal, DX12, OpenGL) | off |
| `regions` | states and provinces of every country (+0.8 MB) | off |
| `spec` | figures from JSON specs (`plot::spec`) | off |

Headless servers and WebAssembly: `default-features = false`.

```toml
dataviz = { version = "1", default-features = false, features = ["pdf"] }
```

No system libraries are needed (the font is built in; on Linux the window
uses X11 or Wayland when present).

## Python

```sh
pip install dataviz-rs
```

See [Python](python.md).

## Claude Code

```text
/plugin marketplace add dataviz-rs/dataviz
/plugin install dataviz@dataviz
```

See [Claude and MCP](claude.md).
