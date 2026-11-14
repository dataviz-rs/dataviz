//! Renders every chart type of the `dataviz::plot` API.
//!
//! ```text
//! cargo run --release --example plot_gallery -- [output_dir] [svg|png|both]
//! ```

use dataviz::plot::series::frameless::TreeNode;
use dataviz::plot::stats::Bins;
use dataviz::prelude::*;
use std::f64::consts::PI;

/// Deterministic pseudo-random numbers.
struct Rng(u64);
impl Rng {
    fn uniform(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn normal(&mut self) -> f64 {
        (0..12).map(|_| self.uniform()).sum::<f64>() - 6.0
    }
}

type Chart = (&'static str, fn(&mut Figure));

fn charts() -> Vec<Chart> {
    vec![
        ("line", |f| {
            let ax = f.axes();
            ax.title("Line styles").x_label("x").y_label("y");
            let x: Vec<f64> = (0..=100).map(|i| i as f64 / 10.0).collect();
            for (k, d) in [
                Dash::Solid,
                Dash::Dashed,
                Dash::Dotted,
                Dash::DashDot,
                Dash::LongDash,
            ]
            .into_iter()
            .enumerate()
            {
                ax.line(&x, x.iter().map(|v| (v + k as f64).sin() + k as f64 * 0.6))
                    .dash(d.clone())
                    .label(format!("{d:?}"));
            }
            ax.legend(LegendPosition::OutsideRight);
        }),
        ("markers_steps_smooth", |f| {
            let ax = f.axes();
            ax.title("Markers, steps and smoothing");
            let x: Vec<f64> = (0..12).map(|i| i as f64).collect();
            let y: Vec<f64> = x.iter().map(|v| (v * 0.7).sin() * 3.0 + 5.0).collect();
            ax.line(&x, &y).marker(Marker::Circle).label("markers");
            ax.line(&x, y.iter().map(|v| v + 4.0))
                .step(Step::Post)
                .label("step (post)");
            ax.line(&x, y.iter().map(|v| v - 4.0))
                .smooth(true)
                .marker(Marker::Diamond)
                .label("smooth");
            ax.line(&x, y.iter().map(|v| v + 8.0))
                .width(0.0)
                .marker(Marker::Star)
                .marker_size(11.0)
                .label("markers only");
        }),
        ("area_fill", |f| {
            let ax = f.axes();
            ax.title("Area and fill between");
            let x: Vec<f64> = (0..=200).map(|i| i as f64 / 20.0).collect();
            ax.area(&x, x.iter().map(|v| v.sin() + 1.5)).label("area");
            ax.fill_between(
                &x,
                x.iter().map(|v| (v * 0.8).cos() - 1.0),
                x.iter().map(|v| (v * 0.8).cos() - 2.5),
            )
            .label("between");
        }),
        ("stacked_area", |f| {
            f.grid(1, 2);
            let x: Vec<f64> = (0..20).map(|i| i as f64).collect();
            let layers = || {
                vec![
                    (
                        "Solar",
                        (0..20).map(|i| 2.0 + i as f64 * 0.5).collect::<Vec<_>>(),
                    ),
                    (
                        "Wind",
                        (0..20)
                            .map(|i| 5.0 + (i as f64 * 0.5).sin() * 2.0)
                            .collect(),
                    ),
                    ("Hydro", vec![4.0; 20]),
                ]
            };
            let ax = f.at(0, 0);
            ax.title("Stacked area");
            ax.stacked_area(&x, layers(), false);
            ax.legend(LegendPosition::UpperLeft);
            let ax = f.at(0, 1);
            ax.title("100 % stacked");
            ax.stacked_area(&x, layers(), true);
            ax.y_axis()
                .format(TickFormat::Affix(String::new(), "%".into()));
            ax.legend(LegendPosition::Hidden);
        }),
        ("scatter_bubble", |f| {
            let mut r = Rng(1);
            f.grid(1, 2);
            let ax = f.at(0, 0);
            ax.title("Scatter, colored by value");
            let x: Vec<f64> = (0..300).map(|_| r.normal()).collect();
            let y: Vec<f64> = x.iter().map(|v| v * 0.6 + r.normal() * 0.5).collect();
            let c: Vec<f64> = x.iter().zip(&y).map(|(a, b)| a.hypot(*b)).collect();
            ax.scatter(&x, &y)
                .values(c)
                .colorbar("distance")
                .opacity(0.85);
            ax.regression(&x, &y, 1);
            let ax = f.at(0, 1);
            ax.title("Bubble chart");
            let gdp = [1.2, 3.5, 7.0, 15.0, 30.0, 45.0];
            let life = [62.0, 68.0, 72.0, 76.0, 80.0, 82.0];
            let pop = [80.0, 300.0, 50.0, 120.0, 30.0, 330.0];
            ax.scatter(gdp, life)
                .bubble(pop, 8.0, 60.0)
                .names(["A", "B", "C", "D", "E", "F"])
                .label("countries");
            ax.x_axis().log().label("GDP per capita (k$)");
            ax.y_label("Life expectancy");
        }),
        ("bars", |f| {
            f.grid(2, 2);
            let cats = ["North", "South", "East", "West"];
            let data = || {
                vec![
                    ("2022", vec![12.0, 9.0, 15.0, 7.0]),
                    ("2023", vec![14.0, 11.0, 13.0, 9.0]),
                    ("2024", vec![17.0, 10.0, 16.0, 12.0]),
                ]
            };
            let ax = f.at(0, 0);
            ax.title("Grouped");
            for b in ax.bars(cats, data(), BarMode::Grouped, false) {
                b.value_labels(LabelPosition::Outside);
            }
            let ax = f.at(0, 1);
            ax.title("Stacked");
            for b in ax.bars(cats, data(), BarMode::Stacked, false) {
                b.value_labels(LabelPosition::Inside);
            }
            let ax = f.at(1, 0);
            ax.title("Percent (horizontal)");
            ax.bars(cats, data(), BarMode::Percent, true);
            ax.x_axis()
                .format(TickFormat::Affix(String::new(), "%".into()));
            ax.legend(LegendPosition::OutsideBottom);
            let ax = f.at(1, 1);
            ax.title("Negative values");
            ax.bar(["Q1", "Q2", "Q3", "Q4", "Q5"], [4.0, -2.5, 6.0, -1.0, 3.0])
                .colors([
                    Color::GREEN,
                    Color::RED,
                    Color::GREEN,
                    Color::RED,
                    Color::GREEN,
                ])
                .value_labels(LabelPosition::Outside)
                .corner_radius(3.0);
        }),
        ("histograms", |f| {
            let mut r = Rng(7);
            f.grid(1, 2);
            let a: Vec<f64> = (0..3000).map(|_| r.normal()).collect();
            let b: Vec<f64> = (0..3000).map(|_| r.normal() * 0.7 + 1.5).collect();
            let ax = f.at(0, 0);
            ax.title("Overlapping + KDE");
            ax.hist(&a).opacity(0.55).density(true).label("A");
            ax.hist(&b).opacity(0.55).density(true).label("B");
            ax.kde(&a, None).label("KDE A");
            let ax = f.at(0, 1);
            ax.title("Cumulative step / ECDF");
            ax.hist(&a)
                .bins(Bins::Count(40))
                .cumulative(true)
                .density(true)
                .step(true)
                .label("cumulative hist");
            ax.ecdf(&b).label("ECDF B");
            ax.legend(LegendPosition::LowerRight);
        }),
        ("box_violin", |f| {
            let mut r = Rng(3);
            f.grid(1, 2);
            let groups = |r: &mut Rng| -> Vec<(String, Vec<f64>)> {
                (0..4)
                    .map(|g| {
                        (
                            format!("group {}", g + 1),
                            (0..200)
                                .map(|_| r.normal() * (1.0 + g as f64 * 0.3) + g as f64)
                                .chain([8.0 + g as f64])
                                .collect(),
                        )
                    })
                    .collect()
            };
            let ax = f.at(0, 0);
            ax.title("Box plot");
            ax.boxplot(groups(&mut r)).show_mean(true).notch(true);
            let ax = f.at(0, 1);
            ax.title("Violin plot");
            ax.violin(groups(&mut r));
        }),
        ("errorbars_band", |f| {
            let ax = f.axes();
            ax.title("Error bars and confidence band");
            let x: Vec<f64> = (0..10).map(|i| i as f64).collect();
            let y: Vec<f64> = x.iter().map(|v| v * 0.8 + (v * 1.3).sin()).collect();
            ax.error_bars(&x, &y, x.iter().map(|v| 0.3 + v * 0.05))
                .label("measurements");
            let xs: Vec<f64> = (0..=90).map(|i| i as f64 / 10.0).collect();
            let fit: Vec<f64> = xs.iter().map(|v| v * 0.8).collect();
            ax.error_band(
                &xs,
                &fit,
                fit.iter().map(|v| v - 1.0),
                fit.iter().map(|v| v + 1.0),
            )
            .label("model ± 1");
        }),
        ("heatmap", |f| {
            let ax = f.axes();
            ax.title("Correlation matrix");
            let names = ["temp", "humidity", "wind", "pressure", "rain"];
            let m = vec![
                vec![1.0, -0.62, 0.15, -0.31, -0.22],
                vec![-0.62, 1.0, -0.08, 0.12, 0.71],
                vec![0.15, -0.08, 1.0, -0.45, 0.33],
                vec![-0.31, 0.12, -0.45, 1.0, -0.54],
                vec![-0.22, 0.71, 0.33, -0.54, 1.0],
            ];
            ax.heatmap_labeled(names, names, m)
                .colormap(Colormap::RdBu)
                .range(-1.0, 1.0)
                .annotate(true)
                .decimals(2)
                .colorbar("r");
        }),
        ("hist2d_hexbin", |f| {
            let mut r = Rng(11);
            f.grid(1, 2);
            let x: Vec<f64> = (0..20000).map(|_| r.normal()).collect();
            let y: Vec<f64> = x.iter().map(|v| v * 0.5 + r.normal() * 0.8).collect();
            let ax = f.at(0, 0);
            ax.title("2D histogram");
            ax.hist2d(&x, &y, 40, 40).colormap(Colormap::Inferno);
            let ax = f.at(0, 1);
            ax.title("Hexbin");
            ax.hexbin(&x, &y).gridsize(25).colormap(Colormap::Viridis);
        }),
        ("contour", |f| {
            f.grid(1, 2);
            let g = |x: f64, y: f64| (x * x + y - 11.0).powi(2) + (x + y * y - 7.0).powi(2);
            let ax = f.at(0, 0);
            ax.title("Contour lines (Himmelblau)");
            ax.contour_fn(|x, y| g(x, y).ln(), (-5.0, 5.0), (-5.0, 5.0), 120)
                .level_count(12)
                .labels(true);
            let ax = f.at(0, 1);
            ax.title("Filled contours");
            ax.contour_fn(
                |x, y| (x * y).sin() * (-0.05 * (x * x + y * y)).exp(),
                (-6.0, 6.0),
                (-6.0, 6.0),
                150,
            )
            .level_count(14)
            .filled(true)
            .colormap(Colormap::Coolwarm)
            .colorbar("z");
        }),
        ("candlestick", |f| {
            let mut r = Rng(5);
            let ax = f.axes();
            ax.title("Candlestick + moving average");
            let n = 60;
            let t: Vec<f64> = (0..n)
                .map(|i| date(2024, 1, 1) + i as f64 * 86400.0)
                .collect();
            let mut price = 100.0;
            let (mut o, mut h, mut l, mut c) = (vec![], vec![], vec![], vec![]);
            for _ in 0..n {
                let open = price;
                let close = open + r.normal() * 2.0;
                o.push(open);
                c.push(close);
                h.push(open.max(close) + r.uniform() * 2.0);
                l.push(open.min(close) - r.uniform() * 2.0);
                price = close;
            }
            ax.candlestick(&t, &o, &h, &l, &c).label("ACME");
            ax.moving_average(&t, &c, 10);
            ax.x_axis().time();
            ax.y_label("price ($)");
        }),
        ("time_series", |f| {
            let ax = f.axes();
            ax.title("Time axis, annotations, twin axis");
            let t: Vec<f64> = (0..24 * 7)
                .map(|h| datetime(2024, 3, 4, 0, 0, 0.0) + h as f64 * 3600.0)
                .collect();
            let temp: Vec<f64> = t
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    12.0 + 6.0 * ((i as f64 - 6.0) / 24.0 * 2.0 * PI).sin() + i as f64 * 0.02
                })
                .collect();
            let rain: Vec<f64> = (0..t.len())
                .map(|i| {
                    if (i / 13) % 4 == 0 {
                        ((i % 13) as f64 * 0.4).sin().abs() * 3.0
                    } else {
                        0.0
                    }
                })
                .collect();
            ax.line(&t, &temp).label("temperature °C");
            ax.bar_xy(&t, &rain)
                .width(3000.0)
                .color(Color::BLUE.with_alpha(0.5))
                .secondary(true)
                .label("rain mm");
            ax.x_axis().time();
            ax.y_label("°C");
            ax.y2_label("mm");
            ax.hline(15.0).label("threshold");
            ax.vspan(
                datetime(2024, 3, 6, 0, 0, 0.0),
                datetime(2024, 3, 7, 0, 0, 0.0),
            )
            .label("holiday");
            ax.legend(LegendPosition::OutsideBottom);
        }),
        ("log_scales", |f| {
            f.grid(1, 2);
            let x: Vec<f64> = (0..=60).map(|i| 10f64.powf(i as f64 / 10.0)).collect();
            let ax = f.at(0, 0);
            ax.title("Log–log");
            ax.line(&x, x.iter().map(|v| v.powf(1.5))).label("x^1.5");
            ax.line(&x, x.iter().map(|v| v.sqrt() * 10.0))
                .label("10·√x");
            ax.x_axis().log().minor_ticks(true);
            ax.y_axis().log().minor_ticks(true);
            let ax = f.at(0, 1);
            ax.title("Symmetric log");
            let x: Vec<f64> = (-100..=100).map(|i| i as f64).collect();
            ax.line(&x, x.iter().map(|v| v.powi(3))).label("x³");
            ax.y_scale(Scale::SymLog(1.0));
        }),
        ("formats", |f| {
            let ax = f.axes();
            ax.title("Tick formats");
            let x: Vec<f64> = (0..10).map(|i| i as f64 * 250_000.0).collect();
            ax.line(&x, x.iter().map(|v| v / 3e6))
                .marker(Marker::Square);
            ax.x_axis().format(TickFormat::Si(1)).label("bytes");
            ax.y_axis().format(TickFormat::Percent(0)).label("share");
        }),
        ("stem_waterfall", |f| {
            f.grid(1, 2);
            let ax = f.at(0, 0);
            ax.title("Stem");
            let x: Vec<f64> = (0..30).map(|i| i as f64).collect();
            ax.stem(&x, x.iter().map(|v| (v * 0.4).sin() * (-v / 15.0).exp()));
            let ax = f.at(0, 1);
            ax.title("Waterfall");
            ax.waterfall(
                ["Start", "Sales", "Services", "Costs", "Tax", "Total"],
                [100.0, 45.0, 20.0, -60.0, -15.0, 0.0],
            )
            .total_at(5);
        }),
        ("quiver_function", |f| {
            f.grid(1, 2);
            let ax = f.at(0, 0);
            ax.title("Vector field");
            let mut x = vec![];
            let mut y = vec![];
            for i in -8..=8 {
                for j in -8..=8 {
                    x.push(i as f64 / 4.0);
                    y.push(j as f64 / 4.0);
                }
            }
            let u: Vec<f64> = y.iter().map(|v| -v).collect();
            let v: Vec<f64> = x.clone();
            ax.quiver(&x, &y, u, v).colormap(Colormap::Plasma);
            ax.equal_aspect(true);
            let ax = f.at(0, 1);
            ax.title("Function plots");
            ax.function(|x| x.sin() / x, -20.0, 20.0).label("sin(x)/x");
            ax.function(|x| (x / 4.0).tan() / 10.0, -20.0, 20.0)
                .label("tan(x/4)/10");
            ax.y_limits(-0.5, 1.2);
        }),
        ("gantt", |f| {
            let ax = f.axes();
            ax.title("Project plan");
            let d = |m: u32, day: u32| date(2024, m, day);
            ax.gantt(
                ["Research", "Design", "Build", "Test", "Launch"],
                [d(1, 1), d(1, 20), d(2, 10), d(3, 15), d(4, 5)],
                [d(1, 25), d(2, 20), d(3, 30), d(4, 10), d(4, 15)],
            )
            .progress([1.0, 1.0, 0.6, 0.1, 0.0]);
            ax.x_axis().time();
            ax.vline(d(3, 1)).label("today");
        }),
        ("pie_donut", |f| {
            f.grid(1, 2);
            let ax = f.at(0, 0);
            ax.title("Pie");
            ax.pie(
                ["Chrome", "Safari", "Edge", "Firefox", "Other"],
                [64.0, 19.0, 5.0, 3.0, 9.0],
            )
            .explode([0.06, 0.0, 0.0, 0.0, 0.0])
            .labels(true);
            ax.legend(LegendPosition::Hidden);
            let ax = f.at(0, 1);
            ax.title("Donut");
            ax.pie(
                ["Rent", "Food", "Transport", "Fun", "Savings"],
                [1200.0, 500.0, 200.0, 250.0, 600.0],
            )
            .donut(0.55)
            .center_text("$2750");
            ax.legend(LegendPosition::OutsideRight);
        }),
        ("polar_radar", |f| {
            f.grid(1, 3);
            let ax = f.at(0, 0);
            ax.title("Polar");
            let t: Vec<f64> = (0..=360).map(|i| (i as f64).to_radians()).collect();
            ax.polar_line(&t, t.iter().map(|a| (4.0 * a).sin().abs() * 2.0 + 1.0))
                .label("rose curve");
            ax.polar_line(&t, t.iter().map(|a| 1.0 + a / PI))
                .label("spiral")
                .dash(Dash::Dashed);
            ax.legend(LegendPosition::Hidden);
            let ax = f.at(0, 1);
            ax.title("Radar");
            ax.radar(
                ["Speed", "Power", "Range", "Comfort", "Price", "Safety"],
                [
                    ("Model A", vec![8.0, 6.0, 7.0, 5.0, 4.0, 9.0]),
                    ("Model B", vec![6.0, 8.0, 5.0, 8.0, 7.0, 6.0]),
                ],
            );
            ax.r_limits(0.0, 10.0);
            ax.legend(LegendPosition::OutsideBottom);
            let ax = f.at(0, 2);
            ax.title("Wind rose");
            ax.rose(
                ["N", "NE", "E", "SE", "S", "SW", "W", "NW"],
                [12.0, 7.0, 5.0, 9.0, 14.0, 20.0, 16.0, 8.0],
            );
            ax.legend(LegendPosition::Hidden);
        }),
        ("treemap_sunburst", |f| {
            f.grid(1, 2);
            let tree = || {
                vec![
                    TreeNode::group(
                        "Tech",
                        vec![
                            TreeNode::leaf("Apple", 34.0),
                            TreeNode::leaf("Microsoft", 30.0),
                            TreeNode::leaf("Nvidia", 28.0),
                            TreeNode::leaf("Google", 21.0),
                        ],
                    ),
                    TreeNode::group(
                        "Finance",
                        vec![
                            TreeNode::leaf("JPM", 6.0),
                            TreeNode::leaf("Visa", 5.0),
                            TreeNode::leaf("Bank B", 3.0),
                        ],
                    ),
                    TreeNode::group(
                        "Health",
                        vec![
                            TreeNode::leaf("Lilly", 7.0),
                            TreeNode::leaf("UNH", 5.0),
                            TreeNode::leaf("J&J", 4.0),
                        ],
                    ),
                    TreeNode::group(
                        "Energy",
                        vec![TreeNode::leaf("Exxon", 5.0), TreeNode::leaf("Chevron", 3.0)],
                    ),
                ]
            };
            let ax = f.at(0, 0);
            ax.title("Treemap");
            ax.treemap(tree());
            ax.legend(LegendPosition::Hidden);
            let ax = f.at(0, 1);
            ax.title("Sunburst");
            ax.sunburst(tree());
            ax.legend(LegendPosition::Hidden);
        }),
        ("sankey", |f| {
            let ax = f.axes();
            ax.title("Energy flow");
            ax.sankey([
                ("Coal", "Electricity", 30.0),
                ("Gas", "Electricity", 25.0),
                ("Gas", "Heat", 20.0),
                ("Solar", "Electricity", 15.0),
                ("Oil", "Transport", 40.0),
                ("Electricity", "Homes", 35.0),
                ("Electricity", "Industry", 30.0),
                ("Electricity", "Losses", 5.0),
                ("Heat", "Homes", 15.0),
                ("Heat", "Industry", 5.0),
                ("Transport", "Losses", 12.0),
                ("Transport", "Mobility", 28.0),
            ]);
        }),
        ("funnel_gauge", |f| {
            f.grid(1, 2);
            let ax = f.at(0, 0);
            ax.title("Sales funnel");
            ax.funnel(
                ["Visits", "Sign-ups", "Trials", "Purchases"],
                [10000.0, 3200.0, 1100.0, 420.0],
            );
            let ax = f.at(0, 1);
            ax.title("CPU load");
            ax.gauge(73.0, 0.0, 100.0)
                .bands([
                    (50.0, Color::GREEN),
                    (80.0, Color::ORANGE),
                    (100.0, Color::RED),
                ])
                .label("percent");
        }),
        ("network", |f| {
            let ax = f.axes();
            ax.title("Network (force-directed)");
            let names: Vec<String> = (0..16).map(|i| format!("n{i}")).collect();
            let mut edges = vec![];
            for i in 0..16usize {
                edges.push((i, (i + 1) % 6 + (i / 6) * 6, 1.0));
                if i % 6 == 0 {
                    edges.push((i, (i + 6) % 16, 2.0));
                }
            }
            ax.network(names, edges).groups((0..16).map(|i| i / 6));
        }),
        ("parallel_calendar", |f| {
            let mut r = Rng(9);
            f.grid(2, 1);
            let ax = f.at(0, 0);
            ax.title("Parallel coordinates");
            let rows: Vec<Vec<f64>> = (0..60)
                .map(|i| {
                    let c = (i % 3) as f64;
                    vec![
                        5.0 + c + r.normal() * 0.4,
                        3.0 - c * 0.3 + r.normal() * 0.3,
                        1.5 + c * 2.0 + r.normal() * 0.4,
                        0.2 + c * 0.8 + r.normal() * 0.2,
                    ]
                })
                .collect();
            ax.parallel_coordinates(["sepal L", "sepal W", "petal L", "petal W"], rows)
                .classes(
                    (0..60).map(|i| i % 3),
                    ["setosa", "versicolor", "virginica"],
                );
            ax.legend(LegendPosition::OutsideRight);
            let ax = f.at(1, 0);
            ax.title("Calendar heatmap");
            let days: Vec<f64> = (0..365)
                .map(|d| date(2024, 1, 1) + d as f64 * 86400.0)
                .collect();
            let vals: Vec<f64> = (0..365)
                .map(|d| ((d as f64 / 20.0).sin() + 1.0) * 5.0 + r.uniform() * 4.0)
                .collect();
            ax.calendar(&days, &vals).colorbar("commits");
        }),
        ("three_d", |f| {
            f.grid(1, 2);
            let ax = f.at(0, 0);
            ax.title("Surface");
            ax.surface_fn(
                |x, y| (x.hypot(y)).sin() / x.hypot(y).max(1e-9),
                (-8.0, 8.0),
                (-8.0, 8.0),
                40,
            )
            .colormap(Colormap::Viridis);
            ax.x_label("x").y_label("y").z_label("z");
            let ax = f.at(0, 1);
            ax.title("3D trajectory");
            let t: Vec<f64> = (0..600).map(|i| i as f64 / 30.0).collect();
            ax.line3d(
                t.iter().map(|v| v.cos() * (1.0 + v / 10.0)),
                t.iter().map(|v| v.sin() * (1.0 + v / 10.0)),
                &t,
            )
            .colormap(Colormap::Plasma);
            ax.x_label("x").y_label("y").z_label("t");
        }),
        ("subplots_shared", |f| {
            f.grid(2, 2).share_x(true);
            let x: Vec<f64> = (0..100).map(|i| i as f64 / 10.0).collect();
            for (i, name) in ["sin", "cos", "tanh", "exp"].iter().enumerate() {
                let ax = f.at(i / 2, i % 2);
                ax.title(*name);
                let y: Vec<f64> = x
                    .iter()
                    .map(|v| match i {
                        0 => v.sin(),
                        1 => v.cos(),
                        2 => (v - 5.0).tanh(),
                        _ => (v / 3.0).exp(),
                    })
                    .collect();
                ax.line(&x, y);
            }
        }),
        ("annotations", |f| {
            let ax = f.axes();
            ax.title("Annotations");
            let x: Vec<f64> = (0..=100).map(|i| i as f64 / 10.0).collect();
            ax.line(&x, x.iter().map(|v| (v * 0.8).sin() * v));
            ax.arrow((2.0, 6.0), (5.9, -4.6), Some("minimum"));
            ax.text(7.5, 6.0, "peak region").bold(true);
            ax.hspan(-1.0, 1.0).label("noise band");
            ax.rectangle(6.5, 3.5, 8.8, 7.5);
            ax.vline(5.0);
        }),
        ("image", |f| {
            let ax = f.axes();
            ax.title("Image (Mandelbrot)");
            let (w, h) = (300u32, 200u32);
            let mut px = Vec::with_capacity((w * h * 4) as usize);
            for j in 0..h {
                for i in 0..w {
                    let (cx, cy) = (
                        -2.2 + 3.2 * i as f64 / w as f64,
                        -1.0 + 2.0 * j as f64 / h as f64,
                    );
                    let (mut x, mut y, mut k) = (0.0, 0.0, 0);
                    while x * x + y * y < 4.0 && k < 60 {
                        (x, y) = (x * x - y * y + cx, 2.0 * x * y + cy);
                        k += 1;
                    }
                    let c = Colormap::Magma.at(k as f64 / 60.0);
                    px.extend_from_slice(&[c.r, c.g, c.b, 255]);
                }
            }
            ax.image(w, h, px).smooth(true);
        }),
    ]
}

fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "plot_gallery".into());
    let mode = args.next().unwrap_or_else(|| "both".into());
    std::fs::create_dir_all(&out)?;
    for (name, build) in charts() {
        let mut fig = Figure::new(1000, 620);
        build(&mut fig);
        if mode != "png" {
            fig.save(format!("{out}/{name}.svg"))?;
        }
        if mode != "svg" {
            fig.save(format!("{out}/{name}.png"))?;
        }
        println!("{name}");
    }
    // Every theme on the same chart.
    for (name, theme) in Theme::presets() {
        let mut fig = Figure::new(800, 500);
        fig.theme(theme);
        let ax = fig.axes();
        ax.title(format!("Theme: {name}"));
        let x: Vec<f64> = (0..=60).map(|i| i as f64 / 6.0).collect();
        for k in 0..4 {
            ax.line(
                &x,
                x.iter()
                    .map(|v| (v + k as f64).sin() * (1.0 + k as f64 * 0.3)),
            )
            .label(format!("series {k}"));
        }
        ax.x_label("x").y_label("y");
        fig.save(format!("{out}/theme_{name}.png"))?;
    }
    Ok(())
}
