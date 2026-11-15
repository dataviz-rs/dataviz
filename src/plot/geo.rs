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
}

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

    /// The outline of the whole globe in this projection.
    fn outline(self) -> Vec<(f64, f64)> {
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
                    .map(|ring| {
                        ring.iter()
                            .map(|(lon, lat)| p.project(*lon, *lat))
                            .collect()
                    })
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
        let mut lat = -top + (top % step);
        while lat <= top + 1e-9 {
            lines.push(
                (0..=72)
                    .map(|i| p.project(-180.0 + i as f64 * 5.0, lat))
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
