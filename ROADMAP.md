# dataviz roadmap

What is done lives in the [CHANGELOG](CHANGELOG.md) and the
[guide](https://dataviz-rs.github.io/dataviz/). This file lists what is
next. Priorities: speed and low CPU/GPU use first, then visual quality,
then breadth. There is no backward-compatibility promise before 1.0.

## Performance

- [x] GPU 3D path for opaque faces: geometry kept in GPU buffers, camera
      as a uniform, depth buffer instead of painter's sort
- [x] Spheres and tubes as analytic GPU shapes (molecule frame 0.7 ms)
- [x] Lines and points retained on the GPU too, so trajectories and
      molecules turn without re-projecting on the CPU
- [ ] Retained 3D geometry on the CPU: re-project and re-sort only when
      the camera moves (cartoons rebuild their splines every frame today)
- [ ] SIMD (std::simd once stable) for the rasterizer's coverage and blend loops
- [ ] Incremental redraw of streaming charts (only the plot area that changed)

## Charts

- [x] Geographic: orthographic globe with rotation, equal-area projections
- [x] Biology: sequence logos, Manhattan / QQ plots for GWAS, gene structure tracks
- [x] 3D: volume rendering (ray-marched), maximum intensity projection
- [ ] 3D: point clouds with level of detail
- [x] Network: hierarchical edge bundling, arc diagrams

## Interactivity

- [x] Linked brushing between subplots in the window and in HTML output
- [x] 2D HTML output with zoom/pan and hover tooltips (3D already rotates)
- [ ] Animated WebP output (GIF, APNG and video already work)

## Ecosystem

- [x] npm package of the WASM build (canvas renderer + `spec` JSON input)
- [ ] Polars/Arrow column inputs in the Python bindings without copies
- [x] Interactive Jupyter widget (rotate 3D, zoom) for the Python `Figure`

## Quality

- [ ] More golden image tests (one per chart family, 3D styles included)
- [ ] Fuzzing of the `spec` JSON parser and the PDB / Newick readers
- [ ] Accessibility: keyboard navigation of HTML output, high-contrast theme
