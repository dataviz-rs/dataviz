//! Renders the same real-time chart with every raster backend and prints
//! the frame time and the CPU time per frame.
//!
//! ```text
//! cargo run --release --example plot_backends              # CPU backends
//! cargo run --release --example plot_backends --features gpu
//! cargo run --release --example plot_backends -- 3840x2160 100000
//! ```

use dataviz::plot::series::line::Line;
use dataviz::prelude::*;
use std::time::{Duration, Instant};

/// CPU time of this process (user + system) on Linux; `None` elsewhere.
fn cpu_time() -> Option<Duration> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let fields: Vec<&str> = stat[stat.rfind(')')? + 2..].split_whitespace().collect();
    let ticks: u64 = fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
    Some(Duration::from_millis(ticks * 10))
}

fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let size = args.next().unwrap_or_else(|| "1920x1080".into());
    let n: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(100_000);
    let (w, h) = size
        .split_once('x')
        .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
        .unwrap_or((1920, 1080));

    let signal = |t: usize| (t as f64 * 0.03).sin() * 5.0 + ((t * 7919) % 1000) as f64 / 1000.0;
    let x: Vec<f64> = (0..n).map(|t| t as f64).collect();
    let y: Vec<f64> = (0..n).map(signal).collect();

    let mut backends = vec![
        ("CPU, 1 thread", Backend::Cpu { threads: 1 }),
        ("CPU, all cores", Backend::Cpu { threads: 0 }),
    ];
    if Backend::gpu_available() {
        backends.push(("GPU", Backend::Gpu));
    } else {
        println!("GPU backend unavailable (build with --features gpu on a machine with a GPU)");
    }

    println!("{w}×{h}, {n} points, one new point per frame:");
    for (name, backend) in backends {
        let mut fig = Figure::new(w, h);
        fig.backend(backend);
        fig.axes().line(&x, &y).label("signal");
        fig.axes()
            .title("real time")
            .legend(LegendPosition::UpperLeft);
        let mut frame = Vec::new();
        fig.render_0rgb(&mut frame)?; // warm-up (GPU initialization, caches)

        let frames = 100;
        let (cpu0, t0) = (cpu_time(), Instant::now());
        for t in n..n + frames {
            let line = fig.axes().series_mut::<Line>(0).expect("line series");
            line.push(t as f64, signal(t));
            line.keep_last(n);
            fig.render_0rgb(&mut frame)?;
        }
        let wall = t0.elapsed() / frames as u32;
        let cpu = cpu0.zip(cpu_time()).map(|(a, b)| (b - a) / frames as u32);
        match cpu {
            Some(cpu) => {
                println!("  {name:<15} {wall:>10.2?} per frame, {cpu:>10.2?} CPU per frame")
            }
            None => println!("  {name:<15} {wall:>10.2?} per frame"),
        }
    }
    Ok(())
}
