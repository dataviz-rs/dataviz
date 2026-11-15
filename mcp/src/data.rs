//! Data loading: CSV/TSV and JSON tables, PDB structures.

use serde_json::Value;

/// A table: column names and the cells as text, row by row.
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn index(&self, name: &str) -> Result<usize, String> {
        self.columns
            .iter()
            .position(|c| c == name)
            .or_else(|| {
                self.columns
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case(name))
            })
            .ok_or_else(|| format!("no column `{name}` (columns: {})", self.columns.join(", ")))
    }
    pub fn text(&self, col: usize) -> Vec<String> {
        self.rows
            .iter()
            .map(|r| r.get(col).cloned().unwrap_or_default())
            .collect()
    }
    pub fn numbers(&self, col: usize) -> Vec<f64> {
        self.rows
            .iter()
            .map(|r| {
                r.get(col)
                    .and_then(|c| c.trim().parse().ok())
                    .unwrap_or(f64::NAN)
            })
            .collect()
    }
    /// Whether most non-empty cells of a column are numbers.
    pub fn numeric(&self, col: usize) -> bool {
        let cells: Vec<&String> = self
            .rows
            .iter()
            .filter_map(|r| r.get(col))
            .filter(|c| !c.trim().is_empty())
            .collect();
        !cells.is_empty()
            && cells
                .iter()
                .filter(|c| c.trim().parse::<f64>().is_ok())
                .count()
                * 10
                >= cells.len() * 9
    }
}

/// Reads a CSV, TSV or JSON file (array of objects, or `{column: [..]}`).
pub fn read_table(path: &str) -> Result<Table, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let lower = path.to_lowercase();
    if lower.ends_with(".json") || text.trim_start().starts_with(['[', '{']) {
        return json_table(&text);
    }
    let sep = if lower.ends_with(".tsv") || lower.ends_with(".tab") {
        '\t'
    } else {
        // The most frequent of , ; \t in the header.
        let head = text.lines().next().unwrap_or("");
        [',', ';', '\t']
            .into_iter()
            .max_by_key(|c| head.matches(*c).count())
            .unwrap_or(',')
    };
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let columns = split(lines.next().ok_or("empty file")?, sep);
    let rows: Vec<Vec<String>> = lines.map(|l| split(l, sep)).collect();
    Ok(Table { columns, rows })
}

/// One CSV line with double-quoted fields.
fn split(line: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => out.push(std::mem::take(&mut cur).trim().to_string()),
            c => cur.push(c),
        }
    }
    out.push(cur.trim().trim_end_matches('\r').to_string());
    out
}

fn cell(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn json_table(text: &str) -> Result<Table, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;
    match v {
        Value::Array(rows) => {
            let mut columns: Vec<String> = Vec::new();
            for r in &rows {
                if let Value::Object(m) = r {
                    for k in m.keys() {
                        if !columns.contains(k) {
                            columns.push(k.clone());
                        }
                    }
                }
            }
            let rows = rows
                .iter()
                .map(|r| {
                    columns
                        .iter()
                        .map(|c| r.get(c).map(cell).unwrap_or_default())
                        .collect()
                })
                .collect();
            Ok(Table { columns, rows })
        }
        Value::Object(m) => {
            let columns: Vec<String> = m.keys().cloned().collect();
            let n = m
                .values()
                .filter_map(Value::as_array)
                .map(Vec::len)
                .max()
                .unwrap_or(0);
            let rows = (0..n)
                .map(|i| {
                    columns
                        .iter()
                        .map(|c| m[c].get(i).map(cell).unwrap_or_default())
                        .collect()
                })
                .collect();
            Ok(Table { columns, rows })
        }
        _ => Err("JSON must be an array of objects or {column: [values]}".into()),
    }
}

/// PDB text from `pdb` (text), `pdb_path` (file) or `pdb_id` (downloaded
/// from the RCSB Protein Data Bank, needs `curl`).
pub fn pdb_text(p: &Value) -> Result<String, String> {
    if let Some(t) = p.get("pdb").and_then(Value::as_str) {
        return Ok(t.to_string());
    }
    if let Some(path) = p.get("pdb_path").and_then(Value::as_str) {
        return std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"));
    }
    if let Some(id) = p.get("pdb_id").and_then(Value::as_str) {
        let id = id.trim();
        if id.len() != 4 || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!("`{id}` is not a 4-character PDB id"));
        }
        let url = format!("https://files.rcsb.org/download/{}.pdb", id.to_uppercase());
        let out = std::process::Command::new("curl")
            .args(["-sSfL", "--max-time", "30", "-A", "dataviz-mcp", &url])
            .output()
            .map_err(|e| format!("downloading {url} needs curl: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "download of {url} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        return String::from_utf8(out.stdout).map_err(|_| "the PDB file is not text".to_string());
    }
    Err("give `pdb` (text), `pdb_path` or `pdb_id`".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_with_quotes() {
        assert_eq!(
            split(r#"a,"b, c","d ""q""""#, ','),
            vec!["a", "b, c", r#"d "q""#]
        );
    }

    #[test]
    fn json_tables() {
        let t = json_table(r#"[{"x": 1, "y": "a"}, {"x": 2}]"#).unwrap();
        assert_eq!(t.columns, vec!["x", "y"]);
        assert_eq!(t.numbers(0), vec![1.0, 2.0]);
        let t = json_table(r#"{"x": [1, 2, 3], "y": [4, 5, 6]}"#).unwrap();
        assert_eq!(t.rows.len(), 3);
        assert!(t.numeric(1));
    }
}
