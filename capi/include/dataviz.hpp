// dataviz for C++ (C++17): a header-only wrapper of dataviz.h with RAII,
// exceptions and containers (std::vector, std::array, std::span, anything
// with data() and size() of doubles).
//
//   dataviz::Figure fig(900, 600);
//   auto ax = fig.axes();
//   ax.line(x, y, dataviz::Style().label("signal").width(2));
//   ax.title("Hello").xlabel("t");
//   fig.save("hello.png");
//
// MIT license. https://github.com/dataviz-rs/dataviz
#ifndef DATAVIZ_HPP
#define DATAVIZ_HPP

#include "dataviz.h"

#include <algorithm>
#include <cstdint>
#include <initializer_list>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

namespace dataviz {

/// Thrown when a call fails; what() is dv_last_error().
struct Error : std::runtime_error {
    Error() : std::runtime_error(dv_last_error()) {}
};

namespace detail {
inline void check(int ok) {
    if (!ok) throw Error();
}
template <class C>
inline const double *ptr(const C &c) {
    return c.size() ? c.data() : nullptr;
}
}  // namespace detail

/// Series style (all optional).
class Style {
public:
    Style() : s_{} {}
    Style &label(std::string l) { label_ = std::move(l); return *this; }
    /// 0xRRGGBBAA, e.g. 0x1f77b4ff.
    Style &color(std::uint32_t rgba) { s_.color = rgba; return *this; }
    Style &width(double w) { s_.width = w; return *this; }
    Style &size(double px) { s_.size = px; return *this; }
    Style &marker(int m) { s_.marker = m; return *this; }
    Style &dash(int d) { s_.dash = d; return *this; }
    Style &opacity(double a) { s_.opacity = a; return *this; }
    Style &colormap(std::string c) { colormap_ = std::move(c); return *this; }

    const dv_style *get() const {
        s_.label = label_.empty() ? nullptr : label_.c_str();
        s_.colormap = colormap_.empty() ? nullptr : colormap_.c_str();
        return &s_;
    }

private:
    mutable dv_style s_;
    std::string label_, colormap_;
};

/// A named array for spec fields written "@name".
struct Array {
    template <class C>
    Array(std::string name, const C &values, std::size_t cols = 0)
        : name(std::move(name)), data(detail::ptr(values)), len(values.size()), cols(cols) {}
    Array(std::string name, const double *data, std::size_t len, std::size_t cols = 0)
        : name(std::move(name)), data(data), len(len), cols(cols) {}
    std::string name;
    const double *data;
    std::size_t len, cols;
};

namespace detail {
inline std::vector<dv_array> arrays(const std::vector<Array> &a) {
    std::vector<dv_array> out;
    out.reserve(a.size());
    for (const auto &x : a) out.push_back({x.name.c_str(), x.data, x.len, x.cols});
    return out;
}
}  // namespace detail

/// A series, for live updates.
class Series {
public:
    explicit Series(dv_series s) : s_(s) {
        if (!s.fig) throw Error();
    }
    template <class X, class Y>
    Series &set_data(const X &x, const Y &y) {
        detail::check(dv_series_set_data(s_, detail::ptr(x), detail::ptr(y),
                                         std::min<std::size_t>(x.size(), y.size())));
        return *this;
    }
    /// Appends a point, keeping at most `keep` points (0: all).
    Series &push(double x, double y, std::size_t keep = 0) {
        detail::check(dv_series_push(s_, x, y, keep));
        return *this;
    }
    dv_series raw() const { return s_; }

private:
    dv_series s_;
};

/// A plot of a figure.
class Axes {
public:
    explicit Axes(dv_axes a) : a_(a) {
        if (!a.fig) throw Error();
    }

    Axes &title(const std::string &t) { detail::check(dv_axes_title(a_, t.c_str())); return *this; }
    Axes &xlabel(const std::string &t) { detail::check(dv_axes_xlabel(a_, t.c_str())); return *this; }
    Axes &ylabel(const std::string &t) { detail::check(dv_axes_ylabel(a_, t.c_str())); return *this; }
    Axes &xlim(double a, double b) { detail::check(dv_axes_xlim(a_, a, b)); return *this; }
    Axes &ylim(double a, double b) { detail::check(dv_axes_ylim(a_, a, b)); return *this; }
    Axes &log(bool x, bool y) { detail::check(dv_axes_log(a_, x, y)); return *this; }
    Axes &legend(const std::string &p) { detail::check(dv_axes_legend(a_, p.c_str())); return *this; }
    Axes &grid(bool on) { detail::check(dv_axes_grid(a_, on)); return *this; }

    /// Any chart type of the JSON spec; `arrays` fill "@name" fields.
    Axes &plot(const std::string &json, const std::vector<Array> &arrays = {}) {
        auto a = detail::arrays(arrays);
        detail::check(dv_plot(a_, json.c_str(), a.data(), a.size()));
        return *this;
    }

    template <class Y>
    Series line(const Y &y, const Style &s = {}) {
        return Series(dv_line(a_, nullptr, detail::ptr(y), y.size(), s.get()));
    }
    template <class X, class Y>
    Series line(const X &x, const Y &y, const Style &s = {}) {
        return Series(dv_line(a_, detail::ptr(x), detail::ptr(y), n(x, y), s.get()));
    }
    template <class X, class Y>
    Series area(const X &x, const Y &y, const Style &s = {}) {
        return Series(dv_area(a_, detail::ptr(x), detail::ptr(y), n(x, y), s.get()));
    }
    template <class X, class Y>
    Series scatter(const X &x, const Y &y, const Style &s = {}) {
        return Series(dv_scatter(a_, detail::ptr(x), detail::ptr(y), n(x, y), nullptr, s.get()));
    }
    /// Points colored by `values` through the style's colormap.
    template <class X, class Y, class V>
    Series scatter(const X &x, const Y &y, const V &values, const Style &s) {
        return Series(dv_scatter(a_, detail::ptr(x), detail::ptr(y),
                                 std::min(n(x, y), values.size()), detail::ptr(values), s.get()));
    }
    template <class V>
    Series hist(const V &v, unsigned bins = 0, const Style &s = {}) {
        return Series(dv_hist(a_, detail::ptr(v), v.size(), bins, s.get()));
    }
    /// Row-major rows × cols.
    template <class Z>
    Series heatmap(const Z &z, std::size_t rows, std::size_t cols, const Style &s = {}) {
        if (z.size() < rows * cols) throw std::invalid_argument("dataviz: z is too short");
        return Series(dv_heatmap(a_, detail::ptr(z), rows, cols, s.get()));
    }
    /// Millions to billions of points, counted per pixel.
    template <class X, class Y>
    Series density(const X &x, const Y &y, int how = DV_DENSITY_EQ_HIST, const Style &s = {}) {
        return Series(dv_density(a_, detail::ptr(x), detail::ptr(y), n(x, y), how, s.get()));
    }
    template <class X, class Y, class Z>
    Series scatter3d(const X &x, const Y &y, const Z &z, const Style &s = {}) {
        return Series(dv_scatter3d(a_, detail::ptr(x), detail::ptr(y), detail::ptr(z),
                                   std::min(n(x, y), z.size()), 0, s.get()));
    }
    template <class X, class Y, class Z>
    Series line3d(const X &x, const Y &y, const Z &z, const Style &s = {}) {
        return Series(dv_scatter3d(a_, detail::ptr(x), detail::ptr(y), detail::ptr(z),
                                   std::min(n(x, y), z.size()), 1, s.get()));
    }
    template <class Z>
    Series surface(const Z &z, std::size_t rows, std::size_t cols, double x0, double x1,
                   double y0, double y1, const Style &s = {}) {
        if (z.size() < rows * cols) throw std::invalid_argument("dataviz: z is too short");
        return Series(dv_surface(a_, detail::ptr(z), rows, cols, x0, x1, y0, y1, s.get()));
    }
    template <class T, class O, class H, class L, class C>
    Series candlestick(const T &t, const O &o, const H &h, const L &l, const C &c) {
        std::size_t k = std::min({t.size(), o.size(), h.size(), l.size(), c.size()});
        return Series(dv_candlestick(a_, detail::ptr(t), detail::ptr(o), detail::ptr(h),
                                     detail::ptr(l), detail::ptr(c), k));
    }
    Axes &text(double x, double y, const std::string &t) {
        detail::check(dv_text(a_, x, y, t.c_str()));
        return *this;
    }
    Axes &hline(double y) { detail::check(dv_refline(a_, y, 0)); return *this; }
    Axes &vline(double x) { detail::check(dv_refline(a_, x, 1)); return *this; }

    dv_axes raw() const { return a_; }

private:
    template <class X, class Y>
    static std::size_t n(const X &x, const Y &y) {
        return std::min<std::size_t>(x.size(), y.size());
    }
    dv_axes a_;
};

/// RGBA pixels owned by the figure (valid until its next render).
struct Pixels {
    const std::uint8_t *data;
    std::uint32_t width, height;
};

/// A figure (move-only; freed by the destructor).
class Figure {
public:
    Figure(std::uint32_t width = 900, std::uint32_t height = 600)
        : f_(dv_figure_new(width, height)) {
        if (!f_) throw Error();
    }
    /// From a JSON spec; `arrays` fill "@name" fields.
    static Figure from_json(const std::string &json, const std::vector<Array> &arrays = {}) {
        auto a = detail::arrays(arrays);
        dv_figure *f = dv_figure_from_json(json.c_str(), a.data(), a.size());
        if (!f) throw Error();
        return Figure(f);
    }
    ~Figure() { dv_figure_free(f_); }
    Figure(Figure &&o) noexcept : f_(std::exchange(o.f_, nullptr)) {}
    Figure &operator=(Figure &&o) noexcept {
        if (this != &o) {
            dv_figure_free(f_);
            f_ = std::exchange(o.f_, nullptr);
        }
        return *this;
    }
    Figure(const Figure &) = delete;
    Figure &operator=(const Figure &) = delete;

    Axes axes(std::uint32_t row = 0, std::uint32_t col = 0) {
        return Axes(dv_figure_axes(f_, row, col));
    }
    Axes span(std::uint32_t row, std::uint32_t col, std::uint32_t rows, std::uint32_t cols) {
        return Axes(dv_figure_span(f_, row, col, rows, cols));
    }
    Figure &title(const std::string &t) { detail::check(dv_figure_title(f_, t.c_str())); return *this; }
    Figure &theme(const std::string &t) { detail::check(dv_figure_theme(f_, t.c_str())); return *this; }
    Figure &resize(std::uint32_t w, std::uint32_t h) { detail::check(dv_figure_resize(f_, w, h)); return *this; }
    Figure &gpu(bool on = true) { detail::check(dv_figure_gpu(f_, on)); return *this; }

    void save(const std::string &path) { detail::check(dv_figure_save(f_, path.c_str())); }
    /// "png", "svg", "pdf", "html", "jpeg", "webp", "bmp".
    std::vector<std::uint8_t> encode(const std::string &format, double scale = 1.0) {
        std::size_t n = 0;
        const std::uint8_t *p = dv_figure_encode(f_, format.c_str(), scale, &n);
        if (!p) throw Error();
        return std::vector<std::uint8_t>(p, p + n);
    }
    std::string svg() {
        auto b = encode("svg");
        return std::string(b.begin(), b.end());
    }
    Pixels render(double scale = 1.0) {
        Pixels px{};
        px.data = dv_figure_render(f_, scale, &px.width, &px.height);
        if (!px.data) throw Error();
        return px;
    }
    void show(const std::string &title = "dataviz") {
        detail::check(dv_figure_show(f_, title.c_str()));
    }
    /// `update(fig)` runs up to `fps` times a second; return true to redraw.
    template <class F>
    void show_live(const std::string &title, unsigned fps, F update) {
        struct Ctx {
            F *f;
            Figure *self;
        } ctx{&update, this};
        auto cb = [](dv_figure *, void *u) -> int {
            auto *c = static_cast<Ctx *>(u);
            return (*c->f)(*c->self) ? 1 : 0;
        };
        detail::check(dv_figure_show_live(f_, title.c_str(), fps, cb, &ctx));
    }

    bool zoom(double x, double y, double factor) { return dv_figure_zoom(f_, x, y, factor); }
    bool pan(double x, double y, double dx, double dy) { return dv_figure_pan(f_, x, y, dx, dy); }
    bool zoom_box(double ax, double ay, double bx, double by) {
        return dv_figure_zoom_box(f_, ax, ay, bx, by);
    }
    void reset() { dv_figure_reset(f_); }
    bool click(double x, double y) { return dv_figure_click(f_, x, y); }
    /// Text of the nearest data point ("" when none).
    std::string hover(double x, double y) {
        const char *t = dv_figure_hover(f_, x, y, nullptr, nullptr);
        return t ? t : "";
    }

    dv_figure *raw() { return f_; }

private:
    explicit Figure(dv_figure *f) : f_(f) {}
    dv_figure *f_;
};

inline std::string version() { return dv_version(); }

}  // namespace dataviz

#endif  // DATAVIZ_HPP
