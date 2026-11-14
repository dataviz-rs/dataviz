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
