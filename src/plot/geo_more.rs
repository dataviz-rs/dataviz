//! More maps: states and provinces (feature `regions`), hexbin density
//! maps, flow maps and web-map tile basemaps.

use super::axes::Axes;
use super::color::{Color, Colormap};
use super::geo::{great_circle, Projection};
#[cfg(feature = "regions")]
use super::geo::{GeoMap, Polygons};
use super::scene::{Anchor, Baseline, LineCap, LineJoin, Node, RasterImage, Stroke};
use super::series::general::Shapes;
use super::series::grid::Hexbin;
use super::series::{short, ColorScale, Context, Extent, Hit, IntoValues, Rect, Series};
use crate::plot::axis::AxisMap;
use std::any::Any;
use std::sync::Arc;

// ----- states and provinces ------------------------------------------------------

/// A first-level subdivision of a country (state, province, region) of the
/// built-in admin-1 map (Natural Earth 1:10m, public domain; feature
/// `regions`).
#[cfg(feature = "regions")]
#[derive(Clone, Debug)]
pub struct Region {
    /// English name ("Bavaria", "Istanbul", "California").
    pub name: String,
    /// ISO 3166-2 code ("DE-BY", "TR-34", "US-CA").
    pub code: String,
    /// ISO 3166-1 alpha-3 code of the country ("DEU").
    pub country: String,
    /// Kind ("State", "Province", "Region", …).
    pub kind: String,
    /// Outline.
    pub polygons: Arc<Polygons>,
}

#[cfg(feature = "regions")]
impl Region {
    /// Whether `key` is this region's ISO 3166-2 code (with or without the
    /// country prefix) or name (case-insensitive).
    pub fn matches(&self, key: &str) -> bool {
        let k = key.trim();
        k.eq_ignore_ascii_case(&self.code)
            || self
                .code
                .split_once('-')
                .is_some_and(|(_, c)| k.eq_ignore_ascii_case(c))
            || k.to_lowercase() == self.name.to_lowercase()
    }

    /// Area in km² (spherical, from the outline).
    pub fn area_km2(&self) -> f64 {
        const R: f64 = 6371.0088;
        let ring_area = |r: &[(f64, f64)]| {
            let mut a = 0.0;
            for i in 0..r.len() {
                let (l1, p1) = r[i];
                let (l2, p2) = r[(i + 1) % r.len()];
                a += (l2 - l1).to_radians() * (2.0 + p1.to_radians().sin() + p2.to_radians().sin());
            }
            (a * R * R / 2.0).abs()
        };
        self.polygons
            .iter()
            .map(|p| {
                p.iter()
                    .enumerate()
                    .map(|(i, r)| if i == 0 { ring_area(r) } else { -ring_area(r) })
                    .sum::<f64>()
            })
            .sum()
    }
}

#[cfg(feature = "regions")]
static REGIONS: std::sync::LazyLock<Vec<Region>> = std::sync::LazyLock::new(|| {
    decode_regions(include_bytes!("../../assets/admin1.bin")).unwrap_or_default()
});

/// The states / provinces of a country (ISO code or name), sorted by name.
#[cfg(feature = "regions")]
pub fn regions(country: &str) -> Vec<&'static Region> {
    let iso3 = super::geo::country(country)
        .map(|c| c.iso3.clone())
        .unwrap_or_else(|| country.trim().to_uppercase());
    let mut v: Vec<&Region> = REGIONS
        .iter()
        .filter(|r| r.country.eq_ignore_ascii_case(&iso3))
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

#[cfg(feature = "regions")]
fn decode_regions(z: &[u8]) -> Option<Vec<Region>> {
    use std::io::Read;
    let mut b = Vec::new();
    flate2::read::ZlibDecoder::new(z).read_to_end(&mut b).ok()?;
    if b.get(..4)? != b"DVA1" {
        return None;
    }
    let mut at = 4usize;
    let u16_ = |at: &mut usize| -> Option<u16> {
        let v = u16::from_le_bytes(b.get(*at..*at + 2)?.try_into().ok()?);
        *at += 2;
        Some(v)
    };
    let i16_ = |at: &mut usize| -> Option<i16> {
        let v = i16::from_le_bytes(b.get(*at..*at + 2)?.try_into().ok()?);
        *at += 2;
        Some(v)
    };
    let text = |at: &mut usize| -> Option<String> {
        let len = *b.get(*at)? as usize;
        let s = std::str::from_utf8(b.get(*at + 1..*at + 1 + len)?)
            .ok()?
            .to_string();
        *at += 1 + len;
        Some(s)
    };
    let n = u16_(&mut at)?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (name, code, country, kind) = (
            text(&mut at)?,
            text(&mut at)?,
            text(&mut at)?,
            text(&mut at)?,
        );
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
        out.push(Region {
            name,
            code,
            country,
            kind,
            polygons: Arc::new(polys),
        });
    }
    Some(out)
}

// ----- flows --------------------------------------------------------------------------

/// A projected flow path (NaN splits at the antimeridian), its value and
/// name.
pub(crate) type FlowPath = (Vec<(f64, f64)>, f64, String);

/// Flows between places along great circles, width by value, with an
/// arrow at the destination. Created by [`Axes::geo_flows`].
#[derive(Clone, Debug)]
pub struct GeoFlows {
    /// Projected paths (NaN splits at the antimeridian) and values.
    pub(crate) paths: Vec<FlowPath>,
    pub(crate) color: Color,
    pub(crate) colormap: Option<Colormap>,
    pub(crate) max_width: f64,
    pub(crate) opacity: f32,
}

impl GeoFlows {
    /// Width of the largest flow in pixels (default 8).
    pub fn max_width(&mut self, w: f64) -> &mut Self {
        self.max_width = w.max(0.5);
        self
    }
    /// Colors by value through a colormap (default: one color).
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = Some(cm);
        self
    }
    /// Uniform color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self.colormap = None;
        self
    }
    /// Opacity of the flows (default 0.75).
    pub fn opacity(&mut self, a: f32) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }
    fn max_value(&self) -> f64 {
        self.paths
            .iter()
            .map(|p| p.1.abs())
            .fold(0.0, f64::max)
            .max(1e-300)
    }
}

impl Series for GeoFlows {
    fn extent(&self) -> Option<Extent> {
        Extent::of_points(
            self.paths
                .iter()
                .flat_map(|p| p.0.iter().copied())
                .filter(|p| p.0.is_finite()),
        )
    }
    fn draw(&self, ctx: &mut Context) {
        let max = self.max_value();
        // Thin flows first, so large ones stay on top.
        let mut order: Vec<usize> = (0..self.paths.len()).collect();
        order.sort_by(|a, b| self.paths[*a].1.abs().total_cmp(&self.paths[*b].1.abs()));
        for i in order {
            let (pts, v, _) = &self.paths[i];
            let t = v.abs() / max;
            let w = 0.8 + (self.max_width - 0.8) * t.sqrt();
            let c = self
                .colormap
                .map_or(self.color, |cm| cm.at(0.15 + 0.85 * t))
                .with_alpha(self.opacity);
            let px: Vec<(f64, f64)> = pts
                .iter()
                .map(|p| {
                    if p.0.is_finite() {
                        ctx.point(p.0, p.1)
                    } else {
                        (f64::NAN, f64::NAN)
                    }
                })
                .collect();
            ctx.polyline(
                &px,
                Stroke::new(c, w).cap(LineCap::Round).join(LineJoin::Round),
            );
            // Arrow head at the end, along the last segment.
            let fin: Vec<&(f64, f64)> = px.iter().filter(|p| p.0.is_finite()).collect();
            if fin.len() >= 2 {
                let (b, a) = (*fin[fin.len() - 1], *fin[fin.len() - 2]);
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let l = dx.hypot(dy).max(1e-9);
                let (ux, uy) = (dx / l, dy / l);
                let h = 4.0 + w * 1.6;
                let base = (b.0 - ux * h, b.1 - uy * h);
                let mut s = Shapes::default();
                s.poly(&[
                    b,
                    (base.0 - uy * h * 0.55, base.1 + ux * h * 0.55),
                    (base.0 + uy * h * 0.55, base.1 - ux * h * 0.55),
                ]);
                s.fill(ctx, c.with_alpha(1.0_f32.min(self.opacity + 0.15)));
            }
        }
    }
    fn hit(&self, x: &AxisMap, y: &AxisMap, _r: Rect, px: f64, py: f64) -> Option<Hit> {
        self.paths
            .iter()
            .filter_map(|(pts, v, name)| {
                let mid = pts.iter().filter(|p| p.0.is_finite()).nth(pts.len() / 2)?;
                let (a, b) = (x.px(mid.0), y.px(mid.1));
                Some(Hit {
                    px: a,
                    py: b,
                    label: format!("{name}: {}", short(*v)),
                    distance: (a - px).hypot(b - py),
                })
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn color_scale(&self) -> Option<ColorScale> {
        self.colormap.map(|cm| ColorScale {
            colormap: cm,
            min: 0.0,
            max: self.max_value(),
            label: None,
        })
    }
    fn kind(&self) -> &'static str {
        "flow map"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// ----- tiles ----------------------------------------------------------------------------

/// Web-map tiles (OpenStreetMap, MapTiler, …) under a Mercator map.
/// Created by [`Axes::basemap`].
#[derive(Clone, Debug)]
pub struct Basemap {
    /// Tiles with their extent in map coordinates (x0, y0, x1, y1).
    pub(crate) tiles: Vec<([f64; 4], Arc<RasterImage>)>,
    pub(crate) attribution: String,
    pub(crate) opacity: f32,
}

impl Basemap {
    /// Opacity of the tiles (lighter under data).
    pub fn opacity(&mut self, a: f32) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }
}

/// Map x (longitude) of tile column `x` at zoom `z`.
fn tile_x(x: f64, z: u32) -> f64 {
    -180.0 + 360.0 * x / (1u64 << z) as f64
}

/// Map y (Mercator, degrees) of tile row `y` at zoom `z`.
fn tile_y(y: f64, z: u32) -> f64 {
    180.0 * (1.0 - 2.0 * y / (1u64 << z) as f64)
}

/// The tiles `(x, y)` at zoom `z` covering a longitude/latitude box.
pub fn tile_range(lon: (f64, f64), lat: (f64, f64), z: u32) -> Vec<(u32, u32)> {
    let n = (1u64 << z) as f64;
    let tx = |l: f64| (((l + 180.0) / 360.0 * n).floor().clamp(0.0, n - 1.0)) as u32;
    let ty = |p: f64| {
        let r = p.clamp(-85.0511, 85.0511).to_radians();
        ((1.0 - (r.tan() + 1.0 / r.cos()).ln() / std::f64::consts::PI) / 2.0 * n)
            .floor()
            .clamp(0.0, n - 1.0) as u32
    };
    let (x0, x1) = (tx(lon.0.min(lon.1)), tx(lon.0.max(lon.1)));
    let (y0, y1) = (ty(lat.0.max(lat.1)), ty(lat.0.min(lat.1)));
    (y0..=y1)
        .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
        .collect()
}

/// A tile source reading `{dir}/{z}/{x}/{y}.png` (or .jpg) files — e.g. a
/// cache filled by a downloader.
pub fn tile_dir(
    dir: impl Into<std::path::PathBuf>,
) -> impl FnMut(u32, u32, u32) -> Option<RasterImage> {
    let dir = dir.into();
    move |z, x, y| {
        let base = dir.join(z.to_string()).join(x.to_string());
        let path = ["png", "jpg", "jpeg", "webp"]
            .iter()
            .map(|e| base.join(format!("{y}.{e}")))
            .find(|p| p.exists())?;
        let img = image::open(path).ok()?.to_rgba8();
        Some(RasterImage {
            width: img.width(),
            height: img.height(),
            rgba: img.into_raw(),
        })
    }
}

impl Series for Basemap {
    fn extent(&self) -> Option<Extent> {
        let mut e: Option<Extent> = None;
        for (b, _) in &self.tiles {
            e = Some(match e {
                None => Extent::point(b[0], b[1]).include(b[2], b[3]),
                Some(e) => e.include(b[0], b[1]).include(b[2], b[3]),
            });
        }
        e
    }
    fn draw(&self, ctx: &mut Context) {
        let mut nodes = Vec::new();
        for (b, img) in &self.tiles {
            let (x0, y0) = (ctx.x.px(b[0]), ctx.y.px(b[3]));
            let (x1, y1) = (ctx.x.px(b[2]), ctx.y.px(b[1]));
            // A hair larger: no seams between neighbouring tiles.
            nodes.push(Node::Image {
                x: x0.min(x1) - 0.25,
                y: y0.min(y1) - 0.25,
                w: (x1 - x0).abs() + 0.5,
                h: (y1 - y0).abs() + 0.5,
                image: img.clone(),
                smooth: true,
            });
        }
        ctx.push(Node::Group {
            children: nodes,
            clip: None,
            opacity: self.opacity as f64,
            tooltip: None,
            id: None,
        });
        if !self.attribution.is_empty() {
            let ts = ctx.theme.tick_size * 0.8;
            let w = ctx.text_width(&self.attribution, ts) + 8.0;
            let (r, bt) = (ctx.rect.right(), ctx.rect.bottom());
            ctx.rect(
                r - w,
                bt - ts * 1.6,
                r,
                bt,
                Some(Color::rgba(255, 255, 255, 0.8)),
                None,
            );
            ctx.text(
                r - 4.0,
                bt - ts * 0.8,
                self.attribution.clone(),
                ts,
                Color::rgb(40, 40, 40),
                Anchor::End,
                Baseline::Middle,
            );
        }
    }
    fn kind(&self) -> &'static str {
        "basemap"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Axes {
    /// Map of a country's states / provinces colored by value: keys are
    /// ISO 3166-2 codes ("TR-34", or "34"), or names; regions without a
    /// value are grey (empty `values`: a plain map). Mercator unless a
    /// projection was set. Feature `regions`.
    #[cfg(feature = "regions")]
    pub fn region_map<K: AsRef<str>>(
        &mut self,
        country: &str,
        values: impl IntoIterator<Item = (K, f64)>,
    ) -> &mut GeoMap {
        if self.projection.is_none() {
            self.map(Projection::Mercator);
        }
        let p = self.map_projection();
        let rs = regions(country);
        let mut v = vec![f64::NAN; rs.len()];
        let mut any = false;
        for (k, val) in values {
            if let Some(i) = rs.iter().position(|r| r.matches(k.as_ref())) {
                v[i] = val;
                any = true;
            }
        }
        if !any {
            v.clear();
        }
        let shapes = rs
            .iter()
            .map(|r| (format!("{} ({})", r.name, r.code), r.polygons.clone()))
            .collect();
        self.add(GeoMap::new(shapes, v, p))
    }

    /// Density of points (longitude, latitude) on a map as hexagonal bins
    /// (log colors, light for few points).
    pub fn geo_hexbin(&mut self, lon: impl IntoValues, lat: impl IntoValues) -> &mut Hexbin {
        let p = self.map_projection();
        let (x, y): (Vec<f64>, Vec<f64>) = lon
            .into_values()
            .into_iter()
            .zip(lat.into_values())
            .map(|(a, b)| p.project(a, b))
            .unzip();
        let h = self.hexbin(x, y);
        // Light low counts over the map, log scale for clustered data.
        h.colormap = Colormap::Reds;
        h.log = true;
        h
    }

    /// Flows `(from lon, from lat, to lon, to lat, value)` along great
    /// circles, wider for larger values, arrows at the destinations; `names`
    /// label them in tooltips (empty: none).
    pub fn geo_flows<S: AsRef<str>>(
        &mut self,
        flows: &[(f64, f64, f64, f64, f64)],
        names: &[S],
    ) -> &mut GeoFlows {
        let p = self.map_projection();
        let paths = flows
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let pts = great_circle((f.0, f.1), (f.2, f.3), 48)
                    .into_iter()
                    .map(|(lo, la)| {
                        if lo.is_finite() {
                            p.project(lo, la)
                        } else {
                            (f64::NAN, f64::NAN)
                        }
                    })
                    .collect();
                (
                    pts,
                    f.4,
                    names
                        .get(i)
                        .map_or(String::new(), |n| n.as_ref().to_string()),
                )
            })
            .collect();
        let color = self.next_color();
        self.add(GeoFlows {
            paths,
            color,
            colormap: None,
            max_width: 8.0,
            opacity: 0.75,
        })
    }

    /// Web-map tiles at `zoom` covering a longitude/latitude box, from a
    /// tile source `(z, x, y) → image` (e.g. [`tile_dir`]); the map is set to
    /// Mercator (the tiles' projection). Credit the tile provider in
    /// `attribution` (OpenStreetMap: "© OpenStreetMap contributors").
    pub fn basemap(
        &mut self,
        lon: (f64, f64),
        lat: (f64, f64),
        zoom: u32,
        mut tiles: impl FnMut(u32, u32, u32) -> Option<RasterImage>,
        attribution: &str,
    ) -> &mut Basemap {
        self.map(Projection::Mercator);
        let z = zoom.min(22);
        let list = tile_range(lon, lat, z)
            .into_iter()
            .filter_map(|(x, y)| {
                let img = tiles(z, x, y)?;
                let b = [
                    tile_x(x as f64, z),
                    tile_y(y as f64 + 1.0, z),
                    tile_x(x as f64 + 1.0, z),
                    tile_y(y as f64, z),
                ];
                Some((b, Arc::new(img)))
            })
            .collect();
        // Only the requested box is shown (tiles reach beyond it).
        let p = Projection::Mercator;
        let (x0, y0) = p.project(lon.0.min(lon.1), lat.0.min(lat.1));
        let (x1, y1) = p.project(lon.0.max(lon.1), lat.0.max(lat.1));
        self.x.limits(x0, x1);
        self.y.limits(y0, y1);
        self.add(Basemap {
            tiles: list,
            attribution: attribution.to_string(),
            opacity: 1.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn tiles_cover_the_box_and_map_linearly() {
        // Zoom 1: four tiles; Istanbul is in the north-east one.
        assert_eq!(tile_range((28.9, 29.1), (41.0, 41.1), 1), vec![(1, 0)]);
        assert_eq!(tile_range((-10.0, 10.0), (-10.0, 10.0), 1).len(), 4);
        // Tile edges in Mercator map units match the projection: row 1 of
        // zoom 2 starts at latitude atan(sinh(π/2)) = 66.513°.
        let lat = (std::f64::consts::FRAC_PI_2).sinh().atan().to_degrees();
        let y = tile_y(1.0, 2);
        assert!(
            (y - Projection::Mercator.project(0.0, lat).1).abs() < 1e-9,
            "{y}"
        );
        assert_eq!(tile_x(1.0, 1), 0.0);
    }

    #[test]
    fn flows_hexbin_and_basemap_render() {
        let mut fig = Figure::new(600, 400);
        fig.grid(1, 2);
        fig.at(0, 0).world_map();
        fig.at(0, 0)
            .geo_flows(
                &[
                    (28.9, 41.0, -74.0, 40.7, 10.0),
                    (2.3, 48.8, 139.7, 35.7, 4.0),
                ],
                &["IST–NYC", "PAR–TYO"],
            )
            .colormap(Colormap::Plasma);
        fig.at(0, 0)
            .geo_hexbin([10.0, 11.0, 12.0], [50.0, 51.0, 50.5]);
        let tile = |_z: u32, x: u32, y: u32| {
            Some(RasterImage {
                width: 2,
                height: 2,
                rgba: vec![((x * 40 + y * 20) % 255) as u8; 16],
            })
        };
        fig.at(0, 1)
            .basemap((20.0, 45.0), (35.0, 43.0), 4, tile, "test tiles");
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
    }

    #[cfg(feature = "regions")]
    #[test]
    fn regions_of_countries() {
        assert_eq!(regions("TUR").len(), 81);
        assert_eq!(regions("Turkey").len(), 81);
        assert_eq!(
            regions("US")
                .iter()
                .filter(|r| r.kind != "Federal District")
                .count(),
            50
        );
        let ist = regions("TUR")
            .into_iter()
            .find(|r| r.matches("TR-34"))
            .unwrap();
        assert!(
            ist.matches("34") && ist.name.to_lowercase().contains("istanbul"),
            "{}",
            ist.name
        );
        // Istanbul province: about 5 460 km².
        assert!(
            (ist.area_km2() - 5460.0).abs() < 700.0,
            "{}",
            ist.area_km2()
        );
        let by = regions("DE")
            .into_iter()
            .find(|r| r.code == "DE-BY")
            .unwrap();
        assert!(
            (by.area_km2() - 70_550.0).abs() / 70_550.0 < 0.05,
            "{}",
            by.area_km2()
        );
        let mut fig = Figure::new(500, 300);
        fig.axes()
            .region_map("TUR", [("TR-34", 15.7), ("TR-06", 5.8), ("Izmir", 4.5)])
            .colorbar("M");
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
    }
}
