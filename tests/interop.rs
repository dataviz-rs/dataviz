//! Data from other crates goes straight into the plotting API: anything
//! iterable over numbers, `Option`s (nulls become gaps) or booleans.

use dataviz::prelude::*;

#[test]
fn ndarray_arrays() {
    let x = ndarray::Array1::linspace(0.0, 10.0, 50);
    let y = x.mapv(f64::sin);
    let m = ndarray::Array2::<f64>::from_shape_fn((3, 4), |(r, c)| (r * 4 + c) as f64);
    let mut fig = Figure::new(400, 300);
    fig.grid(1, 2);
    fig.at(0, 0).line(&x, &y);
    fig.at(0, 0).scatter(x.clone(), y.view());
    // Rows of a 2D array for a heatmap.
    let rows: Vec<Vec<f64>> = m.rows().into_iter().map(|r| r.to_vec()).collect();
    fig.at(0, 1).heatmap(rows);
    let mut px = Pixels::default();
    fig.render_into(&mut px).unwrap();
}

#[test]
fn nullable_columns_become_gaps() {
    // What Arrow's `&Float64Array` and Polars' `&Float64Chunked` iterate.
    let y: Vec<Option<f64>> = vec![Some(1.0), None, Some(3.0)];
    let flags = [true, false, true];
    let mut fig = Figure::new(300, 200);
    fig.axes().line([0, 1, 2], &y);
    fig.axes().scatter([0, 1, 2], flags);
    let mut px = Pixels::default();
    fig.render_into(&mut px).unwrap();
    let v = dataviz::plot::series::IntoValues::into_values(&y);
    assert!(v[1].is_nan() && v[2] == 3.0);
}
