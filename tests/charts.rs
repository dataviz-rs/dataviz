//! End-to-end tests of every chart type on both canvases, including the
//! regressions reported in the GitHub issues.

use dataviz::figure::{
    canvas::{pixelcanvas::PixelCanvas, svgcanvas::SvgCanvas},
    configuration::figureconfig::FigureConfig,
    datasets::{
        areachartdataset::AreaChartDataset, bardataset::BarDataset,
        cartesiangraphdataset::CartesianDataset, dataset::Dataset,
        scattergraphdataset::ScatterGraphDataset,
    },
    display::hover::Hover,
    drawers::drawer::Drawer,
    figurefactory::{FigureFactory, FigureType},
    figuretypes::{
        areachart::AreaChart, cartesiangraph::CartesianGraph, groupbarchart::GroupBarChart,
        histogram::Histogram, piechart::PieChart, quadrant1graph::Quadrant1Graph,
        scattergraph::ScatterGraph,
    },
    utilities::{linetype::LineType, orientation::Orientation, scatterdottype::ScatterDotType},
};
use resvg::usvg;

const BLUE: [u8; 3] = [30, 144, 255];
const RED: [u8; 3] = [220, 20, 60];

fn config(ticks: usize) -> FigureConfig {
    FigureConfig {
        num_axis_ticks: ticks,
        num_grid_horizontal: ticks,
        num_grid_vertical: ticks,
        color_grid: [220, 220, 220],
        color_axis: [0, 0, 0],
        color_background: [255, 255, 255],
        color_title: [0, 0, 0],
        font_size_label: 16.0,
        font_size_title: 20.0,
        font_size_legend: 14.0,
        font_size_axis: 10.0,
        font_label: None,
        font_title: None,
    }
}

fn svg_of(figure: &mut dyn Drawer) -> String {
    let mut svg = SvgCanvas::new(900, 600, "white", 80);
    figure.draw_svg(&mut svg);
    svg.get_svg_as_text()
}

fn png_of(figure: &mut dyn Drawer) -> PixelCanvas {
    let mut canvas = PixelCanvas::new(900, 600, [255, 255, 255], 80);
    figure.draw(&mut canvas);
    canvas
}

/// Parses the SVG like a real renderer would; fails on malformed XML.
fn assert_valid_svg(svg: &str) {
    usvg::Tree::from_str(svg, &usvg::Options::default())
        .unwrap_or_else(|e| panic!("invalid SVG ({e}):\n{svg}"));
}

fn count_pixels(canvas: &PixelCanvas, color: [u8; 3]) -> usize {
    canvas
        .buffer
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|p| **p == color)
        .count()
}

/// The data of issue #6.
fn issue6_graph(ticks: usize) -> Quadrant1Graph {
    let mut graph = Quadrant1Graph::new("Test", "X", "Y", config(ticks));
    let mut ds = CartesianDataset {
        points: Vec::new(),
        color: BLUE,
        label: "ID 1".to_string(),
        line_type: LineType::Solid,
    };
    for i in 0..=3 {
        ds.add_point((i as f64 * 1000.0, i as f64));
    }
    graph.add_dataset(ds);
    graph.update_range();
    graph
}

fn tick_labels(svg: &str) -> usize {
    svg.matches(r#"font-size="10.00""#).count()
}

#[test]
fn issue6_svg_honours_num_axis_ticks_and_grid() {
    for ticks in [2, 5, 13] {
        let svg = svg_of(&mut issue6_graph(ticks));
        assert_valid_svg(&svg);
        // `ticks` divisions → `ticks + 1` labels on each of the two axes.
        assert_eq!(tick_labels(&svg), 2 * (ticks + 1), "ticks = {ticks}");
        // Grid lines in the configured colour: (ticks + 1) per direction.
        let grid = svg.matches(r#"stroke="rgb(220,220,220)""#).count();
        assert_eq!(grid, 2 * (ticks + 1), "ticks = {ticks}");
    }
}

#[test]
fn issue6_svg_uses_config_colours_and_fonts() {
    let mut cfg = config(5);
    cfg.color_axis = [1, 2, 3];
    cfg.color_title = [4, 5, 6];
    cfg.font_size_title = 31.0;
    let mut graph = issue6_graph(5);
    graph.config = cfg;
    let svg = svg_of(&mut graph);
    assert!(svg.contains(
        r#"font-size="31.00" font-family="sans-serif" text-anchor="middle" fill="rgb(4,5,6)">Test<"#
    ));
    assert!(svg.contains(r#"stroke="rgb(1,2,3)""#));
    assert!(!svg.contains("lightgray"));
}

#[test]
fn issue5_no_black_points_by_default_and_coloured_markers_on_request() {
    let mut graph = issue6_graph(5);
    let svg = svg_of(&mut graph);
    assert!(
        !svg.contains("<circle"),
        "points must not be drawn by default"
    );

    let mut canvas = SvgCanvas::new(900, 600, "white", 80);
    graph.draw_svg_with_markers(&mut canvas, ScatterDotType::Circle(3));
    let svg = canvas.get_svg_as_text();
    assert_valid_svg(&svg);
    assert_eq!(svg.matches("<circle").count(), 4);
    assert_eq!(svg.matches(r#"fill="rgb(30,144,255)"/>"#).count(), 4 + 1); // + legend square
    assert!(!svg.contains(r#"fill="black""#));

    // Same on the pixel canvas.
    let mut png = PixelCanvas::new(900, 600, [255, 255, 255], 80);
    graph.draw(&mut png);
    let plain = count_pixels(&png, BLUE);
    graph.draw_with_markers(&mut png, ScatterDotType::Square(9));
    assert!(count_pixels(&png, BLUE) > plain + 3 * 50);
}

#[test]
fn issue1_dense_dashed_and_dotted_lines_have_gaps() {
    for (line_type, max_ratio) in [(LineType::Dashed(10), 0.6), (LineType::Dotted(8), 0.35)] {
        let mut graph = CartesianGraph::new("t", "x", "y", &config(4));
        let mut ds = CartesianDataset::new(BLUE, "sin", line_type.clone());
        let mut solid = CartesianDataset::new(RED, "sin", LineType::Solid);
        for i in 0..=1000 {
            let x = -3.0 + i as f64 * 0.006;
            ds.add_point((x, x.sin()));
            solid.add_point((x, x.sin() + 0.001));
        }
        graph.add_dataset(ds);
        // Count inside the plot area only (the legend square has the same colour).
        let in_plot = |c: &PixelCanvas, color| {
            (81..520u32)
                .flat_map(|y| (81..820u32).map(move |x| (x, y)))
                .filter(|&(x, y)| c.get_pixel(x, y) == Some(color))
                .count()
        };
        let dashed = in_plot(&png_of(&mut graph), BLUE);
        graph.datasets[0] = solid;
        let full = in_plot(&png_of(&mut graph), RED);
        let ratio = dashed as f64 / full as f64;
        assert!(ratio < max_ratio && ratio > 0.1, "{line_type:?}: {ratio}");
    }
}

#[test]
fn issue2_pixel_canvas_uses_background_colour() {
    let canvas = PixelCanvas::new(3, 2, [10, 20, 30], 0);
    assert_eq!(count_pixels(&canvas, [10, 20, 30]), 6);
}

#[test]
fn issue8_svg_background_is_painted() {
    let mut graph = issue6_graph(5);
    let mut svg = SvgCanvas::new(300, 200, "#123456", 20);
    graph.draw_svg(&mut svg);
    assert!(svg.elements[1].contains(r##"fill="#123456""##));
}

#[test]
fn svg_text_is_escaped() {
    let mut graph = issue6_graph(5);
    graph.title = "A & B <C>".into();
    graph.datasets[0].label = "\"quoted\" & 'single'".into();
    let svg = svg_of(&mut graph);
    assert_valid_svg(&svg);
    assert!(svg.contains("A &amp; B &lt;C&gt;"));
}

#[test]
fn cartesian_svg_axis_labels_are_not_swapped() {
    let mut graph = CartesianGraph::new("t", "Time", "Speed", &config(4));
    graph.add_dataset(CartesianDataset::new(BLUE, "a", LineType::Solid));
    graph.datasets[0].points = vec![(-1.0, 2.0), (1.0, -2.0)];
    let svg = svg_of(&mut graph);
    let rotated = svg
        .lines()
        .flat_map(|l| l.split("<text"))
        .find(|t| t.contains("rotate(-90"))
        .unwrap();
    assert!(rotated.contains(">Speed<"), "{rotated}");
}

fn all_charts(fill: impl Fn(usize) -> Vec<(f64, f64)>) -> Vec<(&'static str, Box<dyn Drawer>)> {
    let cfg = config(5);
    let mut cartesian = CartesianGraph::new("c", "x", "y", &cfg);
    let mut quadrant = Quadrant1Graph::new("q", "x", "y", cfg.clone());
    let mut scatter = ScatterGraph::new("s", "x", "y", cfg.clone());
    let mut area = AreaChart::new("a", "x", "y", cfg.clone());
    let mut vbars = GroupBarChart::new("b", "x", "y", Orientation::Vertical, cfg.clone());
    let mut hbars = GroupBarChart::new("b", "x", "y", Orientation::Horizontal, cfg.clone());
    let mut hist = Histogram::new("h", "x", "y", 7, BLUE, cfg.clone());
    let mut pie = PieChart::new("p", cfg.clone());
    for k in 0..2 {
        let points = fill(k);
        let mut c = CartesianDataset::new(BLUE, "c", LineType::Dashed(4));
        let mut s = ScatterGraphDataset::new(RED, "s", ScatterDotType::Triangle(4));
        let mut a = AreaChartDataset::new(BLUE, "a", 0.5);
        let mut b = BarDataset::new("b", RED);
        for &p in &points {
            c.add_point(p);
            s.add_point(p);
            a.add_point(p);
            b.add_data(p.0, p.1);
            hist.add_data(p.1);
            pie.add_slice("slice", p.1, BLUE);
        }
        cartesian.add_dataset(c);
        quadrant.add_dataset(CartesianDataset {
            points: points.clone(),
            color: RED,
            label: "q".into(),
            line_type: LineType::Dotted(3),
        });
        scatter.add_dataset(s);
        area.add_dataset(a);
        vbars.add_dataset(b);
        let mut b = BarDataset::new("b", RED);
        b.data = points;
        hbars.add_dataset(b);
    }
    vec![
        ("cartesian", Box::new(cartesian)),
        ("quadrant1", Box::new(quadrant)),
        ("scatter", Box::new(scatter)),
        ("area", Box::new(area)),
        ("vbars", Box::new(vbars)),
        ("hbars", Box::new(hbars)),
        ("histogram", Box::new(hist)),
        ("pie", Box::new(pie)),
    ]
}

fn draw_everywhere(name: &str, chart: &mut dyn Drawer) {
    for (w, h, margin) in [(900, 600, 80), (50, 40, 200), (1, 1, 0), (0, 0, 10)] {
        let mut png = PixelCanvas::new(w, h, [255, 255, 255], margin);
        chart.draw(&mut png);
        chart.draw_legend(&mut png);
        assert_eq!(png.buffer.len(), (w * h * 3) as usize, "{name}");
        let mut svg = SvgCanvas::new(w.max(1), h.max(1), "white", margin);
        chart.draw_svg(&mut svg);
        let text = svg.get_svg_as_text();
        assert!(
            !text.contains("NaN") && !text.contains("inf"),
            "{name}: {text}"
        );
        assert_valid_svg(&text);
    }
}

#[test]
fn every_chart_handles_ordinary_data() {
    for (name, mut chart) in all_charts(|k| {
        (0..20)
            .map(|i| (i as f64, ((i + k) as f64 / 3.0).sin() * 10.0 + 12.0))
            .collect()
    }) {
        draw_everywhere(name, chart.as_mut());
    }
}

#[test]
fn every_chart_handles_negative_data() {
    for (name, mut chart) in all_charts(|k| {
        (0..20)
            .map(|i| (i as f64 - 10.0, (i as f64 - 9.5) * (k as f64 + 1.0)))
            .collect()
    }) {
        draw_everywhere(name, chart.as_mut());
    }
}

#[test]
fn every_chart_handles_empty_data() {
    for (name, mut chart) in all_charts(|_| Vec::new()) {
        draw_everywhere(name, chart.as_mut());
    }
}

#[test]
fn every_chart_handles_a_single_point() {
    for (name, mut chart) in all_charts(|_| vec![(3.0, 3.0)]) {
        draw_everywhere(name, chart.as_mut());
    }
}

#[test]
fn every_chart_handles_non_finite_and_huge_values() {
    for (name, mut chart) in all_charts(|_| {
        vec![
            (0.0, 1.0),
            (f64::NAN, 2.0),
            (2.0, f64::INFINITY),
            (3.0, 1e300),
            (-1e300, 4.0),
            (5.0, 5.0),
        ]
    }) {
        draw_everywhere(name, chart.as_mut());
    }
}

#[test]
fn figure_factory_figures_can_be_drawn() {
    for kind in [
        FigureType::GroupBarChartVertical,
        FigureType::GroupBarChartHorizontal,
        FigureType::CartesianGraph,
        FigureType::PieChart,
        FigureType::ScatterGraph,
        FigureType::AreaChart,
        FigureType::Histogram,
    ] {
        let mut figure = FigureFactory::create_figure(kind);
        draw_everywhere("factory", figure.as_mut());
        // The concrete type is still reachable for adding data.
        if let Some(h) = figure.as_any().downcast_mut::<Histogram>() {
            h.add_data_vec(vec![1.0, 2.0, 2.0, 3.0]);
            assert_eq!(h.bin_counts.iter().sum::<f64>(), 4.0);
            draw_everywhere("factory histogram", figure.as_mut());
        }
    }
}

#[test]
fn histogram_counts_every_value_exactly_once() {
    let mut h = Histogram::new("h", "x", "n", 5, BLUE, config(5));
    for v in [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 2.5] {
        h.add_data(v);
    }
    assert_eq!(h.bin_counts, vec![1.0, 1.0, 2.0, 1.0, 3.0]);
    assert_eq!((h.min, h.max, h.bin_width), (0.0, 5.0, 1.0));

    let mut h = Histogram::new("h", "x", "n", 3, BLUE, config(5));
    h.add_data_vec(vec![7.0; 4]);
    assert_eq!(h.bin_counts, vec![4.0, 0.0, 0.0]);

    // Changing `bins` and recalculating must not index out of bounds.
    h.bins = 10;
    h.add_data(8.0);
    assert_eq!(h.bin_counts.len(), 10);
    assert_eq!(h.bin_counts.iter().sum::<f64>(), 5.0);
}

#[test]
fn bar_chart_keeps_fractional_and_negative_categories_apart() {
    let mut chart = GroupBarChart::new("b", "x", "y", Orientation::Vertical, config(5));
    let mut d = BarDataset::new("d", RED);
    d.add_data(1.0, 5.0);
    d.add_data(1.5, 6.0);
    d.add_data(-1.0, -2.0);
    chart.add_dataset(d);
    let svg = svg_of(&mut chart);
    assert_valid_svg(&svg);
    for label in [">1<", ">1.5<", ">-1<"] {
        assert!(svg.contains(label), "missing category {label}");
    }
    assert_eq!(svg.matches(r#"fill="rgb(220,20,60)" stroke"#).count(), 3);
}

#[test]
fn pie_chart_single_slice_is_a_full_circle() {
    let mut pie = PieChart::new("p", config(5));
    pie.add_slice("all", 3.0, BLUE);
    let svg = svg_of(&mut pie);
    assert!(svg.contains(r#"<circle cx="450.00" cy="300.00" r="220.00" fill="rgb(30,144,255)"/>"#));
    let png = png_of(&mut pie);
    let disc = std::f64::consts::PI * 220.0 * 220.0;
    assert!(count_pixels(&png, BLUE) as f64 > disc * 0.97);
}

#[test]
fn pie_png_and_svg_put_slices_on_the_same_side() {
    // The first slice (25 %) starts at 3 o'clock and goes counter-clockwise,
    // i.e. it covers the upper-right quarter in both renderers.
    let mut pie = PieChart::new("p", config(5));
    pie.add_slice("a", 1.0, RED);
    pie.add_slice("b", 3.0, BLUE);
    let png = png_of(&mut pie);
    assert_eq!(png.get_pixel(450 + 100, 300 - 100), Some(RED));
    assert_eq!(png.get_pixel(450 + 100, 300 + 100), Some(BLUE));
    let svg = svg_of(&mut pie);
    let first = svg.split("<path").nth(1).unwrap();
    // Ends at 12 o'clock (450, 80).
    assert!(
        first.contains("670.00 300.00 A 220.00 220.00 0 0 0 450.00 80.00"),
        "{first}"
    );
    // Hover agrees with the drawing.
    let (_, value) = pie.find_closest_point(450 + 100, 300 - 100, &png).unwrap();
    assert_eq!(value, 1.0);
}

#[test]
fn hover_mapping_matches_drawing() {
    let mut graph = issue6_graph(5);
    let png = png_of(&mut graph);
    for &(x, y) in &graph.datasets[0].points.clone() {
        let (px, py) = graph.to_canvas_coordinates(x, y, &png);
        let hit = (-1i64..=1).any(|dy| {
            (-1i64..=1).any(|dx| {
                png.get_pixel((px as i64 + dx) as u32, (py as i64 + dy) as u32) == Some(BLUE)
            })
        });
        assert!(hit, "point ({x}, {y}) not drawn at ({px}, {py})");
        let ((cx, cy), dist) = graph.find_closest_point(px, py, &png).unwrap();
        assert_eq!((cx, cy), (x, y));
        assert!(dist < 1.0);
        assert!(graph.handle_hover(px, py, &png).is_some());
    }

    let mut scatter = ScatterGraph::new("s", "x", "y", config(5));
    let mut d = ScatterGraphDataset::new(RED, "d", ScatterDotType::Circle(3));
    d.points = vec![(-5.0, 2.0), (3.0, -4.0), (8.0, 9.0)];
    scatter.add_dataset(d);
    let png = png_of(&mut scatter);
    for &(x, y) in &scatter.datasets[0].points.clone() {
        let (px, py) = scatter.to_canvas_coordinates(x, y, &png);
        // Points on the frame edge are partly covered by the axis.
        let hit = (-2i64..=2).any(|d| {
            png.get_pixel((px as i64 + d) as u32, py) == Some(RED)
                || png.get_pixel(px, (py as i64 + d) as u32) == Some(RED)
        });
        assert!(hit, "({x}, {y}) not drawn at ({px}, {py})");
    }
}

#[test]
fn hover_on_bars_and_histogram() {
    let mut chart = GroupBarChart::new("b", "x", "y", Orientation::Vertical, config(5));
    let mut a = BarDataset::new("A", RED);
    a.add_data(2020.0, 10.0);
    a.add_data(2021.0, 20.0);
    let mut b = BarDataset::new("B", BLUE);
    b.add_data(2020.0, 5.0);
    chart.add_dataset(a);
    chart.add_dataset(b);
    let png = png_of(&mut chart);
    let (px, py) = chart.to_canvas_coordinates(2021.0, 20.0, &png);
    let ((x, value), _) = chart.find_closest_point(px, py + 5, &png).unwrap();
    assert_eq!((x, value), (2021.0, 20.0));
    assert!(chart.handle_hover(px, py + 5, &png).is_some());

    let mut hist = Histogram::new("h", "x", "n", 4, BLUE, config(5));
    hist.add_data_vec(vec![0.0, 1.0, 1.5, 4.0]);
    let png = png_of(&mut hist);
    let (px, _) = hist.to_canvas_coordinates(1.2, 0.0, &png);
    let ((start, end), freq) = hist.find_closest_point(px, 300, &png).unwrap();
    assert_eq!((start, end, freq), (1.0, 2.0, 2.0));
    assert!(hist.find_closest_point(1, 300, &png).is_none());
}

#[test]
fn default_config_draws_text_without_a_font_path() {
    // Used to panic with "Font path is not set".
    let mut graph = issue6_graph(5);
    graph.config = FigureConfig::default();
    let png = png_of(&mut graph);
    let title_area_has_ink = (0..900u32)
        .flat_map(|x| (5..75u32).map(move |y| (x, y)))
        .any(|(x, y)| png.get_pixel(x, y) != Some([255, 255, 255]));
    let have_font = {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        !db.is_empty()
    };
    assert_eq!(title_area_has_ink, have_font);
}

#[test]
fn missing_font_file_does_not_panic() {
    let mut graph = issue6_graph(5);
    graph.config.font_label = Some("/no/such/font.ttf".into());
    graph.config.font_title = Some("/no/such/font.ttf".into());
    png_of(&mut graph);
}

#[test]
fn line_chart_svg_is_one_element_per_dataset() {
    let mut graph = CartesianGraph::new("t", "x", "y", &config(5));
    let mut ds = CartesianDataset::new(BLUE, "d", LineType::Dotted(5));
    for i in 0..1000 {
        ds.add_point((i as f64, (i as f64).sqrt()));
    }
    graph.add_dataset(ds);
    let svg = svg_of(&mut graph);
    assert_eq!(svg.matches("<polyline").count(), 1);
    assert!(svg.contains(r#"stroke-linecap="round""#));
}
