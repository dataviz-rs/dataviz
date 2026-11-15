//! Visual regression tests: representative charts rendered on the CPU and
//! compared with reference images in `tests/golden/`.
//!
//! A chart passes when at most 0.3 % of its pixels differ by more than 40
//! (of 255) in a channel and the mean difference stays under 1.5 — small
//! anti-aliasing differences between platforms pass, a moved line or a
//! missing label does not. On failure the rendered image and a difference
//! image are written to `target/golden/`.
//!
//! `UPDATE_GOLDEN=1 cargo test --test golden` writes new references.

use dataviz::prelude::*;
use std::path::PathBuf;

const W: u32 = 480;
const H: u32 = 320;

type Build = fn(&mut Figure);

fn charts() -> Vec<(&'static str, Build)> {
    vec![
        ("line_legend", |f| {
            let x: Vec<f64> = (0..60).map(|i| i as f64 / 6.0).collect();
            let ax = f.axes();
            ax.title("Lines").x_label("t [s]").y_label("value");
            ax.line(&x, x.iter().map(|v| v.sin())).label("sin");
            ax.line(&x, x.iter().map(|v| v.cos()))
                .label("cos")
                .dash(Dash::Dashed);
            ax.legend(LegendPosition::UpperRight);
        }),
        ("scatter_sizes", |f| {
            let x: Vec<f64> = (0..80).map(|i| (i as f64 * 0.618).fract() * 10.0).collect();
            let y: Vec<f64> = (0..80).map(|i| (i as f64 * 0.414).fract() * 10.0).collect();
            f.axes().title("Scatter").scatter(&x, &y).size(6.0);
        }),
        ("bars_grouped", |f| {
            let ax = f.axes();
            ax.title("Grouped bars");
            ax.bar(["Q1", "Q2", "Q3", "Q4"], [3.0, 5.0, 4.0, 6.0])
                .label("2024");
            ax.bar(["Q1", "Q2", "Q3", "Q4"], [4.0, 4.5, 5.0, 7.0])
                .label("2025");
            ax.legend(LegendPosition::UpperLeft);
        }),
        ("histogram", |f| {
            let v: Vec<f64> = (0..2000)
                .map(|i| ((i as f64 * 0.37).sin() + (i as f64 * 0.11).cos()) * 2.0)
                .collect();
            f.axes().title("Histogram").hist(v).bin_count(30);
        }),
        ("box_violin", |f| {
            f.grid(1, 2);
            let g = |s: f64| {
                (0..200)
                    .map(move |i| (i as f64 * 0.73).sin() * s + (i as f64 * 0.19).cos())
                    .collect::<Vec<f64>>()
            };
            f.at(0, 0)
                .title("Box")
                .boxplot([("a", g(1.0)), ("b", g(2.0))]);
            f.at(0, 1)
                .title("Violin")
                .violin([("a", g(1.0)), ("b", g(2.0))]);
        }),
        ("heatmap_annotated", |f| {
            let z: Vec<Vec<f64>> = (0..5)
                .map(|r| (0..5).map(|c| ((r * 5 + c) as f64 * 0.7).sin()).collect())
                .collect();
            f.axes()
                .title("Heatmap")
                .heatmap_labeled(["a", "b", "c", "d", "e"], ["v", "w", "x", "y", "z"], z)
                .annotate(true)
                .colormap(Colormap::RdBu);
        }),
        ("pie_donut", |f| {
            f.axes()
                .title("Donut")
                .pie(
                    ["alpha", "beta", "gamma", "delta"],
                    [40.0, 25.0, 20.0, 15.0],
                )
                .donut(0.5);
        }),
        ("contour", |f| {
            f.axes()
                .title("Contours")
                .contour_fn(
                    |x, y| (x * x + y * y).sqrt().sin(),
                    (-4.0, 4.0),
                    (-3.0, 3.0),
                    60,
                )
                .filled(true);
        }),
        ("candlestick", |f| {
            let n = 30;
            let x: Vec<f64> = (0..n).map(|i| i as f64).collect();
            let o: Vec<f64> = x.iter().map(|t| 100.0 + (t * 0.5).sin() * 5.0).collect();
            let c: Vec<f64> = x
                .iter()
                .map(|t| 100.0 + (t * 0.5 + 0.4).sin() * 5.0)
                .collect();
            let h: Vec<f64> = o.iter().zip(&c).map(|(a, b)| a.max(*b) + 1.0).collect();
            let l: Vec<f64> = o.iter().zip(&c).map(|(a, b)| a.min(*b) - 1.0).collect();
            f.axes().title("Candles").candlestick(x, o, h, l, c);
        }),
        ("choropleth", |f| {
            let ax = f.axes();
            ax.map(dataviz::plot::geo::Projection::Robinson);
            ax.title("World");
            ax.choropleth([
                ("DEU", 83.0),
                ("TUR", 85.0),
                ("USA", 335.0),
                ("BRA", 203.0),
                ("IND", 1430.0),
                ("CHN", 1410.0),
            ])
            .colorbar("M");
        }),
        ("sankey", |f| {
            f.axes().title("Sankey").sankey([
                ("coal", "power", 30.0),
                ("gas", "power", 20.0),
                ("power", "homes", 25.0),
                ("power", "industry", 25.0),
            ]);
        }),
        ("surface_3d", |f| {
            let ax = f.axes();
            ax.title("Surface");
            ax.surface_fn(
                |x, y| (x * x + y * y).sqrt().sin(),
                (-5.0, 5.0),
                (-5.0, 5.0),
                24,
            )
            .colormap(Colormap::Viridis);
            ax.x_label("x").y_label("y").z_label("z");
        }),
        ("dna_helix", |f| {
            f.axes()
                .dna_helix("ATGCGTATAAAAGC")
                .highlight(4..8, "#ffd400", Some("site"));
        }),
        ("chord_streamgraph", |f| {
            f.grid(1, 2);
            f.at(0, 0).chord(
                ["a", "b", "c"],
                &[
                    vec![1.0, 4.0, 2.0],
                    vec![3.0, 0.0, 1.0],
                    vec![2.0, 2.0, 1.0],
                ],
            );
            let x: Vec<f64> = (0..30).map(|i| i as f64).collect();
            f.at(0, 1).streamgraph(
                &x,
                &[
                    ("p", x.iter().map(|v| 2.0 + (v / 4.0).sin()).collect()),
                    ("q", x.iter().map(|v| 1.5 + (v / 6.0).cos()).collect()),
                ],
            );
        }),
        ("math_labels", |f| {
            let x: Vec<f64> = (0..50).map(|i| i as f64 / 10.0).collect();
            let ax = f.axes();
            ax.title(r"$e^{-x^2/2}$ and $\frac{1}{1+x^2}$");
            ax.line(&x, x.iter().map(|v| (-v * v / 2.0).exp()))
                .label(r"$e^{-x^2/2}$");
            ax.line(&x, x.iter().map(|v| 1.0 / (1.0 + v * v)))
                .label(r"$\frac{1}{1+x^2}$");
            ax.y_label(r"$\sigma^2$ [m²]");
        }),
        ("radar", |f| {
            f.axes().title("Radar").radar(
                ["speed", "power", "range", "cost", "comfort"],
                [
                    ("A", vec![4.0, 3.0, 5.0, 2.0, 4.0]),
                    ("B", vec![3.0, 4.0, 2.0, 4.0, 3.0]),
                ],
            );
        }),
    ]
}

fn render(build: Build) -> Pixels {
    let mut fig = Figure::new(W, H);
    fig.backend(Backend::Cpu { threads: 1 });
    build(&mut fig);
    let mut px = Pixels::default();
    fig.render_into(&mut px).unwrap();
    px
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

fn save_png(path: &PathBuf, px: &Pixels) {
    image::RgbaImage::from_raw(px.width, px.height, px.rgba.clone())
        .unwrap()
        .save(path)
        .unwrap();
}

#[test]
fn charts_match_their_references() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("golden");
    let mut failures = Vec::new();
    for (name, build) in charts() {
        let px = render(build);
        let reference = golden_dir().join(format!("{name}.png"));
        if update {
            std::fs::create_dir_all(golden_dir()).unwrap();
            save_png(&reference, &px);
            continue;
        }
        let Ok(img) = image::open(&reference) else {
            failures.push(format!("{name}: no reference (run with UPDATE_GOLDEN=1)"));
            continue;
        };
        let img = img.to_rgba8();
        if img.width() != px.width || img.height() != px.height {
            failures.push(format!(
                "{name}: size {}×{} vs {}×{}",
                px.width,
                px.height,
                img.width(),
                img.height()
            ));
            continue;
        }
        let (mut bad, mut sum) = (0usize, 0u64);
        let mut diff = vec![255u8; px.rgba.len()];
        for (i, (a, b)) in px
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(img.as_raw().as_chunks::<4>().0)
            .enumerate()
        {
            let d = (0..4).map(|k| a[k].abs_diff(b[k])).max().unwrap();
            sum += d as u64;
            if d > 40 {
                bad += 1;
                diff[i * 4..i * 4 + 4].copy_from_slice(&[220, 30, 30, 255]);
            } else {
                let g = 255 - d * 3;
                diff[i * 4..i * 4 + 4].copy_from_slice(&[g, g, g, 255]);
            }
        }
        let n = (px.width * px.height) as f64;
        let (share, mean) = (bad as f64 / n, sum as f64 / n);
        if share > 0.003 || mean > 1.5 {
            std::fs::create_dir_all(&out).unwrap();
            save_png(&out.join(format!("{name}.png")), &px);
            let d = Pixels {
                width: px.width,
                height: px.height,
                rgba: diff,
            };
            save_png(&out.join(format!("{name}.diff.png")), &d);
            failures.push(format!("{name}: {:.2} % of pixels differ, mean difference {mean:.2} (see target/golden/{name}.png and .diff.png)", share * 100.0));
        }
    }
    assert!(
        failures.is_empty(),
        "visual differences:\n{}",
        failures.join("\n")
    );
}
