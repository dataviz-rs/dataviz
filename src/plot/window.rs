//! Interactive window for figures (requires the `window` feature).
//!
//! Controls: mouse wheel zooms around the cursor, left-drag pans, right-drag
//! zooms to a box, `R` resets the view, `S` saves a PNG next to the program,
//! `H` toggles the hover tooltip, `Esc` closes the window.

use super::axes::{AxesKind, Layout};
use super::color::Color;
use super::figure::Figure;
use super::output::{self, Error, Pixels};
use super::scene::{Anchor, Baseline, Node, Scene, Stroke, TextNode};
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use std::time::{Duration, Instant};

type Limits = Vec<(
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
)>;

impl super::axes::Axes {
    /// Zooms around a canvas position by `factor` (< 1 zooms in).
    pub(crate) fn zoom_at(&mut self, lay: &Layout, px: f64, py: f64, factor: f64) {
        if self.kind != AxesKind::Cartesian {
            return;
        }
        let zoom = |map: &super::axis::AxisMap, at: f64| -> (f64, f64) {
            let (p0, p1) = (map.p0, map.p1);
            let a = at + (p0 - at) * factor;
            let b = at + (p1 - at) * factor;
            let (u, v) = (map.value(a), map.value(b));
            (u.min(v), u.max(v))
        };
        let (x0, x1) = zoom(&lay.x, px);
        let (y0, y1) = zoom(&lay.y, py);
        self.x.limits(x0, x1);
        self.y.limits(y0, y1);
        if let Some(m) = &lay.y2 {
            let (a, b) = zoom(m, py);
            self.y2.limits(a, b);
        }
    }

    /// Pans by a canvas offset.
    pub(crate) fn pan(&mut self, lay: &Layout, dx: f64, dy: f64) {
        if self.kind != AxesKind::Cartesian {
            return;
        }
        let shift = |map: &super::axis::AxisMap, d: f64| -> (f64, f64) {
            let (u, v) = (map.value(map.p0 - d), map.value(map.p1 - d));
            (u.min(v), u.max(v))
        };
        let (x0, x1) = shift(&lay.x, dx);
        let (y0, y1) = shift(&lay.y, dy);
        self.x.limits(x0, x1);
        self.y.limits(y0, y1);
        if let Some(m) = &lay.y2 {
            let (a, b) = shift(m, dy);
            self.y2.limits(a, b);
        }
    }

    /// Zooms to a canvas rectangle.
    pub(crate) fn zoom_box(&mut self, lay: &Layout, a: (f64, f64), b: (f64, f64)) {
        if self.kind != AxesKind::Cartesian || (a.0 - b.0).abs() < 4.0 || (a.1 - b.1).abs() < 4.0 {
            return;
        }
        let (x0, x1) = (lay.x.value(a.0), lay.x.value(b.0));
        let (y0, y1) = (lay.y.value(a.1), lay.y.value(b.1));
        self.x.limits(x0.min(x1), x0.max(x1));
        self.y.limits(y0.min(y1), y0.max(y1));
        if let Some(m) = &lay.y2 {
            let (u, v) = (m.value(a.1), m.value(b.1));
            self.y2.limits(u.min(v), u.max(v));
        }
    }
}

impl Figure {
    fn limits(&mut self) -> Limits {
        self.all_axes()
            .map(|a| (a.x.min, a.x.max, a.y.min, a.y.max, a.y2.min, a.y2.max))
            .collect()
    }

    fn restore(&mut self, l: &Limits) {
        for (a, l) in self.all_axes().zip(l) {
            (a.x.min, a.x.max, a.y.min, a.y.max, a.y2.min, a.y2.max) = *l;
        }
    }

    fn frame(&mut self) -> Result<(Pixels, Vec<Layout>), Error> {
        let (scene, layouts) = self.scene_and_layouts();
        let px = output::rasterize(&scene, &self.fonts(), 1.0)?;
        Ok((px, layouts))
    }

    /// Opens an interactive window showing the figure; returns when it is
    /// closed.
    pub fn show(&mut self, title: &str) -> Result<(), Error> {
        self.show_live(title, 30, |_| false)
    }

    /// Like [`Figure::show`], calling `update` up to `fps` times per second;
    /// when it returns `true` the figure is redrawn (real-time data).
    ///
    /// ```no_run
    /// use dataviz::prelude::*;
    /// use dataviz::plot::series::line::Line;
    /// let mut fig = Figure::new(800, 400);
    /// fig.axes().line(Vec::<f64>::new(), Vec::<f64>::new()).label("signal");
    /// let mut t = 0.0;
    /// fig.show_live("live", 30, |fig| {
    ///     t += 0.05;
    ///     let line = fig.axes().series_mut::<Line>(0).unwrap();
    ///     line.push(t, t.sin());
    ///     line.keep_last(300);
    ///     true
    /// }).unwrap();
    /// ```
    pub fn show_live(
        &mut self,
        title: &str,
        fps: u32,
        mut update: impl FnMut(&mut Figure) -> bool,
    ) -> Result<(), Error> {
        let (w, h) = self.size();
        let (w, h) = (w as usize, h as usize);
        let mut window = Window::new(title, w, h, WindowOptions::default())
            .map_err(|e| Error::Render(format!("cannot open window: {e}")))?;
        window.set_target_fps(60);
        let original = self.limits();
        let (mut base, mut layouts) = self.frame()?;
        let mut buffer = base.to_0rgb();
        let mut dirty = false;
        let mut drag: Option<(f64, f64, usize)> = None;
        let mut box_start: Option<(f64, f64, usize)> = None;
        let mut hover = true;
        let mut last_mouse = (f64::NAN, f64::NAN);
        let period = Duration::from_secs_f64(1.0 / fps.max(1) as f64);
        let mut last_update = Instant::now();
        let mut message: Option<(String, Instant)> = None;

        while window.is_open() && !window.is_key_down(Key::Escape) {
            if last_update.elapsed() >= period {
                last_update = Instant::now();
                if update(self) {
                    dirty = true;
                }
            }
            let mouse = window
                .get_mouse_pos(MouseMode::Discard)
                .map(|(x, y)| (x as f64, y as f64));
            let in_axes = |p: (f64, f64), layouts: &[Layout]| {
                layouts.iter().position(|l| l.plot.contains(p.0, p.1))
            };

            // Wheel zoom.
            if let (Some((_, wy)), Some(m)) = (window.get_scroll_wheel(), mouse) {
                if wy != 0.0 {
                    if let Some(i) = in_axes(m, &layouts) {
                        let f = if wy > 0.0 { 0.85 } else { 1.0 / 0.85 };
                        let lay = layouts[i].clone();
                        if let Some(a) = self.cell_axes(i) {
                            a.zoom_at(&lay, m.0, m.1, f);
                            dirty = true;
                        }
                    }
                }
            }
            // Left-drag pan.
            if window.get_mouse_down(MouseButton::Left) {
                if let Some(m) = mouse {
                    match drag {
                        None => drag = in_axes(m, &layouts).map(|i| (m.0, m.1, i)),
                        Some((x, y, i)) if (m.0 - x).abs() + (m.1 - y).abs() > 0.5 => {
                            let lay = layouts[i].clone();
                            if let Some(a) = self.cell_axes(i) {
                                a.pan(&lay, m.0 - x, m.1 - y);
                                dirty = true;
                            }
                            drag = Some((m.0, m.1, i));
                        }
                        _ => {}
                    }
                }
            } else {
                drag = None;
            }
            // Right-drag box zoom.
            let right = window.get_mouse_down(MouseButton::Right);
            match (right, box_start, mouse) {
                (true, None, Some(m)) => box_start = in_axes(m, &layouts).map(|i| (m.0, m.1, i)),
                (false, Some((x, y, i)), Some(m)) => {
                    let lay = layouts[i].clone();
                    if let Some(a) = self.cell_axes(i) {
                        a.zoom_box(&lay, (x, y), m);
                        dirty = true;
                    }
                    box_start = None;
                }
                (false, Some(_), None) => box_start = None,
                _ => {}
            }
            if window.is_key_pressed(Key::R, KeyRepeat::No) {
                self.restore(&original);
                dirty = true;
            }
            if window.is_key_pressed(Key::H, KeyRepeat::No) {
                hover = !hover;
                last_mouse = (f64::NAN, f64::NAN);
            }
            if window.is_key_pressed(Key::S, KeyRepeat::No) {
                let name = format!("{}.png", sanitize(title));
                message = Some((
                    match base
                        .encode(output::Format::Png)
                        .map(|b| std::fs::write(&name, b))
                    {
                        Ok(Ok(())) => format!("saved {name}"),
                        _ => format!("could not save {name}"),
                    },
                    Instant::now(),
                ));
                last_mouse = (f64::NAN, f64::NAN);
            }

            if dirty {
                (base, layouts) = self.frame()?;
                buffer = base.to_0rgb();
                dirty = false;
                last_mouse = (f64::NAN, f64::NAN);
            }

            // Overlays (hover tooltip, zoom box, messages) only when needed.
            let m = mouse.unwrap_or((f64::NAN, f64::NAN));
            let overlay_needed = m != last_mouse || box_start.is_some() || message.is_some();
            if overlay_needed {
                last_mouse = m;
                let mut nodes = Vec::new();
                if let (Some((x, y, _)), Some(m)) = (box_start, mouse) {
                    nodes.push(Node::Rect {
                        x: x.min(m.0),
                        y: y.min(m.1),
                        w: (m.0 - x).abs(),
                        h: (m.1 - y).abs(),
                        rx: 0.0,
                        fill: Some(Color::rgba(30, 120, 220, 0.15)),
                        stroke: Some(Stroke::new(Color::rgb(30, 120, 220), 1.0)),
                    });
                } else if hover && drag.is_none() {
                    if let Some(m) = mouse {
                        if let Some(i) = in_axes(m, &layouts).or_else(|| {
                            layouts
                                .iter()
                                .position(|l| l.plot.inset(-20.0).contains(m.0, m.1))
                        }) {
                            if let Some(hit) =
                                self.cell_axes(i).and_then(|a| a.hit(&layouts[i], m.0, m.1))
                            {
                                if hit.distance < 40.0 {
                                    tooltip(
                                        &mut nodes,
                                        &hit.label,
                                        (hit.px, hit.py),
                                        w as f64,
                                        h as f64,
                                    );
                                }
                            }
                        }
                    }
                }
                if let Some((text, at)) = &message {
                    if at.elapsed() < Duration::from_secs(2) {
                        tooltip(
                            &mut nodes,
                            text,
                            (10.0, h as f64 - 30.0),
                            w as f64,
                            h as f64,
                        );
                    } else {
                        message = None;
                    }
                }
                buffer = if nodes.is_empty() {
                    base.to_0rgb()
                } else {
                    let mut scene = Scene::new(w as f64, h as f64);
                    scene.background = None;
                    scene.font_family = self.fonts().family.clone();
                    scene.nodes = nodes;
                    let over = output::rasterize(&scene, &self.fonts(), 1.0)?;
                    composite(&base, &over)
                };
            }
            window
                .update_with_buffer(&buffer, w, h)
                .map_err(|e| Error::Render(e.to_string()))?;
        }
        Ok(())
    }
}

fn sanitize(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if t.is_empty() {
        "figure".into()
    } else {
        t
    }
}

/// Tooltip box with a marker at the data point.
fn tooltip(nodes: &mut Vec<Node>, text: &str, at: (f64, f64), w: f64, h: f64) {
    let size = 12.0;
    let tw = text.chars().count() as f64 * size * 0.6 + 16.0;
    let th = size + 12.0;
    let x = (at.0 + 12.0).min(w - tw - 4.0).max(4.0);
    let y = (at.1 - th - 10.0).max(4.0).min(h - th - 4.0);
    nodes.push(Node::Circle {
        cx: at.0,
        cy: at.1,
        r: 5.0,
        fill: None,
        stroke: Some(Stroke::new(Color::rgb(30, 30, 30), 2.0)),
    });
    nodes.push(Node::Rect {
        x,
        y,
        w: tw,
        h: th,
        rx: 4.0,
        fill: Some(Color::rgba(255, 255, 255, 0.95)),
        stroke: Some(Stroke::new(Color::rgb(120, 120, 120), 1.0)),
    });
    nodes.push(Node::Text(TextNode {
        x: x + 8.0,
        y: y + th / 2.0,
        text: text.to_string(),
        size,
        color: Color::rgb(20, 20, 20),
        anchor: Anchor::Start,
        baseline: Baseline::Middle,
        rotate: 0.0,
        bold: false,
    }));
}

/// `over` (straight alpha) over `base`, as 0RGB.
fn composite(base: &Pixels, over: &Pixels) -> Vec<u32> {
    base.rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(over.rgba.as_chunks::<4>().0)
        .map(|(b, o)| {
            let a = o[3] as u32;
            let ba = b[3] as u32;
            let c = |bv: u8, ov: u8| {
                let bv = (bv as u32 * ba + 255 * (255 - ba)) / 255;
                (ov as u32 * a + bv * (255 - a)) / 255
            };
            c(b[0], o[0]) << 16 | c(b[1], o[1]) << 8 | c(b[2], o[2])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fig() -> (Figure, Layout) {
        let mut f = Figure::new(400, 300);
        f.axes().line([0.0, 10.0], [0.0, 10.0]);
        let (_, mut l) = f.scene_and_layouts();
        (f, l.remove(0))
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let (mut f, lay) = fig();
        let (px, py) = (
            lay.plot.x + lay.plot.w * 0.25,
            lay.plot.y + lay.plot.h * 0.5,
        );
        let before = (lay.x.value(px), lay.y.value(py));
        f.axes().zoom_at(&lay, px, py, 0.5);
        let (_, l2) = f.scene_and_layouts();
        let after = (l2[0].x.value(px), l2[0].y.value(py));
        // Tick labels may change width after zooming and shift the plot area
        // by a few pixels, so allow 5 % of the new span.
        let (sx, sy) = (l2[0].x.max - l2[0].x.min, l2[0].y.max - l2[0].y.min);
        assert!(
            (before.0 - after.0).abs() < sx * 0.05 && (before.1 - after.1).abs() < sy * 0.05,
            "{before:?} {after:?}"
        );
        assert!((l2[0].x.max - l2[0].x.min) < (lay.x.max - lay.x.min) * 0.51);
    }

    #[test]
    fn pan_and_box_zoom() {
        let (mut f, lay) = fig();
        f.axes().pan(&lay, lay.plot.w / 2.0, 0.0);
        let (_, l2) = f.scene_and_layouts();
        let span = lay.x.max - lay.x.min;
        assert!((l2[0].x.min - (lay.x.min - span / 2.0)).abs() < 1e-9);

        let (mut f, lay) = fig();
        let p = lay.plot;
        f.axes()
            .zoom_box(&lay, (p.x, p.y), (p.x + p.w / 2.0, p.y + p.h / 2.0));
        let (_, l2) = f.scene_and_layouts();
        assert!((l2[0].x.max - lay.x.value(p.x + p.w / 2.0)).abs() < 1e-9);
        assert!((l2[0].y.min - lay.y.value(p.y + p.h / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn composite_blends() {
        let base = Pixels {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        };
        let over = Pixels {
            width: 1,
            height: 1,
            rgba: vec![255, 255, 255, 128],
        };
        assert_eq!(composite(&base, &over), vec![0x808080]);
    }
}
