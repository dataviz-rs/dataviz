# dataviz for C and C++

The dataviz renderer as a C library (`include/dataviz.h`) with a
header-only C++17 wrapper (`include/dataviz.hpp`). It covers 120+ chart
types, 2D and 3D, with PNG/SVG/PDF/HTML/JPEG/WebP output, raw RGBA pixels
for your own UI, interactive windows and live data. It runs on Linux,
macOS and Windows; iOS and Android get it as a static library.

```cpp
#include "dataviz.hpp"

int main() {
    std::vector<double> t, v;            // your data
    dataviz::Figure fig(900, 500);
    auto ax = fig.axes();
    ax.line(t, v, dataviz::Style().label("signal").width(2));
    ax.title("Hello").xlabel("time (s)");
    fig.save("hello.png");               // or .svg .pdf .html
    fig.show();                          // interactive window
}
```

```c
#include "dataviz.h"

dv_figure *fig = dv_figure_new(900, 500);
dv_axes ax = dv_figure_axes(fig, 0, 0);
dv_style style = {0};
style.label = "signal";
dv_line(ax, t, v, n, &style);
dv_figure_save(fig, "hello.png");
dv_figure_free(fig);
```

## Building

You need Rust 1.89+ ([rustup.rs](https://rustup.rs)) and CMake 3.16+.

```cmake
add_subdirectory(path/to/dataviz/capi dataviz)
target_link_libraries(app PRIVATE dataviz::dataviz)
```

CMake runs cargo and links the shared library. Set `-DDATAVIZ_STATIC=ON`
for one self-contained executable, or `-DDATAVIZ_FEATURES="window;pdf;gpu"`
for GPU rendering. On Windows the DLL must sit next to the executable;
`DATAVIZ_SHARED_LIBRARY` holds its path.

You can also build without CMake:

```sh
cargo build --release -p dataviz-c       # target/release/libdataviz_c.{so,dylib,a}, dataviz_c.dll
cc app.c -Icapi/include -Ltarget/release -ldataviz_c -lm
```

For iOS and Android, build the static library without windows:

```sh
cargo rustc -p dataviz-c --release --target aarch64-apple-ios     --no-default-features --crate-type staticlib
cargo rustc -p dataviz-c --release --target aarch64-linux-android --no-default-features --crate-type staticlib
```

On a phone or in a GUI toolkit (Qt, GTK, Dear ImGui, SDL), draw with
`dv_figure_render`, which returns RGBA pixels you can upload as a texture.
Then forward the user's input to `dv_figure_zoom`, `dv_figure_pan`,
`dv_figure_zoom_box`, `dv_figure_click` and `dv_figure_hover`.

## Every chart type, without JSON numbers

The direct functions cover the most common charts:

- `dv_line`, `dv_area`, `dv_scatter`, `dv_hist`
- `dv_heatmap`, `dv_density`
- `dv_scatter3d`, `dv_surface`, `dv_candlestick`

Every other type (violin, sankey, treemap, chord, molecule, volume, maps,
…, see `dv_chart_types()`) takes a small JSON spec. Large arrays stay
binary: a spec field `"@name"` takes the array called `name`, copied
once.

```c
dv_array arrays[] = {{"a", a, n, 0}, {"b", b, n, 0}};
dv_plot(ax, "{\"type\": \"violin\", \"groups\": {\"A\": \"@a\", \"B\": \"@b\"}}", arrays, 2);
```

```cpp
ax.plot(R"({"type": "heatmap", "z": "@z", "colormap": "magma"})", {{"z", z, cols}});
```

For a 1 million point line, measured on the machine that built this:

| Path | Time |
|---|---:|
| `dv_line` | 4 ms |
| JSON spec with `"@x"`/`"@y"` arrays | 4–7 ms |
| the same spec with numbers as JSON text | 166 ms |

Rendering the line then takes 13 ms.

## Live data

```c
static int update(dv_figure *fig, void *user) {
    dv_series *s = user;
    dv_series_push(*s, t, read_sensor(), 500);  /* keep the last 500 points */
    return 1;                                    /* redraw */
}
dv_figure_show_live(fig, "sensor", 60, update, &series);
```

See [`examples/live.c`](examples/live.c) and
[`examples/hello.cpp`](examples/hello.cpp).

## Rules

- **Ownership:** you own and free a `dv_figure`. Handles to its plots
  (`dv_axes`) and series (`dv_series`) stay valid while the figure lives.
- **Returned buffers:** pixels, encoded bytes and hover text belong to the
  figure and are valid until the next call on that figure.
- **Errors:** a failed call returns 0 or null, and `dv_last_error()` says
  why (per thread). The C++ wrapper throws `dataviz::Error` instead.
  Rust panics never cross into C.
- **Threads:** different figures can be used on different threads at the
  same time; a single figure is used by one thread at a time.
