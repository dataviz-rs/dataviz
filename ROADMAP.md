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
- [x] Idle windows send no frames (0.5 % CPU instead of 7 %)
- [x] GPU: integrated adapter on request (`DATAVIZ_GPU_POWER=low`, opens in ~90 ms)

## Charts

- [x] Geographic: orthographic globe with rotation, equal-area projections
- [x] Biology: sequence logos, Manhattan / QQ plots for GWAS, gene structure tracks
- [x] 3D: volume rendering (ray-marched), maximum intensity projection
- [x] 3D: point clouds: hidden points culled per quarter pixel (1 M points: 73 ms per frame on the CPU)
- [x] Network: hierarchical edge bundling, arc diagrams
- [x] Grammar of graphics (data + mark + encodings, facets) in Rust, JSON, Python
- [x] Volume files: NIfTI, VTK structured points, MetaImage
- [x] DICOM series (a directory of slices) as volumes (uncompressed)
- [ ] Compressed DICOM (JPEG lossless, JPEG 2000, RLE)
- [x] Shaded (gradient-lit) volume rendering on the CPU
- [ ] Volume rendering on the GPU

## Interactivity

- [x] Linked brushing between subplots in the window and in HTML output
- [x] Chart drawing tools: trend lines, rays, price levels, Fibonacci (window keys T, L, F)
- [x] Callbacks: click, hover and view events in HTML, Jupyter widgets, npm, React, Svelte
- [x] 2D HTML output with zoom/pan and hover tooltips (3D already rotates)
- [x] Animated WebP output (GIF, APNG and video already work)

## Ecosystem

- [x] npm package of the WASM build (canvas renderer + `spec` JSON input)
- [x] Polars/Arrow column inputs in the Python bindings without copies
- [x] Interactive Jupyter widget (rotate 3D, zoom) for the Python `Figure`
- [x] C and C++ API (`capi/`) on Linux, macOS, Windows, iOS, Android
- [x] matplotlib-style `dataviz.pyplot` and Altair-style `dv.plot` in Python
- [x] Prebuilt C/C++ libraries attached to GitHub releases (`c-v*` tags; workflow ready, no tag yet)
- [ ] Release dataviz 0.19, Python 0.4.0, npm 0.2.0

## Quality

- [x] More golden image tests (one per chart family, 3D styles included)
- [x] Fuzzing of the `spec` JSON parser and the PDB / Newick readers
- [x] Accessibility: keyboard navigation of HTML output, high-contrast theme
