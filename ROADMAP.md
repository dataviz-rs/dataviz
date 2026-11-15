# dataviz roadmap

What is done lives in the [CHANGELOG](CHANGELOG.md) and the
[guide](https://dataviz-rs.github.io/dataviz/). This file lists what is
next. Priorities: speed and low CPU/GPU use first, then visual quality,
then breadth. There is no backward-compatibility promise before 1.0.

## Release (0.16)

- [ ] Publish `dataviz` 0.16 to crates.io and tag `v0.16.0`
- [ ] Publish `dataviz-mcp`, tag `mcp-v0.1.0` (release binaries for the Claude plugin)
- [ ] Python wheels to PyPI (`py-v0.1.0`, trusted publishing)

## Performance

- [x] GPU 3D path for opaque faces: geometry kept in GPU buffers, camera
      as a uniform, depth buffer instead of painter's sort
- [ ] The same for lines, spheres and tubes (instanced), so molecules and
      trajectories also turn without CPU work
- [ ] Retained 3D geometry on the CPU: re-project and re-sort only when
      the camera moves (cartoons rebuild their splines every frame today)
- [ ] SIMD (std::simd once stable) for the rasterizer's coverage and blend loops
- [ ] Incremental redraw of streaming charts (only the plot area that changed)

## Charts

- [ ] Geographic: orthographic globe with rotation, equal-area projections
- [ ] Biology: sequence logos, Manhattan / QQ plots for GWAS, gene structure tracks
- [ ] 3D: volume rendering (ray-marched slices), point clouds with LOD
- [ ] Network: hierarchical edge bundling, arc diagrams

## Interactivity

- [ ] Linked brushing between subplots in the window and in HTML output
- [ ] 2D HTML output with zoom/pan and hover tooltips (3D already rotates)
- [ ] Animated WebP output (GIF, APNG and video already work)

## Ecosystem

- [ ] npm package of the WASM build (canvas renderer + `spec` JSON input)
- [ ] Polars/Arrow column inputs in the Python bindings without copies
- [ ] Interactive Jupyter widget (rotate 3D, zoom) for the Python `Figure`

## Quality

- [ ] More golden image tests (one per chart family, 3D styles included)
- [ ] Fuzzing of the `spec` JSON parser and the PDB / Newick readers
- [ ] Accessibility: keyboard navigation of HTML output, high-contrast theme
