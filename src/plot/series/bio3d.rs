//! 3D charts for biology: DNA double helix, molecules (PDB / XYZ) and 3D
//! cell embeddings. They draw into the shared 3D scene of the axes (see
//! [`three_d`](super::three_d)), so they turn with the mouse in a window.

use super::three_d::{Bounds3, Projection3d, Scene3d};
use super::{Context, Extent, Hit, IntoLabels, IntoValues, LegendEntry, LegendSymbol, Series};
use crate::plot::axes::{Axes, AxesKind};
use crate::plot::color::{Color, Colormap, Palette};
use crate::plot::raster::HashMap;
use std::any::Any;
use std::ops::Range;

/// Base colors (A green, T red, G orange, C blue, U as T).
fn base_color(b: u8) -> Color {
    match b.to_ascii_uppercase() {
        b'A' => Color::rgb(46, 170, 98),
        b'T' | b'U' => Color::rgb(222, 72, 72),
        b'G' => Color::rgb(245, 160, 40),
        b'C' => Color::rgb(56, 132, 222),
        _ => Color::rgb(150, 150, 150),
    }
}

fn complement(b: u8, rna: bool) -> u8 {
    match b.to_ascii_uppercase() {
        b'A' if rna => b'U',
        b'A' => b'T',
        b'T' | b'U' => b'A',
        b'G' => b'C',
        b'C' => b'G',
        _ => b'N',
    }
}

/// A DNA (or RNA) double helix with base pairs colored by base. Created by
/// [`Axes::dna_helix`].
#[derive(Clone, Debug)]
pub struct DnaHelix {
    pub(crate) seq: Vec<u8>,
    pub(crate) rna: bool,
    pub(crate) per_turn: f64,
    pub(crate) rise: f64,
    pub(crate) radius: f64,
    pub(crate) groove: f64,
    pub(crate) strands: [Color; 2],
    pub(crate) highlights: Vec<(Range<usize>, Color, Option<String>)>,
    pub(crate) phosphates: bool,
}

impl DnaHelix {
    /// Base pairs per turn (10.5 for B-DNA, 11 for A-form RNA).
    pub fn per_turn(&mut self, n: f64) -> &mut Self {
        self.per_turn = n.max(2.0);
        self
    }
    /// Rise per base pair along the axis (3.38 Å for B-DNA).
    pub fn rise(&mut self, r: f64) -> &mut Self {
        self.rise = r.max(0.1);
        self
    }
    /// Radius of the backbone (10 Å for B-DNA).
    pub fn radius(&mut self, r: f64) -> &mut Self {
        self.radius = r.max(0.5);
        self
    }
    /// Colors of the two backbone strands.
    pub fn strand_colors(&mut self, a: impl Into<Color>, b: impl Into<Color>) -> &mut Self {
        self.strands = [a.into(), b.into()];
        self
    }
    /// Marks base pairs `range` (0-based) in a color, e.g. a binding site
    /// or mutation, with an optional label.
    pub fn highlight(
        &mut self,
        range: Range<usize>,
        color: impl Into<Color>,
        label: Option<&str>,
    ) -> &mut Self {
        self.highlights
            .push((range, color.into(), label.map(str::to_string)));
        self
    }
    /// Draws the phosphate groups as spheres on the backbone (default on).
    pub fn phosphates(&mut self, on: bool) -> &mut Self {
        self.phosphates = on;
        self
    }

    fn angle(&self, i: f64) -> f64 {
        i * std::f64::consts::TAU / self.per_turn
    }
    fn point(&self, i: f64, strand: usize, r: f64) -> [f64; 3] {
        let a = self.angle(i)
            + if strand == 1 {
                self.groove.to_radians()
            } else {
                0.0
            };
        [i * self.rise, r * a.cos(), r * a.sin()]
    }
    fn highlight_of(&self, i: usize) -> Option<Color> {
        self.highlights
            .iter()
            .rev()
            .find(|h| h.0.contains(&i))
            .map(|h| h.1)
    }
}

impl Series for DnaHelix {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, _ctx: &mut Context) {}
    fn bounds3d(&self) -> Option<Bounds3> {
        let n = self.seq.len().max(1) as f64;
        let r = self.radius + 2.0;
        Some([(-2.0, (n - 1.0) * self.rise + 2.0), (-r, r), (-r, r)])
    }
    fn draw3d(&self, scene: &mut Scene3d) {
        let n = self.seq.len();
        if n == 0 {
            return;
        }
        // Backbones: smooth tubes.
        let steps = 6;
        for (s, color) in self.strands.iter().enumerate() {
            let pts: Vec<[f64; 3]> = (0..=(n - 1) * steps)
                .map(|k| self.point(k as f64 / steps as f64, s, self.radius))
                .collect();
            for w in pts.windows(2) {
                scene.tube(w[0], w[1], 1.0, *color);
            }
            if self.phosphates {
                for i in 0..n {
                    scene.sphere(
                        self.point(i as f64, s, self.radius),
                        1.7,
                        color.lighten(0.25),
                    );
                }
            }
        }
        // Base pairs: two halves meeting in the middle of the chord.
        let rb = self.radius - 1.2;
        for (i, &b) in self.seq.iter().enumerate() {
            let p1 = self.point(i as f64, 0, rb);
            let p2 = self.point(i as f64, 1, rb);
            let m = [
                (p1[0] + p2[0]) / 2.0,
                (p1[1] + p2[1]) / 2.0,
                (p1[2] + p2[2]) / 2.0,
            ];
            let d = [p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]];
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
            let g = 0.35 / l;
            let h = self.highlight_of(i);
            let c1 = h.unwrap_or_else(|| base_color(b));
            let c2 = h.unwrap_or_else(|| base_color(complement(b, self.rna)));
            let a = [m[0] - d[0] * g, m[1] - d[1] * g, m[2] - d[2] * g];
            let z = [m[0] + d[0] * g, m[1] + d[1] * g, m[2] + d[2] * g];
            scene.tube(p1, a, 1.15, c1);
            scene.tube(z, p2, 1.15, c2);
        }
        for (r, c, label) in &self.highlights {
            if let Some(l) = label {
                let mid = (r.start + r.end.max(r.start + 1) - 1) as f64 / 2.0;
                let p = self.point(mid, 0, self.radius);
                scene.tag(
                    [p[0], p[1], self.radius + 5.0],
                    l.clone(),
                    12.0,
                    c.contrasting_text(),
                    c.with_alpha(0.92),
                );
            }
        }
    }
    fn hit3d(&self, proj: &Projection3d, px: f64, py: f64) -> Option<Hit> {
        let rb = self.radius - 1.2;
        self.seq
            .iter()
            .enumerate()
            .map(|(i, &b)| {
                let p1 = self.point(i as f64, 0, rb);
                let p2 = self.point(i as f64, 1, rb);
                let m = [
                    (p1[0] + p2[0]) / 2.0,
                    (p1[1] + p2[1]) / 2.0,
                    (p1[2] + p2[2]) / 2.0,
                ];
                let (x, y, d) = proj.project(m);
                Hit {
                    px: x,
                    py: y,
                    label: format!(
                        "bp {}: {}–{}",
                        i + 1,
                        b.to_ascii_uppercase() as char,
                        complement(b, self.rna) as char
                    ),
                    distance: (x - px).hypot(y - py) - d * 1e-6,
                }
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn legend(&self) -> Vec<LegendEntry> {
        let bases: &[u8] = if self.rna { b"AUGC" } else { b"ATGC" };
        let names = ["adenine", "thymine", "guanine", "cytosine"];
        bases
            .iter()
            .zip(names)
            .filter(|(b, _)| {
                self.seq
                    .iter()
                    .any(|s| s.to_ascii_uppercase() == **b || complement(*s, self.rna) == **b)
            })
            .map(|(b, name)| LegendEntry {
                label: format!(
                    "{} {}",
                    *b as char,
                    if *b == b'U' { "uracil" } else { name }
                ),
                symbol: LegendSymbol::Patch {
                    fill: base_color(*b),
                    edge: None,
                },
            })
            .collect()
    }
    fn kind(&self) -> &'static str {
        "DNA helix"
    }
    fn describe(&self) -> Option<String> {
        let n = self.seq.len();
        let gc = self
            .seq
            .iter()
            .filter(|b| matches!(b.to_ascii_uppercase(), b'G' | b'C'))
            .count();
        let shown: String = self
            .seq
            .iter()
            .take(30)
            .map(|b| b.to_ascii_uppercase() as char)
            .collect();
        let mut d = format!(
            "{} double helix of {n} base pairs ({:.0} % GC), sequence {shown}{}",
            if self.rna { "RNA" } else { "DNA" },
            gc as f64 * 100.0 / n.max(1) as f64,
            if n > 30 { "…" } else { "" }
        );
        for (r, _, l) in &self.highlights {
            d.push_str(&format!(
                "; base pairs {}–{} marked{}",
                r.start + 1,
                r.end,
                l.as_ref().map_or(String::new(), |l| format!(" as {l}"))
            ));
        }
        Some(d)
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// One atom of a [`Molecule`].
#[derive(Clone, Debug, PartialEq)]
pub struct Atom {
    /// Element symbol ("C", "N", "Fe", …).
    pub element: String,
    /// Position in Å.
    pub pos: [f64; 3],
    /// Atom name in the residue ("CA", "P", "C1'", …).
    pub name: String,
    /// Residue name ("ALA", "DG", "HOH", …).
    pub residue: String,
    /// Chain identifier.
    pub chain: char,
    /// Residue number.
    pub seq: i32,
    /// HETATM record (ligands, water).
    pub hetero: bool,
}

/// Atoms of a molecule, read from PDB or XYZ text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Molecule {
    /// Atoms in file order.
    pub atoms: Vec<Atom>,
    /// Helices and strands from the PDB HELIX / SHEET records: chain,
    /// first and last residue number.
    pub secondary: Vec<(char, i32, i32, SecondaryStructure)>,
}

/// Secondary structure of a residue range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SecondaryStructure {
    /// α-helix (any helix).
    Helix,
    /// β-strand.
    Strand,
    /// Neither.
    Coil,
}

impl Molecule {
    /// A molecule from atoms.
    pub fn new(atoms: Vec<Atom>) -> Self {
        Self {
            atoms,
            secondary: Vec::new(),
        }
    }

    /// Reads the `ATOM` and `HETATM` records of a PDB file (the first model).
    pub fn from_pdb(text: &str) -> Self {
        let mut atoms = Vec::new();
        let mut secondary = Vec::new();
        for line in text.lines() {
            if line.starts_with("ENDMDL") {
                break;
            }
            let col = |a: usize, b: usize| line.get(a..b.min(line.len())).unwrap_or("").trim();
            let chain_at = |k: usize| line.as_bytes().get(k).map_or(' ', |&c| c as char);
            if line.starts_with("HELIX ") {
                if let (Ok(a), Ok(b)) = (col(21, 25).parse(), col(33, 37).parse()) {
                    secondary.push((chain_at(19), a, b, SecondaryStructure::Helix));
                }
                continue;
            }
            if line.starts_with("SHEET ") {
                if let (Ok(a), Ok(b)) = (col(22, 26).parse(), col(33, 37).parse()) {
                    secondary.push((chain_at(21), a, b, SecondaryStructure::Strand));
                }
                continue;
            }
            let hetero = line.starts_with("HETATM");
            if !(hetero || line.starts_with("ATOM")) || line.len() < 54 {
                continue;
            }
            let col = |a: usize, b: usize| line.get(a..b.min(line.len())).unwrap_or("").trim();
            let num = |a, b| col(a, b).parse::<f64>().ok();
            let (Some(x), Some(y), Some(z)) = (num(30, 38), num(38, 46), num(46, 54)) else {
                continue;
            };
            let name = col(12, 16).to_string();
            let mut element = col(76, 78).to_string();
            if element.is_empty() {
                element = name
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .chars()
                    .take(1)
                    .collect();
            }
            atoms.push(Atom {
                element: normalize_element(&element),
                pos: [x, y, z],
                name,
                residue: col(17, 20).to_string(),
                chain: line.as_bytes().get(21).map_or(' ', |&c| c as char),
                seq: col(22, 26).parse().unwrap_or(0),
                hetero,
            });
        }
        Self { atoms, secondary }
    }

    /// Reads an XYZ file (count, comment, then `element x y z` lines).
    pub fn from_xyz(text: &str) -> Self {
        let atoms = text
            .lines()
            .skip(2)
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                let el = it.next()?;
                let v: Vec<f64> = it.take(3).filter_map(|t| t.parse().ok()).collect();
                (v.len() == 3).then(|| Atom {
                    element: normalize_element(el),
                    pos: [v[0], v[1], v[2]],
                    name: el.to_string(),
                    residue: String::new(),
                    chain: 'A',
                    seq: 0,
                    hetero: false,
                })
            })
            .collect();
        Self {
            atoms,
            secondary: Vec::new(),
        }
    }

    /// Backbone dihedral angles (φ, ψ) in degrees of every protein residue
    /// that has both (for a Ramachandran plot), with its label ("ALA 12 A").
    pub fn phi_psi(&self) -> Vec<(f64, f64, String)> {
        let res = self.residues();
        let mut out = Vec::new();
        for w in res.windows(3) {
            let (p, r, n) = (&w[0], &w[1], &w[2]);
            if p.chain != r.chain || n.chain != r.chain || r.seq - p.seq != 1 || n.seq - r.seq != 1
            {
                continue;
            }
            if let (Some(c0), Some(n1), Some(ca), Some(c1), Some(n2)) = (p.c, r.n, r.ca, r.c, n.n) {
                out.push((
                    dihedral(c0, n1, ca, c1),
                    dihedral(n1, ca, c1, n2),
                    format!("{} {} {}", r.name, r.seq, r.chain),
                ));
            }
        }
        out
    }

    /// Protein residues in order with their backbone atoms.
    fn residues(&self) -> Vec<Residue> {
        let mut out: Vec<Residue> = Vec::new();
        for a in self.atoms.iter().filter(|a| !a.hetero) {
            let new = out
                .last()
                .is_none_or(|r| r.chain != a.chain || r.seq != a.seq);
            if new {
                out.push(Residue {
                    chain: a.chain,
                    seq: a.seq,
                    name: a.residue.clone(),
                    n: None,
                    ca: None,
                    c: None,
                    o: None,
                });
            }
            let r = out.last_mut().unwrap();
            match a.name.as_str() {
                "N" => r.n = Some(a.pos),
                "CA" => r.ca = Some(a.pos),
                "C" => r.c = Some(a.pos),
                "O" => r.o = Some(a.pos),
                _ => {}
            }
        }
        out.retain(|r| r.ca.is_some());
        out
    }

    /// Secondary structure of a residue: from the HELIX / SHEET records,
    /// or `None` when the file has none.
    fn recorded_ss(&self, chain: char, seq: i32) -> Option<SecondaryStructure> {
        if self.secondary.is_empty() {
            return None;
        }
        Some(
            self.secondary
                .iter()
                .find(|s| s.0 == chain && (s.1..=s.2).contains(&seq))
                .map_or(SecondaryStructure::Coil, |s| s.3),
        )
    }

    /// Covalent bonds found from distances (sum of covalent radii + 0.45 Å),
    /// in linear time with a spatial grid.
    pub fn bonds(&self) -> Vec<(usize, usize)> {
        let cell = 2.4;
        let key = |p: [f64; 3]| {
            (
                (p[0] / cell).floor() as i32,
                (p[1] / cell).floor() as i32,
                (p[2] / cell).floor() as i32,
            )
        };
        let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::default();
        for (i, a) in self.atoms.iter().enumerate() {
            grid.entry(key(a.pos)).or_default().push(i);
        }
        let mut out = Vec::new();
        for (i, a) in self.atoms.iter().enumerate() {
            let (kx, ky, kz) = key(a.pos);
            let ra = element(&a.element).cov;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(v) = grid.get(&(kx + dx, ky + dy, kz + dz)) else {
                            continue;
                        };
                        for &j in v {
                            if j <= i {
                                continue;
                            }
                            let b = &self.atoms[j];
                            if a.chain != b.chain && a.hetero == b.hetero && !a.hetero {
                                continue;
                            }
                            let d2: f64 = (0..3).map(|k| (a.pos[k] - b.pos[k]).powi(2)).sum();
                            let max = ra + element(&b.element).cov + 0.45;
                            if d2 > 0.16 && d2 < max * max {
                                out.push((i, j));
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

struct Residue {
    chain: char,
    seq: i32,
    name: String,
    n: Option<[f64; 3]>,
    ca: Option<[f64; 3]>,
    c: Option<[f64; 3]>,
    o: Option<[f64; 3]>,
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit3(a: [f64; 3]) -> [f64; 3] {
    let l = dot3(a, a).sqrt().max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// Dihedral angle a–b–c–d in degrees.
fn dihedral(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3]) -> f64 {
    let (b0, b1, b2) = (sub3(a, b), sub3(c, b), sub3(d, c));
    let b1n = unit3(b1);
    let v = sub3(
        b0,
        [
            b1n[0] * dot3(b0, b1n),
            b1n[1] * dot3(b0, b1n),
            b1n[2] * dot3(b0, b1n),
        ],
    );
    let w = sub3(
        b2,
        [
            b1n[0] * dot3(b2, b1n),
            b1n[1] * dot3(b2, b1n),
            b1n[2] * dot3(b2, b1n),
        ],
    );
    let x = dot3(v, w);
    let y = dot3(cross3(b1n, v), w);
    y.atan2(x).to_degrees()
}

fn normalize_element(e: &str) -> String {
    let mut c = e.chars().filter(|c| c.is_ascii_alphabetic());
    match (c.next(), c.next()) {
        (Some(a), Some(b)) => format!("{}{}", a.to_ascii_uppercase(), b.to_ascii_lowercase()),
        (Some(a), None) => a.to_ascii_uppercase().to_string(),
        _ => "X".into(),
    }
}

struct ElementInfo {
    color: Color,
    cov: f64,
    vdw: f64,
}

/// CPK (Jmol) colors, covalent and van der Waals radii in Å.
fn element(e: &str) -> ElementInfo {
    let (rgb, cov, vdw) = match e {
        "H" => ((235, 235, 235), 0.31, 1.20),
        "C" => ((144, 144, 144), 0.76, 1.70),
        "N" => ((48, 80, 248), 0.71, 1.55),
        "O" => ((255, 13, 13), 0.66, 1.52),
        "P" => ((255, 128, 0), 1.07, 1.80),
        "S" => ((230, 200, 40), 1.05, 1.80),
        "F" => ((144, 224, 80), 0.57, 1.47),
        "Cl" => ((31, 240, 31), 1.02, 1.75),
        "Br" => ((166, 41, 41), 1.20, 1.85),
        "I" => ((148, 0, 148), 1.39, 1.98),
        "Na" => ((171, 92, 242), 1.66, 2.27),
        "K" => ((143, 64, 212), 2.03, 2.75),
        "Mg" => ((138, 255, 0), 1.41, 1.73),
        "Ca" => ((61, 255, 0), 1.76, 2.31),
        "Fe" => ((224, 102, 51), 1.32, 2.0),
        "Zn" => ((125, 128, 176), 1.22, 1.39),
        "Cu" => ((200, 128, 51), 1.32, 1.4),
        "Mn" => ((156, 122, 199), 1.39, 2.0),
        "Se" => ((255, 161, 0), 1.20, 1.90),
        _ => ((255, 20, 147), 0.8, 1.7),
    };
    ElementInfo {
        color: Color::rgb(rgb.0, rgb.1, rgb.2),
        cov,
        vdw,
    }
}

/// How a molecule is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum MolStyle {
    /// Atoms as balls, bonds as sticks.
    #[default]
    BallAndStick,
    /// Bonds only, with rounded joints.
    Sticks,
    /// Atoms at their van der Waals radius.
    SpaceFilling,
    /// A smooth tube through the backbone (Cα of proteins, P of nucleic
    /// acids) per chain; ligands as ball-and-stick.
    Backbone,
    /// Protein cartoon: helices as wide ribbons, strands as arrows, loops
    /// as thin tubes (secondary structure from the PDB records, or from
    /// the Cα geometry); ligands as ball-and-stick.
    Cartoon,
}

/// How a molecule is colored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum MolColor {
    /// By element (CPK colors).
    #[default]
    Element,
    /// By chain.
    Chain,
    /// Along each chain from blue to red.
    Rainbow,
}

/// A drawn molecule. Created by [`Axes::molecule`].
#[derive(Clone, Debug)]
pub struct MoleculeView {
    pub(crate) mol: Molecule,
    pub(crate) bonds: Vec<(usize, usize)>,
    pub(crate) style: MolStyle,
    pub(crate) coloring: MolColor,
    pub(crate) hydrogens: bool,
    pub(crate) water: bool,
    pub(crate) scale: f64,
}

impl MoleculeView {
    /// Drawing style.
    pub fn style(&mut self, s: MolStyle) -> &mut Self {
        self.style = s;
        self
    }
    /// Coloring.
    pub fn coloring(&mut self, c: MolColor) -> &mut Self {
        self.coloring = c;
        self
    }
    /// Shows hydrogen atoms (default on).
    pub fn hydrogens(&mut self, on: bool) -> &mut Self {
        self.hydrogens = on;
        self
    }
    /// Shows water molecules (default off).
    pub fn water(&mut self, on: bool) -> &mut Self {
        self.water = on;
        self
    }
    /// Size factor of atoms and bonds.
    pub fn atom_scale(&mut self, f: f64) -> &mut Self {
        self.scale = f.max(0.05);
        self
    }

    fn shown(&self, a: &Atom) -> bool {
        (self.hydrogens || a.element != "H")
            && (self.water || !matches!(a.residue.as_str(), "HOH" | "WAT" | "H2O"))
    }

    fn colors(&self) -> Vec<Color> {
        let atoms = &self.mol.atoms;
        match self.coloring {
            MolColor::Element => atoms.iter().map(|a| element(&a.element).color).collect(),
            MolColor::Chain => {
                let mut chains: Vec<char> = Vec::new();
                atoms
                    .iter()
                    .map(|a| {
                        let k = chains
                            .iter()
                            .position(|c| *c == a.chain)
                            .unwrap_or_else(|| {
                                chains.push(a.chain);
                                chains.len() - 1
                            });
                        Palette::Tab10.color(k)
                    })
                    .collect()
            }
            MolColor::Rainbow => {
                // Position of the residue within its chain.
                let mut range: HashMap<char, (i32, i32)> = HashMap::default();
                for a in atoms.iter().filter(|a| !a.hetero) {
                    let e = range.entry(a.chain).or_insert((a.seq, a.seq));
                    *e = (e.0.min(a.seq), e.1.max(a.seq));
                }
                atoms
                    .iter()
                    .map(|a| match range.get(&a.chain) {
                        Some(&(lo, hi)) if !a.hetero => Colormap::Turbo
                            .at((a.seq - lo) as f64 / (hi - lo).max(1) as f64 * 0.9 + 0.05),
                        _ => element(&a.element).color,
                    })
                    .collect()
            }
        }
    }

    /// Cartoon of each protein chain.
    fn cartoon(&self, scene: &mut Scene3d) {
        let res = self.mol.residues();
        let colors = self.colors();
        // Color of each residue: by secondary structure (element mode) or
        // the coloring of its Cα.
        let ca_color: Vec<Color> = res
            .iter()
            .map(|r| {
                self.mol
                    .atoms
                    .iter()
                    .position(|a| {
                        !a.hetero && a.chain == r.chain && a.seq == r.seq && a.name == "CA"
                    })
                    .map_or(SS_COIL, |i| colors[i])
            })
            .collect();
        let mut start = 0;
        while start < res.len() {
            let mut end = start + 1;
            while end < res.len()
                && res[end].chain == res[start].chain
                && res[end].seq - res[end - 1].seq <= 1
            {
                end += 1;
            }
            if end - start >= 3 {
                self.cartoon_chain(scene, &res[start..end], &ca_color[start..end]);
            }
            start = end;
        }
    }

    fn cartoon_chain(&self, scene: &mut Scene3d, res: &[Residue], cols: &[Color]) {
        let n = res.len();
        let ca: Vec<[f64; 3]> = res.iter().map(|r| r.ca.unwrap()).collect();
        // Secondary structure: recorded, or from the Cα geometry.
        let ss: Vec<SecondaryStructure> = (0..n)
            .map(|i| {
                if let Some(s) = self.mol.recorded_ss(res[i].chain, res[i].seq) {
                    return s;
                }
                let d = |a: usize, b: usize| dot3(sub3(ca[a], ca[b]), sub3(ca[a], ca[b])).sqrt();
                let helix = (i >= 1 && i + 2 < n && (4.5..6.2).contains(&d(i - 1, i + 2)))
                    || (i >= 2 && i + 1 < n && (4.5..6.2).contains(&d(i - 2, i + 1)));
                let strand = i >= 1 && i + 1 < n && d(i - 1, i + 1) > 6.4;
                if helix {
                    SecondaryStructure::Helix
                } else if strand {
                    SecondaryStructure::Strand
                } else {
                    SecondaryStructure::Coil
                }
            })
            .collect();
        // Ribbon orientation: the carbonyl direction (C→O), kept from
        // flipping between residues.
        let mut guide: Vec<[f64; 3]> = Vec::with_capacity(n);
        for (i, r) in res.iter().enumerate() {
            let g = match (r.c, r.o) {
                (Some(c), Some(o)) => unit3(sub3(o, c)),
                _ => {
                    let a = if i > 0 { ca[i - 1] } else { ca[i] };
                    let b = if i + 1 < n { ca[i + 1] } else { ca[i] };
                    unit3(sub3(sub3(ca[i], a), sub3(b, ca[i])))
                }
            };
            let g = match guide.last() {
                Some(p) if dot3(*p, g) < 0.0 => [-g[0], -g[1], -g[2]],
                _ => g,
            };
            guide.push(g);
        }
        let k = 8;
        let curve = spline(&ca, k);
        let gcurve = spline(&guide, k);
        let s = self.scale;
        let m = curve.len();
        // Per sample: residue, half width, frame.
        let mut left = Vec::with_capacity(m);
        let mut right = Vec::with_capacity(m);
        let mut kind = Vec::with_capacity(m);
        for j in 0..m {
            let r = (j / k).min(n - 1);
            let t = unit3(sub3(curve[(j + 1).min(m - 1)], curve[j.saturating_sub(1)]));
            // Guide made perpendicular to the tangent: the ribbon's width
            // direction.
            let g = gcurve[j];
            let g = unit3(sub3(
                g,
                [t[0] * dot3(g, t), t[1] * dot3(g, t), t[2] * dot3(g, t)],
            ));
            let last_of_strand = ss[r] == SecondaryStructure::Strand
                && (r + 1 == n || ss[r + 1] != SecondaryStructure::Strand);
            let w = match ss[r] {
                SecondaryStructure::Helix => 1.15,
                SecondaryStructure::Strand if last_of_strand => {
                    // Arrow head over the strand's last residue.
                    let f = (j % k) as f64 / k as f64;
                    1.7 * (1.0 - f)
                }
                SecondaryStructure::Strand => 0.95,
                SecondaryStructure::Coil => 0.0,
            } * s;
            let p = curve[j];
            left.push([p[0] + g[0] * w, p[1] + g[1] * w, p[2] + g[2] * w]);
            right.push([p[0] - g[0] * w, p[1] - g[1] * w, p[2] - g[2] * w]);
            kind.push(ss[r]);
        }
        let color_at = |j: usize| {
            let r = (j / k).min(n - 1);
            match self.coloring {
                MolColor::Element => match ss[r] {
                    SecondaryStructure::Helix => SS_HELIX,
                    SecondaryStructure::Strand => SS_STRAND,
                    SecondaryStructure::Coil => SS_COIL,
                },
                _ => cols[r].mix(cols[(r + 1).min(n - 1)], (j % k) as f64 / k as f64),
            }
        };
        // Loops as continuous tubes (overlapping one sample into the
        // neighbouring ribbon so there is no gap), the rest as ribbons.
        let mut j = 0;
        while j + 1 < m {
            if kind[j] == SecondaryStructure::Coil {
                let a = j.saturating_sub(1);
                let mut b = j;
                while b + 1 < m && kind[b] == SecondaryStructure::Coil {
                    b += 1;
                }
                let cs: Vec<Color> = (a..=b).map(color_at).collect();
                scene.tube_line(&curve[a..=b], 0.32 * s, cs[0], Some(&cs));
                j = b;
            } else {
                scene.polygon(
                    &[left[j], left[j + 1], right[j + 1], right[j]],
                    color_at(j),
                    None,
                );
                j += 1;
            }
        }
    }

    /// Backbone traces: per chain, indices of Cα / P atoms in order.
    fn traces(&self) -> Vec<Vec<usize>> {
        let mut out: Vec<(char, Vec<usize>)> = Vec::new();
        for (i, a) in self.mol.atoms.iter().enumerate() {
            if a.hetero || !(a.name == "CA" || a.name == "P") {
                continue;
            }
            match out.last_mut() {
                Some((c, v)) if *c == a.chain => v.push(i),
                _ => out.push((a.chain, vec![i])),
            }
        }
        out.into_iter()
            .map(|(_, v)| v)
            .filter(|v| v.len() > 1)
            .collect()
    }
}

/// Cartoon colors: helices, strands and loops.
const SS_HELIX: Color = Color::rgb(222, 61, 130);
const SS_STRAND: Color = Color::rgb(248, 196, 52);
const SS_COIL: Color = Color::rgb(176, 180, 190);

/// Catmull-Rom spline through points, `k` samples per segment.
fn spline(p: &[[f64; 3]], k: usize) -> Vec<[f64; 3]> {
    let n = p.len();
    let mut out = Vec::with_capacity(n * k);
    for i in 0..n - 1 {
        let p0 = p[i.saturating_sub(1)];
        let (p1, p2) = (p[i], p[i + 1]);
        let p3 = p[(i + 2).min(n - 1)];
        for s in 0..k {
            let t = s as f64 / k as f64;
            let (t2, t3) = (t * t, t * t * t);
            let mut q = [0.0; 3];
            for d in 0..3 {
                q[d] = 0.5
                    * (2.0 * p1[d]
                        + (-p0[d] + p2[d]) * t
                        + (2.0 * p0[d] - 5.0 * p1[d] + 4.0 * p2[d] - p3[d]) * t2
                        + (-p0[d] + 3.0 * p1[d] - 3.0 * p2[d] + p3[d]) * t3);
            }
            out.push(q);
        }
    }
    out.push(p[n - 1]);
    out
}

impl Series for MoleculeView {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, _ctx: &mut Context) {}
    fn bounds3d(&self) -> Option<Bounds3> {
        let mut b = [(f64::INFINITY, f64::NEG_INFINITY); 3];
        for a in self.mol.atoms.iter().filter(|a| self.shown(a)) {
            for k in 0..3 {
                b[k] = (b[k].0.min(a.pos[k] - 2.0), b[k].1.max(a.pos[k] + 2.0));
            }
        }
        b[0].0.is_finite().then_some(b)
    }
    fn draw3d(&self, scene: &mut Scene3d) {
        let atoms = &self.mol.atoms;
        let colors = self.colors();
        let s = self.scale;
        let mid = |a: [f64; 3], b: [f64; 3]| {
            [
                (a[0] + b[0]) / 2.0,
                (a[1] + b[1]) / 2.0,
                (a[2] + b[2]) / 2.0,
            ]
        };
        let ball_and_stick = |scene: &mut Scene3d, only_hetero: bool, sticks: bool| {
            let ok = |a: &Atom| self.shown(a) && (!only_hetero || a.hetero);
            let br = if sticks { 0.2 } else { 0.14 } * s;
            for &(i, j) in &self.bonds {
                let (a, b) = (&atoms[i], &atoms[j]);
                if !(ok(a) && ok(b)) {
                    continue;
                }
                let m = mid(a.pos, b.pos);
                scene.tube(a.pos, m, br, colors[i]);
                scene.tube(m, b.pos, br, colors[j]);
            }
            for (i, a) in atoms.iter().enumerate().filter(|(_, a)| ok(a)) {
                let r = if sticks {
                    br
                } else {
                    element(&a.element).vdw * 0.24 * s
                };
                scene.sphere(a.pos, r, colors[i]);
            }
        };
        match self.style {
            MolStyle::BallAndStick => ball_and_stick(scene, false, false),
            MolStyle::Sticks => ball_and_stick(scene, false, true),
            MolStyle::SpaceFilling => {
                for (i, a) in atoms.iter().enumerate().filter(|(_, a)| self.shown(a)) {
                    scene.sphere(a.pos, element(&a.element).vdw * s, colors[i]);
                }
            }
            MolStyle::Cartoon => {
                self.cartoon(scene);
                ball_and_stick(scene, true, false);
            }
            MolStyle::Backbone => {
                for t in self.traces() {
                    let p: Vec<[f64; 3]> = t.iter().map(|&i| atoms[i].pos).collect();
                    let c: Vec<Color> = t
                        .iter()
                        .map(|&i| match self.coloring {
                            // Element colors make no sense for a trace.
                            MolColor::Element => Palette::Tab10.color(0),
                            _ => colors[i],
                        })
                        .collect();
                    let k = 6;
                    let curve = spline(&p, k);
                    let cs: Vec<Color> = (0..curve.len())
                        .map(|n| {
                            let a = (n / k).min(c.len() - 1);
                            c[a].mix(c[(a + 1).min(c.len() - 1)], (n % k) as f64 / k as f64)
                        })
                        .collect();
                    scene.tube_line(&curve, 0.75 * s, cs[0], Some(&cs));
                }
                ball_and_stick(scene, true, false);
            }
        }
    }
    fn hit3d(&self, proj: &Projection3d, px: f64, py: f64) -> Option<Hit> {
        self.mol
            .atoms
            .iter()
            .filter(|a| self.shown(a))
            .filter(|a| {
                !matches!(self.style, MolStyle::Backbone | MolStyle::Cartoon)
                    || a.hetero
                    || a.name == "CA"
                    || a.name == "P"
            })
            .map(|a| {
                let (x, y, d) = proj.project(a.pos);
                let label = if a.residue.is_empty() {
                    a.element.clone()
                } else {
                    format!("{} {}{} {}", a.name, a.residue, a.seq, a.chain)
                };
                Hit {
                    px: x,
                    py: y,
                    label,
                    distance: (x - px).hypot(y - py) - d * 1e-6,
                }
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn legend(&self) -> Vec<LegendEntry> {
        if self.style == MolStyle::Cartoon && self.coloring == MolColor::Element {
            return [
                ("helix", SS_HELIX),
                ("strand", SS_STRAND),
                ("loop", SS_COIL),
            ]
            .iter()
            .map(|(l, c)| LegendEntry {
                label: l.to_string(),
                symbol: LegendSymbol::Patch {
                    fill: *c,
                    edge: None,
                },
            })
            .collect();
        }
        if self.coloring != MolColor::Element || self.style == MolStyle::Backbone {
            return Vec::new();
        }
        let mut seen: Vec<&str> = Vec::new();
        for a in self.mol.atoms.iter().filter(|a| self.shown(a)) {
            if !seen.contains(&a.element.as_str()) {
                seen.push(&a.element);
            }
        }
        seen.sort_by_key(|e| match *e {
            "C" => 0,
            "H" => 1,
            _ => 2,
        });
        seen.into_iter()
            .take(10)
            .map(|e| LegendEntry {
                label: e.to_string(),
                symbol: LegendSymbol::Patch {
                    fill: element(e).color,
                    edge: Some(Color::rgb(90, 90, 90)),
                },
            })
            .collect()
    }
    fn kind(&self) -> &'static str {
        "molecule"
    }
    fn describe(&self) -> Option<String> {
        let atoms: Vec<&Atom> = self.mol.atoms.iter().filter(|a| self.shown(a)).collect();
        let mut chains: Vec<char> = atoms
            .iter()
            .filter(|a| !a.hetero)
            .map(|a| a.chain)
            .collect();
        chains.dedup();
        chains.sort_unstable();
        chains.dedup();
        let residues = self.mol.residues().len();
        let mut ligands: Vec<&str> = atoms
            .iter()
            .filter(|a| a.hetero)
            .map(|a| a.residue.as_str())
            .collect();
        ligands.sort_unstable();
        ligands.dedup();
        let style = match self.style {
            MolStyle::BallAndStick => "ball-and-stick",
            MolStyle::Sticks => "sticks",
            MolStyle::SpaceFilling => "space-filling",
            MolStyle::Backbone => "backbone tube",
            MolStyle::Cartoon => "cartoon",
        };
        let mut d = format!("{style} model of {} atoms", atoms.len());
        if residues > 0 {
            d.push_str(&format!(
                ", {residues} residues in {} chain{}",
                chains.len(),
                if chains.len() == 1 { "" } else { "s" }
            ));
        }
        let (h, s) = self
            .mol
            .secondary
            .iter()
            .fold((0, 0), |(h, s), x| match x.3 {
                SecondaryStructure::Helix => (h + 1, s),
                SecondaryStructure::Strand => (h, s + 1),
                SecondaryStructure::Coil => (h, s),
            });
        if h + s > 0 {
            d.push_str(&format!(", {h} helices and {s} strands"));
        }
        if !ligands.is_empty() {
            d.push_str(&format!(", ligands {}", ligands.join(", ")));
        }
        Some(d)
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    fn scene_3d_for_structures(&mut self) {
        self.kind = AxesKind::ThreeD;
        self.equal_aspect = true;
        self.view3d.show_box = false;
        if self.view3d.fog == 0.0 {
            self.view3d.fog = 0.35;
        }
    }

    /// A DNA double helix (B-form: 10.5 bp per turn, 3.38 Å rise, 20 Å
    /// wide) of `sequence` (the first strand, 5'→3'); the second strand is
    /// its complement. RNA sequences (with U) pair A–U.
    pub fn dna_helix(&mut self, sequence: &str) -> &mut DnaHelix {
        self.scene_3d_for_structures();
        let seq: Vec<u8> = sequence
            .bytes()
            .filter(|b| b.is_ascii_alphabetic())
            .collect();
        let rna = seq.iter().any(|b| b.eq_ignore_ascii_case(&b'U'));
        self.camera(-8.0, 14.0);
        self.add(DnaHelix {
            seq,
            rna,
            per_turn: 10.5,
            rise: 3.38,
            radius: 10.0,
            groove: 150.0,
            strands: [Color::rgb(70, 92, 140), Color::rgb(140, 92, 150)],
            highlights: Vec::new(),
            phosphates: true,
        })
    }

    /// A molecule (from [`Molecule::from_pdb`] or [`Molecule::from_xyz`])
    /// with bonds found from distances.
    pub fn molecule(&mut self, mol: Molecule) -> &mut MoleculeView {
        self.scene_3d_for_structures();
        if self.series.is_empty() {
            // Longest extent across the screen, tilted a little for depth.
            let pts: Vec<[f64; 3]> = mol.atoms.iter().map(|a| a.pos).collect();
            let mut cam = super::three_d::Camera::principal(&pts);
            cam.turn(-25.0, 15.0);
            self.set_camera(cam);
        }
        let bonds = mol.bonds();
        self.add(MoleculeView {
            mol,
            bonds,
            style: MolStyle::BallAndStick,
            coloring: MolColor::Element,
            hydrogens: true,
            water: false,
            scale: 1.0,
        })
    }

    /// A 3D embedding (UMAP, t-SNE, PCA) of cells colored by cluster, each
    /// cluster named at its centre.
    pub fn embedding3d(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        z: impl IntoValues,
        clusters: impl IntoLabels,
    ) -> &mut Self {
        let (x, y, z, c) = (
            x.into_values(),
            y.into_values(),
            z.into_values(),
            clusters.into_labels(),
        );
        let mut names: Vec<String> = Vec::new();
        let mut groups: Vec<[Vec<f64>; 3]> = Vec::new();
        for i in 0..x.len().min(y.len()).min(z.len()).min(c.len()) {
            let k = match names.iter().position(|n| *n == c[i]) {
                Some(k) => k,
                None => {
                    names.push(c[i].clone());
                    groups.push(Default::default());
                    names.len() - 1
                }
            };
            groups[k][0].push(x[i]);
            groups[k][1].push(y[i]);
            groups[k][2].push(z[i]);
        }
        let palette = if names.len() > 10 {
            Palette::Tab20.colors()
        } else {
            Palette::Tab10.colors()
        };
        let size = if x.len() > 20_000 {
            3.0
        } else if x.len() > 2_000 {
            4.5
        } else {
            7.0
        };
        for (k, g) in groups.iter().enumerate() {
            self.scatter3d(g[0].clone(), g[1].clone(), g[2].clone())
                .color(palette[k % palette.len()])
                .size(size)
                .label(names[k].clone());
        }
        let med = |v: &[f64]| {
            let mut s = v.to_vec();
            s.sort_by(f64::total_cmp);
            s[s.len() / 2]
        };
        let labels: Vec<([f64; 3], String)> = groups
            .iter()
            .zip(&names)
            .map(|(g, n)| ([med(&g[0]), med(&g[1]), med(&g[2])], n.clone()))
            .collect();
        self.text3d(&labels)
            .size(12.0)
            .color(Color::rgb(25, 25, 25))
            .background(Color::rgba(255, 255, 255, 0.82));
        self.x.label.get_or_insert_with(|| "UMAP 1".into());
        self.y.label.get_or_insert_with(|| "UMAP 2".into());
        self.view3d.z_label.get_or_insert_with(|| "UMAP 3".into());
        if self.view3d.fog == 0.0 {
            self.view3d.fog = 0.25;
        }
        self.legend(crate::plot::LegendPosition::Hidden);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    const PDB: &str = "\
ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N
ATOM      2  CA  ALA A   1      11.639   6.071  -5.147  1.00  0.00           C
ATOM      3  C   ALA A   1      13.149   5.856  -5.162  1.00  0.00           C
ATOM      4  O   ALA A   1      13.691   5.111  -5.981  1.00  0.00           O
ATOM      5  CB  ALA A   1      11.329   7.371  -4.393  1.00  0.00           C
ATOM      6  N   GLY A   2      13.817   6.523  -4.236  1.00  0.00           N
ATOM      7  CA  GLY A   2      15.264   6.410  -4.153  1.00  0.00           C
HETATM    8  O   HOH A 101      20.000  20.000  20.000  1.00  0.00           O
END
";

    #[test]
    fn pdb_and_bonds() {
        let m = Molecule::from_pdb(PDB);
        assert_eq!(m.atoms.len(), 8);
        assert_eq!(m.atoms[1].name, "CA");
        assert_eq!(m.atoms[1].element, "C");
        assert_eq!(m.atoms[7].residue, "HOH");
        let b = m.bonds();
        // N-CA, CA-C, C-O, CA-CB, C-N(next), N-CA(next).
        assert_eq!(b.len(), 6, "{b:?}");
        assert!(b.contains(&(2, 5)));
    }

    #[test]
    fn xyz() {
        let m = Molecule::from_xyz("3\nwater\nO 0 0 0\nH 0.96 0 0\nH -0.24 0.93 0\n");
        assert_eq!(m.atoms.len(), 3);
        assert_eq!(m.bonds().len(), 2);
    }

    #[test]
    fn renders_dna_molecule_and_cells() {
        let mut fig = Figure::new(900, 300);
        fig.grid(1, 3);
        fig.at(0, 0)
            .dna_helix("ATGCGTACGTTAGC")
            .highlight(3..6, "#ffd400", Some("site"));
        fig.at(0, 1)
            .molecule(Molecule::from_pdb(PDB))
            .style(MolStyle::BallAndStick);
        let x: Vec<f64> = (0..300)
            .map(|i| (i % 3) as f64 * 5.0 + (i as f64 * 0.7).sin())
            .collect();
        let y: Vec<f64> = (0..300).map(|i| (i as f64 * 1.3).cos()).collect();
        let z: Vec<f64> = (0..300).map(|i| (i as f64 * 0.37).sin()).collect();
        let c: Vec<&str> = (0..300).map(|i| ["T", "B", "NK"][i % 3]).collect();
        fig.at(0, 2).embedding3d(&x, &y, &z, &c);
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
        let (_, l) = fig.scene_and_layouts();
        let p = l[0].plot;
        assert!(fig.pan_by(p.x + 50.0, p.y + 50.0, 40.0, 10.0));
        let h = fig.hover(p.x + p.w / 2.0, p.y + p.h / 2.0).unwrap();
        assert!(h.text.starts_with("bp "), "{}", h.text);
        assert!(fig.rotate_view(1, Axis3::Y, 30.0));
    }
}
