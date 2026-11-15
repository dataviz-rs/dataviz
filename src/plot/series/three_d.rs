//! 3D charts: surface, wireframe, scatter, line, spheres, meshes.
//!
//! All 3D series of an axes share one scene: one data box, one camera and
//! one depth order, so a molecule's bonds pass behind its atoms and a
//! trajectory through a surface is hidden where it should be. The camera
//! turns freely around the x, y and z axes ([`Axes::rotate`]); in a window
//! (and through [`Figure::pan_by`](crate::plot::Figure::pan_by)) dragging
//! turns the scene, the wheel zooms, `X`/`Y`/`Z` rotate by 15° around that
//! axis (with Shift the other way) and `R` returns to the start.
//!
//! Faces are lit by a light from the upper left (diffuse and specular),
//! spheres are drawn shaded, and [`Axes::depth_cue`] fades far parts
//! toward the background for depth perception.

use super::{
    short, ColorScale, Context, Extent, Hit, IntoValues, LegendEntry, LegendSymbol, Rect, Series,
};
use crate::plot::axes::{Axes, AxesKind};
use crate::plot::color::{Color, Colormap};
use crate::plot::scene::{Anchor, Baseline, LineCap, LineJoin, Node, PathCmd, Stroke, TextNode};
use crate::plot::style::Marker;
use crate::plot::text::FontSet;
use crate::plot::theme::Theme;
use crate::plot::ticks;
use std::any::Any;

/// Data range of a 3D scene: `[(x_min, x_max), (y_min, y_max), (z_min, z_max)]`.
pub type Bounds3 = [(f64, f64); 3];

type M3 = [[f64; 3]; 3];

fn mul(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for (i, row) in o.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

/// Rotation by `deg` around basis axis `axis` (right-handed).
fn rotation(axis: usize, deg: f64) -> M3 {
    let (s, c) = deg.to_radians().sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        1 => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        _ => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt();
    if l > 0.0 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// Rows made orthonormal again (rotations add up rounding errors).
fn orthonormal(m: M3) -> M3 {
    let r0 = unit(m[0]);
    let r1 = unit(sub(m[1], {
        let d = dot(m[1], r0);
        [r0[0] * d, r0[1] * d, r0[2] * d]
    }));
    [r0, r1, cross(r0, r1)]
}

/// One of the three data axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis3 {
    /// x-axis.
    X,
    /// y-axis.
    Y,
    /// z-axis.
    Z,
}

impl Axis3 {
    fn index(self) -> usize {
        match self {
            Axis3::X => 0,
            Axis3::Y => 1,
            Axis3::Z => 2,
        }
    }
}

/// Orientation, zoom and perspective of a 3D scene.
///
/// The orientation is a rotation matrix, so the scene can be turned
/// freely around any axis (no gimbal lock): start from azimuth and
/// elevation ([`Camera::new`]) and add rotations around the data axes
/// ([`Camera::rotate`]) or the screen axes ([`Camera::turn`],
/// [`Camera::roll`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// Data → view rotation (rows: screen right, screen up, towards viewer).
    m: M3,
    /// Magnification (1 = the scene fits the plot).
    pub zoom: f64,
    /// Perspective strength (0 = orthographic, 1 = strong).
    pub perspective: f64,
    /// Offset of the scene centre from the plot centre, in units of the
    /// plot's smaller side (set by zooming at the cursor).
    pub shift: (f64, f64),
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(-60.0, 30.0)
    }
}

impl Camera {
    /// Camera looking from `azimuth` degrees around the z-axis and
    /// `elevation` degrees above the x/y plane (z points up).
    pub fn new(azimuth: f64, elevation: f64) -> Self {
        let (sa, ca) = azimuth.to_radians().sin_cos();
        let (se, ce) = elevation.clamp(-90.0, 90.0).to_radians().sin_cos();
        Self {
            m: [
                [ca, -sa, 0.0],
                [sa * se, ca * se, ce],
                [-sa * ce, -ca * ce, se],
            ],
            zoom: 1.0,
            perspective: 0.0,
            shift: (0.0, 0.0),
        }
    }

    /// A camera that shows the longest extent of `points` across the
    /// screen and the second longest upwards (principal axes).
    pub fn principal(points: &[[f64; 3]]) -> Self {
        let n = points.len().max(1) as f64;
        let mut mean = [0.0; 3];
        for p in points {
            for k in 0..3 {
                mean[k] += p[k] / n;
            }
        }
        let mut c = [[0.0; 3]; 3];
        for p in points {
            let d = sub(*p, mean);
            for i in 0..3 {
                for j in 0..3 {
                    c[i][j] += d[i] * d[j] / n;
                }
            }
        }
        let (vals, vecs) = jacobi(c);
        let mut order = [0usize, 1, 2];
        order.sort_by(|a, b| vals[*b].total_cmp(&vals[*a]));
        let col = |k: usize| [vecs[0][k], vecs[1][k], vecs[2][k]];
        let (r0, r1) = (unit(col(order[0])), unit(col(order[1])));
        Self {
            m: orthonormal([r0, r1, cross(r0, r1)]),
            zoom: 1.0,
            perspective: 0.0,
            shift: (0.0, 0.0),
        }
    }

    /// Turns the scene by `deg` degrees around a data axis.
    pub fn rotate(&mut self, axis: Axis3, deg: f64) -> &mut Self {
        self.m = orthonormal(mul(&self.m, &rotation(axis.index(), deg)));
        self
    }

    /// Turns the scene around the screen axes: `horizontal` degrees around
    /// the vertical screen axis, `vertical` degrees around the horizontal
    /// one (positive tilts the top towards the viewer).
    pub fn turn(&mut self, horizontal: f64, vertical: f64) -> &mut Self {
        let m = mul(&rotation(1, horizontal), &self.m);
        self.m = orthonormal(mul(&rotation(0, -vertical), &m));
        self
    }

    /// Turns the scene around the viewing direction (clockwise for positive
    /// degrees).
    pub fn roll(&mut self, deg: f64) -> &mut Self {
        self.m = orthonormal(mul(&rotation(2, -deg), &self.m));
        self
    }

    /// Drag rotation: horizontal moves turn around the data z-axis (a
    /// turntable), vertical moves tilt around the screen's horizontal axis.
    pub fn drag(&mut self, dx: f64, dy: f64) -> &mut Self {
        self.rotate(Axis3::Z, dx * 0.5);
        self.m = orthonormal(mul(&rotation(0, dy * 0.5), &self.m));
        self
    }

    /// View coordinates (screen right, screen up, towards the viewer) of a
    /// direction in (normalized) data space.
    pub fn view(&self, v: [f64; 3]) -> [f64; 3] {
        [dot(self.m[0], v), dot(self.m[1], v), dot(self.m[2], v)]
    }

    /// The rotation matrix (data → view).
    pub fn matrix(&self) -> [[f64; 3]; 3] {
        self.m
    }
}

/// Eigenvalues and eigenvectors (columns) of a symmetric 3×3 matrix.
fn jacobi(mut a: M3) -> ([f64; 3], M3) {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let (mut p, mut q, mut big) = (0, 1, 0.0);
        for (i, j) in [(0, 1), (0, 2), (1, 2)] {
            if a[i][j].abs() > big {
                (p, q, big) = (i, j, a[i][j].abs());
            }
        }
        if big < 1e-14 {
            break;
        }
        let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
        let t = theta.signum().max(0.0) * 2.0 - 1.0;
        let t = t / (theta.abs() + (theta * theta + 1.0).sqrt());
        let c = 1.0 / (t * t + 1.0).sqrt();
        let s = t * c;
        let mut r = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        r[p][p] = c;
        r[q][q] = c;
        r[p][q] = s;
        r[q][p] = -s;
        let rt = [
            [r[0][0], r[1][0], r[2][0]],
            [r[0][1], r[1][1], r[2][1]],
            [r[0][2], r[1][2], r[2][2]],
        ];
        a = mul(&rt, &mul(&a, &r));
        v = mul(&v, &r);
    }
    ([a[0][0], a[1][1], a[2][2]], v)
}

/// Settings of the 3D scene of an axes.
#[derive(Clone, Debug)]
pub(crate) struct View3d {
    pub camera: Camera,
    pub home: Camera,
    pub show_box: bool,
    pub z_label: Option<String>,
    pub z_limits: (Option<f64>, Option<f64>),
    pub fog: f64,
    pub light: bool,
}

impl Default for View3d {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            home: Camera::default(),
            show_box: true,
            z_label: None,
            z_limits: (None, None),
            fog: 0.0,
            light: true,
        }
    }
}

/// Mapping of data coordinates to the canvas for one rendered 3D scene.
#[derive(Clone, Debug)]
pub struct Projection3d {
    bounds: Bounds3,
    mid: [f64; 3],
    /// Data unit → normalized box unit, per axis.
    k: [f64; 3],
    /// Half sizes of the box in normalized units.
    ext: [f64; 3],
    cam: Camera,
    scale: f64,
    center: (f64, f64),
    radius: f64,
    dist: f64,
}

impl Projection3d {
    pub(crate) fn new(
        bounds: Bounds3,
        equal: bool,
        cam: Camera,
        home: Camera,
        boxed: bool,
        r: Rect,
    ) -> Self {
        let half: Vec<f64> = bounds
            .iter()
            .map(|(a, b)| ((b - a) / 2.0).max(1e-12))
            .collect();
        let mid = [
            (bounds[0].0 + bounds[0].1) / 2.0,
            (bounds[1].0 + bounds[1].1) / 2.0,
            (bounds[2].0 + bounds[2].1) / 2.0,
        ];
        let (k, ext) = if equal {
            let h = half.iter().copied().fold(0.0, f64::max);
            ([1.0 / h; 3], [half[0] / h, half[1] / h, half[2] / h])
        } else {
            let ext = [1.0, 1.0, 0.75];
            ([ext[0] / half[0], ext[1] / half[1], ext[2] / half[2]], ext)
        };
        let radius = dot(ext, ext).sqrt();
        let dist = if cam.perspective > 0.0 {
            radius * (1.0 + 2.0 / cam.perspective.min(1.0))
        } else {
            0.0
        };
        // The box as seen from the starting camera fits the plot; the
        // scale stays while the scene turns, so it does not pump.
        let (mut x0, mut x1, mut y0, mut y1) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for i in 0..8 {
            let c = [
                if i & 1 == 0 { -ext[0] } else { ext[0] },
                if i & 2 == 0 { -ext[1] } else { ext[1] },
                if i & 4 == 0 { -ext[2] } else { ext[2] },
            ];
            let v = home.view(c);
            let f = if dist > 0.0 {
                dist / (dist - v[2])
            } else {
                1.0
            };
            (x0, x1) = (x0.min(v[0] * f), x1.max(v[0] * f));
            (y0, y1) = (y0.min(v[1] * f), y1.max(v[1] * f));
        }
        // A boxed scene leaves room for tick and axis labels around it.
        let (pw, ph, margin) = if boxed {
            (
                (r.w - 110.0).max(r.w * 0.5),
                (r.h - 50.0).max(r.h * 0.5),
                0.94,
            )
        } else {
            (r.w, r.h, 0.95)
        };
        let scale =
            (pw / (x1 - x0).max(1e-9)).min(ph / (y1 - y0).max(1e-9)) * margin * cam.zoom.max(1e-3);
        Self {
            bounds,
            mid,
            k,
            ext,
            cam,
            scale,
            center: {
                let (cx, cy) = r.center();
                let m = r.w.min(r.h);
                (cx + cam.shift.0 * m, cy + cam.shift.1 * m)
            },
            radius,
            dist,
        }
    }

    /// Data range shown.
    pub fn bounds(&self) -> Bounds3 {
        self.bounds
    }

    /// The camera of the scene.
    pub fn camera(&self) -> Camera {
        self.cam
    }

    /// Normalized box coordinates of a data point.
    fn norm(&self, v: [f64; 3]) -> [f64; 3] {
        [
            (v[0] - self.mid[0]) * self.k[0],
            (v[1] - self.mid[1]) * self.k[1],
            (v[2] - self.mid[2]) * self.k[2],
        ]
    }

    fn screen(&self, q: [f64; 3]) -> (f64, f64, f64) {
        let f = self.persp(q[2]);
        (
            self.center.0 + q[0] * self.scale * f,
            self.center.1 - q[1] * self.scale * f,
            q[2],
        )
    }

    fn persp(&self, depth: f64) -> f64 {
        if self.dist > 0.0 {
            self.dist / (self.dist - depth).max(self.radius * 0.05)
        } else {
            1.0
        }
    }

    /// View coordinates of a data point.
    fn view(&self, v: [f64; 3]) -> [f64; 3] {
        self.cam.view(self.norm(v))
    }

    /// Canvas position and depth (larger = nearer the viewer) of a data point.
    pub fn project(&self, v: [f64; 3]) -> (f64, f64, f64) {
        self.screen(self.view(v))
    }

    fn project_box(&self, c: [f64; 3]) -> (f64, f64, f64) {
        self.screen(self.cam.view(c))
    }

    /// Pixels per data unit along x at a depth.
    pub fn px_per_unit(&self, depth: f64) -> f64 {
        self.k[0] * self.scale * self.persp(depth)
    }

    /// Depth mapped to 0 (farthest) … 1 (nearest).
    fn nearness(&self, depth: f64) -> f64 {
        ((depth / self.radius + 1.0) / 2.0).clamp(0.0, 1.0)
    }
}

/// Light direction in view space (upper left, towards the viewer).
const LIGHT: [f64; 3] = [-0.42, 0.56, 0.71];

/// Brightness of a face with view-space normal `n` (two-sided).
fn lambert(n: [f64; 3]) -> (f64, f64) {
    let n = if n[2] < 0.0 { [-n[0], -n[1], -n[2]] } else { n };
    let d = dot(n, LIGHT).max(0.0);
    let h = unit([LIGHT[0], LIGHT[1], LIGHT[2] + 1.0]);
    let s = dot(n, h).max(0.0).powi(28);
    (0.42 + 0.62 * d, 0.32 * s)
}

fn lit(c: Color, (diffuse, spec): (f64, f64)) -> Color {
    let base = if diffuse < 1.0 {
        c.darken(1.0 - diffuse)
    } else {
        c.lighten((diffuse - 1.0) * 0.6)
    };
    if spec > 0.004 {
        base.lighten(spec)
    } else {
        base
    }
}

enum Prim {
    Poly {
        pts: [(f32, f32); 4],
        n: u8,
        fill: Option<Color>,
        stroke: Option<Box<Stroke>>,
    },
    Ball {
        x: f32,
        y: f32,
        r: f32,
        color: Color,
        shaded: bool,
    },
    Line {
        pts: Vec<(f64, f64)>,
        color: Color,
        width: f64,
    },
    Seg {
        a: (f64, f64),
        b: (f64, f64),
        color: Color,
        width: f64,
        tube: bool,
    },
    /// Already drawn.
    Taken,
}

/// Sorts keys with four 16-bit radix passes (linear time; a million
/// primitives in a few milliseconds).
fn radix_sort(mut keys: Vec<u64>) -> Vec<u64> {
    if keys.len() < 2048 {
        keys.sort_unstable();
        return keys;
    }
    let mut tmp = vec![0u64; keys.len()];
    // Only the depth half (high 32 bits) needs sorting; the index half
    // keeps equal depths in insertion order.
    for shift in [32u32, 48] {
        let mut count = vec![0usize; 1 << 16];
        for &k in &keys {
            count[((k >> shift) & 0xFFFF) as usize] += 1;
        }
        let mut sum = 0;
        for c in count.iter_mut() {
            let v = *c;
            *c = sum;
            sum += v;
        }
        for &k in &keys {
            let b = ((k >> shift) & 0xFFFF) as usize;
            tmp[count[b]] = k;
            count[b] += 1;
        }
        std::mem::swap(&mut keys, &mut tmp);
    }
    keys
}

struct Label3 {
    x: f64,
    y: f64,
    text: String,
    size: f64,
    color: Color,
    background: Option<Color>,
    bold: bool,
}

/// What 3D series draw into: primitives in data coordinates that the axes
/// sorts by depth (farthest first) and draws with lighting.
pub struct Scene3d<'a> {
    proj: &'a Projection3d,
    prims: Vec<Prim>,
    /// Depth (order-preserving bits, high half) and index (low half) of
    /// each primitive: sorted instead of the primitives.
    keys: Vec<u64>,
    labels: Vec<Label3>,
    fog: f64,
    background: Color,
    light: bool,
    /// Recording for interactive HTML instead of drawing.
    rec: Option<Rec3>,
    /// Capture of opaque faces for the GPU instead of drawing.
    mesh: Option<MeshCapture>,
}

/// GPU mesh of an axes: valid while the series (`revision`) and the
/// normalization of the box (`norm`) are unchanged; `vertices` is `None`
/// when the scene cannot be a mesh.
pub(crate) struct MeshCache {
    revision: u64,
    norm: [u64; 6],
    id: u64,
    vertices: Option<std::sync::Arc<Vec<f32>>>,
}

impl MeshCache {
    /// Identity of the cached mesh.
    #[cfg(test)]
    pub(crate) fn id(&self) -> u64 {
        self.id
    }
}

/// Clip rectangle `[l, t, r, b]` of a plot area (canvas units).
fn self_clip(r: Rect) -> [f32; 4] {
    [
        r.x as f32,
        r.y as f32,
        (r.x + r.w) as f32,
        (r.y + r.h) as f32,
    ]
}

/// Faces captured in normalized box coordinates for [`Node::Mesh3d`];
/// `other` is set when the scene has anything a mesh cannot show (lines,
/// points, text, edges, translucent faces).
#[derive(Default)]
struct MeshCapture {
    vertices: Vec<f32>,
    other: bool,
}

/// 3D primitives recorded in normalized box coordinates (before the
/// camera), as JSON fragments for the HTML page's 3D renderer.
#[derive(Default)]
pub(crate) struct Rec3 {
    items: Vec<String>,
}

fn j4(v: f64) -> String {
    if v.is_finite() {
        let r = (v * 10_000.0).round() / 10_000.0;
        format!("{r}")
    } else {
        "null".into()
    }
}

fn jc(c: Color) -> String {
    format!("[{},{},{},{}]", c.r, c.g, c.b, j4(c.a as f64))
}

fn jp(p: [f64; 3]) -> String {
    format!("{},{},{}", j4(p[0]), j4(p[1]), j4(p[2]))
}

impl Scene3d<'_> {
    /// Rotated view coordinates back to normalized box coordinates (the
    /// rotation is orthonormal: its inverse is the transpose).
    fn unview(&self, v: [f64; 3]) -> [f64; 3] {
        let m = self.proj.cam.m;
        [
            m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
            m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
            m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
        ]
    }

    fn record(&mut self, item: String) {
        if let Some(r) = &mut self.rec {
            r.items.push(item);
        }
    }

    /// The projection of this scene.
    pub fn projection(&self) -> &Projection3d {
        self.proj
    }

    /// Room for `n` more primitives (large series).
    pub fn reserve(&mut self, n: usize) {
        self.prims.reserve(n);
        self.keys.reserve(n);
    }

    fn add(&mut self, depth: f64, p: Prim) {
        if let Some(m) = &mut self.mesh {
            m.other = true;
            return;
        }
        // f32 bits flipped so that unsigned order is numeric order.
        let b = (depth as f32).to_bits();
        let ord = if b >> 31 == 1 { !b } else { b | 0x8000_0000 };
        self.keys
            .push(((ord as u64) << 32) | self.prims.len() as u64);
        self.prims.push(p);
    }

    /// A line through points (`width` pixels), drawn in pieces that are
    /// sorted by depth with the rest of the scene. `colors` (one per point)
    /// color the pieces; otherwise `color` is used.
    pub fn polyline(
        &mut self,
        pts: &[[f64; 3]],
        color: Color,
        colors: Option<&[Color]>,
        width: f64,
    ) {
        if self.rec.is_some() {
            let p: Vec<String> = pts.iter().map(|q| jp(self.proj.norm(*q))).collect();
            let cs = colors.map_or(String::new(), |c| {
                format!(
                    ",\"cs\":[{}]",
                    c.iter().map(|c| jc(*c)).collect::<Vec<_>>().join(",")
                )
            });
            let item = format!(
                "{{\"k\":\"l\",\"p\":[{}],\"c\":{},\"w\":{}{cs}}}",
                p.join(","),
                jc(color),
                j4(width)
            );
            self.record(item);
            return;
        }
        // Pieces of up to 64 kept points; steps shorter than a third of a
        // pixel are merged into the next (dense trajectories have most of
        // their segments below a pixel, which adds work but no detail).
        const PIECE: usize = 64;
        const MIN_STEP2: f64 = 0.35 * 0.35;
        let mut screen: Vec<(f64, f64)> = Vec::with_capacity(PIECE + 1);
        let (mut depth, mut first) = (0.0, 0usize);
        let flush = |me: &mut Self,
                     screen: &mut Vec<(f64, f64)>,
                     depth: &mut f64,
                     first: usize,
                     last: usize| {
            if screen.len() >= 2 {
                let d = *depth / screen.len() as f64;
                let c = colors
                    .and_then(|c| c.get((first + last) / 2).copied())
                    .unwrap_or(color);
                let c = me.cue(c, d);
                let w = width * me.proj.persp(d);
                me.add(
                    d,
                    Prim::Line {
                        pts: std::mem::take(screen),
                        color: c,
                        width: w,
                    },
                );
            }
            screen.clear();
            *depth = 0.0;
        };
        for (k, p) in pts.iter().enumerate() {
            let (x, y, d) = self.proj.project(*p);
            if !(x.is_finite() && y.is_finite()) {
                flush(self, &mut screen, &mut depth, first, k);
                first = k + 1;
                continue;
            }
            let last = k + 1 == pts.len();
            if let Some(&(lx, ly)) = screen.last() {
                if !last && (x - lx) * (x - lx) + (y - ly) * (y - ly) < MIN_STEP2 {
                    continue;
                }
            }
            screen.push((x, y));
            depth += d;
            if screen.len() > PIECE {
                // The next piece starts where this one ends (no gap).
                let (end, dd) = (*screen.last().unwrap(), d);
                flush(self, &mut screen, &mut depth, first, k);
                screen.push(end);
                depth = dd;
                first = k;
            }
        }
        flush(
            self,
            &mut screen,
            &mut depth,
            first,
            pts.len().saturating_sub(1),
        );
    }

    fn cue(&self, c: Color, depth: f64) -> Color {
        if self.fog > 0.0 {
            c.mix(
                self.background.with_alpha(c.a),
                self.fog * (1.0 - self.proj.nearness(depth)),
            )
        } else {
            c
        }
    }

    /// A flat polygon (3 or 4 corners; more are split into a fan) lit by
    /// its orientation. `edge` outlines it.
    pub fn polygon(&mut self, v: &[[f64; 3]], color: Color, edge: Option<Stroke>) {
        self.face(v, color, edge, None, false);
    }

    /// A polygon (3 or 4 corners) with a known view-space normal.
    pub(crate) fn face_data(&mut self, v: &[[f64; 3]], color: Color, normal: Option<[f64; 3]>) {
        self.face(v, color, None, normal, false);
    }

    /// A polygon with a given view-space normal (for smooth meshes) or
    /// computed from the corners; with `cull` faces turned away are skipped
    /// (`normal` must then point outwards).
    fn face(
        &mut self,
        v: &[[f64; 3]],
        color: Color,
        edge: Option<Stroke>,
        normal: Option<[f64; 3]>,
        cull: bool,
    ) {
        if v.len() < 3 {
            return;
        }
        if v.len() > 4 {
            for i in 1..v.len() - 1 {
                self.face(&[v[0], v[i], v[i + 1]], color, edge.clone(), normal, cull);
            }
            return;
        }
        let mut q = [[0.0; 3]; 4];
        for (o, p) in q.iter_mut().zip(v) {
            *o = self.proj.view(*p);
        }
        self.face_view(&q[..v.len()], color, edge, normal, cull);
    }

    /// [`Scene3d::face`] with corners already in view coordinates (3 or 4).
    pub(crate) fn face_view(
        &mut self,
        q: &[[f64; 3]],
        color: Color,
        edge: Option<Stroke>,
        normal: Option<[f64; 3]>,
        cull: bool,
    ) {
        if q.iter().any(|p| !p.iter().all(|c| c.is_finite())) {
            return;
        }
        if self.mesh.is_some() {
            let ok = edge.is_none() && color.a >= 0.99 && q.len() >= 3;
            let n = normal.unwrap_or_else(|| unit(cross(sub(q[1], q[0]), sub(q[2], q[0]))));
            let (n, pts) = (
                self.unview(n),
                q.iter().map(|v| self.unview(*v)).collect::<Vec<_>>(),
            );
            if let Some(m) = self.mesh.as_mut() {
                if !ok {
                    m.other = true;
                    return;
                }
                let c = [color.r, color.g, color.b].map(|v| v as f32 / 255.0);
                for i in 1..pts.len() - 1 {
                    for p in [pts[0], pts[i], pts[i + 1]] {
                        m.vertices.extend(p.map(|v| v as f32));
                        m.vertices.extend(n.map(|v| v as f32));
                        m.vertices.extend(c);
                        m.vertices.push(color.a);
                    }
                }
            }
            return;
        }
        if self.rec.is_some() {
            let pts: Vec<String> = q.iter().map(|v| jp(self.unview(*v))).collect();
            let n = normal.map_or(String::new(), |n| {
                format!(",\"n\":[{}]", jp(self.unview(n)))
            });
            let e = edge.map_or(String::new(), |e| {
                format!(",\"e\":{},\"ew\":{}", jc(e.color), j4(e.width))
            });
            let item = format!(
                "{{\"k\":\"f\",\"p\":[{}],\"c\":{}{n}{e}{}}}",
                pts.join(","),
                jc(color),
                if cull { ",\"cull\":1" } else { "" }
            );
            self.record(item);
            return;
        }
        let n = normal.unwrap_or_else(|| unit(cross(sub(q[1], q[0]), sub(q[2], q[0]))));
        if cull && n[2] < -0.02 {
            return;
        }
        let depth = q.iter().map(|p| p[2]).sum::<f64>() / q.len() as f64;
        let c = if self.light {
            lit(color, lambert(n))
        } else {
            color
        };
        let c = self.cue(c, depth);
        let mut pts = [(0.0f64, 0.0f64); 4];
        for (i, p) in q.iter().enumerate() {
            let s = self.proj.screen(*p);
            pts[i] = (s.0, s.1);
        }
        let k = q.len();
        if c.a > 0.99 {
            // Opaque faces grow by a third of a pixel on every edge so
            // anti-aliased neighbours leave no hairline seams (cheaper than
            // an outline).
            grow(&mut pts[..k], 0.35);
        }
        let stroke = edge.map(|s| {
            Box::new(Stroke {
                color: self.cue(s.color, depth),
                ..s
            })
        });
        let pts = pts.map(|(x, y)| (x as f32, y as f32));
        self.add(
            depth,
            Prim::Poly {
                pts,
                n: q.len() as u8,
                fill: Some(c),
                stroke,
            },
        );
    }

    /// A shaded sphere with a radius in data units (x-axis units).
    pub fn sphere(&mut self, c: [f64; 3], radius: f64, color: Color) {
        if self.rec.is_some() {
            let item = format!(
                "{{\"k\":\"b\",\"p\":[{}],\"r\":{},\"c\":{},\"s\":1}}",
                jp(self.proj.norm(c)),
                j4(radius * self.proj.k[0]),
                jc(color)
            );
            self.record(item);
            return;
        }
        let (x, y, d) = self.proj.project(c);
        let r = radius * self.proj.px_per_unit(d);
        self.ball(x, y, d, r, color, true);
    }

    /// A marker (shaded when `shaded`) of `size` pixels across.
    pub fn marker(&mut self, c: [f64; 3], size: f64, color: Color, shaded: bool) {
        if self.rec.is_some() {
            let item = format!(
                "{{\"k\":\"b\",\"p\":[{}],\"px\":{},\"c\":{},\"s\":{}}}",
                jp(self.proj.norm(c)),
                j4(size / 2.0),
                jc(color),
                shaded as u8
            );
            self.record(item);
            return;
        }
        let (x, y, d) = self.proj.project(c);
        let r = size / 2.0 * self.proj.persp(d);
        self.ball(x, y, d, r, color, shaded);
    }

    fn ball(&mut self, x: f64, y: f64, d: f64, r: f64, color: Color, shaded: bool) {
        if !(x.is_finite() && y.is_finite() && r.is_finite()) || r <= 0.0 {
            return;
        }
        let color = self.cue(color, d);
        self.add(
            d,
            Prim::Ball {
                x: x as f32,
                y: y as f32,
                r: r as f32,
                color,
                shaded: shaded && self.light,
            },
        );
    }

    /// A straight line of `width` pixels.
    pub fn line(&mut self, a: [f64; 3], b: [f64; 3], color: Color, width: f64) {
        self.seg(a, b, color, width, false, false);
    }

    /// A smooth tube through points (radius in data units), drawn in
    /// depth-sorted pieces as a continuous shaded line: no seams or beads
    /// between segments. `colors` (one per point) color the pieces.
    pub fn tube_line(
        &mut self,
        pts: &[[f64; 3]],
        radius: f64,
        color: Color,
        colors: Option<&[Color]>,
    ) {
        if self.rec.is_some() {
            let p: Vec<String> = pts.iter().map(|q| jp(self.proj.norm(*q))).collect();
            let cs = colors.map_or(String::new(), |c| {
                format!(
                    ",\"cs\":[{}]",
                    c.iter().map(|c| jc(*c)).collect::<Vec<_>>().join(",")
                )
            });
            let item = format!(
                "{{\"k\":\"t\",\"p\":[{}],\"c\":{},\"r\":{}{cs}}}",
                p.join(","),
                jc(color),
                j4(radius * self.proj.k[0])
            );
            self.record(item);
            return;
        }
        const PIECE: usize = 12;
        let mut start = 0;
        while start + 1 < pts.len() {
            let end = (start + PIECE).min(pts.len() - 1);
            let mut screen = Vec::with_capacity(end - start + 1);
            let mut depth = 0.0;
            for p in &pts[start..=end] {
                let (x, y, d) = self.proj.project(*p);
                if x.is_finite() && y.is_finite() {
                    screen.push((x, y));
                    depth += d;
                }
            }
            if screen.len() >= 2 {
                let depth = depth / screen.len() as f64;
                let c = colors
                    .and_then(|c| c.get((start + end) / 2).copied())
                    .unwrap_or(color);
                let c = self.cue(c, depth);
                let w = 2.0 * radius * self.proj.px_per_unit(depth);
                if self.light && w > 3.0 {
                    // Shadowed body, lit core.
                    self.add(
                        depth,
                        Prim::Line {
                            pts: screen.clone(),
                            color: c.darken(0.3),
                            width: w,
                        },
                    );
                    self.add(
                        depth + 1e-9,
                        Prim::Line {
                            pts: screen,
                            color: c.lighten(0.18),
                            width: w * 0.5,
                        },
                    );
                } else {
                    self.add(
                        depth,
                        Prim::Line {
                            pts: screen,
                            color: c,
                            width: w,
                        },
                    );
                }
            }
            start = end;
        }
    }

    /// An arrow from `a` to `b`: a line of `width` pixels and a head of
    /// about `head` pixels facing the screen.
    pub fn arrow(&mut self, a: [f64; 3], b: [f64; 3], color: Color, width: f64, head: f64) {
        if self.rec.is_some() {
            let item = format!(
                "{{\"k\":\"a\",\"p\":[{},{}],\"c\":{},\"w\":{},\"h\":{}}}",
                jp(self.proj.norm(a)),
                jp(self.proj.norm(b)),
                jc(color),
                j4(width),
                j4(head)
            );
            self.record(item);
            return;
        }
        let (pa, pb) = (self.proj.project(a), self.proj.project(b));
        if !(pa.0.is_finite() && pa.1.is_finite() && pb.0.is_finite() && pb.1.is_finite()) {
            return;
        }
        let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
        let l = dx.hypot(dy);
        let d = (pa.2 + pb.2) / 2.0;
        let color = self.cue(color, d);
        if l < 0.5 {
            return;
        }
        let (ux, uy) = (dx / l, dy / l);
        let h = head.min(l * 0.6);
        let base = (pb.0 - ux * h, pb.1 - uy * h);
        self.add(
            d,
            Prim::Line {
                pts: vec![(pa.0, pa.1), base],
                color,
                width,
            },
        );
        let w = h * 0.45;
        let tri = [
            (pb.0 as f32, pb.1 as f32),
            ((base.0 - uy * w) as f32, (base.1 + ux * w) as f32),
            ((base.0 + uy * w) as f32, (base.1 - ux * w) as f32),
            (0.0, 0.0),
        ];
        self.add(
            pb.2,
            Prim::Poly {
                pts: tri,
                n: 3,
                fill: Some(color),
                stroke: None,
            },
        );
    }

    /// A shaded cylinder (rounded ends) with a radius in data units.
    pub fn tube(&mut self, a: [f64; 3], b: [f64; 3], radius: f64, color: Color) {
        self.seg(a, b, color, radius, true, true);
    }

    fn seg(&mut self, a: [f64; 3], b: [f64; 3], color: Color, w: f64, data: bool, tube: bool) {
        if self.rec.is_some() {
            let (k, size) = if tube {
                ("t", format!("\"r\":{}", j4(w * self.proj.k[0])))
            } else {
                ("l", format!("\"w\":{}", j4(w)))
            };
            let item = format!(
                "{{\"k\":\"{k}\",\"p\":[{},{}],\"c\":{},{size}}}",
                jp(self.proj.norm(a)),
                jp(self.proj.norm(b)),
                jc(color)
            );
            let _ = data;
            self.record(item);
            return;
        }
        let (pa, pb) = (self.proj.project(a), self.proj.project(b));
        if !(pa.0.is_finite() && pa.1.is_finite() && pb.0.is_finite() && pb.1.is_finite()) {
            return;
        }
        let d = (pa.2 + pb.2) / 2.0;
        let width = if data {
            2.0 * w * self.proj.px_per_unit(d)
        } else {
            w * self.proj.persp(d)
        };
        let color = self.cue(color, d);
        self.add(
            d,
            Prim::Seg {
                a: (pa.0, pa.1),
                b: (pb.0, pb.1),
                color,
                width,
                tube: tube && self.light,
            },
        );
    }

    /// A text label at a data point, drawn above everything else.
    pub fn text(&mut self, at: [f64; 3], text: impl Into<String>, size: f64, color: Color) {
        self.label(at, text.into(), size, color, None, false);
    }

    /// A bold text label on a rounded background.
    pub fn tag(
        &mut self,
        at: [f64; 3],
        text: impl Into<String>,
        size: f64,
        color: Color,
        background: Color,
    ) {
        self.label(at, text.into(), size, color, Some(background), true);
    }

    fn label(
        &mut self,
        at: [f64; 3],
        text: String,
        size: f64,
        color: Color,
        background: Option<Color>,
        bold: bool,
    ) {
        if self.rec.is_some() {
            let bg = background.map_or(String::new(), |b| format!(",\"bg\":{}", jc(b)));
            let t = text
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('<', "\\u003c");
            let item = format!(
                "{{\"k\":\"x\",\"p\":[{}],\"t\":\"{t}\",\"z\":{},\"c\":{},\"bold\":{}{bg}}}",
                jp(self.proj.norm(at)),
                j4(size),
                jc(color),
                bold as u8
            );
            self.record(item);
            return;
        }
        let (x, y, _) = self.proj.project(at);
        if x.is_finite() && y.is_finite() {
            self.labels.push(Label3 {
                x,
                y,
                text,
                size,
                color,
                background,
                bold,
            });
        }
    }

    fn finish(self, fonts: &FontSet, out: &mut Vec<Node>) {
        let keys = radix_sort(self.keys);
        let mut prims = self.prims;
        let idx = |key: u64| (key & 0xFFFF_FFFF) as usize;
        let mut k = 0;
        while k < keys.len() {
            let p = std::mem::replace(&mut prims[idx(keys[k])], Prim::Taken);
            k += 1;
            match p {
                Prim::Poly {
                    pts,
                    n,
                    fill,
                    stroke,
                } => {
                    let mut cmds = Vec::with_capacity(n as usize + 1);
                    cmds.push(PathCmd::MoveTo(pts[0].0 as f64, pts[0].1 as f64));
                    for p in &pts[1..n as usize] {
                        cmds.push(PathCmd::LineTo(p.0 as f64, p.1 as f64));
                    }
                    cmds.push(PathCmd::Close);
                    out.push(Node::Path {
                        cmds,
                        fill,
                        stroke: stroke.map(|s| *s),
                    });
                }
                Prim::Ball {
                    x,
                    y,
                    r,
                    color,
                    shaded,
                } => {
                    // Following balls that look the same become one node drawn
                    // from a single cached sprite.
                    let mut offsets = vec![(x, y)];
                    while let Some(&key) = keys.get(k) {
                        match prims[idx(key)] {
                            Prim::Ball {
                                x: bx,
                                y: by,
                                r: br,
                                color: bc,
                                shaded: bs,
                            } if br == r && bc == color && bs == shaded => {
                                offsets.push((bx, by));
                                k += 1;
                            }
                            _ => break,
                        }
                    }
                    if offsets.len() == 1 {
                        draw_ball(out, x as f64, y as f64, r as f64, color, shaded);
                    } else {
                        let mut t = Vec::new();
                        draw_ball(&mut t, 0.0, 0.0, r as f64, color, shaded);
                        let template = if t.len() == 1 {
                            t.pop().unwrap()
                        } else {
                            Node::Group {
                                children: t,
                                clip: None,
                                opacity: 1.0,
                                tooltip: None,
                                id: None,
                            }
                        };
                        out.push(Node::Instances {
                            template: Box::new(template),
                            offsets,
                        });
                    }
                }
                Prim::Line { pts, color, width } => {
                    let mut cmds = Vec::with_capacity(pts.len());
                    cmds.push(PathCmd::MoveTo(pts[0].0, pts[0].1));
                    cmds.extend(pts[1..].iter().map(|p| PathCmd::LineTo(p.0, p.1)));
                    out.push(Node::Path {
                        cmds,
                        fill: None,
                        stroke: Some(
                            Stroke::new(color, width.max(0.3))
                                .cap(LineCap::Round)
                                .join(LineJoin::Round),
                        ),
                    });
                }
                Prim::Seg {
                    a,
                    b,
                    color,
                    width,
                    tube,
                } => {
                    // Following segments of the same look (bonds of one
                    // element, tube pieces) become one path per band.
                    let mut segs = vec![(a, b)];
                    while let Some(&key) = keys.get(k) {
                        match &prims[idx(key)] {
                            Prim::Seg {
                                a: sa,
                                b: sb,
                                color: sc,
                                width: sw,
                                tube: st,
                            } if *sc == color
                                && *st == tube
                                && (sw - width).abs() <= width * 0.05 =>
                            {
                                segs.push((*sa, *sb));
                                k += 1;
                            }
                            _ => break,
                        }
                    }
                    draw_segs(out, &segs, color, width, tube);
                }
                Prim::Taken => {}
            }
        }
        for l in self.labels {
            if let Some(bg) = l.background {
                let w = fonts.width(&l.text, l.size) + l.size * 0.8;
                let h = l.size * 1.45;
                out.push(Node::Rect {
                    x: l.x - w / 2.0,
                    y: l.y - h / 2.0,
                    w,
                    h,
                    rx: h * 0.3,
                    fill: Some(bg),
                    stroke: None,
                });
            }
            out.push(Node::Text(TextNode {
                x: l.x,
                y: l.y,
                text: l.text,
                size: l.size,
                color: l.color,
                anchor: Anchor::Middle,
                baseline: Baseline::Middle,
                rotate: 0.0,
                bold: l.bold,
                italic: false,
            }));
        }
    }
}

/// Moves the edges of a convex polygon outwards by `d` pixels (miter
/// joins, limited for sharp corners).
fn grow(p: &mut [(f64, f64)], d: f64) {
    let k = p.len();
    let area: f64 = (0..k)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % k]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum();
    if area.abs() < 1e-9 {
        return;
    }
    let s = if area > 0.0 { 1.0 } else { -1.0 };
    // Outward unit normal of edge i (from p[i] to p[i + 1]).
    let normal = |a: (f64, f64), b: (f64, f64)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dy);
        if l < 1e-12 {
            None
        } else {
            Some((s * dy / l, -s * dx / l))
        }
    };
    let src: Vec<(f64, f64)> = p.to_vec();
    for i in 0..k {
        let prev = normal(src[(i + k - 1) % k], src[i]);
        let next = normal(src[i], src[(i + 1) % k]);
        let (nx, ny) = match (prev, next) {
            (Some(a), Some(b)) => {
                let c = 1.0 + a.0 * b.0 + a.1 * b.1;
                let m = (2.0 / c.max(0.25)).min(4.0);
                ((a.0 + b.0) / 2.0 * m, (a.1 + b.1) / 2.0 * m)
            }
            (Some(a), None) | (None, Some(a)) => a,
            _ => continue,
        };
        p[i] = (src[i].0 + nx * d, src[i].1 + ny * d);
    }
}

/// A sphere as stacked circles moving towards the light: a soft gradient
/// from the shadowed rim to the highlight that every backend can draw.
fn draw_ball(out: &mut Vec<Node>, x: f64, y: f64, r: f64, color: Color, shaded: bool) {
    if !shaded || r < 1.6 {
        out.push(Node::Circle {
            cx: x,
            cy: y,
            r,
            fill: Some(color),
            stroke: if shaded || r < 1.6 {
                None
            } else {
                Some(Stroke::new(color.darken(0.3), 0.6))
            },
        });
        return;
    }
    out.push(Node::Sphere {
        cx: x,
        cy: y,
        r,
        color,
    });
}

/// Screen segment: start and end point.
type Seg2 = ((f64, f64), (f64, f64));

fn draw_segs(out: &mut Vec<Node>, segs: &[Seg2], color: Color, width: f64, tube: bool) {
    let stroke = |c: Color, w: f64| {
        Stroke::new(c, w.max(0.3))
            .cap(LineCap::Round)
            .join(LineJoin::Round)
    };
    let path = |o: f64| {
        let mut cmds = Vec::with_capacity(segs.len() * 2);
        for &(a, b) in segs {
            // Perpendicular offset towards the light (highlight bands).
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l = dx.hypot(dy).max(1e-9);
            let (mut nx, mut ny) = (-dy / l, dx / l);
            if nx * LIGHT[0] - ny * LIGHT[1] < 0.0 {
                (nx, ny) = (-nx, -ny);
            }
            cmds.push(PathCmd::MoveTo(a.0 + nx * o, a.1 + ny * o));
            cmds.push(PathCmd::LineTo(b.0 + nx * o, b.1 + ny * o));
        }
        cmds
    };
    if !tube || width < 2.5 {
        out.push(Node::Path {
            cmds: path(0.0),
            fill: None,
            stroke: Some(stroke(color, width)),
        });
        return;
    }
    out.push(Node::Tube {
        segs: segs.to_vec(),
        width,
        color,
    });
}

/// Draws the back panes, grid lines, ticks and axis labels of the box.
fn draw_box(
    out: &mut Vec<Node>,
    p: &Projection3d,
    labels: [Option<&str>; 3],
    categories: [&[String]; 3],
    theme: &Theme,
    fonts: &FontSet,
) {
    let e = p.ext;
    let grid = Stroke::new(theme.grid, 1.0);
    // Category axes: a tick at each index 0, 1, … with its name.
    let ticks: Vec<(Vec<f64>, f64)> = (0..3)
        .map(|i| {
            if categories[i].is_empty() {
                ticks::linear_ticks(p.bounds[i].0, p.bounds[i].1, if e[i] < 0.4 { 2 } else { 4 })
            } else {
                ((0..categories[i].len()).map(|k| k as f64).collect(), 1.0)
            }
        })
        .collect();
    let nv = |i: usize, v: f64| (v - p.mid[i]) * p.k[i];
    // For each axis the pane on the side away from the viewer.
    let far: Vec<f64> = (0..3)
        .map(|i| {
            let mut a = [0.0; 3];
            a[i] = 1.0;
            if p.cam.view(a)[2] > 0.0 {
                -e[i]
            } else {
                e[i]
            }
        })
        .collect();
    for i in 0..3 {
        let (u, w) = ((i + 1) % 3, (i + 2) % 3);
        let corner = |a: f64, b: f64| {
            let mut c = [0.0; 3];
            c[i] = far[i];
            c[u] = a;
            c[w] = b;
            let s = p.project_box(c);
            (s.0, s.1)
        };
        let q = [
            corner(-e[u], -e[w]),
            corner(e[u], -e[w]),
            corner(e[u], e[w]),
            corner(-e[u], e[w]),
        ];
        out.push(Node::Path {
            cmds: vec![
                PathCmd::MoveTo(q[0].0, q[0].1),
                PathCmd::LineTo(q[1].0, q[1].1),
                PathCmd::LineTo(q[2].0, q[2].1),
                PathCmd::LineTo(q[3].0, q[3].1),
                PathCmd::Close,
            ],
            fill: Some(theme.plot_background.darken(0.035)),
            stroke: Some(Stroke::new(theme.axis.with_alpha(0.35), 1.0)),
        });
        for (axis, other) in [(u, w), (w, u)] {
            for v in &ticks[axis].0 {
                let t = nv(axis, *v);
                if t.abs() > e[axis] + 1e-9 {
                    continue;
                }
                let (a, b) = if axis == u {
                    (corner(t, -e[other]), corner(t, e[other]))
                } else {
                    (corner(-e[other], t), corner(e[other], t))
                };
                out.push(Node::Path {
                    cmds: vec![PathCmd::MoveTo(a.0, a.1), PathCmd::LineTo(b.0, b.1)],
                    fill: None,
                    stroke: Some(grid.clone()),
                });
            }
        }
    }
    // Ticks along one outer edge per axis: the lowest edge for axes that
    // run across the screen, the leftmost for those that run up.
    let center = p.project_box([0.0; 3]);
    let ts = theme.tick_size * 0.85;
    for i in 0..3 {
        let (u, w) = ((i + 1) % 3, (i + 2) % 3);
        let dir = {
            let mut a = [0.0; 3];
            a[i] = 1.0;
            let v = p.cam.view(a);
            (v[0], v[1])
        };
        if dir.0.hypot(dir.1) < 0.12 {
            continue; // seen end-on
        }
        let across = dir.0.abs() >= dir.1.abs();
        let mut best: Option<([f64; 3], f64)> = None;
        for (su, sw) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let mut c = [0.0; 3];
            c[u] = su * e[u];
            c[w] = sw * e[w];
            // Skip edges between two back panes (inside the box outline).
            let back = (c[u] == far[u]) as u8 + (c[w] == far[w]) as u8;
            if back == 2 {
                continue;
            }
            let s = p.project_box(c);
            let score = if across { s.1 } else { -s.0 };
            if best.is_none_or(|b| score > b.1) {
                best = Some((c, score));
            }
        }
        let Some((base, _)) = best else { continue };
        let at = |t: f64| {
            let mut c = base;
            c[i] = t;
            p.project_box(c)
        };
        let (m0, m1) = (at(-e[i]), at(e[i]));
        let mid = ((m0.0 + m1.0) / 2.0, (m0.1 + m1.1) / 2.0);
        let (mut ox, mut oy) = (mid.0 - center.0, mid.1 - center.1);
        let ol = ox.hypot(oy).max(1e-9);
        (ox, oy) = (ox / ol, oy / ol);
        out.push(Node::Path {
            cmds: vec![PathCmd::MoveTo(m0.0, m0.1), PathCmd::LineTo(m1.0, m1.1)],
            fill: None,
            stroke: Some(Stroke::new(theme.axis, 1.0)),
        });
        let anchor = if ox < -0.45 {
            Anchor::End
        } else if ox > 0.45 {
            Anchor::Start
        } else {
            Anchor::Middle
        };
        let (lo, hi) = p.bounds[i];
        let mut widest: f64 = 0.0;
        for v in &ticks[i].0 {
            let t = nv(i, *v);
            if t.abs() > e[i] + 1e-9 {
                continue;
            }
            let s = at(t);
            let label = match categories[i].get(*v as usize) {
                Some(name) if !categories[i].is_empty() => name.clone(),
                _ => ticks::format_value(
                    *v,
                    &ticks::TickFormat::Auto,
                    &crate::plot::Scale::Linear,
                    ticks[i].1,
                    lo.abs().max(hi.abs()),
                    None,
                ),
            };
            widest = widest.max(fonts.width(&label, ts));
            out.push(Node::Path {
                cmds: vec![
                    PathCmd::MoveTo(s.0, s.1),
                    PathCmd::LineTo(s.0 + ox * 4.0, s.1 + oy * 4.0),
                ],
                fill: None,
                stroke: Some(Stroke::new(theme.axis, 1.0)),
            });
            out.push(Node::Text(TextNode {
                x: s.0 + ox * 9.0,
                y: s.1 + oy * 9.0,
                text: label,
                size: ts,
                color: theme.text,
                anchor,
                baseline: Baseline::Middle,
                rotate: 0.0,
                bold: false,
                italic: false,
            }));
        }
        if let Some(l) = labels[i] {
            // Beyond the tick labels, aligned away from the box (a label
            // to the left ends there instead of reaching over the ticks).
            // Labels of axes running up the screen are turned like a 2D
            // y-axis label (narrow, so they stay inside the plot).
            let (d, anchor, rotate) = if across {
                (14.0 + ts * 0.6 + widest * ox.abs(), anchor, 0.0)
            } else {
                (
                    14.0 + widest * ox.abs() + theme.label_size * 0.6,
                    Anchor::Middle,
                    -90.0,
                )
            };
            out.push(Node::Text(TextNode {
                x: mid.0 + ox * d,
                y: mid.1 + oy * d,
                text: l.to_string(),
                size: theme.label_size,
                color: theme.text,
                anchor,
                baseline: Baseline::Middle,
                rotate,
                bold: false,
                italic: false,
            }));
        }
    }
}

impl Axes {
    fn use_3d(&mut self) {
        self.kind = AxesKind::ThreeD;
    }

    /// Data range of the visible 3D series, with the axis limits applied.
    pub(crate) fn bounds3d(&self) -> Bounds3 {
        let mut b = [(f64::INFINITY, f64::NEG_INFINITY); 3];
        for s in self.shown() {
            if let Some(sb) = s.bounds3d() {
                for i in 0..3 {
                    b[i].0 = b[i].0.min(sb[i].0);
                    b[i].1 = b[i].1.max(sb[i].1);
                }
            }
        }
        let lim = [
            (self.x.min, self.x.max),
            (self.y.min, self.y.max),
            self.view3d.z_limits,
        ];
        for i in 0..3 {
            if let Some(v) = lim[i].0 {
                b[i].0 = v;
            }
            if let Some(v) = lim[i].1 {
                b[i].1 = v;
            }
            if !(b[i].0.is_finite() && b[i].1.is_finite()) {
                b[i] = (0.0, 1.0);
            } else if b[i].1 - b[i].0 <= 1e-12 * b[i].0.abs().max(1.0) {
                b[i] = (b[i].0 - 0.5, b[i].1 + 0.5);
            }
        }
        b
    }

    /// The projection of this axes' 3D scene into the plot rectangle.
    pub(crate) fn projection3d(&self, plot: Rect) -> Projection3d {
        Projection3d::new(
            self.bounds3d(),
            self.equal_aspect,
            self.view3d.camera,
            self.view3d.home,
            self.view3d.show_box,
            plot,
        )
    }

    pub(crate) fn render3d(&self, plot: Rect, theme: &Theme, fonts: &FontSet, out: &mut Vec<Node>) {
        let proj = self.projection3d(plot);
        if self.view3d.show_box {
            draw_box(
                out,
                &proj,
                [
                    self.x.label.as_deref(),
                    self.y.label.as_deref(),
                    self.view3d.z_label.as_deref(),
                ],
                [&self.x.categories, &self.y.categories, &[]],
                theme,
                fonts,
            );
        }
        if self.gpu_mesh {
            if let Some(node) = self.gpu_mesh_node(&proj, plot, theme) {
                out.push(node);
                return;
            }
        }
        let mut scene = Scene3d {
            proj: &proj,
            prims: Vec::new(),
            keys: Vec::new(),
            labels: Vec::new(),
            fog: self.view3d.fog,
            background: theme.plot_background,
            light: self.view3d.light,
            rec: None,
            mesh: None,
        };
        for s in self.shown() {
            s.draw3d(&mut scene);
        }
        scene.finish(fonts, out);
    }

    /// The scene as a GPU mesh when it is only opaque faces: captured once
    /// per change of the series or limits (`Axes::revision`), then only the
    /// camera changes from frame to frame.
    fn gpu_mesh_node(&self, proj: &Projection3d, plot: Rect, theme: &Theme) -> Option<Node> {
        let norm: [u64; 6] = [
            proj.mid[0],
            proj.mid[1],
            proj.mid[2],
            proj.k[0],
            proj.k[1],
            proj.k[2],
        ]
        .map(f64::to_bits);
        let mut cache = self.mesh_cache.lock().unwrap_or_else(|e| e.into_inner());
        let hit = cache
            .as_ref()
            .filter(|c| c.revision == self.revision && c.norm == norm);
        let (id, vertices) = match hit {
            Some(c) => (c.id, c.vertices.clone()?),
            None => {
                let mut scene = Scene3d {
                    proj,
                    prims: Vec::new(),
                    keys: Vec::new(),
                    labels: Vec::new(),
                    fog: self.view3d.fog,
                    background: theme.plot_background,
                    light: self.view3d.light,
                    rec: None,
                    mesh: Some(MeshCapture::default()),
                };
                for s in self.shown() {
                    s.draw3d(&mut scene);
                }
                let m = scene.mesh.take().unwrap_or_default();
                let ok = !m.other && scene.labels.is_empty() && !m.vertices.is_empty();
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let vertices = ok.then(|| std::sync::Arc::new(m.vertices));
                *cache = Some(MeshCache {
                    revision: self.revision,
                    norm,
                    id,
                    vertices: vertices.clone(),
                });
                (id, vertices?)
            }
        };
        let m = proj.cam.m;
        let row = |r: [f64; 3]| [r[0] as f32, r[1] as f32, r[2] as f32, 0.0];
        let bg = theme.plot_background;
        let clip = self_clip(plot);
        let mut view = [0.0f32; 28];
        view[0..4].copy_from_slice(&row(m[0]));
        view[4..8].copy_from_slice(&row(m[1]));
        view[8..12].copy_from_slice(&row(m[2]));
        view[12..16].copy_from_slice(&[
            proj.scale as f32,
            proj.center.0 as f32,
            proj.center.1 as f32,
            proj.dist as f32,
        ]);
        view[16..20].copy_from_slice(&[
            proj.radius as f32,
            self.view3d.fog as f32,
            if self.view3d.light { 1.0 } else { 0.0 },
            0.0,
        ]);
        view[20..24].copy_from_slice(&[
            bg.r as f32 / 255.0,
            bg.g as f32 / 255.0,
            bg.b as f32 / 255.0,
            bg.a,
        ]);
        view[24..28].copy_from_slice(&clip);
        Some(Node::Mesh3d(Box::new(crate::plot::scene::Mesh3d {
            id,
            vertices,
            view,
        })))
    }

    /// The 3D scene as JSON for the interactive HTML page: camera, fit,
    /// box ticks and the primitives in normalized box coordinates.
    pub(crate) fn export3d(&self, plot: Rect, clip: Rect, theme: &Theme) -> String {
        let proj = self.projection3d(plot);
        let mut scene = Scene3d {
            proj: &proj,
            prims: Vec::new(),
            keys: Vec::new(),
            labels: Vec::new(),
            fog: self.view3d.fog,
            background: theme.plot_background,
            light: self.view3d.light,
            rec: Some(Rec3::default()),
            mesh: None,
        };
        for s in self.shown() {
            s.draw3d(&mut scene);
        }
        let items = scene.rec.take().map(|r| r.items).unwrap_or_default();
        // Box: per axis the ticks (normalized position, label) and the name.
        let mut axes = Vec::new();
        if self.view3d.show_box {
            let cats: [&[String]; 3] = [&self.x.categories, &self.y.categories, &[]];
            let names = [
                self.x.label.as_deref(),
                self.y.label.as_deref(),
                self.view3d.z_label.as_deref(),
            ];
            for i in 0..3 {
                let (lo, hi) = proj.bounds[i];
                let ticks: Vec<(f64, String)> = if cats[i].is_empty() {
                    let (t, step) =
                        ticks::linear_ticks(lo, hi, if proj.ext[i] < 0.4 { 2 } else { 4 });
                    t.into_iter()
                        .map(|v| {
                            (
                                v,
                                ticks::format_value(
                                    v,
                                    &ticks::TickFormat::Auto,
                                    &crate::plot::Scale::Linear,
                                    step,
                                    lo.abs().max(hi.abs()),
                                    None,
                                ),
                            )
                        })
                        .collect()
                } else {
                    cats[i]
                        .iter()
                        .enumerate()
                        .map(|(k, c)| (k as f64, c.clone()))
                        .collect()
                };
                let t: Vec<String> = ticks
                    .iter()
                    .map(|(v, l)| (((v - proj.mid[i]) * proj.k[i]), l))
                    .filter(|(n, _)| n.abs() <= proj.ext[i] + 1e-9)
                    .map(|(n, l)| format!("[{},\"{}\"]", j4(n), l.replace('"', "'")))
                    .collect();
                let name = names[i].map_or("null".to_string(), |n| {
                    format!("\"{}\"", n.replace('"', "'"))
                });
                axes.push(format!("{{\"t\":[{}],\"name\":{name}}}", t.join(",")));
            }
        }
        let m = proj.cam.m;
        format!(
            "{{\"m\":[[{}],[{}],[{}]],\"ext\":[{}],\"scale\":{},\"center\":[{},{}],\"dist\":{},\"radius\":{},\"fog\":{},\"light\":{},\"bg\":{},\"text\":{},\"grid\":{},\"axis\":{},\"ts\":{},\"ls\":{},\"clip\":[{},{},{},{}],\"axes\":[{}],\"items\":[{}]}}",
            jp(m[0]),
            jp(m[1]),
            jp(m[2]),
            jp(proj.ext),
            j4(proj.scale),
            j4(proj.center.0),
            j4(proj.center.1),
            j4(proj.dist),
            j4(proj.radius),
            j4(self.view3d.fog),
            self.view3d.light as u8,
            jc(theme.plot_background),
            jc(theme.text),
            jc(theme.grid),
            jc(theme.axis),
            j4(theme.tick_size),
            j4(theme.label_size),
            j4(clip.x),
            j4(clip.y),
            j4(clip.w),
            j4(clip.h),
            axes.join(","),
            items.join(",")
        )
    }

    pub(crate) fn hit3d(&self, plot: Rect, px: f64, py: f64) -> Option<Hit> {
        let proj = self.projection3d(plot);
        self.shown()
            .filter_map(|s| s.hit3d(&proj, px, py))
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    /// Camera of the 3D scene from azimuth and elevation in degrees (also
    /// the view `R` returns to). Keeps zoom and perspective.
    pub fn camera(&mut self, azimuth: f64, elevation: f64) -> &mut Self {
        let c = &mut self.view3d.camera;
        *c = Camera {
            zoom: c.zoom,
            perspective: c.perspective,
            ..Camera::new(azimuth, elevation)
        };
        self.view3d.home = *c;
        self
    }

    /// Turns the 3D scene by `deg` degrees around a data axis.
    pub fn rotate(&mut self, axis: Axis3, deg: f64) -> &mut Self {
        self.view3d.camera.rotate(axis, deg);
        self.view3d.home = self.view3d.camera;
        self
    }

    /// Turns the 3D scene around the viewing direction.
    pub fn roll(&mut self, deg: f64) -> &mut Self {
        self.view3d.camera.roll(deg);
        self.view3d.home = self.view3d.camera;
        self
    }

    /// Replaces the camera of the 3D scene.
    pub fn set_camera(&mut self, camera: Camera) -> &mut Self {
        self.view3d.camera = camera;
        self.view3d.home = camera;
        self
    }

    /// The current camera of the 3D scene (after any interactive rotation).
    pub fn current_camera(&self) -> Camera {
        self.view3d.camera
    }

    /// Magnification of the 3D scene (1 = fits the plot).
    pub fn zoom3d(&mut self, zoom: f64) -> &mut Self {
        self.view3d.camera.zoom = zoom.max(1e-3);
        self.view3d.home.zoom = self.view3d.camera.zoom;
        self
    }

    /// Perspective strength of the 3D scene (0 = orthographic, the default;
    /// 1 = strong).
    pub fn perspective(&mut self, strength: f64) -> &mut Self {
        self.view3d.camera.perspective = strength.clamp(0.0, 1.0);
        self.view3d.home.perspective = self.view3d.camera.perspective;
        self
    }

    /// Shows the 3D box with panes, grid and ticks (default on).
    pub fn box3d(&mut self, on: bool) -> &mut Self {
        self.view3d.show_box = on;
        self
    }

    /// Fades far parts of the 3D scene toward the background (`0..1`).
    pub fn depth_cue(&mut self, amount: f64) -> &mut Self {
        self.view3d.fog = amount.clamp(0.0, 1.0);
        self
    }

    /// Lighting of 3D faces and spheres (default on).
    pub fn lighting(&mut self, on: bool) -> &mut Self {
        self.view3d.light = on;
        self
    }

    /// z-axis label of a 3D scene.
    pub fn z_label(&mut self, label: impl Into<String>) -> &mut Self {
        self.view3d.z_label = Some(label.into());
        self
    }

    /// z limits of a 3D scene.
    pub fn z_limits(&mut self, min: f64, max: f64) -> &mut Self {
        self.view3d.z_limits = (Some(min.min(max)), Some(min.max(max)));
        self
    }

    /// Interactive turn (drag by `dx`, `dy` pixels).
    pub(crate) fn drag3d(&mut self, dx: f64, dy: f64) {
        self.view3d.camera.drag(dx, dy);
    }

    /// Interactive zoom (`factor` < 1 zooms in) keeping the point under
    /// the canvas position `(px, py)` of the plot `plot` in place.
    pub(crate) fn zoom3d_at(&mut self, plot: Rect, px: f64, py: f64, factor: f64) {
        let c = &mut self.view3d.camera;
        let old = c.zoom;
        c.zoom = (old / factor).clamp(0.05, 50.0);
        let r = c.zoom / old;
        let m = plot.w.min(plot.h).max(1e-9);
        let (cx, cy) = plot.center();
        let (dx, dy) = (px - cx, py - cy);
        c.shift = (
            (dx - (dx - c.shift.0 * m) * r) / m,
            (dy - (dy - c.shift.1 * m) * r) / m,
        );
        if c.zoom <= 1.0 + 1e-9 && r < 1.0 {
            // Zoomed out to the whole scene: centre it again.
            c.shift = (0.0, 0.0);
        }
    }

    /// Back to the camera set in code.
    pub(crate) fn reset_camera(&mut self) {
        self.view3d.camera = self.view3d.home;
    }
}

fn finite_range(v: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    let (a, b) = v
        .filter(|t| t.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), t| {
            (a.min(t), b.max(t))
        });
    a.is_finite().then_some((a, b))
}

/// Surface or wireframe `z[row][col]` over `x_range × y_range` (row 0 at
/// `y_range.0`). Created by [`Axes::surface`] / [`Axes::surface_fn`].
#[derive(Clone, Debug)]
pub struct Surface {
    pub(crate) z: Vec<Vec<f64>>,
    pub(crate) x_range: (f64, f64),
    pub(crate) y_range: (f64, f64),
    pub(crate) colormap: Colormap,
    pub(crate) wireframe: bool,
    pub(crate) edges: bool,
    pub(crate) opacity: f32,
    pub(crate) colorbar: Option<String>,
}

impl Surface {
    /// Lines only.
    pub fn wireframe(&mut self, on: bool) -> &mut Self {
        self.wireframe = on;
        self
    }
    /// Thin grid lines on the faces.
    pub fn edges(&mut self, on: bool) -> &mut Self {
        self.edges = on;
        self
    }
    /// Colormap.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = cm;
        self
    }
    /// Opacity of the faces.
    pub fn opacity(&mut self, a: f32) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }
    /// Shows a colorbar.
    pub fn colorbar(&mut self, label: impl Into<String>) -> &mut Self {
        self.colorbar = Some(label.into());
        self
    }
    fn cols(&self) -> usize {
        self.z.iter().map(|r| r.len()).min().unwrap_or(0)
    }
    fn z_range(&self) -> (f64, f64) {
        finite_range(self.z.iter().flatten().copied()).unwrap_or((0.0, 1.0))
    }
}

impl Series for Surface {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, _ctx: &mut Context) {}
    fn bounds3d(&self) -> Option<Bounds3> {
        Some([self.x_range, self.y_range, self.z_range()])
    }
    fn draw3d(&self, scene: &mut Scene3d) {
        let (rows, cols) = (self.z.len(), self.cols());
        if rows < 2 || cols < 2 {
            return;
        }
        let zr = self.z_range();
        let cs = ColorScale {
            colormap: self.colormap,
            min: zr.0,
            max: zr.1,
            label: None,
        };
        // Every grid point projected once (not four times, once per quad).
        let xs: Vec<f64> = (0..cols)
            .map(|c| {
                self.x_range.0 + (self.x_range.1 - self.x_range.0) * c as f64 / (cols - 1) as f64
            })
            .collect();
        let ys: Vec<f64> = (0..rows)
            .map(|r| {
                self.y_range.0 + (self.y_range.1 - self.y_range.0) * r as f64 / (rows - 1) as f64
            })
            .collect();
        let proj = scene.proj;
        let view: Vec<[f64; 3]> = (0..rows * cols)
            .map(|i| proj.view([xs[i % cols], ys[i / cols], self.z[i / cols][i % cols]]))
            .collect();
        let at = |r: usize, c: usize| [xs[c], ys[r], self.z[r][c]];
        scene.reserve((rows - 1) * (cols - 1));
        for r in 0..rows - 1 {
            for c in 0..cols - 1 {
                let z = [
                    self.z[r][c],
                    self.z[r][c + 1],
                    self.z[r + 1][c + 1],
                    self.z[r + 1][c],
                ];
                if z.iter().any(|v| !v.is_finite()) {
                    continue;
                }
                let color = cs.color(z.iter().sum::<f64>() / 4.0);
                if self.wireframe {
                    let v = [at(r, c), at(r, c + 1), at(r + 1, c + 1), at(r + 1, c)];
                    for k in 0..4 {
                        if (k == 2 && r + 2 < rows) || (k == 1 && c + 2 < cols) {
                            continue; // shared with the next quad
                        }
                        scene.line(v[k], v[(k + 1) % 4], color, 1.0);
                    }
                } else {
                    let q = [
                        view[r * cols + c],
                        view[r * cols + c + 1],
                        view[(r + 1) * cols + c + 1],
                        view[(r + 1) * cols + c],
                    ];
                    let edge = self
                        .edges
                        .then(|| Stroke::new(color.darken(0.35).with_alpha(0.7), 0.5));
                    scene.face_view(&q, color.with_alpha(self.opacity), edge, None, false);
                }
            }
        }
    }
    fn color_scale(&self) -> Option<ColorScale> {
        let (a, b) = self.z_range();
        self.colorbar.as_ref().map(|l| ColorScale {
            colormap: self.colormap,
            min: a,
            max: b,
            label: Some(l.clone()),
        })
    }
    fn kind(&self) -> &'static str {
        "3D surface"
    }
    fn describe(&self) -> Option<String> {
        let (a, b) = self.z_range();
        Some(format!(
            "surface on a {} × {} grid, z from {} to {}",
            self.cols(),
            self.z.len(),
            short(a),
            short(b)
        ))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// 3D points or line. Created by [`Axes::scatter3d`] / [`Axes::line3d`].
#[derive(Clone, Debug)]
pub struct Points3d {
    pub(crate) x: Vec<f64>,
    pub(crate) y: Vec<f64>,
    pub(crate) z: Vec<f64>,
    pub(crate) color: Color,
    pub(crate) colormap: Option<Colormap>,
    pub(crate) values: Option<Vec<f64>>,
    pub(crate) line: bool,
    pub(crate) size: f64,
    pub(crate) shaded: bool,
    pub(crate) label: Option<String>,
}

impl Points3d {
    /// Colors points by z (or by [`Points3d::values`]) with a colormap.
    pub fn colormap(&mut self, cm: Colormap) -> &mut Self {
        self.colormap = Some(cm);
        self
    }
    /// Values that set the colors through the colormap (default: z).
    pub fn values(&mut self, v: impl IntoValues) -> &mut Self {
        self.values = Some(v.into_values());
        if self.colormap.is_none() {
            self.colormap = Some(Colormap::Viridis);
        }
        self
    }
    /// Uniform color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
    /// Marker size / line width in pixels.
    pub fn size(&mut self, s: f64) -> &mut Self {
        self.size = s.max(0.5);
        self
    }
    /// Shaded spheres (default) or flat dots.
    pub fn shaded(&mut self, on: bool) -> &mut Self {
        self.shaded = on;
        self
    }
    /// Legend label.
    pub fn label(&mut self, l: impl Into<String>) -> &mut Self {
        self.label = Some(l.into());
        self
    }
    fn n(&self) -> usize {
        self.x.len().min(self.y.len()).min(self.z.len())
    }
    fn scale(&self) -> Option<ColorScale> {
        let v = self.values.as_deref().unwrap_or(&self.z);
        let (a, b) = finite_range(v.iter().copied()).unwrap_or((0.0, 1.0));
        self.colormap.map(|cm| ColorScale {
            colormap: cm,
            min: a,
            max: b,
            label: None,
        })
    }
}

impl Series for Points3d {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, _ctx: &mut Context) {}
    fn bounds3d(&self) -> Option<Bounds3> {
        let r = |v: &[f64]| finite_range(v.iter().copied()).unwrap_or((0.0, 1.0));
        Some([r(&self.x), r(&self.y), r(&self.z)])
    }
    fn draw3d(&self, scene: &mut Scene3d) {
        let n = self.n();
        let cs = self.scale();
        let vals = self.values.as_deref().unwrap_or(&self.z);
        let color = |i: usize| {
            cs.as_ref().map_or(self.color, |s| {
                s.color(vals.get(i).copied().unwrap_or(f64::NAN))
            })
        };
        let p = |i: usize| [self.x[i], self.y[i], self.z[i]];
        if self.line {
            let pts: Vec<[f64; 3]> = (0..n).map(p).collect();
            let colors: Option<Vec<Color>> = cs.as_ref().map(|_| (0..n).map(color).collect());
            scene.polyline(&pts, self.color, colors.as_deref(), self.size);
        } else {
            scene.reserve(n);
            for i in 0..n {
                scene.marker(p(i), self.size, color(i), self.shaded);
            }
        }
    }
    fn hit3d(&self, proj: &Projection3d, px: f64, py: f64) -> Option<Hit> {
        (0..self.n())
            .map(|i| {
                let (x, y, _) = proj.project([self.x[i], self.y[i], self.z[i]]);
                Hit {
                    px: x,
                    py: y,
                    label: format!(
                        "{}({}, {}, {})",
                        self.label
                            .as_ref()
                            .map_or(String::new(), |l| format!("{l}: ")),
                        short(self.x[i]),
                        short(self.y[i]),
                        short(self.z[i])
                    ),
                    distance: (x - px).hypot(y - py),
                }
            })
            .filter(|h| h.distance.is_finite())
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: if self.line {
                    LegendSymbol::Line {
                        color: self.color,
                        width: self.size,
                        dash: Default::default(),
                        marker: None,
                    }
                } else {
                    LegendSymbol::Marker {
                        marker: Marker::Circle,
                        size: self.size.clamp(5.0, 12.0),
                        color: self.color,
                    }
                },
            })
            .collect()
    }
    fn kind(&self) -> &'static str {
        if self.line {
            "3D line"
        } else {
            "3D scatter"
        }
    }
    fn describe(&self) -> Option<String> {
        let what = if self.line { "line through" } else { "" };
        let name = self
            .label
            .as_ref()
            .map_or(String::new(), |l| format!("{l}: "));
        Some(
            format!("{name}{what} {} points", self.n())
                .replace("  ", " ")
                .trim()
                .to_string(),
        )
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Spheres with radii in data units (cells, atoms, particles). Created by
/// [`Axes::spheres3d`].
#[derive(Clone, Debug)]
pub struct Spheres {
    pub(crate) centers: Vec<[f64; 3]>,
    pub(crate) radii: Vec<f64>,
    pub(crate) color: Color,
    pub(crate) colors: Option<Vec<Color>>,
    pub(crate) values: Option<(Vec<f64>, Colormap)>,
    pub(crate) opacity: f32,
    pub(crate) label: Option<String>,
    pub(crate) names: Option<Vec<String>>,
}

impl Spheres {
    /// Uniform color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
    /// One color per sphere.
    pub fn colors(&mut self, c: Vec<Color>) -> &mut Self {
        self.colors = Some(c);
        self
    }
    /// Colors from values through a colormap.
    pub fn values(&mut self, v: impl IntoValues, cm: Colormap) -> &mut Self {
        self.values = Some((v.into_values(), cm));
        self
    }
    /// Opacity.
    pub fn opacity(&mut self, a: f32) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }
    /// Legend label.
    pub fn label(&mut self, l: impl Into<String>) -> &mut Self {
        self.label = Some(l.into());
        self
    }
    /// Names shown in the hover tooltip.
    pub fn names<S: AsRef<str>>(&mut self, names: &[S]) -> &mut Self {
        self.names = Some(names.iter().map(|s| s.as_ref().to_string()).collect());
        self
    }
    fn radius(&self, i: usize) -> f64 {
        self.radii
            .get(i)
            .or(self.radii.last())
            .copied()
            .unwrap_or(1.0)
    }
    fn scale(&self) -> Option<ColorScale> {
        self.values.as_ref().map(|(v, cm)| {
            let (a, b) = finite_range(v.iter().copied()).unwrap_or((0.0, 1.0));
            ColorScale {
                colormap: *cm,
                min: a,
                max: b,
                label: None,
            }
        })
    }
}

impl Series for Spheres {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, _ctx: &mut Context) {}
    fn bounds3d(&self) -> Option<Bounds3> {
        let mut b = [(f64::INFINITY, f64::NEG_INFINITY); 3];
        for (i, c) in self.centers.iter().enumerate() {
            let r = self.radius(i);
            for k in 0..3 {
                if c[k].is_finite() {
                    b[k] = (b[k].0.min(c[k] - r), b[k].1.max(c[k] + r));
                }
            }
        }
        b[0].0.is_finite().then_some(b)
    }
    fn draw3d(&self, scene: &mut Scene3d) {
        let cs = self.scale();
        for (i, c) in self.centers.iter().enumerate() {
            let color = match (&self.colors, &cs, &self.values) {
                (Some(v), _, _) => v.get(i).copied().unwrap_or(self.color),
                (None, Some(s), Some((v, _))) => s.color(v.get(i).copied().unwrap_or(f64::NAN)),
                _ => self.color,
            };
            scene.sphere(*c, self.radius(i), color.with_alpha(color.a * self.opacity));
        }
    }
    fn hit3d(&self, proj: &Projection3d, px: f64, py: f64) -> Option<Hit> {
        self.centers
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let (x, y, d) = proj.project(*c);
                let r = self.radius(i) * proj.px_per_unit(d);
                let name = self
                    .names
                    .as_ref()
                    .and_then(|n| n.get(i).cloned())
                    .or_else(|| self.label.clone())
                    .unwrap_or_else(|| format!("#{i}"));
                Hit {
                    px: x,
                    py: y,
                    label: format!("{name} ({}, {}, {})", short(c[0]), short(c[1]), short(c[2])),
                    distance: ((x - px).hypot(y - py) - r).max(0.0) - d * 1e-6,
                }
            })
            .filter(|h| h.distance.is_finite())
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }
    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Marker {
                    marker: Marker::Circle,
                    size: 9.0,
                    color: self.color,
                },
            })
            .collect()
    }
    fn color_scale(&self) -> Option<ColorScale> {
        self.scale()
    }
    fn kind(&self) -> &'static str {
        "3D spheres"
    }
    fn describe(&self) -> Option<String> {
        let name = self
            .label
            .as_ref()
            .map_or(String::new(), |l| format!("{l}: "));
        Some(format!("{name}{} spheres", self.centers.len()))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A triangle/quad mesh (cell surfaces, organs, isosurfaces). Created by
/// [`Axes::mesh3d`] and [`Axes::ellipsoid`].
#[derive(Clone, Debug)]
pub struct Mesh3d {
    pub(crate) vertices: Vec<[f64; 3]>,
    /// Faces as vertex indices (`u32::MAX` in the last slot for triangles).
    pub(crate) faces: Vec<[u32; 4]>,
    pub(crate) color: Color,
    pub(crate) values: Option<(Vec<f64>, Colormap)>,
    pub(crate) opacity: f32,
    pub(crate) edges: Option<Color>,
    pub(crate) smooth: bool,
    pub(crate) closed: bool,
    pub(crate) label: Option<String>,
}

impl Mesh3d {
    /// Color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = c.into();
        self
    }
    /// Per-vertex values colored through a colormap.
    pub fn values(&mut self, v: impl IntoValues, cm: Colormap) -> &mut Self {
        self.values = Some((v.into_values(), cm));
        self
    }
    /// Opacity (translucent meshes show what is inside).
    pub fn opacity(&mut self, a: f32) -> &mut Self {
        self.opacity = a.clamp(0.0, 1.0);
        self
    }
    /// Draws the face edges in a color.
    pub fn edges(&mut self, c: impl Into<Color>) -> &mut Self {
        self.edges = Some(c.into());
        self
    }
    /// Smooth shading from vertex normals (default on).
    pub fn smooth(&mut self, on: bool) -> &mut Self {
        self.smooth = on;
        self
    }
    /// The mesh is closed with faces wound counter-clockwise seen from
    /// outside: faces turned away are not drawn when it is opaque.
    pub fn closed(&mut self, on: bool) -> &mut Self {
        self.closed = on;
        self
    }
    /// Legend label.
    pub fn label(&mut self, l: impl Into<String>) -> &mut Self {
        self.label = Some(l.into());
        self
    }
    fn face_ids(f: &[u32; 4]) -> &[u32] {
        if f[3] == u32::MAX {
            &f[..3]
        } else {
            &f[..]
        }
    }
}

impl Series for Mesh3d {
    fn extent(&self) -> Option<Extent> {
        Some(Extent::point(0.0, 0.0).include(1.0, 1.0))
    }
    fn draw(&self, _ctx: &mut Context) {}
    fn bounds3d(&self) -> Option<Bounds3> {
        let r = |k: usize| finite_range(self.vertices.iter().map(|v| v[k]));
        Some([r(0)?, r(1)?, r(2)?])
    }
    fn draw3d(&self, scene: &mut Scene3d) {
        let proj = scene.proj;
        let view: Vec<[f64; 3]> = self.vertices.iter().map(|v| proj.view(*v)).collect();
        let face_normal = |ids: &[u32]| {
            let q: Vec<[f64; 3]> = ids.iter().map(|&i| view[i as usize]).collect();
            // Newell's method (robust for quads).
            let mut n = [0.0; 3];
            for k in 0..q.len() {
                let (a, b) = (q[k], q[(k + 1) % q.len()]);
                n[0] += (a[1] - b[1]) * (a[2] + b[2]);
                n[1] += (a[2] - b[2]) * (a[0] + b[0]);
                n[2] += (a[0] - b[0]) * (a[1] + b[1]);
            }
            unit(n)
        };
        let valid = |f: &[u32; 4]| Self::face_ids(f).iter().all(|&i| (i as usize) < view.len());
        let normals: Vec<[f64; 3]> = if self.smooth {
            let mut acc = vec![[0.0; 3]; view.len()];
            for f in self.faces.iter().filter(|f| valid(f)) {
                let ids = Self::face_ids(f);
                let n = face_normal(ids);
                for &i in ids {
                    let a = &mut acc[i as usize];
                    a[0] += n[0];
                    a[1] += n[1];
                    a[2] += n[2];
                }
            }
            acc.into_iter().map(unit).collect()
        } else {
            Vec::new()
        };
        let cs = self.values.as_ref().map(|(v, cm)| {
            let (a, b) = finite_range(v.iter().copied()).unwrap_or((0.0, 1.0));
            (
                v,
                ColorScale {
                    colormap: *cm,
                    min: a,
                    max: b,
                    label: None,
                },
            )
        });
        let cull = self.closed && self.opacity > 0.99;
        let edge = self.edges.map(|c| Stroke::new(c, 0.6));
        let mut pts = Vec::with_capacity(4);
        for f in self.faces.iter().filter(|f| valid(f)) {
            let ids = Self::face_ids(f);
            let flat = face_normal(ids);
            let n = if self.smooth {
                let mut s = [0.0; 3];
                for &i in ids {
                    let v = normals[i as usize];
                    s = [s[0] + v[0], s[1] + v[1], s[2] + v[2]];
                }
                let s = unit(s);
                // Keep the side of the face (two-sided lighting).
                if dot(s, flat) < 0.0 {
                    [-s[0], -s[1], -s[2]]
                } else {
                    s
                }
            } else {
                flat
            };
            if cull && flat[2] < -0.02 {
                continue;
            }
            let color = match &cs {
                Some((v, s)) => {
                    let m = ids
                        .iter()
                        .map(|&i| v.get(i as usize).copied().unwrap_or(f64::NAN))
                        .sum::<f64>()
                        / ids.len() as f64;
                    s.color(m)
                }
                None => self.color,
            };
            pts.clear();
            pts.extend(ids.iter().map(|&i| view[i as usize]));
            // Back faces of translucent meshes a little darker.
            let n = if !cull && flat[2] < 0.0 {
                [n[0] * 0.6, n[1] * 0.6, n[2]]
            } else {
                n
            };
            scene.face_view(
                &pts,
                color.with_alpha(color.a * self.opacity),
                edge.clone(),
                Some(n),
                false,
            );
        }
    }
    fn legend(&self) -> Vec<LegendEntry> {
        self.label
            .iter()
            .map(|l| LegendEntry {
                label: l.clone(),
                symbol: LegendSymbol::Patch {
                    fill: self.color.with_alpha(self.opacity.max(0.4)),
                    edge: None,
                },
            })
            .collect()
    }
    fn kind(&self) -> &'static str {
        "3D mesh"
    }
    fn describe(&self) -> Option<String> {
        let name = self
            .label
            .as_ref()
            .map_or(String::new(), |l| format!("{l}: "));
        Some(format!("{name}surface of {} faces", self.faces.len()))
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Text labels at 3D points, drawn above the scene. Created by
/// [`Axes::text3d`].
#[derive(Clone, Debug)]
pub struct Text3d {
    pub(crate) items: Vec<([f64; 3], String)>,
    pub(crate) size: f64,
    pub(crate) color: Option<Color>,
    pub(crate) background: Option<Color>,
}

impl Text3d {
    /// Font size.
    pub fn size(&mut self, s: f64) -> &mut Self {
        self.size = s;
        self
    }
    /// Text color.
    pub fn color(&mut self, c: impl Into<Color>) -> &mut Self {
        self.color = Some(c.into());
        self
    }
    /// Bold text on a rounded background.
    pub fn background(&mut self, c: impl Into<Color>) -> &mut Self {
        self.background = Some(c.into());
        self
    }
}

impl Series for Text3d {
    fn extent(&self) -> Option<Extent> {
        None
    }
    fn draw(&self, _ctx: &mut Context) {}
    fn draw3d(&self, scene: &mut Scene3d) {
        let color = self.color.unwrap_or(Color::rgb(30, 30, 30));
        for (p, t) in &self.items {
            match self.background {
                Some(bg) => scene.tag(*p, t.clone(), self.size, color, bg),
                None => scene.text(*p, t.clone(), self.size, color),
            }
        }
    }
    fn kind(&self) -> &'static str {
        "annotation"
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Faces of an `n × m` (longitude × latitude) grid on a sphere-like
/// surface, wound outwards.
pub(crate) fn uv_mesh(
    lon: usize,
    lat: usize,
    point: impl Fn(f64, f64) -> [f64; 3],
) -> (Vec<[f64; 3]>, Vec<[u32; 4]>) {
    let mut v = Vec::with_capacity((lon + 1) * (lat + 1));
    for j in 0..=lat {
        let theta = std::f64::consts::PI * j as f64 / lat as f64; // 0 = north pole
        for i in 0..=lon {
            let phi = std::f64::consts::TAU * i as f64 / lon as f64;
            v.push(point(theta, phi));
        }
    }
    let id = |i: usize, j: usize| (j * (lon + 1) + i) as u32;
    let mut f = Vec::with_capacity(lon * lat);
    for j in 0..lat {
        for i in 0..lon {
            let (a, b, c, d) = (id(i, j), id(i, j + 1), id(i + 1, j + 1), id(i + 1, j));
            if j == 0 {
                f.push([a, b, c, u32::MAX]);
            } else if j == lat - 1 {
                f.push([a, b, d, u32::MAX]);
            } else {
                f.push([a, b, c, d]);
            }
        }
    }
    (v, f)
}

impl Axes {
    /// 3D surface of `z[row][col]` over `x_range × y_range`.
    pub fn surface(
        &mut self,
        x_range: (f64, f64),
        y_range: (f64, f64),
        z: Vec<Vec<f64>>,
    ) -> &mut Surface {
        self.use_3d();
        let cm = self.defaults.colormap;
        self.add(Surface {
            z,
            x_range,
            y_range,
            colormap: cm,
            wireframe: false,
            edges: false,
            opacity: 1.0,
            colorbar: None,
        })
    }

    /// 3D surface of `f(x, y)` sampled on an `n × n` grid.
    pub fn surface_fn(
        &mut self,
        f: impl Fn(f64, f64) -> f64,
        x_range: (f64, f64),
        y_range: (f64, f64),
        n: usize,
    ) -> &mut Surface {
        let n = n.clamp(2, 400);
        let z = (0..n)
            .map(|r| {
                let y = y_range.0 + (y_range.1 - y_range.0) * r as f64 / (n - 1) as f64;
                (0..n)
                    .map(|c| {
                        f(
                            x_range.0 + (x_range.1 - x_range.0) * c as f64 / (n - 1) as f64,
                            y,
                        )
                    })
                    .collect()
            })
            .collect();
        self.surface(x_range, y_range, z)
    }

    /// 3D scatter plot (shaded spheres of a fixed pixel size).
    pub fn scatter3d(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        z: impl IntoValues,
    ) -> &mut Points3d {
        self.use_3d();
        let color = self.next_color();
        self.add(Points3d {
            x: x.into_values(),
            y: y.into_values(),
            z: z.into_values(),
            color,
            colormap: None,
            values: None,
            line: false,
            size: 7.0,
            shaded: true,
            label: None,
        })
    }

    /// 3D line (e.g. a trajectory).
    pub fn line3d(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        z: impl IntoValues,
    ) -> &mut Points3d {
        let p = self.scatter3d(x, y, z);
        p.line = true;
        p.size = 2.0;
        p
    }

    /// Spheres at `(x, y, z)` with radii in data units (one radius for all
    /// when `radius` has one value).
    pub fn spheres3d(
        &mut self,
        x: impl IntoValues,
        y: impl IntoValues,
        z: impl IntoValues,
        radius: impl IntoValues,
    ) -> &mut Spheres {
        self.use_3d();
        let (x, y, z) = (x.into_values(), y.into_values(), z.into_values());
        let centers = x
            .iter()
            .zip(&y)
            .zip(&z)
            .map(|((a, b), c)| [*a, *b, *c])
            .collect();
        let color = self.next_color();
        self.add(Spheres {
            centers,
            radii: radius.into_values(),
            color,
            colors: None,
            values: None,
            opacity: 1.0,
            label: None,
            names: None,
        })
    }

    /// A mesh from vertices and faces (triangles `[a, b, c]` as
    /// `[a, b, c, u32::MAX]`, or quads `[a, b, c, d]`).
    pub fn mesh3d(&mut self, vertices: Vec<[f64; 3]>, faces: Vec<[u32; 4]>) -> &mut Mesh3d {
        self.use_3d();
        let color = self.next_color();
        self.add(Mesh3d {
            vertices,
            faces,
            color,
            values: None,
            opacity: 1.0,
            edges: None,
            smooth: true,
            closed: false,
            label: None,
        })
    }

    /// A mesh from triangles.
    pub fn trisurf(&mut self, vertices: Vec<[f64; 3]>, triangles: &[[u32; 3]]) -> &mut Mesh3d {
        let faces = triangles
            .iter()
            .map(|t| [t[0], t[1], t[2], u32::MAX])
            .collect();
        self.mesh3d(vertices, faces)
    }

    /// An ellipsoid (a sphere when the radii are equal) as a smooth closed
    /// mesh.
    pub fn ellipsoid(&mut self, center: [f64; 3], radii: [f64; 3]) -> &mut Mesh3d {
        let (v, f) = uv_mesh(48, 28, |t, p| {
            [
                center[0] + radii[0] * t.sin() * p.cos(),
                center[1] + radii[1] * t.sin() * p.sin(),
                center[2] + radii[2] * t.cos(),
            ]
        });
        let m = self.mesh3d(v, f);
        m.closed = true;
        m
    }

    /// Text labels at 3D points.
    pub fn text3d<S: AsRef<str>>(&mut self, items: &[([f64; 3], S)]) -> &mut Text3d {
        self.use_3d();
        let size = self.defaults_text_size();
        self.add(Text3d {
            items: items
                .iter()
                .map(|(p, s)| (*p, s.as_ref().to_string()))
                .collect(),
            size,
            color: None,
            background: None,
        })
    }

    fn defaults_text_size(&self) -> f64 {
        12.0
    }
}

/// Isosurface of a scalar volume by marching tetrahedra: the surface where
/// `values` equals `level`, as a mesh with shared vertices (smooth
/// shading) and faces wound outwards (towards values below `level`).
/// `values[(z * ny + y) * nx + x]` on a regular grid over `bounds`.
pub fn isosurface_mesh(
    values: &[f64],
    dims: [usize; 3],
    bounds: Bounds3,
    level: f64,
) -> (Vec<[f64; 3]>, Vec<[u32; 4]>) {
    let [nx, ny, nz] = dims;
    if nx < 2 || ny < 2 || nz < 2 || values.len() < nx * ny * nz {
        return (Vec::new(), Vec::new());
    }
    let id = |x: usize, y: usize, z: usize| (z * ny + y) * nx + x;
    let pos = |i: usize| {
        let (x, y, z) = (i % nx, (i / nx) % ny, i / (nx * ny));
        [
            bounds[0].0 + (bounds[0].1 - bounds[0].0) * x as f64 / (nx - 1) as f64,
            bounds[1].0 + (bounds[1].1 - bounds[1].0) * y as f64 / (ny - 1) as f64,
            bounds[2].0 + (bounds[2].1 - bounds[2].0) * z as f64 / (nz - 1) as f64,
        ]
    };
    // Cube corners and the six tetrahedra around the 0–6 diagonal.
    const C: [(usize, usize, usize); 8] = [
        (0, 0, 0),
        (1, 0, 0),
        (1, 1, 0),
        (0, 1, 0),
        (0, 0, 1),
        (1, 0, 1),
        (1, 1, 1),
        (0, 1, 1),
    ];
    const T: [[usize; 4]; 6] = [
        [0, 5, 1, 6],
        [0, 1, 2, 6],
        [0, 2, 3, 6],
        [0, 3, 7, 6],
        [0, 7, 4, 6],
        [0, 4, 5, 6],
    ];
    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut edge: crate::plot::raster::HashMap<(u32, u32), u32> = Default::default();
    let mut faces: Vec<[u32; 4]> = Vec::new();
    let mut vertex = |a: usize, b: usize, verts: &mut Vec<[f64; 3]>| -> u32 {
        let key = if a < b {
            (a as u32, b as u32)
        } else {
            (b as u32, a as u32)
        };
        *edge.entry(key).or_insert_with(|| {
            let (va, vb) = (values[a], values[b]);
            let t = if (vb - va).abs() > 1e-300 {
                ((level - va) / (vb - va)).clamp(0.0, 1.0)
            } else {
                0.5
            };
            let (pa, pb) = (pos(a), pos(b));
            verts.push([
                pa[0] + (pb[0] - pa[0]) * t,
                pa[1] + (pb[1] - pa[1]) * t,
                pa[2] + (pb[2] - pa[2]) * t,
            ]);
            (verts.len() - 1) as u32
        })
    };
    for z in 0..nz - 1 {
        for y in 0..ny - 1 {
            for x in 0..nx - 1 {
                let corner: [usize; 8] = C.map(|(dx, dy, dz)| id(x + dx, y + dy, z + dz));
                // Skip cubes entirely on one side (most of them).
                let ins = corner.iter().filter(|&&c| values[c] > level).count();
                if ins == 0 || ins == 8 || corner.iter().any(|&c| !values[c].is_finite()) {
                    continue;
                }
                for t in T {
                    let v = t.map(|k| corner[k]);
                    let (inside, outside): (Vec<usize>, Vec<usize>) =
                        v.iter().partition(|&&c| values[c] > level);
                    let tris: Vec<[u32; 3]> = match inside.len() {
                        1 => vec![[
                            vertex(inside[0], outside[0], &mut verts),
                            vertex(inside[0], outside[1], &mut verts),
                            vertex(inside[0], outside[2], &mut verts),
                        ]],
                        3 => vec![[
                            vertex(outside[0], inside[0], &mut verts),
                            vertex(outside[0], inside[1], &mut verts),
                            vertex(outside[0], inside[2], &mut verts),
                        ]],
                        2 => {
                            let a = vertex(inside[0], outside[0], &mut verts);
                            let b = vertex(inside[0], outside[1], &mut verts);
                            let c = vertex(inside[1], outside[1], &mut verts);
                            let d = vertex(inside[1], outside[0], &mut verts);
                            vec![[a, b, c], [a, c, d]]
                        }
                        _ => Vec::new(),
                    };
                    // Wind outwards: from the inside corners to the outside ones.
                    let mean = |cs: &[usize]| {
                        let mut m = [0.0; 3];
                        for &c in cs {
                            let p = pos(c);
                            m = [m[0] + p[0], m[1] + p[1], m[2] + p[2]];
                        }
                        let n = cs.len().max(1) as f64;
                        [m[0] / n, m[1] / n, m[2] / n]
                    };
                    let out_dir = sub(mean(&outside), mean(&inside));
                    for tri in tris {
                        let (p0, p1, p2) = (
                            verts[tri[0] as usize],
                            verts[tri[1] as usize],
                            verts[tri[2] as usize],
                        );
                        let nrm = cross(sub(p1, p0), sub(p2, p0));
                        if dot(nrm, nrm) < 1e-30 {
                            continue;
                        }
                        if dot(nrm, out_dir) >= 0.0 {
                            faces.push([tri[0], tri[1], tri[2], u32::MAX]);
                        } else {
                            faces.push([tri[0], tri[2], tri[1], u32::MAX]);
                        }
                    }
                }
            }
        }
    }
    (verts, faces)
}

impl Axes {
    /// Isosurface of a volume (`values[(z * ny + y) * nx + x]` on a
    /// regular grid over `bounds`) at `level`: microscopy and CT stacks,
    /// electron densities, simulation fields. Call again with another level
    /// (and [`Mesh3d::opacity`]) for nested surfaces.
    pub fn isosurface(
        &mut self,
        values: &[f64],
        dims: [usize; 3],
        bounds: Bounds3,
        level: f64,
    ) -> &mut Mesh3d {
        let (v, f) = isosurface_mesh(values, dims, bounds, level);
        let m = self.mesh3d(v, f);
        m.closed = true;
        m
    }

    /// Isosurface of `f(x, y, z) = level` sampled on an `n³` grid.
    pub fn isosurface_fn(
        &mut self,
        f: impl Fn(f64, f64, f64) -> f64,
        bounds: Bounds3,
        n: usize,
        level: f64,
    ) -> &mut Mesh3d {
        let n = n.clamp(4, 256);
        let at = |k: usize, i: usize| {
            bounds[k].0 + (bounds[k].1 - bounds[k].0) * i as f64 / (n - 1) as f64
        };
        let mut values = Vec::with_capacity(n * n * n);
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    values.push(f(at(0, x), at(1, y), at(2, z)));
                }
            }
        }
        self.isosurface(&values, [n, n, n], bounds, level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn camera_rotations_stay_orthonormal() {
        let mut c = Camera::new(-60.0, 30.0);
        for k in 0..1000 {
            c.rotate(Axis3::X, 7.0)
                .rotate(Axis3::Y, 3.0)
                .turn(1.0, k as f64 * 0.01)
                .roll(2.0);
        }
        let m = c.matrix();
        for i in 0..3 {
            for j in 0..3 {
                let d = dot(m[i], m[j]);
                assert!((d - if i == j { 1.0 } else { 0.0 }).abs() < 1e-9);
            }
        }
        // Right-handed.
        let z = cross(m[0], m[1]);
        assert!(dot(z, m[2]) > 0.999);
    }

    #[test]
    fn principal_axes_lay_the_long_side_across() {
        let pts: Vec<[f64; 3]> = (0..200)
            .map(|i| {
                let t = i as f64;
                [t * 0.1, (t * 0.7).sin() * 0.5, 20.0 - t * 0.3]
            })
            .collect();
        let c = Camera::principal(&pts);
        let dir = unit([0.1, 0.0, -0.3]);
        assert!(c.view(dir)[0].abs() > 0.99, "{:?}", c.view(dir));
    }

    #[test]
    fn zoomed_scene_stays_in_its_plot() {
        let mut fig = Figure::new(800, 400);
        fig.grid(1, 2);
        fig.at(0, 0).dna_helix("ATGCGTACGTTAGCATGC");
        fig.at(0, 1).line([0.0, 1.0], [0.0, 1.0]);
        let mut before = Pixels::default();
        fig.render_into(&mut before).unwrap();
        let (_, l) = fig.scene_and_layouts();
        let p = l[0].plot;
        for _ in 0..30 {
            fig.zoom_at(p.x + p.w / 2.0, p.y + p.h / 2.0, 0.85);
        }
        let mut after = Pixels::default();
        fig.render_into(&mut after).unwrap();
        // Nothing changed in the second plot's cell (x >= 400).
        let _ = p;
        let x0 = 400;
        for y in 0..400usize {
            for x in x0..800 {
                let i = (y * 800 + x) * 4;
                assert_eq!(before.rgba[i..i + 4], after.rgba[i..i + 4], "pixel {x},{y}");
            }
        }
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut fig = Figure::new(600, 500);
        fig.axes()
            .scatter3d([0.0, 1.0, 0.3], [0.0, 1.0, 0.8], [0.0, 1.0, 0.5]);
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
        let plot = fig.scene_and_layouts().1[0].plot;
        let at = |fig: &mut Figure| fig.axes().projection3d(plot).project([0.3, 0.8, 0.5]);
        let (x, y, _) = at(&mut fig);
        for _ in 0..5 {
            fig.zoom_at(x, y, 0.8);
        }
        let (x2, y2, _) = at(&mut fig);
        assert!(
            (x2 - x).abs() < 1e-6 && (y2 - y).abs() < 1e-6,
            "{x},{y} -> {x2},{y2}"
        );
        assert!(fig.axes().current_camera().zoom > 2.0);
    }

    #[test]
    fn radix_sort_orders_depths() {
        let mut fig = Figure::new(10, 10);
        let _ = &mut fig;
        let depths: Vec<f64> = (0..5000)
            .map(|i| ((i * 7919) % 5000) as f64 / 37.0 - 60.0)
            .collect();
        let keys: Vec<u64> = depths
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let b = (*d as f32).to_bits();
                let ord = if b >> 31 == 1 { !b } else { b | 0x8000_0000 };
                ((ord as u64) << 32) | i as u64
            })
            .collect();
        let sorted = super::radix_sort(keys);
        let ds: Vec<f64> = sorted
            .iter()
            .map(|k| depths[(k & 0xFFFF_FFFF) as usize])
            .collect();
        assert!(ds.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn isosurface_of_a_sphere() {
        let b = [(-1.0, 1.0); 3];
        let n = 24;
        let at = |i: usize| -1.0 + 2.0 * i as f64 / (n - 1) as f64;
        let mut v = Vec::new();
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    v.push(1.0 - (at(x).powi(2) + at(y).powi(2) + at(z).powi(2)).sqrt());
                }
            }
        }
        let (verts, faces) = isosurface_mesh(&v, [n, n, n], b, 0.5);
        assert!(faces.len() > 500);
        for p in &verts {
            let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((r - 0.5).abs() < 0.03, "{r}");
        }
        // Outward winding: normals point away from the centre.
        let out = faces
            .iter()
            .filter(|f| {
                let (a, b, c) = (
                    verts[f[0] as usize],
                    verts[f[1] as usize],
                    verts[f[2] as usize],
                );
                dot(cross(sub(b, a), sub(c, a)), a) > 0.0
            })
            .count();
        assert_eq!(out, faces.len());
    }

    #[test]
    fn full_turn_returns() {
        let c0 = Camera::new(-60.0, 30.0);
        for axis in [Axis3::X, Axis3::Y, Axis3::Z] {
            let mut c = c0;
            for _ in 0..24 {
                c.rotate(axis, 15.0);
            }
            let (a, b) = (c.matrix(), c0.matrix());
            for i in 0..3 {
                for j in 0..3 {
                    assert!((a[i][j] - b[i][j]).abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn shared_scene_and_depth_order() {
        let mut fig = Figure::new(500, 400);
        let ax = fig.axes();
        ax.scatter3d([0.0, 1.0], [0.0, 1.0], [0.0, 1.0]);
        ax.spheres3d([0.5], [0.5], [0.5], [0.2]).color("red");
        ax.ellipsoid([0.5, 0.5, 0.5], [0.1, 0.1, 0.3]);
        let b = fig.axes().bounds3d();
        assert_eq!(b[0], (0.0, 1.0));
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
        // Rotation changes the picture but not the size of the scene.
        fig.axes().rotate(Axis3::X, 90.0);
        let mut px2 = Pixels::default();
        fig.render_into(&mut px2).unwrap();
        assert_ne!(px.rgba, px2.rgba);
    }
}
