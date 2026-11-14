//! Accessibility: text alternatives for charts and color vision checks.
//!
//! - SVG, PDF and HTML output carry an accessible name (the figure title)
//!   and a description of what the chart shows ([`Figure::alt_text`], or
//!   your own with [`Figure::description`]), read by screen readers.
//! - [`Cvd`] simulates color vision deficiencies ([`Color::simulate`],
//!   [`Pixels::simulate`]) and [`Figure::color_warnings`] lists series whose
//!   colors are hard to tell apart for people with one of them.

use super::color::Color;
use super::figure::Figure;
use super::output::Pixels;
use super::scene::Scene;
use super::series::{short, LegendSymbol};

/// A color vision deficiency (full severity; Machado, Oliveira and
/// Fernandes 2009, achromatopsia as luminance).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cvd {
    /// No red cones (about 1 % of men).
    Protanopia,
    /// No green cones (the most common, about 1 % of men; more have a
    /// milder form).
    Deuteranopia,
    /// No blue cones (rare).
    Tritanopia,
    /// No color vision (grayscale).
    Achromatopsia,
}

impl Cvd {
    /// All kinds.
    pub const ALL: [Cvd; 4] = [
        Cvd::Protanopia,
        Cvd::Deuteranopia,
        Cvd::Tritanopia,
        Cvd::Achromatopsia,
    ];

    fn matrix(self) -> [[f32; 3]; 3] {
        match self {
            Cvd::Protanopia => [
                [0.152_286, 1.052_583, -0.204_868],
                [0.114_503, 0.786_281, 0.099_216],
                [-0.003_882, -0.048_116, 1.051_998],
            ],
            Cvd::Deuteranopia => [
                [0.367_322, 0.860_646, -0.227_968],
                [0.280_085, 0.672_501, 0.047_413],
                [-0.011_820, 0.042_940, 0.968_881],
            ],
            Cvd::Tritanopia => [
                [1.255_528, -0.076_749, -0.178_779],
                [-0.078_411, 0.930_809, 0.147_602],
                [0.004_733, 0.691_367, 0.303_900],
            ],
            Cvd::Achromatopsia => [[0.2126, 0.7152, 0.0722]; 3],
        }
    }
}

fn to_linear(c: u8) -> f32 {
    let v = c as f32 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

fn apply(m: &[[f32; 3]; 3], rgb: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * rgb[0] + m[0][1] * rgb[1] + m[0][2] * rgb[2],
        m[1][0] * rgb[0] + m[1][1] * rgb[1] + m[1][2] * rgb[2],
        m[2][0] * rgb[0] + m[2][1] * rgb[1] + m[2][2] * rgb[2],
    ]
}

impl Color {
    /// How the color looks with a color vision deficiency.
    pub fn simulate(self, cvd: Cvd) -> Color {
        let rgb = apply(
            &cvd.matrix(),
            [to_linear(self.r), to_linear(self.g), to_linear(self.b)],
        );
        Color {
            r: to_srgb(rgb[0]),
            g: to_srgb(rgb[1]),
            b: to_srgb(rgb[2]),
            a: self.a,
        }
    }

    /// OKLab coordinates (perceptually uniform lightness, a, b).
    pub fn oklab(self) -> [f32; 3] {
        let (r, g, b) = (to_linear(self.r), to_linear(self.g), to_linear(self.b));
        let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
        let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
        let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
        [
            0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
            1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
            0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
        ]
    }

    /// Perceptual difference to `other` (OKLab distance; about 0.02 is
    /// just noticeable, series colors should differ by 0.08 or more).
    pub fn difference(self, other: Color) -> f32 {
        let (a, b) = (self.oklab(), other.oklab());
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }
}

impl Pixels {
    /// Recolors the image as seen with a color vision deficiency (to check
    /// a chart: render, simulate, save).
    pub fn simulate(&mut self, cvd: Cvd) {
        let lin: Vec<f32> = (0..=255u8).map(to_linear).collect();
        // Linear → sRGB through a table (4096 steps).
        let out: Vec<u8> = (0..4096).map(|i| to_srgb(i as f32 / 4095.0)).collect();
        let m = cvd.matrix();
        for p in self.rgba.as_chunks_mut::<4>().0 {
            let v = apply(
                &m,
                [lin[p[0] as usize], lin[p[1] as usize], lin[p[2] as usize]],
            );
            let q = |x: f32| out[(x.clamp(0.0, 1.0) * 4095.0 + 0.5) as usize];
            p[0] = q(v[0]);
            p[1] = q(v[1]);
            p[2] = q(v[2]);
        }
    }
}

/// Smallest OKLab distance at which series colors count as distinct.
const MIN_DIFFERENCE: f32 = 0.06;

impl Figure {
    /// Sets the accessible description of the chart (SVG `<desc>`, HTML),
    /// replacing the generated [`Figure::alt_text`].
    pub fn description(&mut self, text: impl Into<String>) -> &mut Self {
        self.description = Some(text.into());
        self
    }

    /// A text alternative describing the chart: its title, the plots with
    /// their axes and ranges, and the series (used as the SVG/HTML
    /// description unless [`Figure::description`] is set).
    pub fn alt_text(&mut self) -> String {
        let (_, layouts) = self.scene_and_layouts();
        self.alt_text_for(&layouts)
    }

    fn alt_text_for(&self, layouts: &[super::axes::Layout]) -> String {
        let mut parts = Vec::new();
        if let Some(t) = self.title_text() {
            parts.push(format!("{t}."));
        }
        let n = layouts.len();
        for (i, lay) in layouts.iter().enumerate() {
            let Some(ax) = self.axes_at(i) else { continue };
            let mut kinds: Vec<String> = Vec::new();
            let mut names = Vec::new();
            for (k, s) in ax.series.iter().enumerate() {
                if !ax.is_visible(k) {
                    continue;
                }
                let kind = s.kind().to_string();
                if kind == "annotation" {
                    continue;
                }
                if !kinds.contains(&kind) {
                    kinds.push(kind);
                }
                let mut d = s.name().unwrap_or_else(|| format!("series {}", k + 1));
                if let Some(e) = s.extent() {
                    if e.y_min.is_finite() && e.y_max.is_finite() {
                        d.push_str(&format!(" ({} to {})", short(e.y_min), short(e.y_max)));
                    }
                }
                names.push(d);
            }
            if kinds.is_empty() {
                continue;
            }
            let mut p = String::new();
            if n > 1 {
                p.push_str(&format!("Plot {}: ", i + 1));
            }
            if let Some(t) = &ax.title {
                p.push_str(&format!("{t}, "));
            }
            p.push_str(&format!("{} chart", kinds.join(" and ")));
            if ax.kind == super::AxesKind::Cartesian && ax.projection.is_none() {
                let axis = |label: &Option<String>, along_y: bool, map: &super::axis::AxisMap| {
                    let name =
                        label
                            .clone()
                            .unwrap_or_else(|| if along_y { "y".into() } else { "x".into() });
                    format!(
                        "{name} from {} to {}",
                        ax.cursor_label(lay, along_y, map.min),
                        ax.cursor_label(lay, along_y, map.max)
                    )
                };
                p.push_str(&format!(
                    "; {}; {}",
                    axis(&ax.x.label, false, &lay.x),
                    axis(&ax.y.label, true, &lay.y)
                ));
            }
            p.push_str(&format!(". Series: {}.", names.join("; ")));
            parts.push(p);
        }
        parts.join(" ")
    }

    /// Pairs of series (in the same plot) whose colors are hard to tell
    /// apart, normally or with a color vision deficiency; empty when all
    /// are distinct. Use a palette such as `Palette::OkabeIto` or add
    /// markers/dashes when it is not.
    pub fn color_warnings(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for i in 0.. {
            let Some(ax) = self.axes_at(i) else { break };
            let entries: Vec<(String, Color)> = ax
                .series
                .iter()
                .flat_map(|s| s.legend())
                .map(|e| {
                    let c = match e.symbol {
                        LegendSymbol::Line { color, .. } => color,
                        LegendSymbol::Patch { fill, .. } => fill,
                        LegendSymbol::Marker { color, .. } => color,
                    };
                    (e.label, c)
                })
                .collect();
            for a in 0..entries.len() {
                for b in a + 1..entries.len() {
                    let (na, ca) = &entries[a];
                    let (nb, cb) = &entries[b];
                    let d = ca.difference(*cb);
                    if d < MIN_DIFFERENCE {
                        out.push(format!(
                            "\"{na}\" and \"{nb}\" have nearly the same color ({d:.3})"
                        ));
                        continue;
                    }
                    let bad: Vec<String> = Cvd::ALL
                        .iter()
                        .filter(|cvd| **cvd != Cvd::Achromatopsia)
                        .filter(|cvd| {
                            ca.simulate(**cvd).difference(cb.simulate(**cvd)) < MIN_DIFFERENCE
                        })
                        .map(|cvd| format!("{cvd:?}").to_lowercase())
                        .collect();
                    if !bad.is_empty() {
                        out.push(format!(
                            "\"{na}\" and \"{nb}\" look alike with {}",
                            bad.join(", ")
                        ));
                    }
                }
            }
        }
        out
    }

    /// The scene with its accessible title and description (SVG, PDF, HTML).
    pub(crate) fn accessible_scene(&mut self) -> Scene {
        let (mut scene, layouts) = self.scene_and_layouts();
        let description = match &self.description {
            Some(d) => d.clone(),
            None => self.alt_text_for(&layouts),
        };
        scene.title = Some(self.title_text().unwrap_or_else(|| "Chart".into()));
        scene.description = (!description.is_empty()).then_some(description);
        scene
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::Figure;

    #[test]
    fn simulation_and_differences() {
        let (red, green) = (Color::rgb(220, 50, 50), Color::rgb(60, 160, 60));
        assert!(red.difference(green) > 0.15);
        // Red and green get much closer for deuteranopes.
        let d = red
            .simulate(Cvd::Deuteranopia)
            .difference(green.simulate(Cvd::Deuteranopia));
        assert!(d < red.difference(green) * 0.6, "{d}");
        let g = Color::rgb(200, 30, 90).simulate(Cvd::Achromatopsia);
        assert!(g.r == g.g && g.g == g.b);
        assert_eq!(Color::WHITE.simulate(Cvd::Protanopia), Color::WHITE);
        let mut px = Pixels {
            width: 1,
            height: 1,
            rgba: vec![200, 30, 90, 255],
        };
        px.simulate(Cvd::Achromatopsia);
        assert!(px.rgba[0] == px.rgba[1] && px.rgba[1] == px.rgba[2]);
    }

    #[test]
    fn alt_text_and_warnings() {
        let mut fig = Figure::new(600, 400);
        fig.title("Sales");
        let ax = fig.axes();
        ax.line(vec![0.0, 1.0, 2.0], vec![3.0, 5.0, 4.0])
            .label("north")
            .color(Color::rgb(220, 50, 50));
        ax.line(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 6.0])
            .label("south")
            .color(Color::rgb(90, 140, 40));
        ax.x_label("quarter").y_label("units");
        let alt = fig.alt_text();
        assert!(alt.starts_with("Sales."), "{alt}");
        assert!(
            alt.contains("line chart")
                && alt.contains("quarter from")
                && alt.contains("north (3 to 5)"),
            "{alt}"
        );
        let svg = fig.to_svg();
        assert!(svg.contains(r#"role="img""#) && svg.contains("<desc id=\"dv-desc\">Sales."));
        let w = fig.color_warnings();
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("deuteranopia"), "{w:?}");
        fig.description("Two lines.");
        assert!(fig
            .to_svg()
            .contains("<desc id=\"dv-desc\">Two lines.</desc>"));
    }
}
