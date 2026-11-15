# Your data

Every chart takes values through `IntoValues`: anything iterable over
numbers — `Vec`, slices, arrays, ranges, iterators — of any numeric type,
`Option`s (`None` becomes a gap) and booleans (1/0).

```rust,ignore
ax.line(&x, &y);                                    // Vec<f64>, &[f32], i64, …
ax.line(0..100, data.iter().map(|d| d.price));      // ranges and iterators
ax.line(&array1, &array1.mapv(f64::sin));           // ndarray
ax.line(&times, df.column("price")?.f64()?);        // Polars (nulls → gaps)
ax.scatter(arrow_x.values(), &arrow_y);             // Arrow arrays
```

Data passed to a series is moved, not copied.

## CSV files

```rust,ignore
use dataviz::plot::data::Table;
let t = Table::read("prices.csv")?;                 // , ; tab or | detected
ax.line(t.time_column("date").unwrap(), t.column("close").unwrap());
ax.x_scale(Scale::Time);
```

## Dates and times

Time values are seconds since 1970 (UTC): `date(2025, 3, 1)`,
`datetime(2025, 3, 1, 9, 30, 0)` or `parse_datetime("2025-03-01T09:30")`;
`ax.x_scale(Scale::Time)` picks readable ticks from seconds to decades.

## Live data

`Line::push`, `keep_last` and `keep_since` stream into a chart;
`plot::data::CsvLog` records the stream to a CSV file.
