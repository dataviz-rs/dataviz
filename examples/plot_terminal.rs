//! Prints a chart in the terminal.
use dataviz::plot::terminal::TerminalMode;
use dataviz::prelude::*;

fn main() -> Result<(), Error> {
    let mut fig = Figure::new(800, 400);
    let ax = fig.axes();
    ax.title("Terminal output");
    let x: Vec<f64> = (0..200).map(|i| i as f64 / 20.0).collect();
    ax.line(&x, x.iter().map(|v| v.sin())).label("sin");
    ax.line(&x, x.iter().map(|v| v.cos())).label("cos");
    print!("{}", fig.to_terminal(100, 25, TerminalMode::HalfBlock)?);
    print!("{}", fig.to_terminal(100, 25, TerminalMode::BrailleMono)?);
    Ok(())
}
