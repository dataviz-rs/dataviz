//! Loading tabular data (CSV/TSV) for plotting.

use super::time;
use std::path::Path;

/// A table of text cells with a header row.
///
/// ```
/// use dataviz::plot::data::Table;
/// let t = Table::parse("date,temp\n2024-01-01,3.5\n2024-01-02,\"4,1\"\n").unwrap();
/// assert_eq!(t.column("temp").unwrap(), vec![3.5, 4.1]);
/// assert_eq!(t.time_column("date").unwrap().len(), 2);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    /// Column names.
    pub headers: Vec<String>,
    /// Rows of cells.
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// Reads a CSV/TSV file (delimiter detected from the header line).
    pub fn read(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::parse(&text)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "empty table"))
    }

    /// Parses CSV/TSV text. Supports quoted fields with embedded delimiters,
    /// quotes (`""`) and newlines; the delimiter is the most frequent of
    /// `,` `;` `\t` `|` in the first line.
    pub fn parse(text: &str) -> Option<Self> {
        let first = text.lines().next()?;
        let delim = [',', ';', '\t', '|']
            .into_iter()
            .max_by_key(|d| first.matches(*d).count())
            .unwrap_or(',');
        let mut rows = parse_records(text.trim_start_matches('\u{feff}'), delim);
        rows.retain(|r| !(r.len() == 1 && r[0].trim().is_empty()));
        if rows.is_empty() {
            return None;
        }
        let headers = rows
            .remove(0)
            .into_iter()
            .map(|h| h.trim().to_string())
            .collect();
        Some(Self { headers, rows })
    }

    fn index(&self, name: &str) -> Option<usize> {
        self.headers.iter().position(|h| h == name).or_else(|| {
            self.headers
                .iter()
                .position(|h| h.eq_ignore_ascii_case(name))
        })
    }

    /// Text cells of a column.
    pub fn text_column(&self, name: &str) -> Option<Vec<String>> {
        let i = self.index(name)?;
        Some(
            self.rows
                .iter()
                .map(|r| r.get(i).map(|c| c.trim().to_string()).unwrap_or_default())
                .collect(),
        )
    }

    /// Numeric column; unparsable cells become NaN. Decimal commas are
    /// accepted when the delimiter is not a comma (`"4,1"` → 4.1).
    pub fn column(&self, name: &str) -> Option<Vec<f64>> {
        Some(
            self.text_column(name)?
                .iter()
                .map(|c| parse_number(c))
                .collect(),
        )
    }

    /// Date/time column as seconds since the Unix epoch (see
    /// [`time::parse_datetime`]); unparsable cells become NaN.
    pub fn time_column(&self, name: &str) -> Option<Vec<f64>> {
        Some(
            self.text_column(name)?
                .iter()
                .map(|c| time::parse_datetime(c).unwrap_or(f64::NAN))
                .collect(),
        )
    }

    /// Number of data rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether there are no data rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

fn parse_number(c: &str) -> f64 {
    let c = c.trim().trim_end_matches('%');
    c.parse::<f64>()
        .or_else(|_| c.replace(',', ".").parse::<f64>())
        .or_else(|_| c.replace(['_', ' '], "").parse::<f64>())
        .unwrap_or(f64::NAN)
}

fn parse_records(text: &str, delim: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field.trim().is_empty() {
            field.clear();
            quoted = true;
        } else if c == delim {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' {
            row.push(
                std::mem::take(&mut field)
                    .trim_end_matches('\r')
                    .to_string(),
            );
            rows.push(std::mem::take(&mut row));
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field.trim_end_matches('\r').to_string());
        rows.push(row);
    }
    rows
}

/// Records live data to a CSV file as it arrives: buffered, written out when
/// a row arrives at least a second after the previous write, on
/// [`CsvLog::flush`] and when dropped; appends to an existing file (the
/// header is written only to a new or empty file). Call `flush` regularly
/// when rows are sparse or the log lives in a thread that may be stopped
/// without being dropped. Timestamps are written
/// as `YYYY-MM-DD HH:MM:SS.mmm`, which [`Table::time_column`] reads back.
///
/// ```
/// use dataviz::plot::data::{CsvLog, Table};
/// use dataviz::plot::time::datetime;
/// let path = std::env::temp_dir().join("dataviz_csvlog_doc.csv");
/// let _ = std::fs::remove_file(&path);
/// {
///     let mut log = CsvLog::create(&path, &["time", "price", "qty"]).unwrap();
///     log.record(datetime(2024, 5, 1, 12, 0, 1.25), &[65000.5, 0.01]).unwrap();
///     log.row(["2024-05-01 12:00:02", "65001", "0.5"]).unwrap();
/// } // flushed on drop
/// let t = Table::read(&path).unwrap();
/// assert_eq!(t.column("price").unwrap(), vec![65000.5, 65001.0]);
/// assert_eq!(t.time_column("time").unwrap()[0], datetime(2024, 5, 1, 12, 0, 1.25));
/// ```
pub struct CsvLog {
    out: std::io::BufWriter<std::fs::File>,
    path: std::path::PathBuf,
    rows: u64,
    last_flush: std::time::Instant,
}

impl CsvLog {
    /// Opens `path` for appending (created if missing) with column names
    /// `header`.
    pub fn create(path: impl AsRef<Path>, header: &[&str]) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let empty = file.metadata()?.len() == 0;
        let mut log = CsvLog {
            out: std::io::BufWriter::new(file),
            path,
            rows: 0,
            last_flush: std::time::Instant::now(),
        };
        if empty {
            log.row(header.iter())?;
            log.rows = 0;
        }
        Ok(log)
    }

    /// Writes a row: the time `t` (seconds since 1970) and `values` (NaN as
    /// an empty cell).
    pub fn record(&mut self, t: f64, values: &[f64]) -> std::io::Result<()> {
        use std::io::Write;
        write!(self.out, "{}", format_timestamp(t))?;
        for v in values {
            if v.is_finite() {
                write!(self.out, ",{v}")?;
            } else {
                self.out.write_all(b",")?;
            }
        }
        self.end_row()
    }

    /// Writes a row of arbitrary fields (quoted when they contain a comma,
    /// quote or line break).
    pub fn row<T: std::fmt::Display>(
        &mut self,
        fields: impl IntoIterator<Item = T>,
    ) -> std::io::Result<()> {
        use std::io::Write;
        for (i, f) in fields.into_iter().enumerate() {
            if i > 0 {
                self.out.write_all(b",")?;
            }
            let f = f.to_string();
            if f.contains([',', '"', '\n', '\r']) {
                write!(self.out, "\"{}\"", f.replace('"', "\"\""))?;
            } else {
                self.out.write_all(f.as_bytes())?;
            }
        }
        self.end_row()
    }

    fn end_row(&mut self) -> std::io::Result<()> {
        use std::io::Write;
        self.out.write_all(b"\n")?;
        self.rows += 1;
        if self.last_flush.elapsed() >= std::time::Duration::from_secs(1) {
            self.flush()?;
        }
        Ok(())
    }

    /// Writes buffered rows to the file.
    pub fn flush(&mut self) -> std::io::Result<()> {
        use std::io::Write;
        self.last_flush = std::time::Instant::now();
        self.out.flush()
    }

    /// Rows written since the log was opened.
    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// The file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CsvLog {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

/// `YYYY-MM-DD HH:MM:SS.mmm` for a timestamp in seconds since 1970.
pub fn format_timestamp(t: f64) -> String {
    if !t.is_finite() {
        return String::new();
    }
    let ms = ((t * 1000.0).round() as i64).rem_euclid(1000);
    let whole = (t * 1000.0).round().div_euclid(1000.0);
    format!("{}.{ms:03}", time::format_time(whole, "%Y-%m-%d %H:%M:%S"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quotes_and_delimiters() {
        let t = Table::parse("a;b;c\r\n1;\"x;y\";3,5\r\n2;\"he said \"\"hi\"\"\";4\r\n").unwrap();
        assert_eq!(t.headers, ["a", "b", "c"]);
        assert_eq!(t.text_column("b").unwrap(), ["x;y", "he said \"hi\""]);
        assert_eq!(t.column("c").unwrap(), vec![3.5, 4.0]);
        assert_eq!(t.column("A").unwrap(), vec![1.0, 2.0]);
        assert!(t.column("missing").is_none());
        let tsv = Table::parse("x\ty\n1\t2\n3\tnope\n").unwrap();
        assert!(tsv.column("y").unwrap()[1].is_nan());
        assert!(Table::parse("").is_none());
    }
}
