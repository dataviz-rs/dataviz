# **DataViz** 🚀  

<a href="https://crates.io/crates/dataviz">
    <img style="display: inline!important" src="https://img.shields.io/crates/v/dataviz.svg"></img>
</a>
<a href="https://docs.rs/dataviz">
    <img style="display: inline!important" src="https://docs.rs/dataviz/badge.svg"></img>
</a>
<a href="https://docs.rs/dataviz">
    <img style="display: inline!important" src="https://img.shields.io/crates/d/dataviz"></img>
</a>


**Pure-Rust plotting and data visualization.** 40+ chart types, publication-quality
output to SVG, PNG, JPEG, WebP and PDF, interactive windows with zoom and hover,
live streaming data and terminal output, without Python, JavaScript or a browser.

```rust no_run
use dataviz::prelude::*;

fn main() -> Result<(), Error> {
    let x: Vec<f64> = (0..200).map(|i| i as f64 / 20.0).collect();

    let mut fig = Figure::new(900, 500);
    let ax = fig.axes();
    ax.title("Signals").x_label("time [s]").y_label("amplitude");
    ax.line(&x, x.iter().map(|t| t.sin())).label("sin");
    ax.line(&x, x.iter().map(|t| t.cos())).label("cos").dash(Dash::Dashed);
    ax.scatter([1.6, 4.7, 7.9], [1.0, -1.0, 1.0]).label("peaks").marker(Marker::Star).size(12.0);

    fig.save("signals.png")?;   // .svg .png .jpg .webp .bmp, .pdf with the `pdf` feature
    fig.show("Signals")?;       // interactive window: wheel zoom, drag pan, hover
    Ok(())
}
```

## **Chart types** (`dataviz::plot`)

| Category | Charts |
|---|---|
| Lines & areas | line, step, smooth spline, area, fill-between, stacked / 100 % area, function plot, sparkline |
| Points | scatter, colormapped scatter, bubble, 3D scatter |
| Bars | vertical, horizontal, grouped, stacked, percent, overlay, value labels, waterfall, Gantt, lollipop |
| Distributions | histogram (bin rules, density, cumulative, step), KDE, ECDF, box plot (notches), violin, 2D histogram, hexbin |
| Uncertainty & fits | error bars (x/y, asymmetric), error bands, polynomial regression with R², moving average |
| Grids & fields | heatmap (annotated), correlation matrix, calendar heatmap, image, contour lines, filled contours, quiver, 3D surface/wireframe |
| Finance | candlestick, OHLC |
| Parts of a whole | pie, donut, treemap, sunburst, funnel, gauge |
| Polar | polar line/scatter, radar (spider), rose / wind rose |
| Relations & flows | sankey, network graph (force-directed), parallel coordinates |
| Annotations | text, arrows, reference lines, shaded spans, rectangles, SVG tooltips |

Run `cargo run --release --example plot_gallery -- out/` to render all of them.

## **Highlights**

- **Axes**: linear, log, symlog, date/time and categorical scales; secondary y-axis; subplots with spans and shared axes; nice ticks with minor ticks; tick formats (percent, SI, scientific, time, custom); automatic label thinning and rotation.
- **Styling**: 7 themes (light, dark, minimal, ggplot, seaborn, publication, solarized), palettes including the colour-blind safe Okabe–Ito, 14 colormaps with colorbars, 10 marker shapes, dash styles, legends in 10 positions, any TTF/OTF font.
- **Output**: SVG, PNG, JPEG, WebP, BMP, PDF (`pdf` feature), raw RGBA pixels, high-DPI scaling, terminal (`fig.print()`).
- **Interactive**: `fig.show(..)` (zoom, pan, box zoom, reset, hover, save) and `fig.show_live(..)` for streaming data.
- **Data**: any numeric slice/vector/iterator, CSV/TSV with dates (`plot::data::Table`), statistics helpers, custom chart types via the `Series` trait.
- **Features**: `window` (default, interactive windows via `minifb`), `pdf` (PDF output). `default-features = false` builds headless.

## **Installation**
```toml
[dependencies]
dataviz = "0.2"
# dataviz = { version = "0.2", features = ["pdf"] }
```

Requires Rust 1.89 or newer. On Linux the `window` feature needs the usual
windowing libraries (`libxkbcommon-dev`, `libwayland-dev`).

---

# **Classic API (0.1)**

The original chart types in `dataviz::figure` keep working unchanged.

A modular, customizable, and feature-rich 2D plotting library written in Rust. With **DataViz**, you can create a wide variety of plots tailored to your needs, supporting both raster (PNG) and vector (SVG) outputs. You can save and display (interactive or real-time) your DataViz figures.

PNG and SVG output share the same layout and honour every `FigureConfig` setting: tick and grid counts, colors and font sizes.

### **Supported Plot Types**  
- **Bar Charts**: Create grouped horizontal and vertical bar charts.  
- **Scatter Graphs**: Visualize data points with various shapes (circle, square, triangle, etc.).  
- **Pie Charts**: Represent data proportions as slices of a circle.  
- **Area Charts**: Highlight trends with filled areas under data lines.  
- **Histograms**: Analyze frequency distributions with dynamic bin calculations.  
- **Cartesian Graphs**: Plot mathematical functions or datasets on a coordinate plane.  
- **Quadrant 1 Graphs**: Focused plotting in the first quadrant for non-negative data.

## **Classic Quick Start**

```rust no_run
use dataviz::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    datasets::{cartesiangraphdataset::CartesianDataset, dataset::Dataset},
    drawers::drawer::Drawer,
    figuretypes::quadrant1graph::Quadrant1Graph,
    utilities::{linetype::LineType, scatterdottype::ScatterDotType},
};

fn main() -> std::io::Result<()> {
    let config = FigureConfig {
        num_axis_ticks: 5,      // 5 intervals -> 6 labels per axis
        num_grid_horizontal: 5,
        num_grid_vertical: 10,
        ..FigureConfig::default()
    };
    let mut graph = Quadrant1Graph::new("Throughput", "Seconds", "MB", config);

    let mut upload = CartesianDataset::new([30, 144, 255], "upload", LineType::Solid);
    let mut download = CartesianDataset::new([220, 20, 60], "download", LineType::Dashed(8));
    for t in 0..=60 {
        let t = t as f64;
        upload.add_point((t, t * 1.5));
        download.add_point((t, (t * 40.0).sqrt() * 4.0));
    }
    graph.add_dataset(upload);
    graph.add_dataset(download);

    // PNG
    let mut png = PixelCanvas::new(800, 600, [255, 255, 255], 80);
    graph.draw(&mut png);
    png.try_save_as_image("throughput.png")?;

    // SVG, with a marker at every data point
    let mut svg = SvgCanvas::new(800, 600, "white", 80);
    graph.draw_svg_with_markers(&mut svg, ScatterDotType::Circle(2));
    svg.save("throughput.svg")?;
    Ok(())
}
```

Run `cargo run --example gallery -- out/` in this repository to render every
chart type to PNG and SVG.

## **Examples**  
To see more examples you can visit: https://github.com/dataviz-rs/dataviz-examples

---

## **Usage Example**  

#### 🔗 [Grouped Vertical Bar Chart Interactive Display Example Code](https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelgroupedbarchartdisplay/src/main.rs)
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelgroupedbarchartdisplay/screenshots/interactivedisplay.png?raw=true" alt="" width="500px">

---

#### 🔗 [Grouped Vertical Bar Chart Implementatiton Using Figure Factory Source Code](https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelgroupedbarchartfigurefactory/src/main.rs)
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelgroupedbarchartfigurefactory/screenshots/grouped_vertical_bar_chart.png?raw=true" alt="" width="500px">

---

#### 🔗 [Pixel Area Chart Real Time Display Example Code](https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelareachartdisplay/src/main.rs)
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelareachartdisplay/screenshots/areachartrealtime.gif?raw=true" alt="" width="500px">
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelareachartdisplay/screenshots/areachartdisplay.png?raw=true" alt="" width="500px">

---

#### 🔗 [Pixel Cartesian Graph Real Time Display Example Code](https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelcartesiandisplay/src/main.rs)
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelcartesiandisplay/screenshots/cartesiangraphrealtime.gif?raw=true" alt="" width="500px">
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelcartesiandisplay/screenshots/cartesiangraphdisplay.png?raw=true" alt="" width="500px">

---

#### 🔗 [Pixel Histogram Real Time Display Example Code](https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelhistogramdisplay/src/main.rs)
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelhistogramdisplay/screenshots/histogramrealtime.gif?raw=true" alt="" width="500px">
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelhistogramdisplay/screenshots/histogramdisplay.png?raw=true" alt="" width="500px">

---

#### 🔗 [Pixel Pie Chart Real Time Display Example Code](https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelpiechartdisplay/src/main.rs)
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelpiechartdisplay/screenshots/piechartrealtime.gif?raw=true" alt="" width="500px">
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelpiechartdisplay/screenshots/piechart.png?raw=true" alt="" width="500px">

---

#### 🔗 [Pixel Quadrant 1 Graph Real Time Display Example Code](https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelquadrant1graphdisplay/src/main.rs)
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelquadrant1graphdisplay/screenshots/quadrant1realtime.gif?raw=true" alt="" width="500px">
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelquadrant1graphdisplay/screenshots/quadrant1graph.png?raw=true" alt="" width="500px">

---

#### 🔗 [Pixel Scatter Graph Real Time Display Example Code](https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelscattergraphdisplay/src/main.rs)
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelscattergraphdisplay/screenshots/scatterrealtime.gif?raw=true" alt="" width="500px">
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelscattergraphdisplay/screenshots/scatter_graph.png?raw=true" alt="" width="500px">

---

#### 🔗 [SVG Cartesian Graph Example Code](https://github.com/dataviz-rs/dataviz-examples/blob/main/svgcartesiangraph/src/main.rs)
<br/>
<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/svgcartesiangraph/screenshots/cartesian_graph.svg?raw=true" alt="" width="500px">

---

## **More Examples**  
### **PNG Outputs**  
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/cartesian_graph.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/grouped_horizontal_bar_chart.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/grouped_vertical_bar_chart.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/pie_chart.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/quadrant1_graph.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/scatter_graph.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/area_chart.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/histogram.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/real_time.gif?raw=true" alt="" width="300px">


### **SVG Outputs**
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/cartesian_graph.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/grouped_horizontal_bar_chart.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/grouped_vertical_bar_chart.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/pie_chart.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/quadrant1_graph.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/scatter_graph.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/area_chart.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/histogram.svg?raw=true" alt="" width="300px">


## **Changelog**  
See [CHANGELOG.md](CHANGELOG.md).

## **License**  
This project is licensed under the **MIT**. See the `LICENSE` file for details.

---

## **Contributing**  
We welcome contributions to make **DataViz** even better!
- Visit DataViz repository: https://github.com/dataviz-rs/dataviz
- Report bugs and suggest features through GitHub Issues.  
- Submit pull requests for enhancements or fixes.

Let’s make data visualization in Rust easy and accessible!

---
