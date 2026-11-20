//! Deterministic fuzzing of the readers and the JSON spec: seed inputs are
//! mutated (bytes flipped, inserted, deleted, duplicated; numbers replaced
//! by extreme values; truncation) and every reader must return normally —
//! never panic or hang. Runs on stable Rust in every `cargo test`; set
//! `DATAVIZ_FUZZ=<iterations>` for a longer run. A failure prints the
//! input that broke the reader.

use dataviz::plot::data::Table;
use dataviz::plot::geo::read_geojson;
use dataviz::plot::series::genome::parse_newick;
use dataviz::prelude::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// xorshift64*: small, fast, reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const TOKENS: &[&str] = &[
    "NaN",
    "inf",
    "-inf",
    "1e308",
    "-1e308",
    "1e-320",
    "0",
    "-0",
    "99999999999999999999",
    "-1",
    "(",
    ")",
    ",",
    ":",
    ";",
    "\"",
    "\n",
    "\r\n",
    "\t",
    "{",
    "}",
    "[",
    "]",
    "null",
    "ATOM  ",
    "HETATM",
    "END",
    "é",
    "日本",
    "\u{feff}",
    "\0",
    "  ",
    ",,",
    "::",
    "((",
    "))",
];

fn mutate(rng: &mut Rng, seed: &[u8]) -> String {
    let mut b = seed.to_vec();
    for _ in 0..1 + rng.below(6) {
        let n = b.len();
        match rng.below(7) {
            0 if n > 0 => {
                let i = rng.below(n);
                b[i] = rng.next() as u8;
            }
            1 => {
                let i = rng.below(n + 1);
                b.insert(i, rng.next() as u8);
            }
            2 if n > 0 => {
                let i = rng.below(n);
                let len = rng.below((n - i).min(40) + 1);
                b.drain(i..i + len);
            }
            3 if n > 0 => {
                let i = rng.below(n);
                let len = rng.below((n - i).min(60) + 1);
                let chunk = b[i..i + len].to_vec();
                let at = rng.below(n + 1);
                b.splice(at..at, chunk);
            }
            4 => {
                let t = TOKENS[rng.below(TOKENS.len())].as_bytes();
                let i = rng.below(n + 1);
                b.splice(i..i, t.iter().copied());
            }
            5 if n > 0 => b.truncate(rng.below(n)),
            _ => {
                // Replace a run of digits by an extreme token.
                if let Some(i) = (0..n)
                    .map(|k| (k + rng.below(n)) % n)
                    .find(|&k| b[k].is_ascii_digit())
                {
                    let mut j = i;
                    while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
                        j += 1;
                    }
                    let t = TOKENS[rng.below(9)].as_bytes();
                    b.splice(i..j, t.iter().copied());
                }
            }
        }
    }
    String::from_utf8_lossy(&b).into_owned()
}

fn iterations(default: usize) -> usize {
    std::env::var("DATAVIZ_FUZZ")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Runs `f` on mutations of the seeds; panics (with the input) when `f`
/// panics.
fn fuzz(name: &str, seeds: &[&str], n: usize, f: impl Fn(&str)) {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ name.len() as u64);
    for k in 0..iterations(n) {
        let seed = seeds[k % seeds.len()].as_bytes();
        let input = mutate(&mut rng, seed);
        if catch_unwind(AssertUnwindSafe(|| f(&input))).is_err() {
            panic!("{name}: reader panicked on input {input:?}");
        }
    }
}

const PDB: &str = "\
HEADER    DNA                                     01-JAN-00   1ABC
HELIX    1   1 ALA A    1  GLY A    2  1
ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N
ATOM      2  CA  ALA A   1      11.639   6.071  -5.147  1.00  0.00           C
ATOM      3  C   ALA A   1      13.149   5.856  -5.162  1.00  0.00           C
ATOM      4  O   ALA A   1      13.691   5.111  -5.981  1.00  0.00           O
ATOM      5  N   GLY A   2      13.817   6.523  -4.236  1.00  0.00           N
ATOM      6  CA  GLY A   2      15.264   6.410  -4.153  1.00  0.00           C
HETATM    7  O   HOH A 101      20.000  20.000  20.000  1.00  0.00           O
CONECT    1    2
END
";

const XYZ: &str = "3\nwater\nO 0.0 0.0 0.1\nH 0.0 0.75 -0.47\nH 0.0 -0.75 -0.47\n";

#[test]
fn fuzz_pdb_and_xyz_readers() {
    fuzz("pdb", &[PDB], 3000, |s| {
        let m = Molecule::from_pdb(s);
        if !m.atoms.is_empty() && s.len() < 3000 {
            // Bonds, secondary structure and drawing must cope as well.
            let mut fig = Figure::new(120, 100);
            fig.axes().molecule(m).style(MolStyle::Cartoon);
            let _ = fig.to_pixels();
        }
    });
    fuzz("xyz", &[XYZ], 2000, |s| {
        let _ = Molecule::from_xyz(s);
    });
}

#[test]
fn fuzz_newick_reader() {
    let seeds = [
        "((A:0.1,B:0.2)90:0.3,(C:0.3,D:0.4):0.5);",
        "(((a,b),(c,d)),e,'quoted name':1e-3);",
        "(A,B,(C,D)E)F;",
    ];
    fuzz("newick", &seeds, 4000, |s| {
        if let Some(_tree) = parse_newick(s) {
            let mut fig = Figure::new(120, 100);
            if fig.axes().phylo_tree(s).is_some() {
                let _ = fig.to_svg();
            }
        }
    });
}

#[test]
fn fuzz_csv_and_geojson_readers() {
    let csv = "date,price,\"name, quoted\"\n2024-01-02,10.5,\"a \"\"b\"\"\"\n2024-01-03,,c\n";
    fuzz("csv", &[csv, "a;b\n1;2\n", "x\ty\n1\t2\n"], 3000, |s| {
        if let Some(t) = Table::parse(s) {
            for h in t.headers.clone() {
                let _ = t.column(&h);
                let _ = t.text_column(&h);
                let _ = t.time_column(&h);
            }
        }
    });
    let geo = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"A"},
        "geometry":{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,0]]]}},
        {"type":"Feature","properties":{"name":"B"},"geometry":{"type":"MultiPolygon",
        "coordinates":[[[[20,20],[30,20],[30,30],[20,20]]]]}}]}"#;
    fuzz("geojson", &[geo], 3000, |s| {
        if let Ok(f) = read_geojson(s) {
            let mut fig = Figure::new(120, 100);
            fig.axes().geo_shapes(&f, Vec::<f64>::new(), "name");
            let _ = fig.to_pixels();
        }
    });
}

#[test]
fn fuzz_color_parser() {
    fuzz(
        "color",
        &[
            "#1f77b4",
            "rgb(10, 20, 30)",
            "rgba(1,2,3,0.5)",
            "crimson",
            "#abc",
        ],
        4000,
        |s| {
            let _ = Color::parse(s);
        },
    );
}

#[cfg(feature = "spec")]
#[test]
fn fuzz_json_specs() {
    // Every chart type with its documented example-like fields, then
    // mutated: wrong types, empty or huge arrays, extreme numbers.
    let seeds = [
        r#"{"type":"line","x":[1,2,3],"y":[3,1,2],"label":"a"}"#,
        r#"{"type":"bar","categories":["a","b"],"values":[1,2]}"#,
        r#"{"type":"hist","values":[1,2,2,3,1e9]}"#,
        r#"{"type":"pie","labels":["a","b"],"values":[1,0]}"#,
        r#"{"type":"heatmap","z":[[1,2],[3,4]]}"#,
        r#"{"type":"sankey","links":[["a","b",3],["b","c",1]]}"#,
        r#"{"type":"surface","z":[[0,1],[1,0]],"x_range":[0,1],"y_range":[0,1]}"#,
        r#"{"type":"density_map","x":[1,2,3,2,1e9],"y":[3,1,2,2,-5],"how":"log"}"#,
        r#"{"type":"volume","values":[[[0,1],[1,2]],[[1,2],[2,3]]]}"#,
        r#"{"type":"arc_diagram","nodes":["a","b","c"],"edges":[["a","c",2],[0,1]]}"#,
        r#"{"type":"edge_bundling","nodes":["a","b","c"],"groups":[0,0,1],"edges":[[0,2]]}"#,
        r#"{"type":"choropleth","values":{"TUR":1,"BRA":2},"projection":"orthographic"}"#,
        r#"{"panels":[{"type":"scatter","x":[1,2],"y":[2,1]},{"type":"box","groups":{"a":[1,2,3]}}]}"#,
        r#"{"data":[{"g":"a","x":1,"y":2},{"g":"b","x":2,"y":3}],"mark":"bar","encoding":{"x":"g","y":{"field":"y","aggregate":"mean"},"color":"g","facet":"g"}}"#,
    ];
    fuzz("spec", &seeds, 1500, |s| {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
            if let Ok(mut fig) = dataviz::plot::spec::figure(&v) {
                fig.resize(160, 120);
                let _ = fig.to_pixels();
            }
        }
    });
}
