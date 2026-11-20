/* C API test: every entry point once; exits nonzero on failure. */
#include "dataviz.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CHECK(c)                                                              \
    do {                                                                      \
        if (!(c)) {                                                           \
            fprintf(stderr, "%s:%d: %s failed: %s\n", __FILE__, __LINE__, #c, \
                    dv_last_error());                                         \
            return 1;                                                         \
        }                                                                     \
    } while (0)

int main(int argc, char **argv) {
    const char *dir = argc > 1 ? argv[1] : ".";
    char path[1024];
    enum { N = 1000 };
    static double x[N], y[N], z[N];
    for (int i = 0; i < N; i++) {
        x[i] = i * 0.01;
        y[i] = sin(x[i] * 3.0);
        z[i] = cos(x[i] * 2.0);
    }
    printf("dataviz %s\n", dv_version());
    CHECK(strstr(dv_chart_types(), "\"violin\"") != NULL);

    dv_figure *fig = dv_figure_new(800, 500);
    CHECK(fig);
    CHECK(dv_figure_title(fig, "C API"));
    CHECK(dv_figure_theme(fig, "light"));
    dv_axes a = dv_figure_axes(fig, 0, 0);
    CHECK(a.fig);
    dv_style s = {0};
    s.label = "sin";
    s.width = 2.0;
    s.color = 0x1f77b4ff;
    dv_series line = dv_line(a, x, y, N, &s);
    CHECK(line.fig);
    dv_style s2 = {0};
    s2.label = "cos";
    s2.dash = DV_DASH_DASHED;
    CHECK(dv_line(a, x, z, N, &s2).fig);
    CHECK(dv_axes_xlabel(a, "t") && dv_axes_ylabel(a, "value") && dv_axes_legend(a, "upper_right"));
    CHECK(dv_series_push(line, 10.5, 0.5, 0));
    CHECK(dv_series_set_data(line, x, y, N));

    /* A second plot with a chart type from the spec, data as arrays. */
    dv_axes b = dv_figure_axes(fig, 0, 1);
    dv_array arr[] = {{"a", y, N, 0}, {"b", z, N, 0}};
    CHECK(dv_plot(b, "{\"type\":\"violin\",\"groups\":{\"sin\":\"@a\",\"cos\":\"@b\"}}", arr, 2));
    CHECK(!dv_plot(b, "{\"type\":\"line\",\"y\":\"@nope\"}", arr, 2));
    CHECK(strlen(dv_last_error()) > 0);

    uint32_t w = 0, h = 0;
    const uint8_t *px = dv_figure_render(fig, 2.0, &w, &h);
    CHECK(px && w == 1600 && h == 1000);
    size_t len = 0;
    const uint8_t *png = dv_figure_encode(fig, "png", 1.0, &len);
    CHECK(png && len > 1000 && memcmp(png, "\x89PNG", 4) == 0);
    const char *svg = (const char *)dv_figure_encode(fig, "svg", 1.0, &len);
    CHECK(svg && strncmp(svg, "<svg", 4) == 0 && strlen(svg) == len);
    CHECK(dv_figure_zoom(fig, 200, 250, 0.5));
    dv_figure_reset(fig);
    double hx = 0, hy = 0;
    (void)dv_figure_hover(fig, 200, 250, &hx, &hy);
    snprintf(path, sizeof path, "%s/c_api.png", dir);
    CHECK(dv_figure_save(fig, path));
    dv_figure_free(fig);

    /* 3D, density, heatmap, candlesticks and a figure from JSON. */
    dv_array zs[] = {{"z", x, 100, 10}};
    dv_figure *g = dv_figure_from_json(
        "{\"width\":600,\"height\":400,\"type\":\"surface\",\"z\":\"@z\","
        "\"x_range\":[0,1],\"y_range\":[0,1]}",
        zs, 1);
    CHECK(g);
    dv_axes c = dv_figure_axes(g, 0, 1);
    CHECK(dv_scatter3d(c, x, y, z, N, 1, NULL).fig);
    CHECK(dv_density(dv_figure_axes(g, 1, 0), x, y, N, DV_DENSITY_LOG, NULL).fig);
    CHECK(dv_heatmap(dv_figure_axes(g, 1, 1), x, 10, 10, NULL).fig);
    CHECK(dv_candlestick(dv_figure_axes(g, 2, 0), x, y, z, y, z, 50).fig);
    snprintf(path, sizeof path, "%s/c_api_grid.svg", dir);
    CHECK(dv_figure_save(g, path));
    dv_figure_free(g);

    CHECK(dv_figure_from_json("{", NULL, 0) == NULL);
    CHECK(strncmp(dv_last_error(), "JSON", 4) == 0);
    dv_axes none = {NULL, 0};
    CHECK(dv_axes_title(none, "x") == 0);
    puts("C API: ok");
    return 0;
}
