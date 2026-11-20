/* Streaming data in a window from C: a new point every frame, the last
 * 500 kept. Zoom with the wheel, pan by dragging, double-click to reset. */
#include "dataviz.h"

#include <math.h>
#include <stdio.h>

typedef struct {
    dv_series series;
    double t;
} state;

static int update(dv_figure *fig, void *user) {
    state *s = (state *)user;
    (void)fig;
    for (int i = 0; i < 4; i++) {
        s->t += 0.02;
        dv_series_push(s->series, s->t, sin(s->t) + 0.3 * sin(s->t * 7.3), 500);
    }
    return 1; /* redraw */
}

int main(void) {
    dv_figure *fig = dv_figure_new(900, 500);
    dv_axes ax = dv_figure_axes(fig, 0, 0);
    dv_axes_title(ax, "live signal");
    dv_style style = {0};
    style.label = "sensor";
    style.width = 1.8;
    state s = {dv_line(ax, NULL, NULL, 0, &style), 0.0};
    if (!dv_figure_show_live(fig, "dataviz live", 60, update, &s))
        fprintf(stderr, "%s\n", dv_last_error());
    dv_figure_free(fig);
    return 0;
}
