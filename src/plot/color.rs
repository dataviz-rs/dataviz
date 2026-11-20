//! Colors, categorical palettes and continuous colormaps.

use std::fmt;
use std::str::FromStr;

/// An sRGB color with alpha (`a` from `0.0` = transparent to `1.0` = opaque).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Opacity, `0.0..=1.0`.
    pub a: f32,
}

impl Color {
    /// Opaque color from RGB channels.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Color with alpha (`0.0..=1.0`).
    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Fully transparent.
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0.0);
    /// `#000000`
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    /// `#ffffff`
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    /// `#808080`
    pub const GRAY: Color = Color::rgb(128, 128, 128);
    /// `#d62728`
    pub const RED: Color = Color::rgb(214, 39, 40);
    /// `#2ca02c`
    pub const GREEN: Color = Color::rgb(44, 160, 44);
    /// `#1f77b4`
    pub const BLUE: Color = Color::rgb(31, 119, 180);
    /// `#ff7f0e`
    pub const ORANGE: Color = Color::rgb(255, 127, 14);
    /// `#9467bd`
    pub const PURPLE: Color = Color::rgb(148, 103, 189);

    /// Parses `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r,g,b)`, `rgba(r,g,b,a)`
    /// or a CSS color name (`"steelblue"`).
    pub fn parse(s: &str) -> Result<Self, ColorParseError> {
        s.parse()
    }

    /// The same color with a different opacity.
    pub fn with_alpha(self, a: f32) -> Self {
        Self {
            a: a.clamp(0.0, 1.0),
            ..self
        }
    }

    /// Linear interpolation between `self` (`t = 0`) and `other` (`t = 1`).
    pub fn mix(self, other: Color, t: f64) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t).round() as u8;
        Self {
            r: lerp(self.r, other.r),
            g: lerp(self.g, other.g),
            b: lerp(self.b, other.b),
            a: (self.a as f64 + (other.a as f64 - self.a as f64) * t) as f32,
        }
    }

    /// Mixes with white by `amount` (`0..1`).
    pub fn lighten(self, amount: f64) -> Self {
        self.mix(Color::WHITE.with_alpha(self.a), amount)
    }

    /// Mixes with black by `amount` (`0..1`).
    pub fn darken(self, amount: f64) -> Self {
        self.mix(Color::BLACK.with_alpha(self.a), amount)
    }

    /// Relative luminance (`0` black … `1` white).
    pub fn luminance(self) -> f64 {
        let lin = |c: u8| {
            let c = c as f64 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }

    /// Black or white, whichever is more readable on top of `self`.
    pub fn contrasting_text(self) -> Color {
        if self.luminance() > 0.4 {
            Color::BLACK
        } else {
            Color::WHITE
        }
    }

    /// `#rrggbb` (alpha is not included).
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub(crate) fn is_visible(self) -> bool {
        self.a > 0.0
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a >= 1.0 {
            write!(f, "{}", self.to_hex())
        } else {
            write!(f, "rgba({},{},{},{:.3})", self.r, self.g, self.b, self.a)
        }
    }
}

impl From<[u8; 3]> for Color {
    fn from(c: [u8; 3]) -> Self {
        Color::rgb(c[0], c[1], c[2])
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Color::rgb(r, g, b)
    }
}

impl From<[u8; 4]> for Color {
    fn from(c: [u8; 4]) -> Self {
        Color::rgba(c[0], c[1], c[2], c[3] as f32 / 255.0)
    }
}

/// Parses with [`Color::parse`]; an invalid string gives black (use
/// `Color::parse` to handle the error).
impl From<&str> for Color {
    fn from(s: &str) -> Self {
        s.parse().unwrap_or(Color::BLACK)
    }
}

/// Error returned by [`Color::parse`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorParseError(pub String);

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid color: {:?}", self.0)
    }
}

impl std::error::Error for ColorParseError {}

impl FromStr for Color {
    type Err = ColorParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ColorParseError(s.to_string());
        let t = s.trim().to_ascii_lowercase();
        if let Some(hex) = t.strip_prefix('#') {
            let digits: Vec<u8> = hex
                .chars()
                .map(|c| c.to_digit(16).map(|d| d as u8))
                .collect::<Option<_>>()
                .ok_or_else(err)?;
            return match digits.len() {
                3 => Ok(Color::rgb(digits[0] * 17, digits[1] * 17, digits[2] * 17)),
                6 | 8 => {
                    let byte = |i: usize| digits[i] * 16 + digits[i + 1];
                    let a = if digits.len() == 8 {
                        byte(6) as f32 / 255.0
                    } else {
                        1.0
                    };
                    Ok(Color::rgba(byte(0), byte(2), byte(4), a))
                }
                _ => Err(err()),
            };
        }
        let func = t
            .strip_prefix("rgba(")
            .or_else(|| t.strip_prefix("rgb("))
            .and_then(|r| r.strip_suffix(')'));
        if let Some(args) = func {
            let parts: Vec<&str> = args.split(',').map(str::trim).collect();
            if parts.len() != 3 && parts.len() != 4 {
                return Err(err());
            }
            let channel = |p: &str| {
                p.parse::<f64>()
                    .ok()
                    .map(|v| v.clamp(0.0, 255.0).round() as u8)
            };
            let a = match parts.get(3) {
                Some(p) => p.parse::<f32>().map_err(|_| err())?.clamp(0.0, 1.0),
                None => 1.0,
            };
            return Ok(Color::rgba(
                channel(parts[0]).ok_or_else(err)?,
                channel(parts[1]).ok_or_else(err)?,
                channel(parts[2]).ok_or_else(err)?,
                a,
            ));
        }
        if t == "transparent" || t == "none" {
            return Ok(Color::TRANSPARENT);
        }
        NAMED
            .iter()
            .find(|(name, _)| *name == t)
            .map(|(_, rgb)| Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, *rgb as u8))
            .ok_or_else(err)
    }
}

/// CSS color names.
const NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff),
    ("antiquewhite", 0xfaebd7),
    ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff),
    ("beige", 0xf5f5dc),
    ("bisque", 0xffe4c4),
    ("black", 0x000000),
    ("blanchedalmond", 0xffebcd),
    ("blue", 0x0000ff),
    ("blueviolet", 0x8a2be2),
    ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887),
    ("cadetblue", 0x5f9ea0),
    ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e),
    ("coral", 0xff7f50),
    ("cornflowerblue", 0x6495ed),
    ("cornsilk", 0xfff8dc),
    ("crimson", 0xdc143c),
    ("cyan", 0x00ffff),
    ("darkblue", 0x00008b),
    ("darkcyan", 0x008b8b),
    ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xa9a9a9),
    ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b),
    ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00),
    ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000),
    ("darksalmon", 0xe9967a),
    ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f),
    ("darkturquoise", 0x00ced1),
    ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493),
    ("deepskyblue", 0x00bfff),
    ("dimgray", 0x696969),
    ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222),
    ("floralwhite", 0xfffaf0),
    ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff),
    ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff),
    ("gold", 0xffd700),
    ("goldenrod", 0xdaa520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xadff2f),
    ("grey", 0x808080),
    ("honeydew", 0xf0fff0),
    ("hotpink", 0xff69b4),
    ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082),
    ("ivory", 0xfffff0),
    ("khaki", 0xf0e68c),
    ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00),
    ("lemonchiffon", 0xfffacd),
    ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080),
    ("lightcyan", 0xe0ffff),
    ("lightgoldenrodyellow", 0xfafad2),
    ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90),
    ("lightgrey", 0xd3d3d3),
    ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa),
    ("lightskyblue", 0x87cefa),
    ("lightslategray", 0x778899),
    ("lightsteelblue", 0xb0c4de),
    ("lightyellow", 0xffffe0),
    ("lime", 0x00ff00),
    ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6),
    ("magenta", 0xff00ff),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd),
    ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db),
    ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a),
    ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xf5fffa),
    ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead),
    ("navy", 0x000080),
    ("oldlace", 0xfdf5e6),
    ("olive", 0x808000),
    ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500),
    ("orangered", 0xff4500),
    ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa),
    ("palegreen", 0x98fb98),
    ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5),
    ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f),
    ("pink", 0xffc0cb),
    ("plum", 0xdda0dd),
    ("powderblue", 0xb0e0e6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xff0000),
    ("rosybrown", 0xbc8f8f),
    ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072),
    ("sandybrown", 0xf4a460),
    ("seagreen", 0x2e8b57),
    ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d),
    ("silver", 0xc0c0c0),
    ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd),
    ("slategray", 0x708090),
    ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4),
    ("tan", 0xd2b48c),
    ("teal", 0x008080),
    ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347),
    ("turquoise", 0x40e0d0),
    ("violet", 0xee82ee),
    ("wheat", 0xf5deb3),
    ("white", 0xffffff),
    ("whitesmoke", 0xf5f5f5),
    ("yellow", 0xffff00),
    ("yellowgreen", 0x9acd32),
];

fn hex(v: u32) -> Color {
    Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Categorical color palettes (color cycles).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Palette {
    /// matplotlib/D3 "category10".
    Tab10,
    /// Okabe–Ito, safe for color-vision deficiencies.
    OkabeIto,
    /// ColorBrewer Set2 (soft).
    Set2,
    /// ColorBrewer Dark2.
    Dark2,
    /// Tableau 20 (pairs of dark/light).
    Tab20,
    /// Bright colors for dark backgrounds.
    Neon,
    /// Muted, print-friendly tones.
    Muted,
}

impl Palette {
    /// The colors of the palette.
    pub fn colors(self) -> Vec<Color> {
        let v: &[u32] = match self {
            Palette::Tab10 => &[
                0x1f77b4, 0xff7f0e, 0x2ca02c, 0xd62728, 0x9467bd, 0x8c564b, 0xe377c2, 0x7f7f7f,
                0xbcbd22, 0x17becf,
            ],
            Palette::OkabeIto => &[
                0x0072b2, 0xe69f00, 0x009e73, 0xd55e00, 0xcc79a7, 0x56b4e9, 0xf0e442, 0x000000,
            ],
            Palette::Set2 => &[
                0x66c2a5, 0xfc8d62, 0x8da0cb, 0xe78ac3, 0xa6d854, 0xffd92f, 0xe5c494, 0xb3b3b3,
            ],
            Palette::Dark2 => &[
                0x1b9e77, 0xd95f02, 0x7570b3, 0xe7298a, 0x66a61e, 0xe6ab02, 0xa6761d, 0x666666,
            ],
            Palette::Tab20 => &[
                0x1f77b4, 0xaec7e8, 0xff7f0e, 0xffbb78, 0x2ca02c, 0x98df8a, 0xd62728, 0xff9896,
                0x9467bd, 0xc5b0d5, 0x8c564b, 0xc49c94, 0xe377c2, 0xf7b6d2, 0x7f7f7f, 0xc7c7c7,
                0xbcbd22, 0xdbdb8d, 0x17becf, 0x9edae5,
            ],
            Palette::Neon => &[
                0x4cc9f0, 0xf72585, 0x80ed99, 0xffd166, 0xb388ff, 0xff8c42, 0x00f5d4, 0xff5d8f,
            ],
            Palette::Muted => &[
                0x4878d0, 0xee854a, 0x6acc64, 0xd65f5f, 0x956cb4, 0x8c613c, 0xdc7ec0, 0x797979,
                0xd5bb67, 0x82c6e2,
            ],
        };
        v.iter().copied().map(hex).collect()
    }

    /// The `i`-th color, cycling.
    pub fn color(self, i: usize) -> Color {
        let c = self.colors();
        c[i % c.len()]
    }
}

/// Continuous colormaps for heatmaps, contours, scatter coloring, …
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Colormap {
    /// Perceptually uniform, blue → green → yellow (default).
    Viridis,
    /// Perceptually uniform, purple → orange → yellow.
    Plasma,
    /// Perceptually uniform, black → red → yellow.
    Inferno,
    /// Perceptually uniform, black → purple → cream.
    Magma,
    /// Color-vision-deficiency friendly, blue → yellow.
    Cividis,
    /// Improved rainbow.
    Turbo,
    /// Black → white.
    Gray,
    /// Diverging blue → white → red.
    Coolwarm,
    /// Diverging red → white → blue (ColorBrewer RdBu).
    RdBu,
    /// White → dark blue.
    Blues,
    /// White → dark red.
    Reds,
    /// White → dark green.
    Greens,
    /// Yellow → green → blue (ColorBrewer YlGnBu).
    YlGnBu,
    /// Diverging brown → white → teal.
    BrBG,
}

impl Colormap {
    fn stops(self) -> &'static [u32] {
        match self {
            Colormap::Viridis => &[
                0x440154, 0x482475, 0x414487, 0x355f8d, 0x2a788e, 0x21918c, 0x22a884, 0x44bf70,
                0x7ad151, 0xbddf26, 0xfde725,
            ],
            Colormap::Plasma => &[
                0x0d0887, 0x41049d, 0x6a00a8, 0x8f0da4, 0xb12a90, 0xcc4778, 0xe16462, 0xf2844b,
                0xfca636, 0xfcce25, 0xf0f921,
            ],
            Colormap::Inferno => &[
                0x000004, 0x160b39, 0x420a68, 0x6a176e, 0x932667, 0xbc3754, 0xdd513a, 0xf37819,
                0xfca50a, 0xf6d746, 0xfcffa4,
            ],
            Colormap::Magma => &[
                0x000004, 0x140e36, 0x3b0f70, 0x641a80, 0x8c2981, 0xb73779, 0xde4968, 0xf7705c,
                0xfe9f6d, 0xfecf92, 0xfcfdbf,
            ],
            Colormap::Cividis => &[
                0x00224e, 0x123570, 0x3b496c, 0x575d6d, 0x707173, 0x8a8678, 0xa59c74, 0xc3b369,
                0xe1cc55, 0xfee838,
            ],
            Colormap::Turbo => &[
                0x30123b, 0x4145ab, 0x4675ed, 0x39a2fc, 0x1bcfd4, 0x24eca6, 0x61fc6c, 0xa4fc3b,
                0xd1e834, 0xf3c63a, 0xfe9b2d, 0xf36315, 0xd93806, 0xb11901, 0x7a0402,
            ],
            Colormap::Gray => &[0x000000, 0xffffff],
            Colormap::Coolwarm => &[
                0x3b4cc0, 0x6788ee, 0x9abbff, 0xc9d7f0, 0xedd1c2, 0xf7a889, 0xe26952, 0xb40426,
            ],
            Colormap::RdBu => &[
                0x67001f, 0xb2182b, 0xd6604d, 0xf4a582, 0xfddbc7, 0xf7f7f7, 0xd1e5f0, 0x92c5de,
                0x4393c3, 0x2166ac, 0x053061,
            ],
            Colormap::Blues => &[
                0xf7fbff, 0xdeebf7, 0xc6dbef, 0x9ecae1, 0x6baed6, 0x4292c6, 0x2171b5, 0x08519c,
                0x08306b,
            ],
            Colormap::Reds => &[
                0xfff5f0, 0xfee0d2, 0xfcbba1, 0xfc9272, 0xfb6a4a, 0xef3b2c, 0xcb181d, 0xa50f15,
                0x67000d,
            ],
            Colormap::Greens => &[
                0xf7fcf5, 0xe5f5e0, 0xc7e9c0, 0xa1d99b, 0x74c476, 0x41ab5d, 0x238b45, 0x006d2c,
                0x00441b,
            ],
            Colormap::YlGnBu => &[
                0xffffd9, 0xedf8b1, 0xc7e9b4, 0x7fcdbb, 0x41b6c4, 0x1d91c0, 0x225ea8, 0x253494,
                0x081d58,
            ],
            Colormap::BrBG => &[
                0x543005, 0x8c510a, 0xbf812d, 0xdfc27d, 0xf6e8c3, 0xf5f5f5, 0xc7eae5, 0x80cdc1,
                0x35978f, 0x01665e, 0x003c30,
            ],
        }
    }

    /// Color at position `t` (`0..=1`, clamped; NaN gives the low end).
    pub fn at(self, t: f64) -> Color {
        let stops = self.stops();
        let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
        let pos = t * (stops.len() - 1) as f64;
        let i = (pos.floor() as usize).min(stops.len() - 2);
        hex(stops[i]).mix(hex(stops[i + 1]), pos - i as f64)
    }

    /// `n` evenly spaced colors.
    pub fn sample(self, n: usize) -> Vec<Color> {
        match n {
            0 => Vec::new(),
            1 => vec![self.at(0.5)],
            _ => (0..n).map(|i| self.at(i as f64 / (n - 1) as f64)).collect(),
        }
    }

    /// Every colormap, for galleries.
    pub const ALL: [Colormap; 14] = [
        Colormap::Viridis,
        Colormap::Plasma,
        Colormap::Inferno,
        Colormap::Magma,
        Colormap::Cividis,
        Colormap::Turbo,
        Colormap::Gray,
        Colormap::Coolwarm,
        Colormap::RdBu,
        Colormap::Blues,
        Colormap::Reds,
        Colormap::Greens,
        Colormap::YlGnBu,
        Colormap::BrBG,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing() {
        assert_eq!(Color::parse("#fff").unwrap(), Color::WHITE);
        assert_eq!(Color::parse("#1f77b4").unwrap(), Color::BLUE);
        assert_eq!(Color::parse("#ff000080").unwrap().a, 128.0 / 255.0);
        assert_eq!(Color::parse("rgb(1, 2, 3)").unwrap(), Color::rgb(1, 2, 3));
        assert_eq!(
            Color::parse("rgba(1,2,3,0.5)").unwrap(),
            Color::rgba(1, 2, 3, 0.5)
        );
        assert_eq!(Color::parse("SteelBlue").unwrap(), Color::rgb(70, 130, 180));
        assert!(Color::parse("#12").is_err());
        assert!(Color::parse("nope").is_err());
        assert_eq!(Color::from("nope"), Color::BLACK);
    }

    #[test]
    fn display_and_mix() {
        assert_eq!(Color::BLUE.to_string(), "#1f77b4");
        assert_eq!(Color::rgba(1, 2, 3, 0.5).to_string(), "rgba(1,2,3,0.500)");
        assert_eq!(
            Color::BLACK.mix(Color::WHITE, 0.5),
            Color::rgb(128, 128, 128)
        );
        assert_eq!(Color::WHITE.contrasting_text(), Color::BLACK);
        assert_eq!(Color::BLACK.contrasting_text(), Color::WHITE);
    }

    #[test]
    fn colormaps() {
        for cm in Colormap::ALL {
            assert_eq!(cm.at(0.0), hex(cm.stops()[0]));
            assert_eq!(cm.at(1.0), hex(*cm.stops().last().unwrap()));
            assert_eq!(cm.at(f64::NAN), cm.at(0.0));
            assert_eq!(cm.sample(5).len(), 5);
        }
        assert_eq!(Palette::Tab10.color(10), Palette::Tab10.color(0));
    }
}
