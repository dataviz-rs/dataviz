// A first chart from C++: two lines and a histogram, saved as PNG and SVG.
#include "dataviz.hpp"

#include <cmath>
#include <random>
#include <vector>

int main() {
    std::vector<double> t(400), a(400), b(400);
    for (size_t i = 0; i < t.size(); i++) {
        t[i] = i * 0.025;
        a[i] = std::sin(t[i]) * std::exp(-t[i] * 0.1);
        b[i] = std::cos(t[i]) * 0.8;
    }
    std::mt19937 rng(7);
    std::normal_distribution<double> normal(0.0, 1.0);
    std::vector<double> samples(100000);
    for (auto &v : samples) v = normal(rng);

    dataviz::Figure fig(1000, 420);
    fig.title("dataviz from C++");
    auto left = fig.axes(0, 0);
    left.line(t, a, dataviz::Style().label("damped sine").width(2));
    left.line(t, b, dataviz::Style().label("cosine").dash(DV_DASH_DASHED));
    left.xlabel("time (s)").ylabel("amplitude").legend("upper_right");
    auto right = fig.axes(0, 1);
    right.hist(samples, 60, dataviz::Style().label("normal"));
    right.title("100 000 samples");
    fig.save("hello.png");
    fig.save("hello.svg");
    return 0;
}
