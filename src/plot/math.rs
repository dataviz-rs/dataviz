//! Math notation in text: `$...$` holds a TeX subset that is laid out into
//! plain text runs and lines, so every backend (CPU, GPU, SVG, PDF) draws it
//! with the same glyphs as other text.
//!
//! Supported inside `$...$`:
//!
//! - superscripts and subscripts: `x^2`, `x_{i+1}`, `e^{-x^2}`, `a_i^2`;
//! - `\frac{a}{b}`, `\sqrt{x}`, `\overline{x}`, `\bar{x}`, `\hat{x}`,
//!   `\vec{x}`, `\dot{x}`, `\tilde{x}`;
//! - Greek letters (`\alpha` … `\omega`, `\Gamma` … `\Omega`, `\varepsilon`,
//!   `\varphi`, `\vartheta`) and symbols such as `\pm \times \cdot \leq \geq
//!   \neq \approx \infty \partial \nabla \sum \prod \int \to \in \degree`;
//! - `\mathrm{..}`, `\text{..}` (upright), `\mathbf{..}` (bold),
//!   `\mathit{..}`; function names `\sin \cos \log \exp \lim …` upright;
//! - spacing `\, \: \; \! \quad \qquad`, and `\{ \} \$ \% \_ \# \&`.
//!
//! Letters are set in italic, digits and operators upright, with TeX-like
//! spacing around binary operators and relations. Text outside `$...$` is
//! drawn as is; `\$` is a literal dollar sign.

use super::scene::{Anchor, Baseline, LineCap, LineJoin, Node, PathCmd, Stroke, TextNode};
use super::text::FontSet;

/// Whether `s` contains math (an unescaped `$` pair).
pub(crate) fn has_math(s: &str) -> bool {
    if !s.contains('$') {
        return false;
    }
    let mut n = 0;
    let mut esc = false;
    for c in s.chars() {
        match c {
            '\\' => esc = !esc,
            '$' if !esc => n += 1,
            _ => esc = false,
        }
        if c == '$' {
            esc = false;
        }
    }
    n >= 2
}

/// A text run of a layout; `y` is the baseline (down is positive).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Run {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub text: String,
    pub italic: bool,
    pub bold: bool,
}

/// A laid out text: runs and polylines relative to the origin on the
/// baseline.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Layout {
    pub runs: Vec<Run>,
    /// Polylines with their stroke width (fraction bars, radicals).
    pub lines: Vec<(Vec<(f64, f64)>, f64)>,
    pub width: f64,
    /// Extent above the baseline (positive).
    pub ascent: f64,
    /// Extent below the baseline (positive).
    pub descent: f64,
}

impl Layout {
    fn shift(&mut self, dx: f64, dy: f64) {
        for r in &mut self.runs {
            r.x += dx;
            r.y += dy;
        }
        for (pts, _) in &mut self.lines {
            for p in pts {
                p.0 += dx;
                p.1 += dy;
            }
        }
    }
}

/// Lays out `text` (with or without `$...$`) at `size` pixels.
pub(crate) fn layout(fonts: &FontSet, text: &str, size: f64, bold: bool) -> Layout {
    let mut b = Builder {
        fonts,
        out: Layout::default(),
        pen: 0.0,
    };
    let mut plain = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'$') => {
                chars.next();
                plain.push('$');
            }
            '$' => {
                let mut src = String::new();
                let mut closed = false;
                while let Some(d) = chars.next() {
                    if d == '\\' && chars.peek() == Some(&'$') {
                        chars.next();
                        src.push_str("\\$");
                    } else if d == '$' {
                        closed = true;
                        break;
                    } else {
                        src.push(d);
                    }
                }
                if !closed {
                    plain.push('$');
                    plain.push_str(&src);
                    continue;
                }
                if !plain.is_empty() {
                    b.text(&std::mem::take(&mut plain), size, false, bold, 0.0);
                }
                let atoms = Parser::new(&src).list(None);
                let m = math_list(fonts, &atoms, size, bold, false);
                let pen = b.pen;
                b.pen += m.width;
                b.out.ascent = b.out.ascent.max(m.ascent);
                b.out.descent = b.out.descent.max(m.descent);
                b.out.append_raw(m, pen, 0.0);
            }
            _ => plain.push(c),
        }
    }
    if !plain.is_empty() {
        b.text(&plain, size, false, bold, 0.0);
    }
    b.out.width = b.pen;
    b.out
}

impl Layout {
    fn append_raw(&mut self, mut o: Layout, dx: f64, dy: f64) {
        o.shift(dx, dy);
        self.runs.extend(o.runs);
        self.lines.extend(o.lines);
    }
}

/// Builds a horizontal list of runs.
struct Builder<'a> {
    fonts: &'a FontSet,
    out: Layout,
    pen: f64,
}

const ASC: f64 = 0.74;
const DESC: f64 = 0.24;

impl Builder<'_> {
    fn text(&mut self, s: &str, size: f64, italic: bool, bold: bool, y: f64) {
        if s.is_empty() {
            return;
        }
        let w = self.fonts.width_plain(s, size);
        // Continue the previous run when it ends where this one starts.
        match self.out.runs.last_mut() {
            Some(last)
                if last.y == y
                    && last.size == size
                    && last.italic == italic
                    && last.bold == bold
                    && (last.x + self.fonts.width_plain(&last.text, size) - self.pen).abs()
                        < 1e-6 =>
            {
                last.text.push_str(s);
            }
            _ => self.out.runs.push(Run {
                x: self.pen,
                y,
                size,
                text: s.to_string(),
                italic,
                bold,
            }),
        }
        self.pen += w;
        self.out.ascent = self.out.ascent.max(size * ASC - y);
        self.out.descent = self.out.descent.max(size * DESC + y);
    }
}

/// Spacing class of an atom (TeX's ord, bin, rel, punct, op).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Class {
    Ord,
    Bin,
    Rel,
    Punct,
    Op,
    Open,
    Close,
}

#[derive(Clone, Debug, PartialEq)]
enum Atom {
    /// Text with its class; `italic`, `bold` (None = inherit).
    Sym {
        text: String,
        class: Class,
        italic: bool,
        bold: Option<bool>,
        scale: f64,
    },
    Group(Vec<Atom>),
    Scripts {
        base: Box<Atom>,
        sup: Option<Box<Atom>>,
        sub: Option<Box<Atom>>,
    },
    Frac(Box<Atom>, Box<Atom>),
    Sqrt(Box<Atom>),
    Over(Box<Atom>),
    Accent(Box<Atom>, char),
    /// Horizontal space in em.
    Space(f64),
}

impl Atom {
    fn class(&self) -> Class {
        match self {
            Atom::Sym { class, .. } => *class,
            Atom::Scripts { base, .. } => base.class(),
            Atom::Space(_) => Class::Ord,
            _ => Class::Ord,
        }
    }
}

struct Parser<'a> {
    s: &'a [u8],
    src: &'a str,
    i: usize,
}

/// Font style inside a group.
#[derive(Clone, Copy, PartialEq)]
enum Style {
    Math,
    Roman,
    Bold,
    Italic,
    Text,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            s: src.as_bytes(),
            src,
            i: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.i..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }

    /// Atoms until `}` (when `close`) or the end.
    fn list(&mut self, close: Option<char>) -> Vec<Atom> {
        self.list_styled(close, Style::Math)
    }

    fn list_styled(&mut self, close: Option<char>, style: Style) -> Vec<Atom> {
        let mut out: Vec<Atom> = Vec::new();
        while let Some(c) = self.peek() {
            if Some(c) == close {
                self.bump();
                break;
            }
            match c {
                '^' | '_' => {
                    self.bump();
                    let arg = self.argument(style);
                    let base = out.pop().unwrap_or(Atom::Group(Vec::new()));
                    let (base, mut sup, mut sub) = match base {
                        Atom::Scripts { base, sup, sub } => (base, sup, sub),
                        b => (Box::new(b), None, None),
                    };
                    if c == '^' {
                        sup = Some(Box::new(arg));
                    } else {
                        sub = Some(Box::new(arg));
                    }
                    out.push(Atom::Scripts { base, sup, sub });
                }
                _ => {
                    if let Some(a) = self.atom(style) {
                        out.push(a);
                    }
                }
            }
        }
        out
    }

    /// A single atom or a `{...}` group (script and command arguments).
    fn argument(&mut self, style: Style) -> Atom {
        while self.peek() == Some(' ') {
            self.bump();
        }
        match self.peek() {
            Some('{') => {
                self.bump();
                Atom::Group(self.list_styled(Some('}'), style))
            }
            Some(_) => self.atom(style).unwrap_or(Atom::Group(Vec::new())),
            None => Atom::Group(Vec::new()),
        }
    }

    fn atom(&mut self, style: Style) -> Option<Atom> {
        let c = self.bump()?;
        let sym = |text: &str, class: Class, italic: bool| Atom::Sym {
            text: text.to_string(),
            class,
            italic,
            bold: (style == Style::Bold).then_some(true),
            scale: 1.0,
        };
        let upright = !matches!(style, Style::Math | Style::Italic);
        Some(match c {
            '{' => Atom::Group(self.list_styled(Some('}'), style)),
            '}' => return None,
            ' ' if style == Style::Text => Atom::Space(0.3),
            ' ' | '\t' | '\n' => return None,
            '\\' => return self.command(style),
            _ if style == Style::Text => sym(&c.to_string(), Class::Ord, false),
            'a'..='z' | 'A'..='Z' => sym(
                &c.to_string(),
                Class::Ord,
                !upright || style == Style::Italic,
            ),
            '0'..='9' | '.' => {
                let mut t = c.to_string();
                while let Some(d @ ('0'..='9' | '.')) = self.peek() {
                    t.push(d);
                    self.bump();
                }
                sym(&t, Class::Ord, false)
            }
            '+' => sym("+", Class::Bin, false),
            '-' => sym("\u{2212}", Class::Bin, false),
            '*' => sym("\u{2217}", Class::Bin, false),
            '/' => sym("/", Class::Ord, false),
            '=' | '<' | '>' => sym(&c.to_string(), Class::Rel, false),
            ':' => sym(":", Class::Rel, false),
            ',' | ';' => sym(&c.to_string(), Class::Punct, false),
            '(' | '[' => sym(&c.to_string(), Class::Open, false),
            ')' | ']' => sym(&c.to_string(), Class::Close, false),
            '\'' => sym("\u{2032}", Class::Ord, false),
            '|' | '!' | '?' => sym(&c.to_string(), Class::Ord, false),
            _ => sym(&c.to_string(), Class::Ord, false),
        })
    }

    fn name(&mut self) -> String {
        let start = self.i;
        while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() {
            self.i += 1;
        }
        self.src[start..self.i].to_string()
    }

    fn command(&mut self, style: Style) -> Option<Atom> {
        let name = self.name();
        let bold = (style == Style::Bold).then_some(true);
        let sym = |text: &str, class: Class, italic: bool| {
            Some(Atom::Sym {
                text: text.to_string(),
                class,
                italic,
                bold,
                scale: 1.0,
            })
        };
        if name.is_empty() {
            // \, \: \; \! \  and escaped characters.
            let c = self.bump()?;
            return match c {
                ',' => Some(Atom::Space(3.0 / 18.0)),
                ':' | '>' => Some(Atom::Space(4.0 / 18.0)),
                ';' => Some(Atom::Space(5.0 / 18.0)),
                '!' => Some(Atom::Space(-3.0 / 18.0)),
                ' ' => Some(Atom::Space(0.33)),
                '{' => sym("{", Class::Open, false),
                '}' => sym("}", Class::Close, false),
                '|' => sym("\u{2016}", Class::Ord, false),
                c => sym(&c.to_string(), Class::Ord, false),
            };
        }
        let styled = |p: &mut Self, s: Style| Some(Atom::Group(p.styled_argument(s)));
        match name.as_str() {
            "frac" | "dfrac" | "tfrac" => {
                let a = self.argument(style);
                let b = self.argument(style);
                Some(Atom::Frac(Box::new(a), Box::new(b)))
            }
            "sqrt" => Some(Atom::Sqrt(Box::new(self.argument(style)))),
            "overline" | "bar" => Some(Atom::Over(Box::new(self.argument(style)))),
            "hat" | "widehat" => Some(Atom::Accent(Box::new(self.argument(style)), '\u{02C6}')),
            "vec" => Some(Atom::Accent(Box::new(self.argument(style)), '\u{2192}')),
            "dot" => Some(Atom::Accent(Box::new(self.argument(style)), '\u{02D9}')),
            "ddot" => Some(Atom::Accent(Box::new(self.argument(style)), '\u{00A8}')),
            "tilde" | "widetilde" => Some(Atom::Accent(Box::new(self.argument(style)), '\u{02DC}')),
            "mathrm" | "operatorname" | "rm" => styled(self, Style::Roman),
            "text" | "textrm" | "mbox" => styled(self, Style::Text),
            "mathbf" | "textbf" | "bf" => styled(self, Style::Bold),
            "mathit" | "textit" | "it" => styled(self, Style::Italic),
            "mathbb" | "mathcal" | "mathfrak" => {
                let inner = self.styled_argument(Style::Roman);
                Some(Atom::Group(
                    inner.into_iter().map(|a| letterlike(a, &name)).collect(),
                ))
            }
            "left" | "right" | "big" | "Big" | "bigg" | "Bigg" | "displaystyle" | "textstyle"
            | "limits" | "nolimits" => None,
            "quad" => Some(Atom::Space(1.0)),
            "qquad" => Some(Atom::Space(2.0)),
            "sum" => op("\u{2211}", bold),
            "prod" => op("\u{220F}", bold),
            "coprod" => op("\u{2210}", bold),
            "int" => op("\u{222B}", bold),
            "iint" => op("\u{222C}", bold),
            "oint" => op("\u{222E}", bold),
            "bigcup" => op("\u{22C3}", bold),
            "bigcap" => op("\u{22C2}", bold),
            "sin" | "cos" | "tan" | "cot" | "sec" | "csc" | "sinh" | "cosh" | "tanh" | "arcsin"
            | "arccos" | "arctan" | "log" | "ln" | "lg" | "exp" | "max" | "min" | "sup" | "inf"
            | "lim" | "det" | "arg" | "deg" | "dim" | "gcd" | "ker" | "Pr" | "mod" => {
                sym(&name, Class::Op, false)
            }
            _ => {
                let (text, class, italic) = symbol(&name)?;
                sym(
                    text,
                    class,
                    italic && style != Style::Roman && style != Style::Bold,
                )
            }
        }
    }

    fn styled_argument(&mut self, s: Style) -> Vec<Atom> {
        while self.peek() == Some(' ') {
            self.bump();
        }
        if self.peek() == Some('{') {
            self.bump();
            self.list_styled(Some('}'), s)
        } else {
            self.atom(s).into_iter().collect()
        }
    }
}

/// Double-struck, calligraphic or fraktur letters (`\mathbb{R}`).
fn letterlike(a: Atom, style: &str) -> Atom {
    let Atom::Sym {
        text,
        class,
        bold,
        scale,
        ..
    } = a
    else {
        return a;
    };
    let map = |c: char| -> char {
        let special = match (style, c) {
            ("mathbb", 'C') => Some('ℂ'),
            ("mathbb", 'H') => Some('ℍ'),
            ("mathbb", 'N') => Some('ℕ'),
            ("mathbb", 'P') => Some('ℙ'),
            ("mathbb", 'Q') => Some('ℚ'),
            ("mathbb", 'R') => Some('ℝ'),
            ("mathbb", 'Z') => Some('ℤ'),
            ("mathcal", 'B') => Some('ℬ'),
            ("mathcal", 'E') => Some('ℰ'),
            ("mathcal", 'F') => Some('ℱ'),
            ("mathcal", 'H') => Some('ℋ'),
            ("mathcal", 'I') => Some('ℐ'),
            ("mathcal", 'L') => Some('ℒ'),
            ("mathcal", 'M') => Some('ℳ'),
            ("mathcal", 'R') => Some('ℛ'),
            ("mathfrak", 'C') => Some('ℭ'),
            ("mathfrak", 'H') => Some('ℌ'),
            ("mathfrak", 'I') => Some('ℑ'),
            ("mathfrak", 'R') => Some('ℜ'),
            ("mathfrak", 'Z') => Some('ℨ'),
            _ => None,
        };
        if let Some(s) = special {
            return s;
        }
        let base = match style {
            "mathbb" => 0x1D538,
            "mathcal" => 0x1D49C,
            _ => 0x1D504,
        };
        let code = match c {
            'A'..='Z' => base + (c as u32 - 'A' as u32),
            'a'..='z' => base + 26 + (c as u32 - 'a' as u32),
            '0'..='9' if style == "mathbb" => 0x1D7D8 + (c as u32 - '0' as u32),
            _ => return c,
        };
        char::from_u32(code).unwrap_or(c)
    };
    Atom::Sym {
        text: text.chars().map(map).collect(),
        class,
        italic: false,
        bold,
        scale,
    }
}

fn op(text: &str, bold: Option<bool>) -> Option<Atom> {
    Some(Atom::Sym {
        text: text.to_string(),
        class: Class::Op,
        italic: false,
        bold,
        scale: 1.2,
    })
}

/// Named symbols: text, class, italic.
fn symbol(name: &str) -> Option<(&'static str, Class, bool)> {
    use Class::*;
    let greek = |s| Some((s, Ord, true));
    let up = |s| Some((s, Ord, false));
    let bin = |s| Some((s, Bin, false));
    let rel = |s| Some((s, Rel, false));
    match name {
        "alpha" => greek("α"),
        "beta" => greek("β"),
        "gamma" => greek("γ"),
        "delta" => greek("δ"),
        "epsilon" => greek("ϵ"),
        "varepsilon" => greek("ε"),
        "zeta" => greek("ζ"),
        "eta" => greek("η"),
        "theta" => greek("θ"),
        "vartheta" => greek("ϑ"),
        "iota" => greek("ι"),
        "kappa" => greek("κ"),
        "lambda" => greek("λ"),
        "mu" => greek("μ"),
        "nu" => greek("ν"),
        "xi" => greek("ξ"),
        "omicron" => greek("ο"),
        "pi" => greek("π"),
        "varpi" => greek("ϖ"),
        "rho" => greek("ρ"),
        "varrho" => greek("ϱ"),
        "sigma" => greek("σ"),
        "varsigma" => greek("ς"),
        "tau" => greek("τ"),
        "upsilon" => greek("υ"),
        "phi" => greek("ϕ"),
        "varphi" => greek("φ"),
        "chi" => greek("χ"),
        "psi" => greek("ψ"),
        "omega" => greek("ω"),
        "Gamma" => up("Γ"),
        "Delta" => up("Δ"),
        "Theta" => up("Θ"),
        "Lambda" => up("Λ"),
        "Xi" => up("Ξ"),
        "Pi" => up("Π"),
        "Sigma" => up("Σ"),
        "Upsilon" => up("Υ"),
        "Phi" => up("Φ"),
        "Psi" => up("Ψ"),
        "Omega" => up("Ω"),
        "pm" => bin("±"),
        "mp" => bin("∓"),
        "times" => bin("×"),
        "div" => bin("÷"),
        "cdot" => bin("⋅"),
        "ast" => bin("∗"),
        "star" => bin("⋆"),
        "circ" => bin("∘"),
        "bullet" => bin("∙"),
        "oplus" => bin("⊕"),
        "otimes" => bin("⊗"),
        "cup" => bin("∪"),
        "cap" => bin("∩"),
        "setminus" => bin("∖"),
        "wedge" | "land" => bin("∧"),
        "vee" | "lor" => bin("∨"),
        "leq" | "le" => rel("≤"),
        "geq" | "ge" => rel("≥"),
        "neq" | "ne" => rel("≠"),
        "ll" => rel("≪"),
        "gg" => rel("≫"),
        "approx" => rel("≈"),
        "equiv" => rel("≡"),
        "sim" => rel("∼"),
        "simeq" => rel("≃"),
        "cong" => rel("≅"),
        "propto" => rel("∝"),
        "in" => rel("∈"),
        "notin" => rel("∉"),
        "ni" => rel("∋"),
        "subset" => rel("⊂"),
        "supset" => rel("⊃"),
        "subseteq" => rel("⊆"),
        "supseteq" => rel("⊇"),
        "to" | "rightarrow" => rel("→"),
        "leftarrow" | "gets" => rel("←"),
        "leftrightarrow" => rel("↔"),
        "Rightarrow" | "implies" => rel("⇒"),
        "Leftarrow" => rel("⇐"),
        "Leftrightarrow" | "iff" => rel("⇔"),
        "mapsto" => rel("↦"),
        "uparrow" => rel("↑"),
        "downarrow" => rel("↓"),
        "perp" => rel("⊥"),
        "parallel" => rel("∥"),
        "mid" => rel("∣"),
        "infty" => up("∞"),
        "partial" => up("∂"),
        "nabla" => up("∇"),
        "forall" => up("∀"),
        "exists" => up("∃"),
        "neg" | "lnot" => up("¬"),
        "emptyset" | "varnothing" => up("∅"),
        "degree" => up("°"),
        "angle" => up("∠"),
        "prime" => up("′"),
        "hbar" => up("ℏ"),
        "ell" => up("ℓ"),
        "Re" => up("ℜ"),
        "Im" => up("ℑ"),
        "aleph" => up("ℵ"),
        "ldots" | "dots" => up("…"),
        "cdots" => up("⋯"),
        "vdots" => up("⋮"),
        "ddots" => up("⋱"),
        "langle" => Some(("⟨", Open, false)),
        "rangle" => Some(("⟩", Close, false)),
        "lfloor" => Some(("⌊", Open, false)),
        "rfloor" => Some(("⌋", Close, false)),
        "lceil" => Some(("⌈", Open, false)),
        "rceil" => Some(("⌉", Close, false)),
        "lbrace" => Some(("{", Open, false)),
        "rbrace" => Some(("}", Close, false)),
        "vert" => up("|"),
        "Vert" => up("‖"),
        "backslash" => up("\\"),
        "percent" => up("%"),
        "dagger" => up("†"),
        "checkmark" => up("✓"),
        _ => None,
    }
}

/// Space (in em) between atoms of classes `a` and `b` (TeX's table for
/// text style, simplified).
fn space(a: Class, b: Class) -> f64 {
    use Class::*;
    let thin = 3.0 / 18.0;
    let med = 4.0 / 18.0;
    let thick = 5.0 / 18.0;
    match (a, b) {
        (Bin, _) | (_, Bin) => med,
        (Rel, Rel) => 0.0,
        (Rel, _) | (_, Rel) => thick,
        (Op, Ord) | (Ord, Op) | (Op, Op) | (Close, Op) => thin,
        (Punct, _) => thin,
        _ => 0.0,
    }
}

/// Lays out a list of atoms.
/// `tight`: script style (scripts, fractions) without operator spacing.
fn math_list(fonts: &FontSet, atoms: &[Atom], size: f64, bold: bool, tight: bool) -> Layout {
    // A binary operator without an operand on its left (or before a
    // relation) is unary (e.g. `-x`): no spaces around it.
    let mut classes: Vec<Class> = atoms.iter().map(Atom::class).collect();
    for i in 0..classes.len() {
        if classes[i] == Class::Bin {
            let prev = i.checked_sub(1).map(|j| classes[j]);
            let next = classes.get(i + 1).copied();
            if matches!(
                prev,
                None | Some(Class::Bin | Class::Rel | Class::Open | Class::Punct | Class::Op)
            ) || matches!(next, Some(Class::Rel | Class::Close | Class::Punct) | None)
            {
                classes[i] = Class::Ord;
            }
        }
    }
    let mut out = Layout::default();
    let mut pen = 0.0;
    let mut prev: Option<Class> = None;
    let mut prev_italic = false;
    for (atom, &class) in atoms.iter().zip(&classes) {
        if let Atom::Space(em) = atom {
            pen += em * size;
            continue;
        }
        if let (Some(p), false) = (prev, tight) {
            pen += space(p, class) * size;
        }
        let m = atom_layout(fonts, atom, size, bold, tight);
        // Italic correction before upright text: the slanted glyph reaches
        // past its advance.
        if prev_italic && m.runs.first().is_some_and(|r| !r.italic) {
            pen += size * 0.05;
        }
        prev_italic = m.runs.last().is_some_and(|r| r.italic) && m.lines.is_empty();
        let w = m.width;
        out.ascent = out.ascent.max(m.ascent);
        out.descent = out.descent.max(m.descent);
        out.append_raw(m, pen, 0.0);
        pen += w;
        prev = Some(class);
    }
    out.width = pen;
    merge_runs(fonts, &mut out);
    out
}

/// Joins runs that continue each other into one (kerning, fewer nodes).
fn merge_runs(fonts: &FontSet, l: &mut Layout) {
    let mut merged: Vec<Run> = Vec::with_capacity(l.runs.len());
    for r in l.runs.drain(..) {
        if let Some(last) = merged.last_mut() {
            if last.y == r.y
                && last.size == r.size
                && last.italic == r.italic
                && last.bold == r.bold
                && (last.x + fonts.width_plain(&last.text, last.size) - r.x).abs() < 1e-6
            {
                last.text.push_str(&r.text);
                continue;
            }
        }
        merged.push(r);
    }
    l.runs = merged;
}

fn run_layout(fonts: &FontSet, text: &str, size: f64, italic: bool, bold: bool) -> Layout {
    let mut b = Builder {
        fonts,
        out: Layout::default(),
        pen: 0.0,
    };
    b.text(text, size, italic, bold, 0.0);
    // Math boxes use the ink extent of the glyphs (accents, scripts and
    // fractions sit close to the letters).
    if let Some((_, _, top, bottom)) = fonts.ink(text, size) {
        b.out.ascent = (-top).max(0.0);
        b.out.descent = bottom.max(0.0);
    }
    b.out.width = b.pen;
    b.out
}

fn script_size(size: f64) -> f64 {
    (size * 0.7).max(5.0)
}

fn atom_layout(fonts: &FontSet, atom: &Atom, size: f64, bold: bool, tight: bool) -> Layout {
    match atom {
        Atom::Sym {
            text,
            italic,
            bold: b,
            scale,
            class,
        } => {
            let s = size * scale;
            let mut l = run_layout(fonts, text, s, *italic, b.unwrap_or(bold));
            if *scale != 1.0 && *class == Class::Op {
                // Large operators sit centred on the math axis.
                let shift = (s - size) * 0.3;
                l.shift(0.0, shift);
                l.ascent -= shift;
                l.descent += shift;
            }
            l
        }
        Atom::Group(atoms) => math_list(fonts, atoms, size, bold, tight),
        Atom::Space(em) => Layout {
            width: em * size,
            ..Layout::default()
        },
        Atom::Scripts { base, sup, sub } => {
            let b = atom_layout(fonts, base, size, bold, tight);
            let ss = script_size(size);
            let sup = sup.as_ref().map(|a| atom_layout(fonts, a, ss, bold, true));
            let sub = sub.as_ref().map(|a| atom_layout(fonts, a, ss, bold, true));
            // Superscripts of slanted letters start a bit further right.
            let slant = if b.runs.last().is_some_and(|r| r.italic) && b.lines.is_empty() {
                size * 0.06
            } else {
                0.0
            };
            let mut up = sup
                .as_ref()
                .map_or(0.0, |s| (size * 0.38).max(b.ascent - s.ascent * 0.6));
            let mut down = sub.as_ref().map_or(0.0, |s| {
                (size * 0.2).max(b.descent + s.ascent * 0.5 - size * 0.25)
            });
            if let (Some(p), Some(q)) = (&sup, &sub) {
                // Keep a gap between the superscript and the subscript.
                let gap = (down - q.ascent) - (-up + p.descent);
                if gap < size * 0.12 {
                    let extra = size * 0.12 - gap;
                    up += extra / 2.0;
                    down += extra / 2.0;
                }
            }
            let x = b.width;
            let mut out = Layout {
                ascent: b.ascent,
                descent: b.descent,
                ..Layout::default()
            };
            out.append_raw(b, 0.0, 0.0);
            let mut w = 0.0f64;
            if let Some(p) = sup {
                out.ascent = out.ascent.max(up + p.ascent);
                w = w.max(p.width + slant);
                out.append_raw(p, x + slant, -up);
            }
            if let Some(q) = sub {
                out.descent = out.descent.max(down + q.descent);
                // Subscripts tuck under italic overhangs.
                w = w.max(q.width);
                out.append_raw(q, x - size * 0.03, down);
            }
            out.width = x + w + size * 0.04;
            out
        }
        Atom::Frac(a, b) => {
            let fs = (size * 0.8).max(5.0);
            let num = atom_layout(fonts, a, fs, bold, true);
            let den = atom_layout(fonts, b, fs, bold, true);
            let t = (size * 0.055).max(0.8);
            let gap = size * 0.12;
            let axis = size * 0.27;
            let pad = size * 0.12;
            let w = num.width.max(den.width) + 2.0 * pad;
            let ny = -axis - t / 2.0 - gap - num.descent;
            let dy = -axis + t / 2.0 + gap + den.ascent;
            let mut out = Layout {
                width: w,
                ascent: axis + t / 2.0 + gap + num.descent + num.ascent,
                descent: (-axis + t / 2.0 + gap + den.ascent + den.descent).max(0.0),
                ..Layout::default()
            };
            out.lines
                .push((vec![(pad * 0.5, -axis), (w - pad * 0.5, -axis)], t));
            let (nw, dw) = (num.width, den.width);
            out.append_raw(num, (w - nw) / 2.0, ny);
            out.append_raw(den, (w - dw) / 2.0, dy);
            out
        }
        Atom::Sqrt(a) => {
            let c = atom_layout(fonts, a, size, bold, tight);
            let t = (size * 0.055).max(0.8);
            let pad = size * 0.1;
            let top = c.ascent.max(size * 0.7) + pad;
            let bottom = c.descent.max(size * 0.05);
            let rw = size * 0.55;
            let cw = c.width;
            let mut out = Layout {
                width: rw + cw + pad * 1.5,
                ascent: top + t,
                descent: bottom + t / 2.0,
                ..Layout::default()
            };
            out.lines.push((
                vec![
                    (size * 0.05, -top * 0.42),
                    (size * 0.17, -top * 0.5),
                    (size * 0.32, bottom),
                    (rw, -top),
                    (rw + cw + pad * 1.2, -top),
                ],
                t,
            ));
            out.append_raw(c, rw + pad * 0.4, 0.0);
            out
        }
        Atom::Over(a) => {
            let c = atom_layout(fonts, a, size, bold, tight);
            let t = (size * 0.05).max(0.8);
            let y = -(c.ascent + size * 0.1);
            let mut out = Layout {
                width: c.width,
                ascent: -y + t,
                descent: c.descent,
                ..Layout::default()
            };
            out.lines
                .push((vec![(size * 0.05, y), (c.width - size * 0.02, y)], t));
            out.append_raw(c, 0.0, 0.0);
            out
        }
        Atom::Accent(a, ch) => {
            let c = atom_layout(fonts, a, size, bold, tight);
            let s = if *ch == '\u{2192}' { size * 0.75 } else { size };
            let acc = ch.to_string();
            let (ax0, ax1, atop, abottom) =
                fonts.ink(&acc, s).unwrap_or((0.0, s * 0.5, -s, -s * 0.6));
            // Centre the accent over the content's ink; on slanted letters
            // the top is further right.
            let (cx0, cx1) = match a.as_ref() {
                Atom::Sym { text, .. } => fonts
                    .ink(text, size)
                    .map_or((0.0, c.width), |(x0, x1, _, _)| (x0, x1)),
                _ => (0.0, c.width),
            };
            let slant = if c.runs.len() == 1 && c.runs[0].italic {
                0.21 * c.ascent * 0.8
            } else {
                0.0
            };
            let centre = (cx0 + cx1) / 2.0 + slant;
            let gap = size * 0.08;
            let y = -c.ascent - gap - abottom;
            let mut out = Layout {
                width: c.width,
                ascent: c.ascent + gap + (abottom - atop),
                descent: c.descent,
                ..Layout::default()
            };
            out.runs.push(Run {
                x: centre - (ax0 + ax1) / 2.0,
                y,
                size: s,
                text: acc,
                italic: false,
                bold,
            });
            out.append_raw(c, 0.0, 0.0);
            out
        }
    }
}

/// Replaces text nodes containing math by groups of plain text runs and
/// lines (recursively).
pub(crate) fn expand(nodes: &mut [Node], fonts: &FontSet) {
    for n in nodes {
        match n {
            Node::Text(t) if has_math(&t.text) => {
                *n = text_node(t, fonts);
            }
            Node::Group { children, .. } => expand(children, fonts),
            _ => {}
        }
    }
}

fn text_node(t: &TextNode, fonts: &FontSet) -> Node {
    let l = layout(fonts, &t.text, t.size, t.bold);
    let x0 = match t.anchor {
        Anchor::Start => 0.0,
        Anchor::Middle => -l.width / 2.0,
        Anchor::End => -l.width,
    };
    let s = t.size;
    let extra_up = (l.ascent - s * 0.76).max(0.0);
    let extra_down = (l.descent - s * 0.24).max(0.0);
    let y0 = match t.baseline {
        Baseline::Alphabetic => 0.0,
        Baseline::Top => s * 0.76 + extra_up,
        Baseline::Middle => s * 0.35 + (extra_up - extra_down) / 2.0,
        Baseline::Bottom => -s * 0.24 - extra_down,
    };
    let (sin, cos) = t.rotate.to_radians().sin_cos();
    let place = |x: f64, y: f64| {
        let (dx, dy) = (x + x0, y + y0);
        (t.x + dx * cos - dy * sin, t.y + dx * sin + dy * cos)
    };
    let mut children = Vec::with_capacity(l.runs.len() + l.lines.len());
    for r in l.runs {
        let (x, y) = place(r.x, r.y);
        children.push(Node::Text(TextNode {
            x,
            y,
            text: r.text,
            size: r.size,
            color: t.color,
            anchor: Anchor::Start,
            baseline: Baseline::Alphabetic,
            rotate: t.rotate,
            bold: r.bold,
            italic: r.italic,
        }));
    }
    for (pts, w) in l.lines {
        let mut cmds = Vec::with_capacity(pts.len());
        for (i, (x, y)) in pts.into_iter().enumerate() {
            let (x, y) = place(x, y);
            cmds.push(if i == 0 {
                PathCmd::MoveTo(x, y)
            } else {
                PathCmd::LineTo(x, y)
            });
        }
        children.push(Node::Path {
            cmds,
            fill: None,
            stroke: Some(Stroke {
                color: t.color,
                width: w,
                dash: Vec::new(),
                cap: LineCap::Butt,
                join: LineJoin::Miter,
            }),
        });
    }
    Node::Group {
        children,
        clip: None,
        opacity: 1.0,
        tooltip: None,
        id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::color::Color;

    fn fonts() -> std::sync::Arc<FontSet> {
        FontSet::resolve(None)
    }

    fn texts(l: &Layout) -> Vec<(&str, bool)> {
        l.runs.iter().map(|r| (r.text.as_str(), r.italic)).collect()
    }

    #[test]
    fn detects_math() {
        assert!(has_math("energy $E = mc^2$"));
        assert!(!has_math("costs $5"));
        assert!(!has_math(r"\$5 and \$6"));
        assert!(!has_math("plain"));
    }

    #[test]
    fn plain_text_is_one_run() {
        let f = fonts();
        let l = layout(&f, "hello world", 12.0, false);
        assert_eq!(texts(&l), vec![("hello world", false)]);
        assert!((l.width - f.width_plain("hello world", 12.0)).abs() < 1e-9);
        let l = layout(&f, r"costs \$5", 12.0, false);
        assert_eq!(texts(&l), vec![("costs $5", false)]);
    }

    #[test]
    fn superscript_is_smaller_and_raised() {
        let f = fonts();
        let l = layout(&f, "$E = mc^2$", 20.0, false);
        let two = l.runs.iter().find(|r| r.text == "2").unwrap();
        assert!(two.size < 20.0 && two.y < -5.0, "{two:?}");
        let e = l.runs.iter().find(|r| r.text.starts_with('E')).unwrap();
        assert!(e.italic);
        // Spaces around the relation.
        let eq = l.runs.iter().find(|r| r.text.contains('=')).unwrap();
        assert!(eq.x > f.width_plain("E", 20.0) + 2.0, "{:?}", l.runs);
    }

    #[test]
    fn greek_functions_and_digits() {
        let f = fonts();
        let l = layout(
            &f,
            r"$\alpha + \sin\theta \leq 10.5\,\mathrm{m}$",
            12.0,
            false,
        );
        let t = texts(&l);
        assert!(t.contains(&("α", true)), "{t:?}");
        assert!(t.iter().any(|(s, i)| s.contains("sin") && !i), "{t:?}");
        assert!(t.iter().any(|(s, _)| s.contains("10.5")), "{t:?}");
        assert!(t.contains(&("m", false)), "{t:?}");
        assert!(t.iter().any(|(s, _)| s.contains('≤')), "{t:?}");
    }

    #[test]
    fn fraction_and_root_draw_lines() {
        let f = fonts();
        let l = layout(&f, r"$\frac{a+b}{2}$", 20.0, false);
        assert_eq!(l.lines.len(), 1);
        let num = l.runs.iter().find(|r| r.text.contains('a')).unwrap();
        let den = l.runs.iter().find(|r| r.text == "2").unwrap();
        assert!(num.y < 0.0 && den.y > num.y + 10.0, "{:?}", l.runs);
        assert!(l.ascent > 20.0 * 0.76);
        let l = layout(&f, r"$\sqrt{x^2+1}$", 20.0, false);
        assert_eq!(l.lines.len(), 1);
        assert_eq!(l.lines[0].0.len(), 5);
    }

    #[test]
    fn unary_minus_has_no_spaces() {
        let f = fonts();
        let l = layout(&f, "$-x$", 20.0, false);
        let w = l.width;
        let l2 = layout(&f, "$a-x$", 20.0, false);
        assert!(l2.width - f.width_plain("a", 20.0) > w + 5.0);
    }

    #[test]
    fn expands_rotated_text() {
        let f = fonts();
        let mut nodes = vec![Node::Text(TextNode {
            x: 100.0,
            y: 100.0,
            text: "$x^2$".into(),
            size: 20.0,
            color: Color::BLACK,
            anchor: Anchor::Middle,
            baseline: Baseline::Middle,
            rotate: -90.0,
            bold: false,
            italic: false,
        })];
        expand(&mut nodes, &f);
        let Node::Group { children, .. } = &nodes[0] else {
            panic!()
        };
        assert_eq!(children.len(), 2);
        let Node::Text(x) = &children[0] else {
            panic!()
        };
        let Node::Text(two) = &children[1] else {
            panic!()
        };
        // Rotated by -90°: "x" is below the anchor, the exponent above it
        // (further along the text direction, which points up).
        assert!(x.y > two.y, "{x:?} {two:?}");
        assert_eq!(x.rotate, -90.0);
    }
}
