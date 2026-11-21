//! The MCP tools: definitions (JSON Schema) and calls.

use crate::data::{self, Table};
use dataviz::plot::spec::{self, CHART_TYPES};
use dataviz::prelude::*;
use serde_json::{json, Value};

pub const INSTRUCTIONS: &str = "dataviz draws charts and returns them as PNG images you can look at. \
Use `plot` for data you have (a JSON spec of one or more panels; call `chart_types` for the 44 panel types and their fields), \
`plot_file` to chart a CSV/TSV/JSON file directly, and `molecule` for protein/DNA structures from the PDB. \
Look at the returned image and refine (labels, limits, chart type) when needed. \
Pass `output` with a .png/.svg/.pdf/.html path to save the chart for the user.";

fn common_props() -> Value {
    json!({
        "title": { "type": "string", "description": "Figure title." },
        "width": { "type": "number", "description": "Width in pixels (default 900)." },
        "height": { "type": "number", "description": "Height in pixels (default 600)." },
        "theme": { "type": "string", "enum": ["light", "dark", "minimal", "ggplot", "seaborn", "publication", "solarized", "high_contrast"], "description": "Color theme (default light). high_contrast uses dark colors with at least 4.5:1 contrast and larger text." },
        "output": { "type": "string", "description": "Also save the chart to this file path; the format follows the extension (.png, .svg, .pdf, or .html for an interactive page). An existing file at that path is overwritten. Omit to write nothing." },
        "image": { "type": "boolean", "description": "Return the image (default true; false returns only the text summary)." }
    })
}

fn merge(mut a: Value, b: Value) -> Value {
    if let (Some(a), Value::Object(b)) = (a.as_object_mut(), b) {
        a.extend(b);
    }
    a
}

pub fn list() -> Value {
    let types: Vec<&str> = CHART_TYPES.iter().map(|t| t.0).collect();
    json!([
        {
            "name": "plot",
            "title": "Plot data",
            "description": "Draw one or more charts from data passed inline in the call and return them as a PNG image plus a short text description of what was drawn.\n\nWhen to use: you already have the numbers (from the conversation, a computation or another tool). To chart a CSV/TSV/JSON file on disk use plot_file instead; for a 3D protein or DNA structure on its own, molecule is simpler (a molecule panel here is only needed to combine it with other charts).\n\nHow: `panels` is a list of charts laid out in a grid. Each panel has a `type` (line, scatter, bar, hist, box, violin, heatmap, pie, sankey, candlestick, roc, surface, scatter3d, … 44 in all) plus that type's data fields, for example {\"type\": \"line\", \"x\": [1, 2, 3], \"y\": [2, 4, 3]}. Call chart_types first if unsure of a type's fields. Options every panel takes: title, x_label, y_label, z_label, x_limits [min, max], y_limits, x_log, y_log, legend, camera [azimuth, elevation] and rotate {x, y, z} in degrees (3D), drawings [{type: trend|ray|fib, from [x, y], to [x, y]} or {type: level, y}].\n\nBehavior: draws in memory and writes nothing unless `output` is given. Molecule panels with a pdb_id download the structure from files.rcsb.org; nothing else uses the network. Width and height are clamped to 100-6000 px. An invalid spec (unknown type, missing or mismatched data fields) returns an error result whose text names the problem, and nothing is drawn or saved.",
            "inputSchema": {
                "type": "object",
                "properties": merge(common_props(), json!({
                    "panels": {
                        "type": "array",
                        "description": "Charts, one per panel, laid out in a grid (rows/cols automatic unless given). Each item needs `type` (or `mark` for the grammar of graphics) and that type's data fields.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": types },
                                "mark": { "type": "string", "description": "Grammar of graphics instead of `type` (with data and encoding; see chart_types)." }
                            },
                            "anyOf": [{ "required": ["type"] }, { "required": ["mark"] }],
                            "additionalProperties": true
                        }
                    },
                    "rows": { "type": "number", "description": "Rows of the panel grid (default: chosen from the number of panels)." },
                    "cols": { "type": "number", "description": "Columns of the panel grid (default: chosen from the number of panels)." }
                })),
                "required": ["panels"]
            },
            "annotations": { "title": "Plot data", "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": true }
        },
        {
            "name": "plot_file",
            "title": "Plot a data file",
            "description": "Chart the columns of a CSV, TSV or JSON file on disk and return a PNG image plus a text summary (columns found, what was drawn).\n\nWhen to use: the data is in a file the server can read, especially a large one you should not paste into a call. For data you already have in the conversation use plot, which also offers all 44 chart types; plot_file covers the common kinds below.\n\nHow: give `path`; JSON may be an array of objects or {column: [values]}. kind=auto (default) picks a line chart for a numeric x column, bars for a text x column and a histogram for a single numeric column; `correlation` draws the correlation matrix of all numeric columns. `x`, `y` and `group` choose columns by name.\n\nBehavior: only reads the file; writes nothing unless `output` is given, and uses no network. A missing file, an unknown column or a file with no rows returns an error result whose text says which.",
            "inputSchema": {
                "type": "object",
                "properties": merge(common_props(), json!({
                    "path": { "type": "string", "description": "Path of the CSV, TSV or JSON file, absolute or relative to the server's working directory; the format follows the extension." },
                    "kind": { "type": "string", "enum": ["auto", "line", "scatter", "bar", "hist", "box", "violin", "correlation"], "description": "Chart kind (default auto)." },
                    "x": { "type": "string", "description": "Column for x (categories for bar)." },
                    "y": { "description": "Column(s) for y: a name or a list of names (default: all numeric columns except x).", "anyOf": [{ "type": "string" }, { "type": "array", "items": { "type": "string" } }] },
                    "group": { "type": "string", "description": "Column with group names (box/violin: one box per group)." }
                })),
                "required": ["path"]
            },
            "annotations": { "title": "Plot a data file", "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": false }
        },
        {
            "name": "molecule",
            "title": "Draw a molecule",
            "description": "Draw a protein, DNA or small molecule in 3D and return a PNG image plus a text summary.\n\nWhen to use: to show one structure. To put a structure next to other charts, use a molecule panel in plot instead.\n\nHow: give exactly one source: pdb_id (a 4-character Protein Data Bank id such as 1UBQ), pdb_path (a local .pdb file) or pdb (the PDB text itself). The style defaults to cartoon for proteins and ball_and_stick otherwise; coloring defaults to element. rotate {x, y, z} turns the model by degrees; ramachandran adds a Ramachandran plot of the backbone angles.\n\nBehavior: pdb_id downloads the structure from files.rcsb.org (the only network access); pdb_path only reads the file; nothing is written unless `output` is given. An unknown id, an unreadable file or PDB data without ATOM/HETATM records returns an error result whose text says which.",
            "inputSchema": {
                "type": "object",
                "properties": merge(common_props(), json!({
                    "pdb_id": { "type": "string", "description": "4-character PDB id, e.g. 1UBQ." },
                    "pdb_path": { "type": "string", "description": "Path of a local .pdb file to read." },
                    "pdb": { "type": "string", "description": "PDB file content." },
                    "style": { "type": "string", "enum": ["cartoon", "ball_and_stick", "sticks", "space_filling", "backbone"], "description": "Drawing style (default cartoon for proteins, ball_and_stick otherwise)." },
                    "coloring": { "type": "string", "enum": ["element", "chain", "rainbow"], "description": "Atom colors: by element (default), by chain, or a rainbow along the chain." },
                    "rotate": { "type": "object", "description": "Rotation in degrees about each axis, applied x, then y, then z.", "properties": { "x": { "type": "number" }, "y": { "type": "number" }, "z": { "type": "number" } } },
                    "ramachandran": { "type": "boolean", "description": "Add a Ramachandran plot next to the structure." }
                }))
            },
            "annotations": { "title": "Draw a molecule", "readOnlyHint": false, "destructiveHint": true, "idempotentHint": true, "openWorldHint": true }
        },
        {
            "name": "chart_types",
            "title": "Chart types",
            "description": "List every panel type that plot accepts (44 types plus the grammar-of-graphics `mark`), each with its data fields and a one-line description, followed by a complete example call. Call it before plot when unsure which type fits the data or which fields a type needs. Takes no arguments, reads and writes nothing, and always returns the same text.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "title": "Chart types", "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false }
        }
    ])
}

pub fn call(name: &str, args: &Value) -> Value {
    let result = match name {
        "plot" => resolve_pdb_ids(args)
            .and_then(|a| spec::figure(&a))
            .and_then(|f| finish(f, args)),
        "plot_file" => plot_file(args).and_then(|f| finish(f, args)),
        "molecule" => molecule(args).and_then(|f| finish(f, args)),
        "chart_types" => Ok(vec![json!({ "type": "text", "text": chart_types() })]),
        _ => Err(format!("unknown tool `{name}`")),
    };
    match result {
        Ok(content) => json!({ "content": content }),
        Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
    }
}

/// Molecule panels with a `pdb_id`: downloaded here (the library does not
/// use the network).
fn resolve_pdb_ids(args: &Value) -> Result<Value, String> {
    let mut a = args.clone();
    let fix = |p: &mut Value| -> Result<(), String> {
        if p.get("pdb_id").is_some() && p.get("pdb").is_none() {
            let text = data::pdb_text(p)?;
            p["pdb"] = Value::String(text);
        }
        Ok(())
    };
    if let Some(panels) = a.get_mut("panels").and_then(Value::as_array_mut) {
        for p in panels {
            fix(p)?;
            if let Some(ls) = p.get_mut("layers").and_then(Value::as_array_mut) {
                for l in ls {
                    fix(l)?;
                }
            }
        }
    } else {
        fix(&mut a)?;
    }
    Ok(a)
}

fn chart_types() -> String {
    let mut s = String::from("Panel types of `plot` (fields; all panels also take title, x_label, y_label, z_label, x_limits, y_limits, x_log, y_log, legend, camera, rotate):\n\n");
    for (t, fields, what) in CHART_TYPES {
        s.push_str(&format!("- {t}: {fields} — {what}\n"));
    }
    let (fields, what) = spec::GRAMMAR;
    s.push_str(&format!("- mark (instead of type): {fields} — {what}\n"));
    s.push_str("\nmolecule panels also take pdb_id (downloaded from the PDB). Several chart types on one axes: {\"layers\": [{type, …}, …]} as a panel.");
    s.push_str("\nExample: {\"panels\": [{\"type\": \"line\", \"series\": [{\"name\": \"a\", \"x\": [1,2,3], \"y\": [2,4,3]}], \"title\": \"Sales\"}]}");
    s
}

/// Saves the figure when asked and returns the image and its summary.
fn finish(mut fig: Figure, args: &Value) -> Result<Vec<Value>, String> {
    let mut text = fig.alt_text();
    if let Some(path) = args.get("output").and_then(Value::as_str) {
        fig.save(path)
            .map_err(|e| format!("cannot save {path}: {e}"))?;
        text.push_str(&format!("\nSaved to {path}."));
    }
    let mut content = Vec::new();
    if args.get("image").and_then(Value::as_bool).unwrap_or(true) {
        let png = fig.encode(Format::Png).map_err(|e| e.to_string())?;
        content.push(json!({ "type": "image", "data": base64(&png), "mimeType": "image/png" }));
    }
    content.push(json!({ "type": "text", "text": text }));
    Ok(content)
}

fn size(fig_args: &Value) -> (u32, u32) {
    let g = |k: &str, d: f64| {
        fig_args
            .get(k)
            .and_then(Value::as_f64)
            .unwrap_or(d)
            .clamp(100.0, 6000.0) as u32
    };
    (g("width", 900.0), g("height", 600.0))
}

fn styled(fig: &mut Figure, args: &Value) {
    if let Some(t) = args.get("title").and_then(Value::as_str) {
        fig.title(t);
    }
    if let Some(t) = args.get("theme").and_then(Value::as_str) {
        fig.theme(spec::theme(t));
    }
}

fn plot_file(args: &Value) -> Result<Figure, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or("missing `path`")?;
    let t = data::read_table(path)?;
    if t.rows.is_empty() {
        return Err(format!("{path} has no rows"));
    }
    let (w, h) = size(args);
    let mut fig = Figure::new(w, h);
    styled(&mut fig, args);
    let kind = args.get("kind").and_then(Value::as_str).unwrap_or("auto");
    let x = args
        .get("x")
        .and_then(Value::as_str)
        .map(|c| t.index(c))
        .transpose()?;
    let ys: Vec<usize> = match args.get("y") {
        Some(Value::String(c)) => vec![t.index(c)?],
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .map(|c| t.index(c))
            .collect::<Result<_, _>>()?,
        _ => (0..t.columns.len())
            .filter(|&c| Some(c) != x && t.numeric(c))
            .collect(),
    };
    let ax = fig.axes();
    ax.title(
        std::path::Path::new(path)
            .file_name()
            .map_or(path.to_string(), |f| f.to_string_lossy().into_owned()),
    );
    if let Some(t) = args.get("title").and_then(Value::as_str) {
        ax.title(t);
    }
    match kind {
        "correlation" => correlation(ax, &t)?,
        "hist" => {
            for &c in &ys {
                ax.hist(t.numbers(c))
                    .label(t.columns[c].clone())
                    .opacity(if ys.len() > 1 { 0.6 } else { 1.0 });
            }
        }
        "box" | "violin" => {
            let groups: Vec<(String, Vec<f64>)> = match args.get("group").and_then(Value::as_str) {
                Some(g) => {
                    let (gc, vc) = (t.index(g)?, *ys.first().ok_or("no numeric column")?);
                    let (names, vals) = (t.text(gc), t.numbers(vc));
                    let mut out: Vec<(String, Vec<f64>)> = Vec::new();
                    for (n, v) in names.into_iter().zip(vals) {
                        match out.iter_mut().find(|o| o.0 == n) {
                            Some(o) => o.1.push(v),
                            None => out.push((n, vec![v])),
                        }
                    }
                    ax.y_label(t.columns[vc].clone());
                    out
                }
                None => ys
                    .iter()
                    .map(|&c| (t.columns[c].clone(), t.numbers(c)))
                    .collect(),
            };
            if kind == "box" {
                ax.boxplot(groups);
            } else {
                ax.violin(groups);
            }
        }
        _ => {
            let x = match x {
                Some(c) => c,
                None if ys.len() == 1 && kind == "auto" => {
                    ax.hist(t.numbers(ys[0])).label(t.columns[ys[0]].clone());
                    ax.x_label(t.columns[ys[0]].clone());
                    return Ok(fig);
                }
                // The first column as x.
                None => 0,
            };
            let ys: Vec<usize> = ys.into_iter().filter(|&c| c != x).collect();
            if ys.is_empty() {
                return Err("no numeric y column".into());
            }
            ax.x_label(t.columns[x].clone());
            if ys.len() == 1 {
                ax.y_label(t.columns[ys[0]].clone());
            }
            let text_x = !t.numeric(x);
            let dates: Option<Vec<f64>> = if text_x {
                let d: Vec<f64> = t
                    .text(x)
                    .iter()
                    .map(|s| parse_datetime(s).unwrap_or(f64::NAN))
                    .collect();
                (d.iter().filter(|v| v.is_finite()).count() * 10 >= d.len() * 9).then_some(d)
            } else {
                None
            };
            let kind = match kind {
                "auto" if text_x && dates.is_none() => "bar",
                "auto" => {
                    let xs = t.numbers(x);
                    if dates.is_some()
                        || xs
                            .windows(2)
                            .all(|w| w[1] >= w[0] || w[0].is_nan() || w[1].is_nan())
                    {
                        "line"
                    } else {
                        "scatter"
                    }
                }
                k => k,
            };
            for &c in &ys {
                let name = t.columns[c].clone();
                match kind {
                    "bar" => {
                        ax.bar(t.text(x), t.numbers(c)).label(name);
                    }
                    "scatter" => {
                        ax.scatter(t.numbers(x), t.numbers(c)).label(name).size(5.0);
                    }
                    _ => {
                        let xs = dates.clone().unwrap_or_else(|| t.numbers(x));
                        ax.line(xs, t.numbers(c)).label(name);
                    }
                }
            }
            if dates.is_some() {
                ax.x_scale(Scale::Time);
            }
        }
    }
    Ok(fig)
}

fn correlation(ax: &mut Axes, t: &Table) -> Result<(), String> {
    let cols: Vec<usize> = (0..t.columns.len()).filter(|&c| t.numeric(c)).collect();
    if cols.len() < 2 {
        return Err("correlation needs at least two numeric columns".into());
    }
    let data: Vec<Vec<f64>> = cols.iter().map(|&c| t.numbers(c)).collect();
    let corr = |a: &[f64], b: &[f64]| {
        let pairs: Vec<(f64, f64)> = a
            .iter()
            .zip(b)
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .map(|(x, y)| (*x, *y))
            .collect();
        let n = pairs.len() as f64;
        let (mx, my) = (
            pairs.iter().map(|p| p.0).sum::<f64>() / n,
            pairs.iter().map(|p| p.1).sum::<f64>() / n,
        );
        let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
        for (x, y) in &pairs {
            sxy += (x - mx) * (y - my);
            sxx += (x - mx) * (x - mx);
            syy += (y - my) * (y - my);
        }
        sxy / (sxx * syy).sqrt()
    };
    let m: Vec<Vec<f64>> = data
        .iter()
        .map(|a| data.iter().map(|b| corr(a, b)).collect())
        .collect();
    let names: Vec<String> = cols.iter().map(|&c| t.columns[c].clone()).collect();
    ax.heatmap_labeled(names.clone(), names, m)
        .colormap(Colormap::RdBu)
        .range(-1.0, 1.0)
        .annotate(true)
        .colorbar("correlation");
    Ok(())
}

fn molecule(args: &Value) -> Result<Figure, String> {
    let text = data::pdb_text(args)?;
    let mol = Molecule::from_pdb(&text);
    if mol.atoms.is_empty() {
        return Err("no ATOM/HETATM records in the PDB data".into());
    }
    let rama = args
        .get("ramachandran")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let (w, h) = size(args);
    let mut fig = Figure::new(
        if rama && args.get("width").is_none() {
            1300
        } else {
            w
        },
        h,
    );
    styled(&mut fig, args);
    let protein = mol.atoms.iter().any(|a| a.name == "CA");
    let default_style = if protein { "cartoon" } else { "ball_and_stick" };
    let ax = if rama {
        fig.grid(1, 2);
        fig.at(0, 0)
    } else {
        fig.axes()
    };
    let m = ax.molecule(mol.clone());
    m.style(spec::mol_style(
        args.get("style")
            .and_then(Value::as_str)
            .unwrap_or(default_style),
    ));
    m.coloring(spec::mol_color(
        args.get("coloring")
            .and_then(Value::as_str)
            .unwrap_or("element"),
    ));
    if let Some(r) = args.get("rotate") {
        for (k, axis) in [("x", Axis3::X), ("y", Axis3::Y), ("z", Axis3::Z)] {
            if let Some(d) = r.get(k).and_then(Value::as_f64) {
                ax.rotate(axis, d);
            }
        }
    }
    if let Some(id) = args.get("pdb_id").and_then(Value::as_str) {
        ax.title(id.to_uppercase());
    }
    if rama {
        let a = mol.phi_psi();
        let (phi, psi): (Vec<f64>, Vec<f64>) = a.iter().map(|x| (x.0, x.1)).unzip();
        fig.at(0, 1)
            .title("Ramachandran plot")
            .ramachandran(phi, psi);
    }
    Ok(fig)
}

/// Standard base64 (RFC 4648) with padding.
pub fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            A[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            A[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_documented() {
        for t in list().as_array().unwrap() {
            let name = t["name"].as_str().unwrap();
            assert!(t["description"].as_str().unwrap().len() > 100, "{name}");
            assert!(t["annotations"]["readOnlyHint"].is_boolean(), "{name}");
            for (k, v) in t["inputSchema"]["properties"].as_object().unwrap() {
                assert!(v["description"].is_string(), "{name}.{k} has no description");
            }
        }
    }

    #[test]
    fn base64_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn plot_returns_an_image() {
        let r = call(
            "plot",
            &json!({ "panels": [
                { "type": "line", "series": [{ "name": "a", "x": [1, 2, 3], "y": [2, 4, 3] }], "title": "t" },
                { "type": "bar", "categories": ["x", "y"], "values": [3, 5] },
                { "type": "box", "groups": { "a": [1, 2, 3, 4], "b": [2, 3, 5, 8] } },
                { "type": "dna_helix", "sequence": "ATGCGT", "rotate": { "x": 20 } }
            ]}),
        );
        assert!(r.get("isError").is_none(), "{r}");
        let c = r["content"].as_array().unwrap();
        assert_eq!(c[0]["type"], "image");
        assert!(c[0]["data"].as_str().unwrap().starts_with("iVBOR"));
        assert!(c[1]["text"].as_str().unwrap().len() > 10);
    }

    #[test]
    fn errors_are_tool_errors() {
        let r = call("plot", &json!({ "panels": [{ "type": "nope" }] }));
        assert_eq!(r["isError"], true);
        assert!(r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown type"));
        let r = call("plot", &json!({ "panels": [{ "type": "line" }] }));
        assert!(r["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("missing `y`"));
    }

    #[test]
    fn every_chart_type_draws() {
        let p = |v: Value| call("plot", &json!({ "panels": [v] }));
        let cases = vec![
            json!({"type": "scatter", "x": [1, 2], "y": [3, 4]}),
            json!({"type": "area", "y": [1, 3, 2]}),
            json!({"type": "barh", "categories": ["a"], "values": [1]}),
            json!({"type": "bar", "x": ["a", "b"], "y": [1, 2]}),
            json!({"type": "pie", "categories": ["a", "b"], "y": [1, 2]}),
            json!({"type": "hist", "values": [1, 2, 2, 3], "bins": 3}),
            json!({"type": "kde", "values": [1, 2, 2, 3]}),
            json!({"type": "violin", "groups": [["a", [1, 2, 3]]]}),
            json!({"type": "swarm", "groups": {"a": [1, 2, 3]}}),
            json!({"type": "raincloud", "groups": {"a": [1, 2, 3, 4]}}),
            json!({"type": "ridgeline", "groups": {"a": [1, 2, 3], "b": [2, 3, 4]}}),
            json!({"type": "regression", "x": [1, 2, 3], "y": [2, 4, 7]}),
            json!({"type": "heatmap", "z": [[1, 2], [3, 4]], "annotate": true}),
            json!({"type": "contour", "z": [[1, 2, 3], [2, 3, 4], [3, 4, 5]]}),
            json!({"type": "donut", "labels": ["a", "b"], "values": [1, 2]}),
            json!({"type": "funnel", "labels": ["a", "b"], "values": [3, 1]}),
            json!({"type": "waffle", "labels": ["a", "b"], "values": [60, 40]}),
            json!({"type": "sankey", "links": [["a", "b", 3], ["a", "c", 1]]}),
            json!({"type": "chord", "labels": ["a", "b"], "matrix": [[0, 2], [1, 0]]}),
            json!({"type": "streamgraph", "x": [0, 1, 2], "layers": {"a": [1, 2, 1], "b": [2, 1, 2]}}),
            json!({"type": "bullet", "labels": ["k"], "values": [5], "targets": [6], "ranges": [[3, 6, 9]]}),
            json!({"type": "dumbbell", "labels": ["a"], "start": [1], "end": [2]}),
            json!({"type": "slope", "labels": ["a", "b"], "before": [1, 2], "after": [2, 1]}),
            json!({"type": "pareto", "labels": ["a", "b"], "values": [5, 2]}),
            json!({"type": "control_chart", "values": [1, 2, 1.5, 9, 1.2]}),
            json!({"type": "candlestick", "dates": ["2024-01-01", "2024-01-02"], "open": [1, 2], "high": [3, 3], "low": [0, 1], "close": [2, 1]}),
            json!({"type": "roc", "labels": [true, false, true, false], "scores": [0.9, 0.2, 0.7, 0.4]}),
            json!({"type": "calibration", "labels": [1, 0, 1, 0], "scores": [0.9, 0.2, 0.7, 0.4]}),
            json!({"type": "confusion", "labels": ["a", "b"], "actual": ["a", "b", "a"], "predicted": ["a", "a", "a"]}),
            json!({"type": "upset", "sets": {"A": ["1", "2"], "B": ["2", "3"]}}),
            json!({"type": "volcano", "names": ["g1", "g2"], "log2fc": [2, -1], "pvalues": [0.001, 0.5]}),
            json!({"type": "phylo_tree", "newick": "((A:1,B:2):1,C:3);", "circular": true}),
            json!({"type": "choropleth", "values": {"DEU": 1, "TUR": 2}}),
            json!({"type": "surface", "z": [[1, 2], [3, 4]], "camera": [-40, 20]}),
            json!({"type": "scatter3d", "x": [1, 2], "y": [1, 2], "z": [1, 2], "values": [1, 2]}),
            json!({"type": "line3d", "x": [1, 2], "y": [1, 2], "z": [1, 2]}),
            json!({"type": "molecule", "pdb": "ATOM      1  CA  ALA A   1      11.639   6.071  -5.147  1.00  0.00           C\nATOM      2  CA  GLY A   2      15.264   6.410  -4.153  1.00  0.00           C\n", "style": "backbone"}),
        ];
        for c in cases {
            let r = p(c.clone());
            assert!(r.get("isError").is_none(), "{c} -> {r}");
        }
    }

    #[test]
    fn plot_file_kinds() {
        let dir = std::env::temp_dir().join("dataviz-mcp-test");
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("d.csv");
        std::fs::write(
            &csv,
            "day,sales,cost\n2024-01-01,10,4\n2024-01-02,12,5\n2024-01-03,9,6\n",
        )
        .unwrap();
        for kind in [
            "auto",
            "scatter",
            "bar",
            "hist",
            "box",
            "violin",
            "correlation",
        ] {
            let r = call(
                "plot_file",
                &json!({ "path": csv, "kind": kind, "image": false }),
            );
            assert!(r.get("isError").is_none(), "{kind}: {r}");
        }
        let svg = dir.join("out.svg");
        let r = call(
            "plot_file",
            &json!({ "path": csv, "x": "day", "y": ["sales"], "output": svg, "image": false }),
        );
        assert!(
            r["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Saved to"),
            "{r}"
        );
        assert!(std::fs::metadata(&svg).unwrap().len() > 100);
    }
}
