use dataviz::prelude::*;

fn main() -> Result<(), Error> {
    let out = std::env::args().nth(1).unwrap_or_else(|| "smoke".into());
    std::fs::create_dir_all(&out).unwrap();
    let mut fig = Figure::new(1200, 900);
    fig.title("dataviz plot API").grid(2, 2);

    let x: Vec<f64> = (0..200).map(|i| i as f64 / 20.0).collect();
    let ax = fig.at(0, 0);
    ax.title("Lines").x_label("t [s]").y_label("amplitude");
    ax.line(&x, x.iter().map(|v| v.sin())).label("sin");
    ax.line(&x, x.iter().map(|v| v.cos()))
        .label("cos")
        .dash(Dash::Dashed);
    ax.line(&x, x.iter().map(|v| (v * 0.5).sin() * 0.5))
        .label("slow")
        .dash(Dash::Dotted)
        .width(3.0);
    ax.scatter([1.0, 3.0, 5.0, 7.0], [0.5, -0.5, 0.8, -0.2])
        .label("events")
        .marker(Marker::Star)
        .size(12.0);

    let ax = fig.at(0, 1);
    ax.title("Grouped bars");
    ax.bars(
        ["Q1", "Q2", "Q3", "Q4"],
        [
            ("2023", vec![12.0, 15.0, 9.0, 18.0]),
            ("2024", vec![14.0, 13.0, 16.0, 21.0]),
        ],
        BarMode::Grouped,
        false,
    );
    for b in ax.last_series_bars() {
        b.value_labels(LabelPosition::Outside);
    }

    let ax = fig.at(1, 0);
    ax.title("Log scale + twin axis").y_axis().log();
    let t: Vec<f64> = (1..50).map(|i| i as f64).collect();
    ax.line(&t, t.iter().map(|v| v.powf(3.0)))
        .label("x³")
        .marker(Marker::Circle)
        .marker_size(4.0);
    ax.line(&t, t.iter().map(|v| (v * 0.3).sin()))
        .label("sin (right)")
        .secondary(true)
        .color("crimson");
    ax.y2_label("sin");

    let ax = fig.at(1, 1);
    ax.title("Stacked, time axis");
    let days: Vec<f64> = (0..12).map(|m| date(2024, m + 1, 1)).collect();
    ax.x_axis().time();
    ax.stacked_area(
        &days,
        [
            ("A", (0..12).map(|i| 5.0 + i as f64).collect::<Vec<_>>()),
            ("B", vec![3.0; 12]),
            ("C", (0..12).map(|i| (i as f64).sin() + 2.0).collect()),
        ],
        false,
    );
    ax.legend(LegendPosition::UpperLeft);

    fig.save(format!("{out}/smoke.svg"))?;
    fig.save(format!("{out}/smoke.png"))?;
    Ok(())
}
