//! Integration tests of the `dataviz::plot` API: every chart type, edge-case
//! data, themes, output formats and hit-testing.

use dataviz::plot::data::Table;
use dataviz::plot::series::frameless::TreeNode;
use dataviz::plot::series::line::Line;
use dataviz::plot::terminal::TerminalMode;
use dataviz::prelude::*;
use resvg::usvg;

/// Builds one chart type on an axes from data `(x, y)`.
type Build = fn(&mut Axes, &[f64], &[f64]);

fn builders() -> Vec<(&'static str, Build)> {
    vec![
        ("line", |a, x, y| {
            a.line(x, y).marker(Marker::Circle).label("l");
        }),
        ("step", |a, x, y| {
            a.line(x, y).step(Step::Mid);
        }),
        ("smooth", |a, x, y| {
            a.line(x, y).smooth(true);
        }),
        ("area", |a, x, y| {
            a.area(x, y);
        }),
        ("fill_between", |a, x, y| {
            a.fill_between(x, y, y.iter().map(|v| v - 1.0));
        }),
        ("stacked_area", |a, x, y| {
            a.stacked_area(x, [("a", y.to_vec()), ("b", y.to_vec())], true);
        }),
        ("scatter", |a, x, y| {
            a.scatter(x, y)
                .values(y)
                .colorbar("v")
                .names(x.iter().map(|v| v.to_string()));
        }),
        ("bubble", |a, x, y| {
            a.scatter(x, y).bubble(y.iter().map(|v| v.abs()), 4.0, 30.0);
        }),
        ("bar", |a, _, y| {
            a.bar(y.iter().enumerate().map(|(i, _)| format!("c{i}")), y)
                .value_labels(LabelPosition::Outside);
        }),
        ("bar_xy", |a, x, y| {
            a.bar_xy(x, y);
        }),
        ("barh", |a, _, y| {
            a.barh(y.iter().enumerate().map(|(i, _)| format!("c{i}")), y);
        }),
        ("bars_grouped", |a, _, y| {
            let c: Vec<String> = (0..y.len()).map(|i| format!("c{i}")).collect();
            a.bars(
                &c,
                [
                    ("a", y.to_vec()),
                    ("b", y.iter().map(|v| v * 0.5).collect()),
                ],
                BarMode::Grouped,
                false,
            );
        }),
        ("bars_stacked", |a, _, y| {
            let c: Vec<String> = (0..y.len()).map(|i| format!("c{i}")).collect();
            for b in a.bars(
                &c,
                [("a", y.to_vec()), ("b", y.to_vec())],
                BarMode::Stacked,
                true,
            ) {
                b.value_labels(LabelPosition::Inside);
            }
        }),
        ("bars_percent", |a, _, y| {
            let c: Vec<String> = (0..y.len()).map(|i| format!("c{i}")).collect();
            a.bars(
                &c,
                [("a", y.to_vec()), ("b", y.to_vec())],
                BarMode::Percent,
                false,
            );
        }),
        ("hist", |a, _, y| {
            a.hist(y).density(true);
        }),
        ("hist_cumulative_step", |a, _, y| {
            a.hist(y).cumulative(true).step(true).horizontal(true);
        }),
        ("boxplot", |a, _, y| {
            a.boxplot([
                ("a", y.to_vec()),
                ("b", y.iter().map(|v| v * 2.0).collect()),
            ])
            .notch(true)
            .show_mean(true);
        }),
        ("violin", |a, _, y| {
            a.violin([("a", y.to_vec())]);
        }),
        ("error_bars", |a, x, y| {
            a.error_bars(x, y, y.iter().map(|v| v.abs() * 0.1))
                .x_err(x.iter().map(|_| 0.2));
        }),
        ("error_band", |a, x, y| {
            a.error_band(x, y, y.iter().map(|v| v - 1.0), y.iter().map(|v| v + 1.0));
        }),
        ("kde_ecdf_regression", |a, x, y| {
            a.kde(y, None);
            a.ecdf(y);
            a.regression(x, y, 2);
            a.moving_average(x, y, 3);
        }),
        ("heatmap", |a, _, y| {
            let m: Vec<Vec<f64>> = (0..4)
                .map(|r| y.iter().map(|v| v * r as f64).collect())
                .collect();
            a.heatmap(m).annotate(true);
        }),
        ("hist2d", |a, x, y| {
            a.hist2d(x, y, 10, 10);
        }),
        ("hexbin", |a, x, y| {
            a.hexbin(x, y).log(true);
        }),
        ("contour", |a, _, y| {
            let m: Vec<Vec<f64>> = (0..y.len().max(2))
                .map(|r| (0..y.len().max(2)).map(|c| (r * c) as f64).collect())
                .collect();
            a.contour((0.0, 1.0), (0.0, 1.0), m)
                .filled(true)
                .labels(true);
        }),
        ("candlestick", |a, x, y| {
            a.candlestick(
                x,
                y,
                y.iter().map(|v| v + 1.0),
                y.iter().map(|v| v - 1.0),
                y.iter().rev().copied().collect::<Vec<_>>(),
            );
        }),
        ("stem", |a, x, y| {
            a.stem(x, y);
        }),
        ("waterfall", |a, _, y| {
            a.waterfall(y.iter().enumerate().map(|(i, _)| format!("s{i}")), y)
                .total_at(0);
        }),
        ("quiver", |a, x, y| {
            a.quiver(x, y, y, x);
        }),
        ("gantt", |a, x, _| {
            a.gantt(
                x.iter().map(|v| format!("t{v}")),
                x,
                x.iter().map(|v| v + 2.0),
            )
            .progress([0.5]);
        }),
        ("annotations", |a, x, y| {
            a.line(x, y);
            a.hline(0.5).label("h");
            a.vspan(0.0, 1.0);
            a.text(1.0, 1.0, "t & <x>");
            a.arrow((0.0, 0.0), (1.0, 1.0), Some("arrow"));
            a.rectangle(0.0, 0.0, 1.0, 1.0);
        }),
        ("function", |a, _, _| {
            a.function(|x| 1.0 / x, -1.0, 1.0);
        }),
        ("pie", |a, _, y| {
            a.pie(y.iter().enumerate().map(|(i, _)| format!("p{i}")), y)
                .donut(0.5)
                .labels(true)
                .center_text("c");
        }),
        ("treemap_sunburst", |a, _, y| {
            let t = vec![
                TreeNode::group(
                    "g",
                    y.iter()
                        .enumerate()
                        .map(|(i, v)| TreeNode::leaf(format!("l{i}"), *v))
                        .collect(),
                ),
                TreeNode::leaf("x", 1.0),
            ];
            a.treemap(t);
        }),
        ("sunburst", |a, _, y| {
            a.sunburst(vec![TreeNode::group(
                "g",
                y.iter()
                    .enumerate()
                    .map(|(i, v)| TreeNode::leaf(format!("l{i}"), *v))
                    .collect(),
            )]);
        }),
        ("sankey", |a, _, y| {
            a.sankey(
                y.iter()
                    .enumerate()
                    .map(|(i, v)| (format!("a{}", i % 2), format!("b{i}"), *v)),
            );
        }),
        ("funnel", |a, _, y| {
            a.funnel(y.iter().enumerate().map(|(i, _)| format!("s{i}")), y);
        }),
        ("gauge", |a, _, y| {
            a.gauge(y.first().copied().unwrap_or(f64::NAN), 0.0, 10.0)
                .bands([(5.0, Color::GREEN), (10.0, Color::RED)]);
        }),
        ("network", |a, _, y| {
            a.network(
                y.iter().enumerate().map(|(i, _)| i.to_string()),
                (1..y.len()).map(|i| (i - 1, i, 1.0)),
            );
        }),
        ("parallel", |a, x, y| {
            a.parallel_coordinates(
                ["x", "y"],
                x.iter().zip(y).map(|(a, b)| vec![*a, *b]).collect(),
            );
        }),
        ("calendar", |a, x, y| {
            a.calendar(x.iter().map(|v| date(2024, 1, 1) + v * 86400.0), y);
        }),
        ("polar", |a, x, y| {
            a.polar_line(x, y.iter().map(|v| v.abs()))
                .marker(Marker::Circle);
            a.polar_scatter(x, y.iter().map(|v| v.abs()));
        }),
        ("radar", |a, _, y| {
            a.radar(
                y.iter().enumerate().map(|(i, _)| format!("k{i}")),
                [("a", y.iter().map(|v| v.abs()).collect::<Vec<_>>())],
            );
        }),
        ("rose", |a, _, y| {
            a.rose(
                y.iter().enumerate().map(|(i, _)| format!("k{i}")),
                y.iter().map(|v| v.abs()),
            );
        }),
        ("surface", |a, _, y| {
            let m: Vec<Vec<f64>> = (0..y.len().max(2))
                .map(|r| y.iter().map(|v| v + r as f64).chain([0.0, 0.0]).collect())
                .collect();
            a.surface((0.0, 1.0), (0.0, 1.0), m).wireframe(true);
        }),
        ("scatter3d", |a, x, y| {
            a.scatter3d(x, y, y).colormap(Colormap::Viridis);
            a.line3d(x, y, x);
        }),
        ("image", |a, _, _| {
            a.image(2, 2, vec![255; 16]);
        }),
    ]
}

fn datasets() -> Vec<(&'static str, Vec<f64>, Vec<f64>)> {
    let x: Vec<f64> = (0..30).map(|i| i as f64).collect();
    vec![
        (
            "normal",
            x.clone(),
            x.iter().map(|v| (v * 0.4).sin() * 5.0 + 6.0).collect(),
        ),
        (
            "negative",
            x.iter().map(|v| v - 15.0).collect(),
            x.iter().map(|v| -(v * 0.3).cos() * 3.0 - 1.0).collect(),
        ),
        ("empty", vec![], vec![]),
        ("single", vec![2.0], vec![3.0]),
        ("constant", x.clone(), vec![4.0; 30]),
        (
            "nonfinite",
            vec![0.0, 1.0, f64::NAN, 3.0, 4.0, 5.0],
            vec![1.0, f64::INFINITY, 2.0, f64::NAN, -1e300, 1e300],
        ),
    ]
}

fn assert_valid_svg(svg: &str, ctx: &str) {
    usvg::Tree::from_str(svg, &usvg::Options::default())
        .unwrap_or_else(|e| panic!("{ctx}: invalid SVG ({e})"));
    // Numbers in attributes must be finite (text content may legitimately say "NaN").
    let mut markup = String::new();
    let mut in_text = false;
    for c in svg.chars() {
        match c {
            '<' => in_text = false,
            '>' => {
                in_text = true;
                markup.push(c);
                continue;
            }
            _ => {}
        }
        if !in_text {
            markup.push(c);
        }
    }
    assert!(
        !markup.contains("NaN") && !markup.contains("inf"),
        "{ctx}: non-finite number in SVG markup"
    );
}

#[test]
fn every_chart_type_renders_with_every_dataset() {
    for (name, build) in builders() {
        for (dname, x, y) in datasets() {
            let ctx = format!("{name}/{dname}");
            let mut fig = Figure::new(500, 320);
            build(fig.axes(), &x, &y);
            let svg = fig.to_svg();
            assert_valid_svg(&svg, &ctx);
            let px = fig.to_pixels().unwrap_or_else(|e| panic!("{ctx}: {e}"));
            assert_eq!((px.width, px.height), (500, 320), "{ctx}");
        }
    }
}

#[test]
fn every_chart_type_survives_tiny_canvases_and_themes() {
    let (_, x, y) = datasets().remove(0);
    for (name, build) in builders() {
        for (w, h) in [(1, 1), (40, 30)] {
            let mut fig = Figure::new(w, h);
            build(fig.axes(), &x, &y);
            assert_valid_svg(&fig.to_svg(), &format!("{name} {w}x{h}"));
        }
        for (tname, theme) in Theme::presets() {
            let mut fig = Figure::new(300, 200);
            fig.theme(theme);
            build(fig.axes(), &x, &y);
            assert_valid_svg(&fig.to_svg(), &format!("{name} {tname}"));
        }
    }
}

#[test]
fn output_formats() {
    let dir = std::env::temp_dir().join(format!("dataviz-plot-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut fig = Figure::new(200, 120);
    fig.axes().line([0, 1, 2], [1.0, 3.0, 2.0]);
    for ext in ["svg", "png", "jpg", "jpeg", "webp", "bmp"] {
        let p = dir.join(format!("f.{ext}"));
        fig.save(&p).unwrap_or_else(|e| panic!("{ext}: {e}"));
        assert!(std::fs::metadata(&p).unwrap().len() > 100, "{ext}");
    }
    let png = image::open(dir.join("f.png")).unwrap();
    assert_eq!((png.width(), png.height()), (200, 120));
    fig.scale(2.0);
    fig.save(dir.join("f2.png")).unwrap();
    assert_eq!(image::open(dir.join("f2.png")).unwrap().width(), 400);
    assert!(matches!(
        fig.save(dir.join("f.xyz")),
        Err(Error::UnsupportedFormat(_))
    ));
    #[cfg(feature = "pdf")]
    {
        fig.save(dir.join("f.pdf")).unwrap();
        assert!(std::fs::read(dir.join("f.pdf"))
            .unwrap()
            .starts_with(b"%PDF"));
    }
    #[cfg(not(feature = "pdf"))]
    assert!(matches!(
        fig.save(dir.join("f.pdf")),
        Err(Error::UnsupportedFormat(_))
    ));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn text_is_escaped_and_labels_present() {
    let mut fig = Figure::new(400, 300);
    fig.title("A & B <C>");
    let ax = fig.axes();
    ax.line([0, 1], [0, 1]).label("x < y");
    ax.x_label("time \"s\"");
    let svg = fig.to_svg();
    assert_valid_svg(&svg, "escape");
    assert!(
        svg.contains("A &amp; B &lt;C&gt;")
            && svg.contains("x &lt; y")
            && svg.contains("time &quot;s&quot;")
    );
}

fn tick_labels(svg: &str) -> Vec<String> {
    svg.split("<text")
        .skip(1)
        .filter_map(|t| {
            t.split_once('>')
                .map(|(_, r)| r.split("</text>").next().unwrap_or("").to_string())
        })
        .collect()
}

#[test]
fn axis_scales_and_formats() {
    let mut fig = Figure::new(600, 400);
    let ax = fig.axes();
    ax.line([1.0, 10.0, 100.0, 1000.0], [1.0, 2.0, 3.0, 4.0]);
    ax.x_axis().log();
    let labels = tick_labels(&fig.to_svg());
    for l in ["1", "10", "100", "1000"] {
        assert!(
            labels.iter().any(|x| x == l),
            "missing log tick {l}: {labels:?}"
        );
    }

    let mut fig = Figure::new(600, 400);
    let ax = fig.axes();
    ax.line([date(2024, 1, 1), date(2024, 6, 1)], [1, 2]);
    ax.x_axis().time();
    let labels = tick_labels(&fig.to_svg());
    assert!(labels.iter().any(|l| l.ends_with("2024")), "{labels:?}");

    let mut fig = Figure::new(600, 400);
    let ax = fig.axes();
    ax.line([0, 1], [0.0, 0.5]);
    ax.y_axis().format(TickFormat::Percent(0));
    assert!(tick_labels(&fig.to_svg()).iter().any(|l| l == "50%"));

    // Narrow log range still gets several ticks.
    let mut fig = Figure::new(600, 400);
    let ax = fig.axes();
    ax.scatter([1.2, 45.0], [1, 2]);
    ax.x_axis().log();
    let labels = tick_labels(&fig.to_svg());
    assert!(
        ["2", "5", "10", "20"]
            .iter()
            .all(|l| labels.iter().any(|x| x == l)),
        "{labels:?}"
    );
}

#[test]
fn explicit_limits_ticks_and_inversion() {
    let mut fig = Figure::new(600, 400);
    let ax = fig.axes();
    ax.line([0, 10], [0, 10]);
    ax.x_limits(0.0, 100.0);
    ax.x_axis()
        .ticks(TickSpec::Labeled(vec![
            (0.0, "start".into()),
            (100.0, "end".into()),
        ]))
        .inverted(true);
    let labels = tick_labels(&fig.to_svg());
    assert!(labels.contains(&"start".to_string()) && labels.contains(&"end".to_string()));
}

#[test]
fn legend_positions_and_hidden() {
    for pos in [
        LegendPosition::Best,
        LegendPosition::UpperLeft,
        LegendPosition::UpperRight,
        LegendPosition::LowerLeft,
        LegendPosition::LowerRight,
        LegendPosition::UpperCenter,
        LegendPosition::LowerCenter,
        LegendPosition::OutsideRight,
        LegendPosition::OutsideBottom,
        LegendPosition::OutsideTop,
    ] {
        let mut fig = Figure::new(500, 300);
        let ax = fig.axes();
        ax.line([0, 1], [0, 1]).label("first series");
        ax.legend(pos).legend_columns(2);
        let svg = fig.to_svg();
        assert!(svg.contains(">first series<"), "{pos:?}");
    }
    let mut fig = Figure::new(500, 300);
    let ax = fig.axes();
    ax.line([0, 1], [0, 1]).label("hidden");
    ax.legend(LegendPosition::Hidden);
    assert!(!fig.to_svg().contains(">hidden<"));
}

#[test]
fn subplots_spans_and_shared_axes() {
    let mut fig = Figure::new(800, 600);
    fig.grid(2, 2).share_y(true).title("grid");
    fig.at(0, 0).line([0, 1], [0, 1]);
    fig.at(0, 1).line([0, 1], [0, 100]);
    fig.span(1, 0, 1, 2).bar(["a", "b"], [1, 2]);
    assert_eq!(fig.all_axes().count(), 3);
    let svg = fig.to_svg();
    assert_valid_svg(&svg, "grid");
    // Shared y: the first plot also shows 100.
    assert!(tick_labels(&svg).iter().filter(|l| *l == "100").count() >= 2);
}

#[test]
fn streaming_data_updates() {
    let mut fig = Figure::new(300, 200);
    fig.axes().line(Vec::<f64>::new(), Vec::<f64>::new());
    for i in 0..100 {
        let l = fig.axes().series_mut::<Line>(0).unwrap();
        l.push(i as f64, (i as f64).sin());
        l.keep_last(10);
    }
    let l = fig.axes().series_mut::<Line>(0).unwrap();
    assert_eq!(l.data().0.len(), 10);
    assert_eq!(l.data().0[0], 90.0);
    assert!(fig
        .axes()
        .series_mut::<dataviz::plot::series::scatter::Scatter>(0)
        .is_none());
}

#[test]
fn rasterized_colors_match() {
    let mut fig = Figure::new(200, 200);
    fig.theme_mut().padding = 0.0;
    let ax = fig.axes();
    ax.bar(["only"], [10.0]).color("#ff0000").width(1.0);
    ax.x_axis().hidden();
    ax.y_axis().hidden();
    let px = fig.to_pixels().unwrap();
    // The centre of the plot is inside the bar.
    assert_eq!(px.get(100, 120), Some([255, 0, 0, 255]));
}

#[test]
fn terminal_and_sparkline() {
    let mut fig = Figure::new(300, 150);
    fig.axes().line([0, 1, 2, 3], [0, 3, 1, 2]);
    for mode in [
        TerminalMode::HalfBlock,
        TerminalMode::Braille,
        TerminalMode::BrailleMono,
    ] {
        let t = fig.to_terminal(40, 10, mode).unwrap();
        assert_eq!(t.lines().count(), 10);
    }
    let mono = fig.to_terminal(40, 10, TerminalMode::BrailleMono).unwrap();
    assert!(mono.chars().any(|c| ('\u{2801}'..='\u{28ff}').contains(&c)));
    let mut s = Figure::sparkline([1.0, f64::NAN, 3.0, 2.0], 100, 30);
    assert_valid_svg(&s.to_svg(), "sparkline");
}

#[test]
fn csv_to_chart() {
    let t = Table::parse("day,visits\n2024-01-01,10\n2024-01-02,15\n2024-01-03,12\n").unwrap();
    let mut fig = Figure::new(400, 300);
    let ax = fig.axes();
    ax.line(t.time_column("day").unwrap(), t.column("visits").unwrap());
    ax.x_axis().time();
    assert_valid_svg(&fig.to_svg(), "csv");
}

#[test]
fn custom_series_can_be_added() {
    use dataviz::plot::series::{Context, Extent};
    struct Cross;
    impl Series for Cross {
        fn extent(&self) -> Option<Extent> {
            Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
        }
        fn draw(&self, ctx: &mut Context) {
            let a = ctx.point(0.0, 0.0);
            let b = ctx.point(1.0, 1.0);
            ctx.polyline(&[a, b], dataviz::plot::scene::Stroke::new(Color::RED, 3.0));
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }
    let mut fig = Figure::new(300, 200);
    fig.axes().add(Cross);
    assert!(fig.to_svg().contains(r##"stroke="#d62728""##));
}

#[test]
fn missing_font_falls_back() {
    let mut fig = Figure::new(300, 200);
    fig.font("/no/such/font.ttf");
    fig.axes().line([0, 1], [0, 1]).label("x");
    fig.to_pixels().unwrap();
}

#[test]
fn output_is_deterministic() {
    let (_, x, y) = datasets().remove(0);
    for (name, build) in builders() {
        let render = || {
            let mut fig = Figure::new(400, 300);
            build(fig.axes(), &x, &y);
            fig.to_svg()
        };
        assert_eq!(render(), render(), "{name}");
    }
}
