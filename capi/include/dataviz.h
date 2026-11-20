/*
 * dataviz for C: fast charts (120+ types, 2D and 3D) as PNG, SVG, PDF,
 * HTML or raw pixels, interactive windows and live data.
 *
 * Link with the `dataviz_c` library (shared or static) built from the
 * `capi/` crate; see capi/README.md. All functions are thread-safe for
 * different figures; one figure must be used from one thread at a time.
 *
 * Errors: functions return 0, a null pointer or a null handle, and
 * dv_last_error() (per thread) says why. Strings are UTF-8.
 *
 * MIT license. https://github.com/dataviz-rs/dataviz
 */
#ifndef DATAVIZ_H
#define DATAVIZ_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32) && defined(DATAVIZ_SHARED)
#define DV_API __declspec(dllimport)
#else
#define DV_API
#endif

/* A figure: one or more plots. Free it with dv_figure_free. */
typedef struct dv_figure dv_figure;

/* A plot (grid cell) of a figure. Valid while the figure lives. */
typedef struct dv_axes {
    dv_figure *fig; /* null: failed */
    uint32_t index;
} dv_axes;

/* A series of a plot, for live data updates. */
typedef struct dv_series {
    dv_figure *fig; /* null: failed */
    uint32_t axes;
    uint32_t index;
} dv_series;

enum {
    DV_MARKER_DEFAULT = 0,
    DV_MARKER_NONE,
    DV_MARKER_CIRCLE,
    DV_MARKER_SQUARE,
    DV_MARKER_DIAMOND,
    DV_MARKER_TRIANGLE_UP,
    DV_MARKER_TRIANGLE_DOWN,
    DV_MARKER_PLUS,
    DV_MARKER_CROSS,
    DV_MARKER_STAR,
    DV_MARKER_HEXAGON
};

enum { DV_DASH_SOLID = 0, DV_DASH_DASHED, DV_DASH_DOTTED, DV_DASH_DASH_DOT, DV_DASH_LONG };

enum { DV_DENSITY_EQ_HIST = 0, DV_DENSITY_LOG, DV_DENSITY_LINEAR };

/* Style of a series. Zero-initialize (`dv_style s = {0};`) for the
 * theme's defaults, or pass NULL. */
typedef struct dv_style {
    const char *label;    /* legend label or NULL */
    uint32_t color;       /* 0xRRGGBBAA; 0 = next palette color */
    double width;         /* line width in px; 0 = default */
    double size;          /* marker size in px; 0 = default */
    int marker;           /* DV_MARKER_* */
    int dash;             /* DV_DASH_* */
    double opacity;       /* 0..1; 0 = default */
    const char *colormap; /* viridis, magma, inferno, plasma, cividis, turbo, … or NULL */
} dv_style;

/* An array for dv_plot / dv_figure_from_json: a spec field "@name" takes
 * it (copied once, never converted to JSON text). */
typedef struct dv_array {
    const char *name;
    const double *data;
    size_t len;
    size_t cols; /* row length of a matrix (heatmap/surface "z"); 0 otherwise */
} dv_array;

/* ---- library ---------------------------------------------------------- */

DV_API const char *dv_version(void);
DV_API const char *dv_last_error(void);
/* JSON [[type, fields, description], …] of every chart type. */
DV_API const char *dv_chart_types(void);

/* ---- figures ---------------------------------------------------------- */

DV_API dv_figure *dv_figure_new(uint32_t width, uint32_t height);
/* A whole figure from a JSON spec: {"type": "line", "x": "@x", …} or
 * {"panels": [ … ], "title": …, "theme": …, "width": …, "height": …}. */
DV_API dv_figure *dv_figure_from_json(const char *json, const dv_array *arrays, size_t n);
DV_API void dv_figure_free(dv_figure *fig);

DV_API int dv_figure_title(dv_figure *fig, const char *title);
/* light, dark, minimal, publication, solarized, nord, high_contrast, … */
DV_API int dv_figure_theme(dv_figure *fig, const char *name);
DV_API int dv_figure_resize(dv_figure *fig, uint32_t width, uint32_t height);
DV_API void dv_figure_size(dv_figure *fig, double *width, double *height);
/* GPU rendering if the library was built with the `gpu` feature. */
DV_API int dv_figure_gpu(dv_figure *fig, int on);

/* The plot in grid cell (row, col), created on first use. */
DV_API dv_axes dv_figure_axes(dv_figure *fig, uint32_t row, uint32_t col);
DV_API dv_axes dv_figure_span(dv_figure *fig, uint32_t row, uint32_t col, uint32_t rowspan,
                              uint32_t colspan);

/* RGBA pixels (straight alpha) at `scale` device pixels per pixel, owned
 * by the figure until the next call. */
DV_API const uint8_t *dv_figure_render(dv_figure *fig, double scale, uint32_t *width,
                                       uint32_t *height);
/* "png", "svg", "pdf", "html", "jpeg", "webp", "bmp": bytes owned by the
 * figure until the next call (text formats are NUL-terminated). */
DV_API const uint8_t *dv_figure_encode(dv_figure *fig, const char *format, double scale,
                                       size_t *len);
/* The format comes from the extension. */
DV_API int dv_figure_save(dv_figure *fig, const char *path);

/* Interactive window (zoom, pan, box zoom, 3D turning, hover); returns
 * when closed. */
DV_API int dv_figure_show(dv_figure *fig, const char *title);
/* `update` runs up to `fps` times a second; return nonzero to redraw. */
typedef int (*dv_update_fn)(dv_figure *fig, void *user);
DV_API int dv_figure_show_live(dv_figure *fig, const char *title, uint32_t fps,
                               dv_update_fn update, void *user);

/* Interaction for embedding in your own UI (coordinates in pixels). */
DV_API int dv_figure_zoom(dv_figure *fig, double x, double y, double factor);
DV_API int dv_figure_pan(dv_figure *fig, double x, double y, double dx, double dy);
DV_API int dv_figure_zoom_box(dv_figure *fig, double ax, double ay, double bx, double by);
DV_API void dv_figure_reset(dv_figure *fig);
DV_API int dv_figure_click(dv_figure *fig, double x, double y);
/* Text of the nearest data point (NULL when none) and its position. */
DV_API const char *dv_figure_hover(dv_figure *fig, double x, double y, double *px,
                                   double *py);

/* ---- plots ------------------------------------------------------------ */

DV_API int dv_axes_title(dv_axes ax, const char *title);
DV_API int dv_axes_xlabel(dv_axes ax, const char *label);
DV_API int dv_axes_ylabel(dv_axes ax, const char *label);
DV_API int dv_axes_xlim(dv_axes ax, double min, double max);
DV_API int dv_axes_ylim(dv_axes ax, double min, double max);
DV_API int dv_axes_log(dv_axes ax, int x, int y);
/* best, upper_right, upper_left, lower_left, lower_right, outside_right,
 * outside_top, none */
DV_API int dv_axes_legend(dv_axes ax, const char *position);
DV_API int dv_axes_grid(dv_axes ax, int on);

/* Any chart type (see dv_chart_types) into this plot:
 * dv_plot(ax, "{\"type\":\"violin\",\"groups\":{\"a\":\"@a\"}}", arrays, 1) */
DV_API int dv_plot(dv_axes ax, const char *json, const dv_array *arrays, size_t n);

/* ---- series (direct, no JSON) ----------------------------------------- */

/* x may be NULL (0, 1, 2, …). */
DV_API dv_series dv_line(dv_axes ax, const double *x, const double *y, size_t n,
                         const dv_style *style);
DV_API dv_series dv_area(dv_axes ax, const double *x, const double *y, size_t n,
                         const dv_style *style);
/* values (may be NULL) color the points through style->colormap. */
DV_API dv_series dv_scatter(dv_axes ax, const double *x, const double *y, size_t n,
                            const double *values, const dv_style *style);
DV_API dv_series dv_hist(dv_axes ax, const double *values, size_t n, uint32_t bins,
                         const dv_style *style);
/* Row-major rows × cols. */
DV_API dv_series dv_heatmap(dv_axes ax, const double *z, size_t rows, size_t cols,
                            const dv_style *style);
/* Millions to billions of points, counted per pixel (DV_DENSITY_*). */
DV_API dv_series dv_density(dv_axes ax, const double *x, const double *y, size_t n, int how,
                            const dv_style *style);
DV_API dv_series dv_scatter3d(dv_axes ax, const double *x, const double *y, const double *z,
                              size_t n, int line, const dv_style *style);
DV_API dv_series dv_surface(dv_axes ax, const double *z, size_t rows, size_t cols, double x0,
                            double x1, double y0, double y1, const dv_style *style);
DV_API dv_series dv_candlestick(dv_axes ax, const double *x, const double *open,
                                const double *high, const double *low, const double *close,
                                size_t n);
DV_API int dv_text(dv_axes ax, double x, double y, const char *text);
DV_API int dv_refline(dv_axes ax, double at, int vertical);

/* Live data for line and scatter series. */
DV_API int dv_series_set_data(dv_series s, const double *x, const double *y, size_t n);
/* Appends a point; keeps at most `keep` points (0: all). */
DV_API int dv_series_push(dv_series s, double x, double y, size_t keep);

#ifdef __cplusplus
}
#endif

#endif /* DATAVIZ_H */
