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
        ("globe", |f| {
            let ax = f.axes();
            ax.title("Globe").globe(30.0, 35.0);
            ax.choropleth([
                ("TUR", 85.0),
                ("DEU", 83.0),
                ("EGY", 110.0),
                ("IND", 1400.0),
            ])
            .graticule(30.0);
        }),
        ("equal_earth", |f| {
            let ax = f.axes();
            ax.title("Equal Earth")
                .map(dataviz::plot::geo::Projection::EqualEarth);
            ax.world_map().graticule(30.0);
        }),
        ("arc_diagram", |f| {
            let n: Vec<String> = (0..12).map(|i| format!("n{i}")).collect();
            let e: Vec<(usize, usize, f64)> = (0..18)
                .map(|k| (k * 5 % 12, (k * 7 + 3) % 12, 1.0 + (k % 3) as f64))
                .collect();
            f.axes()
                .title("Arc diagram")
                .arc_diagram(&n, e)
                .groups((0..12).map(|i| i / 4));
        }),
        ("edge_bundling", |f| {
            let n: Vec<String> = (0..16).map(|i| format!("n{i}")).collect();
            let e: Vec<(usize, usize, f64)> = (0..30)
                .map(|k| (k * 5 % 16, (k * 7 + 3) % 16, 1.0))
                .collect();
            f.axes()
                .title("Edge bundling")
                .edge_bundling(&n, (0..16).map(|i| i / 4), e);
        }),
        ("volume", |f| {
            let n = 20;
            let mut v = Vec::new();
            for k in 0..n {
                for j in 0..n {
                    for i in 0..n {
                        let c = |a: usize| a as f64 / (n - 1) as f64 * 2.0 - 1.0;
                        let r2 = c(i) * c(i) + c(j) * c(j) * 2.0 + c(k) * c(k);
                        v.push((-r2 * 4.0).exp());
                    }
                }
            }
            f.axes().title("Volume").volume(v, [n, n, n]).opacity(0.5);
        }),
        ("scatter3d_line3d", |f| {
            let t: Vec<f64> = (0..300).map(|i| i as f64 * 0.05).collect();
            let ax = f.axes();
            ax.title("3D points and a line");
            ax.line3d(
                t.iter().map(|v| v.cos() * v).collect::<Vec<_>>(),
                t.iter().map(|v| v.sin() * v).collect::<Vec<_>>(),
                t.clone(),
            )
            .size(1.5);
            ax.scatter3d(
                t.iter()
                    .step_by(10)
                    .map(|v| v.cos() * v)
                    .collect::<Vec<_>>(),
                t.iter()
                    .step_by(10)
                    .map(|v| v.sin() * v)
                    .collect::<Vec<_>>(),
                t.iter().step_by(10).copied().collect::<Vec<_>>(),
            )
            .size(7.0);
        }),
        ("molecule", |f| {
            const PDB: &str = "\
ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N
ATOM      2  CA  ALA A   1      11.639   6.071  -5.147  1.00  0.00           C
ATOM      3  C   ALA A   1      13.149   5.856  -5.162  1.00  0.00           C
ATOM      4  O   ALA A   1      13.691   5.111  -5.981  1.00  0.00           O
ATOM      5  CB  ALA A   1      11.329   7.371  -4.393  1.00  0.00           C
ATOM      6  N   GLY A   2      13.817   6.523  -4.236  1.00  0.00           N
ATOM      7  CA  GLY A   2      15.264   6.410  -4.153  1.00  0.00           C
END
";
            f.axes().title("Molecule").molecule(Molecule::from_pdb(PDB));
        }),
        ("high_contrast", |f| {
            f.theme(Theme::high_contrast());
            let x: Vec<f64> = (0..40).map(|i| i as f64 / 4.0).collect();
            let ax = f.axes();
            ax.title("High contrast");
            for k in 0..3 {
                ax.line(&x, x.iter().map(|v| (v + k as f64).sin()))
                    .label(format!("s{k}"));
            }
            ax.legend(LegendPosition::LowerLeft);
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
