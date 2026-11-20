// C++ wrapper test.
#include "dataviz.hpp"

#include <cmath>
#include <cstdio>
#include <string>
#include <vector>

int main(int argc, char **argv) {
    std::string dir = argc > 1 ? argv[1] : ".";
    std::vector<double> x(500), y(500);
    for (size_t i = 0; i < x.size(); i++) {
        x[i] = i * 0.02;
        y[i] = std::exp(-x[i] * 0.3) * std::sin(x[i] * 4);
    }
    dataviz::Figure fig(800, 500);
    fig.title("C++ API").theme("dark");
    auto ax = fig.axes();
    auto s = ax.line(x, y, dataviz::Style().label("damped").width(2.5));
    ax.scatter(x, y, y, dataviz::Style().colormap("viridis").size(3));
    ax.title("signal").xlabel("t").ylabel("v").hline(0);
    s.push(10.1, 0.0, 400);
    ax.plot(R"({"type":"line","x":"@x","y":"@y","label":"via spec"})",
            {{"x", x}, {"y", y}});
    auto px = fig.render(1.0);
    if (px.width != 800 || px.height != 500) return 1;
    if (fig.svg().rfind("<svg", 0) != 0) return 1;
    fig.save(dir + "/cpp_api.png");

    bool threw = false;
    try {
        dataviz::Figure::from_json("{\"type\":\"nope\"}");
    } catch (const dataviz::Error &e) {
        threw = std::string(e.what()).size() > 0;
    }
    if (!threw) return 1;
    std::printf("C++ API %s: ok\n", dataviz::version().c_str());
    return 0;
}
