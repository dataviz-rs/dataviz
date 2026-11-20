//! Maps: world countries, projections, choropleths, points and lines on a
//! map, and GeoJSON shapes.
//!
//! ```
//! use dataviz::prelude::*;
//! use dataviz::plot::geo::Projection;
//! let mut fig = Figure::new(1000, 520);
//! let ax = fig.axes();
//! ax.map(Projection::Robinson);
//! // Values by ISO code (alpha-3 or alpha-2) or country name.
//! ax.choropleth([("DEU", 83.2), ("FRA", 68.0), ("TUR", 85.3), ("JPN", 124.5)])
//!     .colorbar("population [M]");
//! ax.geo_scatter([28.98, 2.35], [41.01, 48.86]).label("cities");
//! ```
//!
//! The built-in world map is Natural Earth's 1:110m countries (public
//! domain): [`world`] lists the countries with their ISO codes, continent,
//! population and GDP. Other shapes (regions, provinces) can be read from
//! GeoJSON with [`read_geojson`] and drawn with [`Axes::geo_shapes`].

use super::axes::Axes;
use super::axis::AxisMap;
use super::color::{Color, Colormap};
use super::output::Error;
use super::scene::{Node, PathCmd, Stroke};
use super::series::{short, ColorScale, Context, Extent, Hit, IntoValues, Rect, Series};
use std::any::Any;
use std::sync::{Arc, LazyLock};

/// A map projection: longitude/latitude in degrees → plane coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum Projection {
    /// Plate carrée: longitude and latitude as they are.
    Equirectangular,
    /// Conformal (shapes kept, areas grow towards the poles); latitudes are
    /// cut at ±85°.
    Mercator,
    /// Robinson: a compromise for world maps.
    #[default]
    Robinson,
    /// Natural Earth: a compromise for world maps with rounded corners.
    NaturalEarth,
    /// Equal Earth: equal-area (areas compare truly), pleasing shapes.
    EqualEarth,
    /// Mollweide: equal-area ellipse.
    Mollweide,
    /// The globe seen from space above (`lon`, `lat`): the far side is
    /// hidden. Turn it with [`Axes::globe_center`].
    Orthographic { lon: f64, lat: f64 },
    /// Albers equal-area conic with standard parallels `lat1`, `lat2`,
    /// centred on (`lon0`, `lat0`): the usual projection for maps of a
    /// country or continent (USA: -96, 37.5, 29.5, 45.5).
    Albers {
        lon0: f64,
        lat0: f64,
        lat1: f64,
        lat2: f64,
    },
    /// Lambert conformal conic (shapes kept; aviation, weather, Europe:
    /// 10, 52, 35, 65).
    LambertConformal {
        lon0: f64,
        lat0: f64,
        lat1: f64,
        lat2: f64,
    },
    /// Stereographic, centred on (`lon`, `lat`) (polar maps: lat ±90);
    /// conformal, the hemisphere around the centre.
    Stereographic { lon: f64, lat: f64 },
    /// Lambert azimuthal equal-area, centred on (`lon`, `lat`): the
    /// hemisphere around the centre with true areas (polar regions,
    /// continents).
    AzimuthalEqualArea { lon: f64, lat: f64 },
}

impl Projection {
    /// Albers for the contiguous United States.
    pub const USA: Projection = Projection::Albers {
        lon0: -96.0,
        lat0: 37.5,
        lat1: 29.5,
        lat2: 45.5,
    };
    /// Lambert conformal conic for Europe.
    pub const EUROPE: Projection = Projection::LambertConformal {
        lon0: 10.0,
        lat0: 52.0,
        lat1: 35.0,
        lat2: 65.0,
    };

    /// The centre of an azimuthal projection (globe, stereographic, …).
    fn center(self) -> Option<(f64, f64)> {
        match self {
            Projection::Orthographic { lon, lat }
            | Projection::Stereographic { lon, lat }
            | Projection::AzimuthalEqualArea { lon, lat } => Some((lon, lat)),
            _ => None,
        }
    }

    /// Plane position of a point of the visible hemisphere in view
    /// coordinates (azimuthal projections); the rim maps to radius GLOBE_R.
    fn plane(self, v: [f64; 3]) -> (f64, f64) {
        let k = match self {
            // 2 / (1 + z) at the rim is 2.
            Projection::Stereographic { .. } => 2.0 / (1.0 + v[2]).max(1e-9) / 2.0,
            // √(2 / (1 + z)) is √2 at the rim.
            Projection::AzimuthalEqualArea { .. } => {
                (2.0 / (1.0 + v[2]).max(1e-9)).sqrt() / std::f64::consts::SQRT_2
            }
            _ => 1.0,
        };
        (v[0] * k * GLOBE_R, v[1] * k * GLOBE_R)
    }

    /// The central meridian of a conic projection.
    fn conic(self) -> Option<f64> {
        match self {
            Projection::Albers { lon0, .. } | Projection::LambertConformal { lon0, .. } => {
                Some(lon0)
            }
            _ => None,
        }
    }
}

/// Radius of the orthographic globe in plane units (degree-like, as the
/// other projections).
const GLOBE_R: f64 = 90.0;

/// Robinson's table: (X, Y) at every 5° of latitude.
const ROBINSON: [(f64, f64); 19] = [
    (1.0000, 0.0000),
    (0.9986, 0.0620),
    (0.9954, 0.1240),
    (0.9900, 0.1860),
    (0.9822, 0.2480),
    (0.9730, 0.3100),
    (0.9600, 0.3720),
    (0.9427, 0.4340),
    (0.9216, 0.4958),
    (0.8962, 0.5571),
    (0.8679, 0.6176),
    (0.8350, 0.6769),
    (0.7986, 0.7346),
    (0.7597, 0.7903),
    (0.7186, 0.8435),
    (0.6732, 0.8936),
    (0.6213, 0.9394),
    (0.5722, 0.9761),
    (0.5322, 1.0000),
];

impl Projection {
    /// Plane coordinates (x right, y up) of a point in degrees.
    pub fn project(self, lon: f64, lat: f64) -> (f64, f64) {
        let l = lon.to_radians();
        match self {
            Projection::Equirectangular => (lon, lat),
            Projection::Mercator => {
                let p = lat.clamp(-85.0, 85.0).to_radians();
                (
                    lon,
                    (std::f64::consts::FRAC_PI_4 + p / 2.0)
                        .tan()
                        .ln()
                        .to_degrees(),
                )
            }
            Projection::Robinson => {
                let a = lat.abs().min(90.0) / 5.0;
                let i = (a.floor() as usize).min(17);
                let f = a - i as f64;
                let (x0, y0) = ROBINSON[i];
                let (x1, y1) = ROBINSON[i + 1];
                let x = x0 + (x1 - x0) * f;
                let y = y0 + (y1 - y0) * f;
                (
                    (0.8487 * x * l).to_degrees(),
                    (1.3523 * y * lat.signum()).to_degrees(),
                )
            }
            Projection::EqualEarth => {
                const A: [f64; 4] = [1.340264, -0.081106, 0.000893, 0.003796];
                let p = lat.clamp(-90.0, 90.0).to_radians();
                let t = (3f64.sqrt() / 2.0 * p.sin()).asin();
                let (t2, t6) = (t * t, t.powi(6));
                let x = 2.0 * 3f64.sqrt() * l * t.cos()
                    / (3.0 * (A[0] + 3.0 * A[1] * t2 + t6 * (7.0 * A[2] + 9.0 * A[3] * t2)));
                let y = t * (A[0] + A[1] * t2 + t6 * (A[2] + A[3] * t2));
                (x.to_degrees(), y.to_degrees())
            }
            Projection::Mollweide => {
                let p = lat.clamp(-90.0, 90.0).to_radians();
                // 2θ + sin 2θ = π sin φ (Newton; θ = φ at the poles).
                let k = std::f64::consts::PI * p.sin();
                let mut t = p;
                if (p.abs() - std::f64::consts::FRAC_PI_2).abs() > 1e-9 {
                    for _ in 0..12 {
                        let d = (2.0 * t + (2.0 * t).sin() - k) / (2.0 + 2.0 * (2.0 * t).cos());
                        t -= d;
                        if d.abs() < 1e-12 {
                            break;
                        }
                    }
                }
                let x = 2.0 * std::f64::consts::SQRT_2 / std::f64::consts::PI * l * t.cos();
                let y = std::f64::consts::SQRT_2 * t.sin();
                (x.to_degrees(), y.to_degrees())
            }
            Projection::Orthographic { .. }
            | Projection::Stereographic { .. }
            | Projection::AzimuthalEqualArea { .. } => match self.globe(lon, lat) {
                v if v[2] >= 0.0 => self.plane(v),
                _ => (f64::NAN, f64::NAN),
            },
            Projection::Albers {
                lon0,
                lat0,
                lat1,
                lat2,
            } => {
                let r = f64::to_radians;
                let (p1, p2) = (r(lat1), r(lat2));
                let mut n = (p1.sin() + p2.sin()) / 2.0;
                if n.abs() < 1e-9 {
                    n = 1e-9;
                }
                let c = p1.cos().powi(2) + 2.0 * n * p1.sin();
                let rho = |p: f64| (c - 2.0 * n * p.sin()).max(0.0).sqrt() / n;
                let t = n * r(wrap180(lon - lon0));
                let (rh, rh0) = (rho(r(lat.clamp(-90.0, 90.0))), rho(r(lat0)));
                (
                    (rh * t.sin()).to_degrees(),
                    (rh0 - rh * t.cos()).to_degrees(),
                )
            }
            Projection::LambertConformal {
                lon0,
                lat0,
                lat1,
                lat2,
            } => {
                use std::f64::consts::FRAC_PI_4;
                let r = f64::to_radians;
                let (p1, p2) = (r(lat1), r(lat2));
                let tn = |p: f64| (FRAC_PI_4 + p / 2.0).tan();
                let n = if (lat1 - lat2).abs() < 1e-9 {
                    p1.sin()
                } else {
                    (p1.cos() / p2.cos()).ln() / (tn(p2) / tn(p1)).ln()
                };
                let n = if n.abs() < 1e-9 { 1e-9 } else { n };
                let f = p1.cos() * tn(p1).powf(n) / n;
                // The pole away from the cone's apex is at infinity.
                let lim = if n > 0.0 {
                    (-80.0, 90.0)
                } else {
                    (-90.0, 80.0)
                };
                let rho = |p: f64| f / tn(p).powf(n);
                let t = n * r(wrap180(lon - lon0));
                let (rh, rh0) = (rho(r(lat.clamp(lim.0, lim.1))), rho(r(lat0)));
                (
                    (rh * t.sin()).to_degrees(),
                    (rh0 - rh * t.cos()).to_degrees(),
                )
            }
            Projection::NaturalEarth => {
                let p = lat.clamp(-90.0, 90.0).to_radians();
                let p2 = p * p;
                let p4 = p2 * p2;
                let x = l
                    * (0.8707 - 0.131979 * p2
                        + p4 * (-0.013791 + p4 * (0.003971 * p2 - 0.001529 * p4)));
                let y = p
                    * (1.007226
                        + p2 * (0.015085 + p4 * (-0.044475 + 0.028874 * p2 - 0.005916 * p4)));
                (x.to_degrees(), y.to_degrees())
            }
        }
    }

    /// A point on the unit sphere in the globe's view: x right, y up, z
    /// towards the viewer (only for [`Projection::Orthographic`]).
    fn globe(self, lon: f64, lat: f64) -> [f64; 3] {
        let Some((l0, p0)) = self.center() else {
            return [0.0; 3];
        };
        let (l, p) = ((lon - l0).to_radians(), lat.to_radians());
        let p0 = p0.to_radians();
        let (cp, sp, c0, s0) = (p.cos(), p.sin(), p0.cos(), p0.sin());
        [
            cp * l.sin(),
            c0 * sp - s0 * cp * l.cos(),
            s0 * sp + c0 * cp * l.cos(),
        ]
    }

    /// A ring (longitude, latitude) in plane coordinates; on the globe
    /// the part on the far side is cut away along the rim (no ring when
    /// nothing is visible).
    pub(crate) fn project_ring(self, ring: &[(f64, f64)]) -> Vec<Vec<(f64, f64)>> {
        if let Some(lon0) = self.conic() {
            // Cut at the meridian opposite the centre, where a conic map
            // splits.
            // Long edges (e.g. along a pole) are curves on a cone.
            return split_at_seam(&densify(ring, 2.0), lon0)
                .into_iter()
                .map(|r| r.iter().map(|&(a, b)| self.project(a, b)).collect())
                .collect();
        }
        if self.center().is_none() {
            return vec![ring.iter().map(|&(a, b)| self.project(a, b)).collect()];
        }
        self.clip_hemisphere(ring).into_iter().collect()
    }

    fn clip_hemisphere(self, ring: &[(f64, f64)]) -> Option<Vec<(f64, f64)>> {
        let v: Vec<[f64; 3]> = ring.iter().map(|&(a, b)| self.globe(a, b)).collect();
        if v.iter().all(|p| p[2] >= 0.0) {
            return Some(v.iter().map(|p| self.plane(*p)).collect());
        }
        // Sutherland–Hodgman against the plane z = 0; points on the rim
        // are marked so that the edge between two of them follows the rim.
        let n = v.len();
        let mut out: Vec<([f64; 3], bool)> = Vec::with_capacity(n + 8);
        for i in 0..n {
            let (a, b) = (v[i], v[(i + 1) % n]);
            let (ia, ib) = (a[2] >= 0.0, b[2] >= 0.0);
            if ia {
                out.push((a, false));
            }
            if ia != ib {
                let t = a[2] / (a[2] - b[2]);
                let c = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, 0.0];
                let r = (c[0] * c[0] + c[1] * c[1]).sqrt().max(1e-12);
                out.push(([c[0] / r, c[1] / r, 0.0], true));
            }
        }
        if out.len() < 3 {
            return None;
        }
        let mut pts = Vec::with_capacity(out.len() * 2);
        let m = out.len();
        for i in 0..m {
            let (p, rim) = out[i];
            pts.push(self.plane(p));
            let (q, rim_q) = out[(i + 1) % m];
            if rim && rim_q {
                // Along the rim, the short way round.
                let (a0, a1) = (p[1].atan2(p[0]), q[1].atan2(q[0]));
                let mut d = a1 - a0;
                if d > std::f64::consts::PI {
                    d -= std::f64::consts::TAU;
                } else if d < -std::f64::consts::PI {
                    d += std::f64::consts::TAU;
                }
                let k = (d.abs() / 0.05).ceil() as usize;
                for j in 1..k {
                    let a = a0 + d * j as f64 / k as f64;
                    pts.push((a.cos() * GLOBE_R, a.sin() * GLOBE_R));
                }
            }
        }
        Some(pts)
    }

    /// The outline of the whole globe in this projection.
    fn outline(self) -> Vec<(f64, f64)> {
        if let Some(lon0) = self.conic() {
            // The fan between the seam meridians and the latitude limits.
            let (lo, hi) = match self {
                Projection::LambertConformal { lat1, lat2, .. } if lat1 + lat2 < 0.0 => {
                    (-90.0, 80.0)
                }
                Projection::LambertConformal { .. } => (-80.0, 90.0),
                _ => (-90.0, 90.0),
            };
            let e = 179.999;
            let mut v = Vec::new();
            for i in 0..=72 {
                v.push(self.project(lon0 - e + i as f64 * 2.0 * e / 72.0, hi));
            }
            for i in 0..=36 {
                v.push(self.project(lon0 + e, hi - i as f64 * (hi - lo) / 36.0));
            }
            for i in 0..=72 {
                v.push(self.project(lon0 + e - i as f64 * 2.0 * e / 72.0, lo));
            }
            for i in 0..=36 {
                v.push(self.project(lon0 - e, lo + i as f64 * (hi - lo) / 36.0));
            }
            return v;
        }
        if self.center().is_some() {
            return (0..=120)
                .map(|i| {
                    let a = i as f64 / 120.0 * std::f64::consts::TAU;
                    (a.cos() * GLOBE_R, a.sin() * GLOBE_R)
                })
                .collect();
        }
        let top = if self == Projection::Mercator {
            85.0
        } else {
            90.0
        };
        let mut v = Vec::new();
        for i in 0..=36 {
            v.push(self.project(-180.0 + i as f64 * 10.0, top));
        }
        for i in 0..=36 {
            v.push(self.project(180.0, top - i as f64 * top / 18.0));
        }
        for i in 0..=36 {
            v.push(self.project(180.0 - i as f64 * 10.0, -top));
        }
        for i in 0..=36 {
            v.push(self.project(-180.0, -top + i as f64 * top / 18.0));
        }
        v
    }
}

/// The ring with points inserted so that no edge is longer than `step`
/// degrees in longitude or latitude.
fn densify(ring: &[(f64, f64)], step: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(ring.len());
    for (i, &p) in ring.iter().enumerate() {
        out.push(p);
        if let Some(&q) = ring.get(i + 1) {
            let k = ((q.0 - p.0).abs().max((q.1 - p.1).abs()) / step).ceil();
            if k > 1.0 && k < 1e4 {
                for j in 1..k as usize {
                    let t = j as f64 / k;
                    out.push((p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t));
                }
            }
        }
    }
    out
}

/// `v` in [-180, 180).
fn wrap180(v: f64) -> f64 {
    (v + 180.0).rem_euclid(360.0) - 180.0
}

/// A ring cut into pieces that do not cross the meridian opposite `lon0`
/// (longitudes made continuous, then clipped to each 360° window).
fn split_at_seam(ring: &[(f64, f64)], lon0: f64) -> Vec<Vec<(f64, f64)>> {
    if ring.is_empty() {
        return Vec::new();
    }
    // Continuous longitudes relative to the centre.
    let mut rel = Vec::with_capacity(ring.len());
    let mut prev = wrap180(ring[0].0 - lon0);
    for &(lon, lat) in ring {
        let mut a = wrap180(lon - lon0);
        while a - prev > 180.0 {
            a -= 360.0;
        }
        while a - prev < -180.0 {
            a += 360.0;
        }
        rel.push((a, lat));
        prev = a;
    }
    let (lo, hi) = rel
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
            (a.min(p.0), b.max(p.0))
        });
    if lo >= -180.0 && hi <= 180.0 {
        return vec![ring.to_vec()];
    }
    // Each 360° window the ring reaches, clipped to it (Sutherland–Hodgman
    // with two vertical lines) and shifted back.
    let mut out = Vec::new();
    let mut k = ((lo + 180.0) / 360.0).floor() as i64;
    while (k as f64) * 360.0 - 180.0 < hi {
        let (a, b) = (
            k as f64 * 360.0 - 180.0 + 1e-6,
            k as f64 * 360.0 + 180.0 - 1e-6,
        );
        let clip = |pts: &[(f64, f64)], edge: f64, keep_above: bool| -> Vec<(f64, f64)> {
            let inside = |p: &(f64, f64)| if keep_above { p.0 >= edge } else { p.0 <= edge };
            let mut r = Vec::new();
            for i in 0..pts.len() {
                let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
                if inside(&p) {
                    r.push(p);
                }
                if inside(&p) != inside(&q) {
                    let t = (edge - p.0) / (q.0 - p.0);
                    r.push((edge, p.1 + (q.1 - p.1) * t));
                }
            }
            r
        };
        let piece = clip(&clip(&rel, a, true), b, false);
        if piece.len() >= 3 {
            let shift = k as f64 * 360.0;
            out.push(
                piece
                    .into_iter()
                    .map(|(x, y)| (x - shift + lon0, y))
                    .collect(),
            );
        }
        k += 1;
        if out.len() > 4 {
            break;
        }
    }
    out
}

/// Polygons of a shape: each a list of rings (the first is the outline,
/// the others holes) of (longitude, latitude) in degrees.
pub type Polygons = Vec<Vec<Vec<(f64, f64)>>>;

/// A country of the built-in world map.
#[derive(Clone, Debug)]
pub struct Country {
    /// Short name ("Germany").
    pub name: String,
    /// Long name ("Federal Republic of Germany" → "Germany" style names).
    pub long_name: String,
    /// ISO 3166-1 alpha-2 code ("DE"; empty if none).
    pub iso2: String,
    /// ISO 3166-1 alpha-3 code ("DEU").
    pub iso3: String,
    /// Continent.
    pub continent: String,
    /// Population estimate.
    pub population: f64,
    /// GDP in million US dollars.
    pub gdp_musd: f64,
    /// Outline.
    pub polygons: Arc<Polygons>,
}

impl Country {
    /// Whether `key` is this country's ISO code (alpha-3 or alpha-2) or name
    /// (case-insensitive).
    pub fn matches(&self, key: &str) -> bool {
        let k = key.trim();
        k.eq_ignore_ascii_case(&self.iso3)
            || (!self.iso2.is_empty() && k.eq_ignore_ascii_case(&self.iso2))
            || k.eq_ignore_ascii_case(&self.name)
            || k.eq_ignore_ascii_case(&self.long_name)
    }
}

static WORLD: LazyLock<Vec<Country>> =
    LazyLock::new(|| decode(include_bytes!("../../assets/world_110m.bin")).unwrap_or_default());

/// The countries of the built-in world map (Natural Earth 1:110m, public
/// domain), sorted by name.
pub fn world() -> &'static [Country] {
    &WORLD
}

/// Looks a country up by ISO code or name.
pub fn country(key: &str) -> Option<&'static Country> {
    world().iter().find(|c| c.matches(key))
}

fn decode(b: &[u8]) -> Option<Vec<Country>> {
    let mut at = 4;
    if b.get(..4)? != b"DVW1" {
        return None;
    }
    let u16_ = |at: &mut usize| -> Option<u16> {
        let v = u16::from_le_bytes(b.get(*at..*at + 2)?.try_into().ok()?);
        *at += 2;
        Some(v)
    };
    let n = u16_(&mut at)?;
    let text = |at: &mut usize| -> Option<String> {
        let len = *b.get(*at)? as usize;
        let s = std::str::from_utf8(b.get(*at + 1..*at + 1 + len)?)
            .ok()?
            .to_string();
        *at += 1 + len;
        Some(s)
    };
    let f64_ = |at: &mut usize| -> Option<f64> {
        let v = f64::from_le_bytes(b.get(*at..*at + 8)?.try_into().ok()?);
        *at += 8;
        Some(v)
    };
    let i16_ = |at: &mut usize| -> Option<i16> {
        let v = i16::from_le_bytes(b.get(*at..*at + 2)?.try_into().ok()?);
        *at += 2;
        Some(v)
    };
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (name, long_name, iso2, iso3, continent) = (
            text(&mut at)?,
            text(&mut at)?,
            text(&mut at)?,
            text(&mut at)?,
            text(&mut at)?,
        );
        let (population, gdp_musd) = (f64_(&mut at)?, f64_(&mut at)?);
        let np = u16_(&mut at)?;
        let mut polys = Vec::with_capacity(np as usize);
        for _ in 0..np {
            let nr = u16_(&mut at)?;
            let mut rings = Vec::with_capacity(nr as usize);
            for _ in 0..nr {
                let k = u16_(&mut at)?;
                let mut ring = Vec::with_capacity(k as usize);
                for _ in 0..k {
                    let lon = i16_(&mut at)? as f64 / 100.0;
                    let lat = i16_(&mut at)? as f64 / 100.0;
                    ring.push((lon, lat));
                }
                rings.push(ring);
            }
            polys.push(rings);
        }
        out.push(Country {
            name,
            long_name,
            iso2,
            iso3,
            continent,
            population,
            gdp_musd,
            polygons: Arc::new(polys),
        });
    }
    Some(out)
}

/// A shape of a GeoJSON file: its polygons, lines and points with the
/// feature's properties (as text).
#[derive(Clone, Debug, Default)]
pub struct Feature {
    /// Properties (`"name"` → `"Bavaria"`, numbers as written).
    pub properties: Vec<(String, String)>,
    /// Polygons (Polygon and MultiPolygon geometries).
    pub polygons: Arc<Polygons>,
    /// Lines (LineString and MultiLineString geometries).
    pub lines: Vec<Vec<(f64, f64)>>,
    /// Points (Point and MultiPoint geometries).
    pub points: Vec<(f64, f64)>,
}

impl Feature {
    /// A property by name.
    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Minimal JSON value for GeoJSON.
#[derive(Debug)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

struct JsonParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl JsonParser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.s.get(self.i) {
            Some(b'{') => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.s.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(Json::Obj(v));
                    }
                    let Json::Str(k) = self.value()? else {
                        return Err("expected a key".into());
                    };
                    self.ws();
                    if self.s.get(self.i) != Some(&b':') {
                        return Err("expected ':'".into());
                    }
                    self.i += 1;
                    v.push((k, self.value()?));
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {}
                        _ => return Err("expected ',' or '}'".into()),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.s.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Json::Arr(v));
                    }
                    v.push(self.value()?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {}
                        _ => return Err("expected ',' or ']'".into()),
                    }
                }
            }
            Some(b'"') => {
                self.i += 1;
                let mut out = Vec::new();
                while let Some(&c) = self.s.get(self.i) {
                    self.i += 1;
                    match c {
                        b'"' => {
                            return String::from_utf8(out)
                                .map(Json::Str)
                                .map_err(|e| e.to_string())
                        }
                        b'\\' => {
                            let e = *self.s.get(self.i).ok_or("bad escape")?;
                            self.i += 1;
                            match e {
                                b'n' => out.push(b'\n'),
                                b't' => out.push(b'\t'),
                                b'r' => out.push(b'\r'),
                                b'b' | b'f' => {}
                                b'u' => {
                                    let hex = std::str::from_utf8(
                                        self.s.get(self.i..self.i + 4).ok_or("bad escape")?,
                                    )
                                    .map_err(|e| e.to_string())?;
                                    let code =
                                        u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
                                    self.i += 4;
                                    let c = char::from_u32(code).unwrap_or('\u{FFFD}');
                                    let mut buf = [0; 4];
                                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                                }
                                c => out.push(c),
                            }
                        }
                        c => out.push(c),
                    }
                }
                Err("unterminated string".into())
            }
            Some(b't') if self.s[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(Json::Bool(true))
            }
            Some(b'f') if self.s[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(Json::Bool(false))
            }
            Some(b'n') if self.s[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(Json::Null)
            }
            Some(_) => {
                let start = self.i;
                while self.i < self.s.len()
                    && matches!(
                        self.s[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[start..self.i])
                    .ok()
                    .and_then(|t| t.parse().ok())
                    .map(Json::Num)
                    .ok_or_else(|| format!("unexpected character at byte {start}"))
            }
            None => Err("unexpected end".into()),
        }
    }
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(v) => v.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(v) => v,
            _ => &[],
        }
    }
    fn point(&self) -> Option<(f64, f64)> {
        match self.arr() {
            [Json::Num(x), Json::Num(y), ..] => Some((*x, *y)),
            _ => None,
        }
    }
    fn line(&self) -> Vec<(f64, f64)> {
        self.arr().iter().filter_map(Json::point).collect()
    }
    fn polygon(&self) -> Vec<Vec<(f64, f64)>> {
        self.arr().iter().map(Json::line).collect()
    }
    fn text(&self) -> String {
        match self {
            Json::Null => String::new(),
            Json::Bool(b) => b.to_string(),
            Json::Num(n) => short(*n),
            Json::Str(s) => s.clone(),
            Json::Arr(_) | Json::Obj(_) => String::from("…"),
        }
    }
}

fn geometry(g: &Json, f: &mut Feature, polys: &mut Polygons) {
    let c = g.get("coordinates");
    let Some(Json::Str(t)) = g.get("type") else {
        return;
    };
    match t.as_str() {
        "Polygon" => polys.extend(c.map(Json::polygon)),
        "MultiPolygon" => polys.extend(
            c.map(|c| c.arr().iter().map(Json::polygon).collect::<Vec<_>>())
                .unwrap_or_default(),
        ),
        "LineString" => f.lines.extend(c.map(Json::line)),
        "MultiLineString" => f.lines.extend(c.map(Json::polygon).unwrap_or_default()),
        "Point" => f.points.extend(c.and_then(Json::point)),
        "MultiPoint" => f.points.extend(c.map(Json::line).unwrap_or_default()),
        "GeometryCollection" => {
            for g in g.get("geometries").map(Json::arr).unwrap_or(&[]) {
                geometry(g, f, polys);
            }
        }
        _ => {}
    }
}

/// Reads the features of a GeoJSON document (a FeatureCollection, a single
/// Feature or a bare geometry); coordinates are longitude, latitude.
pub fn read_geojson(text: &str) -> Result<Vec<Feature>, Error> {
    let mut p = JsonParser {
        s: text.as_bytes(),
        i: 0,
    };
    let doc = p
        .value()
        .map_err(|e| Error::Render(format!("GeoJSON: {e}")))?;
    let list: Vec<&Json> = match doc.get("type") {
        Some(Json::Str(t)) if t == "FeatureCollection" => doc
            .get("features")
            .map(|f| f.arr().iter().collect())
            .unwrap_or_default(),
        _ => vec![&doc],
    };
    let mut out = Vec::with_capacity(list.len());
    for item in list {
        let mut f = Feature::default();
        if let Some(Json::Obj(props)) = item.get("properties") {
            f.properties = props.iter().map(|(k, v)| (k.clone(), v.text())).collect();
        }
        let mut polys = Vec::new();
        geometry(item.get("geometry").unwrap_or(item), &mut f, &mut polys);
        f.polygons = Arc::new(polys);
        out.push(f);
    }
    Ok(out)
}

/// Filled shapes on a map (countries or GeoJSON features), colored by value
/// (choropleth) or plain. Created by [`Axes::world_map`],
/// [`Axes::choropleth`] and [`Axes::geo_shapes`].
#[derive(Clone, Debug)]
pub struct GeoMap {
    pub(crate) shapes: Vec<(String, Arc<Polygons>)>,
    pub(crate) values: Vec<f64>,
    pub(crate) projection: Projection,
    pub(crate) colormap: Colormap,
    pub(crate) range: Option<(f64, f64)>,
    pub(crate) fill: Color,
    pub(crate) missing: Color,
    pub(crate) edge: Stroke,
    pub(crate) colorbar: Option<String>,
    pub(crate) log: bool,
    pub(crate) graticule: Option<f64>,
    /// Projected outlines (cached): per shape, rings of plane points.
    projected: Vec<Vec<Vec<(f64, f64)>>>,
}

impl GeoMap {
    pub(crate) fn new(
        shapes: Vec<(String, Arc<Polygons>)>,
        values: Vec<f64>,
        projection: Projection,
    ) -> Self {
        let mut m = Self {
            shapes,
            values,
            projection,
            colormap: Colormap::Viridis,
            range: None,
            fill: Color::rgb(205, 210, 216),
            missing: Color::rgb(225, 228, 232),
            edge: Stroke::new(Color::WHITE, 0.6),
            colorbar: None,
            log: false,
            graticule: None,
            projected: Vec::new(),
        };
        m.reproject();
        m
    }

    fn reproject(&mut self) {
        let p = self.projection;
        self.projected = self
            .shapes
            .iter()
            .map(|(_, polys)| {
                polys
                    .iter()
                    .flatten()
                    .flat_map(|ring| p.project_ring(ring))
                    .collect()
            })
            .collect();
    }

    /// Colormap for the values (default viridis).
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
    /// Value range of the colormap (default: the data range).
    pub fn range(&mut self, min: f64, max: f64) -> &mut Self {
        self.range = Some((min, max));
        self
    }
    /// Logarithmic color scale (for values spanning orders of magnitude).
    pub fn log(&mut self, on: bool) -> &mut Self {
        self.log = on;
        self
    }
    /// Shows a colorbar with this label.
    pub fn colorbar(&mut self, label: impl Into<String>) -> &mut Self {
        self.colorbar = Some(label.into());
        self
    }
    /// Fill of shapes without a value (plain maps: all shapes).
    pub fn fill(&mut self, c: impl Into<Color>) -> &mut Self {
        let c = c.into();
        self.fill = c;
        self.missing = c;
        self
    }
    /// Border color and width.
    pub fn edge(&mut self, c: impl Into<Color>, width: f64) -> &mut Self {
        self.edge = Stroke::new(c.into(), width);
        self
    }
    /// Draws meridians and parallels every `degrees` and the outline of
    /// the globe.
    pub fn graticule(&mut self, degrees: f64) -> &mut Self {
        self.graticule = (degrees > 0.0).then_some(degrees);
        self
    }

    fn scale(&self) -> Option<ColorScale> {
        let (lo, hi) = self.range.or_else(|| {
            let v = self
                .values
                .iter()
                .copied()
                .filter(|v| v.is_finite() && (!self.log || *v > 0.0));
            let (a, b) = v.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(v), b.max(v))
            });
            a.is_finite().then_some((a, b))
        })?;
        let t = |v: f64| {
            if self.log {
                v.max(f64::MIN_POSITIVE).log10()
            } else {
                v
            }
        };
        Some(ColorScale {
            colormap: self.colormap,
            min: t(lo),
            max: t(hi),
            label: self.colorbar.clone().map(|l| {
                if self.log {
                    format!("{l} (log₁₀)")
                } else {
                    l
                }
            }),
        })
    }

    fn color_of(&self, i: usize, scale: &Option<ColorScale>) -> Color {
        match (self.values.get(i), scale) {
            (Some(v), Some(s)) if v.is_finite() && (!self.log || *v > 0.0) => {
                s.color(if self.log { v.log10() } else { *v })
            }
            (Some(_), _) => self.missing,
            (None, _) => self.fill,
        }
    }

    fn graticule_lines(&self) -> Vec<Vec<(f64, f64)>> {
        let Some(step) = self.graticule else {
            return Vec::new();
        };
        let p = self.projection;
        let top = if p == Projection::Mercator {
            85.0
        } else {
            90.0
        };
        let mut lines = Vec::new();
        let mut lon = -180.0;
        while lon <= 180.0 + 1e-9 {
            lines.push(
                (0..=60)
                    .map(|i| p.project(lon, -top + i as f64 * 2.0 * top / 60.0))
                    .collect(),
            );
            lon += step;
        }
        let c = p.conic().unwrap_or(0.0);
        let mut lat = -top + (top % step);
        while lat <= top + 1e-9 {
            lines.push(
                // From seam to seam (conic maps split opposite their centre).
                (0..=72)
                    .map(|i| p.project(c - 179.999 + i as f64 * 359.998 / 72.0, lat))
                    .collect(),
            );
            lat += step;
        }
        lines
    }
}

/// Even-odd point-in-polygon test over all rings.
fn inside(rings: &[Vec<(f64, f64)>], x: f64, y: f64) -> bool {
    let mut c = false;
    for r in rings {
        let n = r.len();
        if n < 3 {
            continue;
        }
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = r[i];
            let (xj, yj) = r[j];
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                c = !c;
            }
            j = i;
        }
    }
    c
}

impl Series for GeoMap {
    fn extent(&self) -> Option<Extent> {
        let mut pts: Vec<(f64, f64)> = self.projected.iter().flatten().flatten().copied().collect();
        if self.graticule.is_some() {
            pts.extend(self.projection.outline());
        }
        let mut e = Extent::of_points(pts)?;
        e.sticky_x_min = true;
        e.sticky_x_max = true;
        e.sticky_y_min = true;
        e.sticky_y_max = true;
        Some(e)
    }

    fn draw(&self, ctx: &mut Context) {
        if self.graticule.is_some() {
            let outline: Vec<(f64, f64)> = self
                .projection
                .outline()
                .into_iter()
                .map(|(x, y)| ctx.point(x, y))
                .collect();
            ctx.polygon(&outline, Some(Color::rgb(234, 242, 250)), None);
            let grid = Stroke::new(Color::rgba(120, 140, 160, 0.35), 0.6);
            for l in self.graticule_lines() {
                let pts: Vec<(f64, f64)> = l.into_iter().map(|(x, y)| ctx.point(x, y)).collect();
                ctx.polyline(&pts, grid.clone());
            }
        }
        let scale = self.scale();
        for (i, rings) in self.projected.iter().enumerate() {
            let mut cmds = Vec::new();
            for r in rings {
                for (k, (x, y)) in r.iter().enumerate() {
                    let (px, py) = ctx.point(*x, *y);
                    cmds.push(if k == 0 {
                        PathCmd::MoveTo(px, py)
                    } else {
                        PathCmd::LineTo(px, py)
                    });
                }
                cmds.push(PathCmd::Close);
            }
            if cmds.is_empty() {
                continue;
            }
            ctx.push(Node::Path {
                cmds,
                fill: Some(self.color_of(i, &scale)),
                stroke: (self.edge.width > 0.0).then(|| self.edge.clone()),
            });
        }
        if self.graticule.is_some() {
            let outline: Vec<(f64, f64)> = self
                .projection
                .outline()
                .into_iter()
                .map(|(x, y)| ctx.point(x, y))
                .collect();
            ctx.polyline(&outline, Stroke::new(Color::rgba(90, 100, 110, 0.8), 1.0));
        }
    }

    fn color_scale(&self) -> Option<ColorScale> {
        self.colorbar.as_ref()?;
        self.scale()
    }

    fn hit(&self, x: &AxisMap, y: &AxisMap, _rect: Rect, px: f64, py: f64) -> Option<Hit> {
        let (dx, dy) = (x.value(px), y.value(py));
        let i = self.projected.iter().position(|r| inside(r, dx, dy))?;
        let name = &self.shapes[i].0;
        let label = match self.values.get(i) {
            Some(v) if v.is_finite() => format!("{name}: {}", short(*v)),
            _ => name.clone(),
        };
        Some(Hit {
            px,
            py,
            label,
            distance: 0.0,
        })
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// Sets the axes up for a map in `projection`: no axes, ticks, grid or
    /// frame, equal scales. Called by the map methods with the default
    /// projection (Robinson) when not called first.
    pub fn map(&mut self, projection: Projection) -> &mut Self {
        self.projection = Some(projection);
        self.x.hidden();
        self.y.hidden();
        self.x.grid = Some(false);
        self.y.grid = Some(false);
        self.equal_aspect = true;
        self.frame = false;
        self.x.margin = 0.01;
        self.y.margin = 0.01;
        self
    }

    /// Shows the region between the longitudes `lon` and latitudes `lat`
    /// (degrees) of the map's projection, e.g. a country on a conic map.
    pub fn map_bounds(&mut self, lon: (f64, f64), lat: (f64, f64)) -> &mut Self {
        let p = self.projection.unwrap_or_default();
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for i in 0..=40 {
            let t = i as f64 / 40.0;
            let a = lon.0 + (lon.1 - lon.0) * t;
            let b = lat.0 + (lat.1 - lat.0) * t;
            for (x, y) in [
                p.project(a, lat.0),
                p.project(a, lat.1),
                p.project(lon.0, b),
                p.project(lon.1, b),
            ] {
                if x.is_finite() && y.is_finite() {
                    (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
                }
            }
        }
        if x1 > x0 && y1 > y0 {
            self.x.limits(x0, x1);
            self.y.limits(y0, y1);
        }
        self
    }

    /// A globe seen from above (`lon`, `lat`) (orthographic projection);
    /// maps added afterwards are drawn on it.
    pub fn globe(&mut self, lon: f64, lat: f64) -> &mut Self {
        self.map(Projection::Orthographic { lon, lat });
        self.x.margin = 0.06;
        self.y.margin = 0.06;
        self
    }

    /// Turns the globe to look at (`lon`, `lat`): the maps on it are
    /// projected again (points and lines added with `geo_scatter` /
    /// `geo_line` keep their positions; add them again for a new view).
    pub fn globe_center(&mut self, lon: f64, lat: f64) -> &mut Self {
        let p = Projection::Orthographic { lon, lat };
        self.projection = Some(p);
        self.revision += 1;
        for s in &mut self.series {
            if let Some(m) = s.as_any_mut().downcast_mut::<GeoMap>() {
                m.projection = p;
                m.reproject();
            }
        }
        self
    }

    pub(crate) fn map_projection(&mut self) -> Projection {
        match self.projection {
            Some(p) => p,
            None => {
                self.map(Projection::default());
                Projection::default()
            }
        }
    }

    /// The countries of the world (plain), e.g. as a base for points.
    pub fn world_map(&mut self) -> &mut GeoMap {
        let p = self.map_projection();
        let shapes = world()
            .iter()
            .map(|c| (c.name.clone(), c.polygons.clone()))
            .collect();
        self.add(GeoMap::new(shapes, Vec::new(), p))
    }

    /// World choropleth: countries colored by value. Keys are ISO codes
    /// (alpha-3 or alpha-2) or country names; countries without a value
    /// are grey.
    pub fn choropleth<K: AsRef<str>>(
        &mut self,
        values: impl IntoIterator<Item = (K, f64)>,
    ) -> &mut GeoMap {
        let p = self.map_projection();
        let w = world();
        let mut v = vec![f64::NAN; w.len()];
        for (k, val) in values {
            if let Some(i) = w.iter().position(|c| c.matches(k.as_ref())) {
                v[i] = val;
            }
        }
        let shapes = w
            .iter()
            .map(|c| (c.name.clone(), c.polygons.clone()))
            .collect();
        self.add(GeoMap::new(shapes, v, p))
    }

    /// GeoJSON features (or any polygons) colored by `values` (one per
    /// feature, NaN = no value; empty for a plain map). Tooltips show the
    /// feature's `name_property`.
    pub fn geo_shapes(
        &mut self,
        features: &[Feature],
        values: impl IntoValues,
        name_property: &str,
    ) -> &mut GeoMap {
        let p = self.map_projection();
        let shapes = features
            .iter()
            .map(|f| {
                (
                    f.property(name_property).unwrap_or_default().to_string(),
                    f.polygons.clone(),
                )
            })
            .collect();
        let mut v = values.into_values();
        if !v.is_empty() {
            v.resize(features.len(), f64::NAN);
        }
        self.add(GeoMap::new(shapes, v, p))
    }

    /// Points at longitudes/latitudes (degrees) on the map.
    pub fn geo_scatter(
        &mut self,
        lon: impl IntoValues,
        lat: impl IntoValues,
    ) -> &mut super::series::scatter::Scatter {
        let p = self.map_projection();
        let (x, y): (Vec<f64>, Vec<f64>) = lon
            .into_values()
            .into_iter()
            .zip(lat.into_values())
            .map(|(a, b)| p.project(a, b))
            .unzip();
        self.scatter(x, y)
    }

    /// A route through longitudes/latitudes (degrees) on the map, following
    /// great circles between the points.
    pub fn geo_line(
        &mut self,
        lon: impl IntoValues,
        lat: impl IntoValues,
    ) -> &mut super::series::line::Line {
        let p = self.map_projection();
        let pts: Vec<(f64, f64)> = lon
            .into_values()
            .into_iter()
            .zip(lat.into_values())
            .collect();
        let mut out: Vec<(f64, f64)> = Vec::new();
        for w in pts.windows(2) {
            for (lo, la) in great_circle(w[0], w[1], 48) {
                out.push(p.project(lo, la));
            }
        }
        if let (Some(last), true) = (pts.last(), pts.len() == 1) {
            out.push(p.project(last.0, last.1));
        }
        let (x, y): (Vec<f64>, Vec<f64>) = out.into_iter().unzip();
        let l = self.line(x, y);
        l.decimate = false;
        l
    }
}

/// Points along the great circle from `a` to `b` (degrees), split where it
/// crosses the antimeridian (NaN between).
pub(crate) fn great_circle(a: (f64, f64), b: (f64, f64), n: usize) -> Vec<(f64, f64)> {
    let to_v = |(lon, lat): (f64, f64)| {
        let (l, p) = (lon.to_radians(), lat.to_radians());
        [p.cos() * l.cos(), p.cos() * l.sin(), p.sin()]
    };
    let (va, vb) = (to_v(a), to_v(b));
    let dot = (va[0] * vb[0] + va[1] * vb[1] + va[2] * vb[2]).clamp(-1.0, 1.0);
    let w = dot.acos();
    let mut out = Vec::with_capacity(n + 2);
    let mut prev_lon: Option<f64> = None;
    for i in 0..=n {
        let t = i as f64 / n as f64;
        let (s0, s1) = if w.abs() < 1e-9 {
            (1.0 - t, t)
        } else {
            (((1.0 - t) * w).sin() / w.sin(), (t * w).sin() / w.sin())
        };
        let v = [
            s0 * va[0] + s1 * vb[0],
            s0 * va[1] + s1 * vb[1],
            s0 * va[2] + s1 * vb[2],
        ];
        let lon = v[1].atan2(v[0]).to_degrees();
        let lat = v[2].atan2((v[0] * v[0] + v[1] * v[1]).sqrt()).to_degrees();
        if let Some(pl) = prev_lon {
            if (lon - pl).abs() > 180.0 {
                out.push((f64::NAN, f64::NAN));
            }
        }
        if i > 0 || out.is_empty() {
            out.push((lon, lat));
        }
        prev_lon = Some(lon);
    }
    out
}

#[cfg(test)]
mod tests {

    #[test]
    fn equal_area_and_globe_projections() {
        // Mollweide: poles at y = √2 rad, the equator 2√2 rad wide each way.
        let (x, y) = Projection::Mollweide.project(0.0, 90.0);
        assert!(x.abs() < 1e-9 && (y - 2f64.sqrt().to_degrees()).abs() < 1e-6);
        let (x, _) = Projection::Mollweide.project(180.0, 0.0);
        assert!((x - (2.0 * 2f64.sqrt()).to_degrees()).abs() < 1e-6);
        // Equal Earth: symmetric, poles flat (x shrinks but stays > 0).
        let (a, b) = Projection::EqualEarth.project(-120.0, -40.0);
        let (c, d) = Projection::EqualEarth.project(120.0, 40.0);
        assert!((a + c).abs() < 1e-9 && (b + d).abs() < 1e-9);
        assert!(Projection::EqualEarth.project(180.0, 90.0).0 > 0.0);
        // Globe: the centre in the middle, the far side hidden, rings cut
        // at the rim.
        let g = Projection::Orthographic {
            lon: 30.0,
            lat: 10.0,
        };
        let (x, y) = g.project(30.0, 10.0);
        assert!(x.abs() < 1e-9 && y.abs() < 1e-9);
        assert!(g.project(-150.0, -10.0).0.is_nan());
        let ring: Vec<(f64, f64)> = (0..=36).map(|i| (-180.0 + i as f64 * 10.0, 0.0)).collect();
        let cut = g.project_ring(&ring).pop().unwrap();
        assert!(cut.iter().all(|(x, y)| x.hypot(*y) <= GLOBE_R + 1e-6));
        let far: Vec<(f64, f64)> = vec![(-150.0, -5.0), (-140.0, -5.0), (-145.0, -15.0)];
        assert!(g.project_ring(&far).is_empty());
    }

    use super::*;

    #[test]
    fn world_is_complete() {
        let w = world();
        assert_eq!(w.len(), 177);
        let de = country("DEU").unwrap();
        assert_eq!(de.name, "Germany");
        assert_eq!(de.iso2, "DE");
        assert!(de.population > 70e6);
        assert!(country("france").is_some());
        assert!(country("NO").is_some());
        assert!(country("Türkiye").or(country("TUR")).is_some());
    }

    #[test]
    fn projections_are_sane() {
        for p in [
            Projection::Equirectangular,
            Projection::Mercator,
            Projection::Robinson,
            Projection::NaturalEarth,
        ] {
            let (x, y) = p.project(0.0, 0.0);
            assert!(x.abs() < 1e-9 && y.abs() < 1e-9, "{p:?}");
            let (x1, _) = p.project(90.0, 0.0);
            let (x2, _) = p.project(-90.0, 0.0);
            assert!(x1 > 0.0 && (x1 + x2).abs() < 1e-9);
            let (_, y1) = p.project(0.0, 45.0);
            assert!(y1 > 0.0);
        }
        // Robinson: the equator is 0.8487 × 2π wide in radians.
        let (x, _) = Projection::Robinson.project(180.0, 0.0);
        assert!((x.to_radians() - 0.8487 * std::f64::consts::PI).abs() < 1e-9);
    }

    #[test]
    fn reads_geojson() {
        let text = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"name":"Square","pop":12.5},
           "geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}},
          {"type":"Feature","properties":{"name":"Café"},
           "geometry":{"type":"Point","coordinates":[2.5,48.8]}}]}"#;
        let f = read_geojson(text).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].property("name"), Some("Square"));
        assert_eq!(f[0].property("pop"), Some("12.5"));
        assert_eq!(f[0].polygons[0][0].len(), 5);
        assert_eq!(f[1].property("name"), Some("Café"));
        assert_eq!(f[1].points, vec![(2.5, 48.8)]);
        assert!(read_geojson("{").is_err());
    }

    #[test]
    fn choropleth_renders_and_hits() {
        let mut fig = super::super::Figure::new(800, 420);
        fig.axes()
            .choropleth([("DEU", 83.0), ("BRA", 203.0), ("IND", 1400.0)])
            .colorbar("population [M]");
        fig.axes().geo_line([28.98, -74.0], [41.01, 40.7]);
        fig.axes().geo_scatter([28.98], [41.01]);
        let (_, l) = fig.scene_and_layouts();
        // Over Brazil.
        let (x, y) = Projection::Robinson.project(-50.0, -10.0);
        let hit = fig
            .axes_at(0)
            .unwrap()
            .hit(&l[0], l[0].x.px(x), l[0].y.px(y))
            .unwrap();
        assert_eq!(hit.label, "Brazil: 203");
        assert!(fig.to_svg().contains("population"));
    }

    #[test]
    fn great_circle_splits_at_the_antimeridian() {
        let pts = great_circle((170.0, 0.0), (-170.0, 0.0), 10);
        assert!(pts.iter().any(|p| p.0.is_nan()));
        assert!(pts
            .iter()
            .filter(|p| p.0.is_finite())
            .all(|p| p.0.abs() >= 170.0 - 1e-6));
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;

    #[test]
    fn conic_and_azimuthal_projections() {
        // Centres map to the origin (y of conics at lat0).
        for p in [
            Projection::USA,
            Projection::EUROPE,
            Projection::Stereographic {
                lon: 0.0,
                lat: 90.0,
            },
            Projection::AzimuthalEqualArea {
                lon: 10.0,
                lat: 50.0,
            },
        ] {
            let (lon, lat) = match p {
                Projection::Albers { lon0, lat0, .. }
                | Projection::LambertConformal { lon0, lat0, .. } => (lon0, lat0),
                Projection::Stereographic { lon, lat }
                | Projection::AzimuthalEqualArea { lon, lat } => (lon, lat),
                _ => unreachable!(),
            };
            let (x, y) = p.project(lon, lat);
            assert!(x.abs() < 1e-9 && y.abs() < 1e-9, "{p:?}");
        }
        // Polar stereographic: the equator is the rim, the far hemisphere
        // hidden.
        let s = Projection::Stereographic {
            lon: 0.0,
            lat: 90.0,
        };
        let (x, y) = s.project(45.0, 0.0);
        assert!((x.hypot(y) - GLOBE_R).abs() < 1e-6);
        assert!(s.project(0.0, -10.0).0.is_nan());
        // A ring across the seam of a conic map (84°E for the USA map)
        // becomes two pieces; one away from it stays whole.
        let across = [(80.0, 50.0), (90.0, 50.0), (90.0, 60.0), (80.0, 60.0)];
        assert_eq!(Projection::USA.project_ring(&across).len(), 2);
        let away = [(-100.0, 30.0), (-90.0, 30.0), (-95.0, 40.0)];
        assert_eq!(Projection::USA.project_ring(&away).len(), 1);
        // Albers keeps areas: two 1°×1° cells at different longitudes match.
        let area = |lon: f64| {
            let r = [
                (lon, 40.0),
                (lon + 1.0, 40.0),
                (lon + 1.0, 41.0),
                (lon, 41.0),
            ];
            let q: Vec<_> = r
                .iter()
                .map(|&(a, b)| Projection::USA.project(a, b))
                .collect();
            (0..4)
                .map(|i| q[i].0 * q[(i + 1) % 4].1 - q[(i + 1) % 4].0 * q[i].1)
                .sum::<f64>()
                .abs()
                / 2.0
        };
        assert!((area(-120.0) / area(-80.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn map_bounds_frame_the_region() {
        let mut fig = crate::plot::Figure::new(500, 400);
        let ax = fig.axes();
        ax.map(Projection::USA);
        ax.map_bounds((-126.0, -66.0), (23.0, 50.0));
        let (x, y) = Projection::USA.project(-96.0, 37.5);
        assert!(ax.x.min.unwrap() < x && ax.x.max.unwrap() > x);
        assert!(ax.y.min.unwrap() < y && ax.y.max.unwrap() > y);
    }
}
