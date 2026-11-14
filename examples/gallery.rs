//! Renders every chart type to PNG and SVG.
//!
//! ```text
//! cargo run --example gallery -- [output_dir] [font.ttf]
//! ```
//!
//! Without a font argument, `DATAVIZ_FONT` is used; without either, a system
//! sans-serif font is used for PNG text.

use dataviz::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    datasets::{
        areachartdataset::AreaChartDataset, bardataset::BarDataset,
        cartesiangraphdataset::CartesianDataset, dataset::Dataset,
        scattergraphdataset::ScatterGraphDataset,
    },
    drawers::drawer::Drawer,
    figuretypes::{
        areachart::AreaChart, cartesiangraph::CartesianGraph, groupbarchart::GroupBarChart,
        histogram::Histogram, piechart::PieChart, quadrant1graph::Quadrant1Graph,
        scattergraph::ScatterGraph,
    },
    utilities::{linetype::LineType, orientation::Orientation, scatterdottype::ScatterDotType},
};
use std::f64::consts::PI;

fn config(font: &Option<String>) -> FigureConfig {
    FigureConfig {
        num_axis_ticks: 5,
        num_grid_horizontal: 5,
        num_grid_vertical: 10,
        font_label: font.clone(),
        font_title: font.clone(),
        ..FigureConfig::default()
    }
}

fn render(name: &str, out: &str, figure: &mut dyn Drawer) {
    let mut png = PixelCanvas::new(800, 600, [255, 255, 255], 80);
    figure.draw(&mut png);
    png.save_as_image(&format!("{out}/{name}.png"));

    let mut svg = SvgCanvas::new(800, 600, "white", 80);
    figure.draw_svg(&mut svg);
    svg.save(&format!("{out}/{name}.svg")).expect("write svg");
    println!("{out}/{name}.{{png,svg}}");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "gallery".to_string());
    let font = args.next().or_else(|| std::env::var("DATAVIZ_FONT").ok());
    std::fs::create_dir_all(&out).expect("create output dir");
    let cfg = config(&font);

    // Cartesian graph: dense sine (dashed), sparse lines (dotted / solid).
    let mut cartesian = CartesianGraph::new("Cartesian Graph", "X Axis", "Y Axis", &cfg);
    let mut sine = CartesianDataset::new([0, 0, 255], "sin(x)", LineType::Dashed(10));
    let mut cosine = CartesianDataset::new([255, 0, 0], "cos(x)", LineType::Dotted(6));
    let mut line = CartesianDataset::new([0, 160, 0], "x / 2", LineType::Solid);
    for i in 0..=1000 {
        let x = -PI + i as f64 * 2.0 * PI / 1000.0;
        sine.add_point((x, x.sin()));
        cosine.add_point((x, x.cos()));
    }
    for i in 0..=10 {
        let x = -PI + i as f64 * 2.0 * PI / 10.0;
        line.add_point((x, x / 2.0));
    }
    cartesian.add_dataset(sine);
    cartesian.add_dataset(cosine);
    cartesian.add_dataset(line);
    render("cartesian", &out, &mut cartesian);

    // Quadrant 1 graph (issue #5 / #6 reproduction).
    let mut q1 = Quadrant1Graph::new("Quadrant 1 & <Test>", "Bytes", "Seconds", cfg.clone());
    let mut a = CartesianDataset::new([30, 144, 255], "ID 1", LineType::Solid);
    let mut b = CartesianDataset::new([220, 20, 60], "ID 2", LineType::Dashed(8));
    for i in 0..=300 {
        let t = i as f64 / 100.0;
        a.add_point((t * 1000.0, t));
        b.add_point((t * 1000.0, (t * 2.0).sqrt()));
    }
    q1.add_dataset(a);
    q1.add_dataset(b);
    render("quadrant1", &out, &mut q1);

    // Scatter graph with every dot type.
    let mut scatter = ScatterGraph::new("Scatter Graph", "X", "Y", cfg.clone());
    let kinds = [
        ([255, 0, 0], "circle", ScatterDotType::Circle(4)),
        ([0, 128, 0], "square", ScatterDotType::Square(7)),
        ([0, 0, 255], "cross", ScatterDotType::Cross(5)),
        ([200, 120, 0], "triangle", ScatterDotType::Triangle(6)),
    ];
    for (k, (color, label, dot)) in kinds.into_iter().enumerate() {
        let mut ds = ScatterGraphDataset::new(color, label, dot);
        for i in 0..15 {
            let x = i as f64 - 4.0;
            ds.add_point((x, (x * (k as f64 + 1.0)).sin() * 10.0 + k as f64 * 5.0));
        }
        scatter.add_dataset(ds);
    }
    render("scatter", &out, &mut scatter);

    // Area chart.
    let mut area = AreaChart::new("Area Chart", "Month", "Sales", cfg.clone());
    let mut s1 = AreaChartDataset::new([0, 120, 255], "Product A", 0.4);
    let mut s2 = AreaChartDataset::new([255, 80, 0], "Product B", 0.6);
    for m in 0..12 {
        let x = m as f64;
        s1.add_point((x, 20.0 + 10.0 * (x / 2.0).sin()));
        s2.add_point((x, 10.0 + x));
    }
    area.add_dataset(s1);
    area.add_dataset(s2);
    render("area", &out, &mut area);

    // Grouped bar charts.
    for (name, orientation) in [
        ("bar_vertical", Orientation::Vertical),
        ("bar_horizontal", Orientation::Horizontal),
    ] {
        let mut bars = GroupBarChart::new("Revenue", "Year", "Revenue", orientation, cfg.clone());
        let mut c1 = BarDataset::new("Company A", [255, 0, 0]);
        let mut c2 = BarDataset::new("Company B", [0, 0, 255]);
        let mut c3 = BarDataset::new("Company C", [0, 160, 0]);
        for (i, year) in [2020.0, 2021.0, 2022.0, 2023.0].into_iter().enumerate() {
            c1.add_data(year, 100.0 + i as f64 * 30.0);
            c2.add_data(year, 80.0 + i as f64 * 45.0);
            c3.add_data(year, 150.0 - i as f64 * 20.0);
        }
        bars.add_dataset(c1);
        bars.add_dataset(c2);
        bars.add_dataset(c3);
        render(name, &out, &mut bars);
    }

    // Histogram.
    let mut hist = Histogram::new(
        "Histogram",
        "Value",
        "Frequency",
        10,
        [100, 100, 255],
        cfg.clone(),
    );
    let mut seed = 42_u64;
    let mut values = Vec::new();
    for _ in 0..2000 {
        // Sum of uniforms -> roughly normal.
        let mut s = 0.0;
        for _ in 0..6 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            s += (seed >> 11) as f64 / (1u64 << 53) as f64;
        }
        values.push(s);
    }
    hist.add_data_vec(values);
    render("histogram", &out, &mut hist);

    // Pie chart.
    let mut pie = PieChart::new("Market Share", cfg.clone());
    pie.add_slice("Product A", 40.0, [255, 99, 71]);
    pie.add_slice("Product B", 30.0, [60, 179, 113]);
    pie.add_slice("Product C", 20.0, [65, 105, 225]);
    pie.add_slice("Other", 10.0, [200, 200, 200]);
    render("pie", &out, &mut pie);
}
