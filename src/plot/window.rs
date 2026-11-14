//! Interactive window for figures (requires the `window` feature).
//!
//! Controls: mouse wheel zooms around the cursor, left-drag pans, right-drag
//! zooms to a box, clicking a legend row hides or shows its series,
//! Shift+drag measures (Δx, Δy and the change in percent), Ctrl+drag
//! selects a box ([`Figure::selection`]), a range slider
//! ([`Axes::range_slider`](crate::plot::Axes::range_slider)) is dragged to
//! pan and at its edges to zoom, `R` resets the view, `S` saves a PNG and
//! Shift+S an SVG (and PDF) next to the program, Ctrl+C copies the plots to
//! the clipboard, `V` starts and
//! stops recording a GIF next to the program (the plots without the mouse
//! overlays, at the rate they change; see [`crate::plot::anim`]), `H`
//! toggles the hover tooltip, `C` the crosshair, `Esc` closes the window.
//!
//! The crosshair follows the mouse with the cursor's x and y values on the
//! axes; plots showing the same x range (e.g. [`Figure::share_x`]) get a
//! linked cursor at the same x with the values of their series there, and
//! are zoomed and panned together.
//!
//! In [`Figure::show_live`] a plot the user zoomed or panned keeps that view
//! while new data arrives; `R` returns to following the data. Auto scale
//! (on by default, `A` toggles it) fits the y-axis to the data inside the
//! chosen x range, so zooming and panning only move along x; with it off
//! the y range is held as well.
//!
//! On Linux the window uses X11 (XWayland under Wayland) when available, so
//! the desktop draws a title bar to move and resize it (GNOME's Wayland
//! compositor draws none for such windows); set `DATAVIZ_WAYLAND=1` to use
//! Wayland directly.

use super::axes::{AxesKind, Layout};
use super::color::Color;
use super::figure::Figure;
use super::output::{self, Error, Pixels};
use super::scene::{Anchor, Baseline, Node, PathCmd, Scene, Stroke, TextNode};
use super::series::Rect;
use super::text::FontSet;
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};
use std::time::{Duration, Instant};

/// Manual limits of one axes: x, y and secondary y (min, max).
type AxesLimits = (
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
);
type Limits = Vec<AxesLimits>;
/// A measurement: plot, start and (once released) end in canvas units.
type Measure = (usize, (f64, f64), Option<(f64, f64)>);

/// Opens a window, preferring X11 on Linux desktops that also run Wayland
/// (see the module documentation).
fn open_window(title: &str, w: usize, h: usize) -> Result<Window, minifb::Error> {
    #[cfg(target_os = "linux")]
    {
        let both =
            std::env::var_os("DISPLAY").is_some() && std::env::var_os("WAYLAND_DISPLAY").is_some();
        if both && std::env::var_os("DATAVIZ_WAYLAND").is_none() {
            // minifb tries Wayland first and falls back to X11 when it has no
            // Wayland display: hide it while the window is created.
            let wayland = std::env::var_os("WAYLAND_DISPLAY");
            // SAFETY (edition 2024 semantics): environment changes are
            // serialized by std; restored right after.
            std::env::remove_var("WAYLAND_DISPLAY");
            let window = Window::new(title, w, h, WindowOptions::default());
            if let Some(v) = wayland {
                std::env::set_var("WAYLAND_DISPLAY", v);
            }
            if window.is_ok() {
                return window;
            }
        }
    }
    Window::new(title, w, h, WindowOptions::default())
}

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

    /// Limits of axes `i`.
    fn axes_limits(&mut self, i: usize) -> Option<AxesLimits> {
        self.cell_axes(i)
            .map(|a| (a.x.min, a.x.max, a.y.min, a.y.max, a.y2.min, a.y2.max))
    }

    /// Re-applies the views the user chose (after a data update). With
    /// `auto` only x is held and y fits the visible data.
    fn apply_views(&mut self, views: &[Option<AxesLimits>], auto: bool) {
        for (i, v) in views.iter().enumerate() {
            if let (Some(v), Some(a)) = (v, self.cell_axes(i)) {
                if auto {
                    (a.x.min, a.x.max) = (v.0, v.1);
                    (a.y.min, a.y.max, a.y2.min, a.y2.max) = (None, None, None, None);
                    a.fit_y = true;
                } else {
                    (a.x.min, a.x.max, a.y.min, a.y.max, a.y2.min, a.y2.max) = *v;
                }
            }
        }
    }

    fn frame(&mut self, px: &mut Pixels) -> Result<Vec<Layout>, Error> {
        let (scene, layouts) = self.scene_and_layouts();
        let mut pm = self.pixmap.take();
        let fonts = self.fonts();
        let r = super::render::render_to(
            &scene,
            &fonts,
            1.0,
            self.backend,
            &mut pm,
            super::render::Out::Pixels(px),
        );
        self.pixmap = pm;
        r.map(|_| layouts)
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
        let mut window = open_window(title, w, h)
            .map_err(|e| Error::Render(format!("cannot open window: {e}")))?;
        window.set_target_fps(60);
        let original = self.limits();
        let mut base = Pixels {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
        let mut layouts = self.frame(&mut base)?;
        // The frame as 0RGB, the same as opaque RGBA (for overlays), and
        // what the window shows.
        let mut plain: Vec<u32> = Vec::new();
        let mut under: Vec<u32> = Vec::new();
        let mut buffer: Vec<u32> = Vec::new();
        let mut dirty = true;
        let mut drag: Option<(f64, f64, usize)> = None;
        let mut box_start: Option<(f64, f64, usize)> = None;
        let mut hover = true;
        let mut cross = true;
        // Left button state of the previous frame (press detection) and
        // whether the current press clicked a legend row.
        let mut was_down = false;
        let mut clicked = false;
        let mut last_mouse = (f64::NAN, f64::NAN);
        let period = Duration::from_secs_f64(1.0 / fps.max(1) as f64);
        let mut last_update = Instant::now();
        let mut message: Option<(String, Instant)> = None;
        // Views the user zoomed or panned to, per plot (kept across updates).
        let mut views: Vec<Option<AxesLimits>> = Vec::new();
        let mut live = false;
        let mut auto = true;
        let mut recording: Option<Recording> = None;
        // Range slider drag: plot, mode, grab offset from the window start.
        let mut slider_drag: Option<(usize, SliderGrab, f64)> = None;
        // Measurement (Shift+drag): plot, start, end once released.
        let mut measure: Option<Measure> = None;
        // Box selection (Ctrl+drag) in progress: plot, start.
        let mut selecting: Option<(usize, (f64, f64))> = None;

        while window.is_open() && !window.is_key_down(Key::Escape) {
            if last_update.elapsed() >= period {
                last_update = Instant::now();
                if update(self) {
                    live = true;
                    self.apply_views(&views, auto);
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
                        remember(&mut views, i, self.axes_limits(i));
                        self.link_x(&layouts, i, &mut views);
                        self.apply_views(&views, auto);
                    }
                }
            }
            // Click on a legend row: hide or show the series.
            let down = window.get_mouse_down(MouseButton::Left);
            let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
            let ctrl = window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl);
            if down && !was_down {
                if measure.is_some_and(|m| m.2.is_some()) {
                    measure = None;
                    last_mouse = (f64::NAN, f64::NAN);
                }
                if let Some(m) = mouse {
                    if let Some((i, k)) = legend_at(&layouts, m) {
                        if let Some(a) = self.cell_axes(i) {
                            let on = !a.is_visible(k);
                            a.set_visible(k, on);
                            clicked = true;
                            dirty = true;
                        }
                    } else if let Some((i, grab, off)) = slider_at(&layouts, m) {
                        slider_drag = Some((i, grab, off));
                        clicked = true;
                    } else if let (true, Some(i)) = (shift, in_axes(m, &layouts)) {
                        measure = Some((i, m, None));
                        clicked = true;
                    } else if let (true, Some(i)) = (ctrl, in_axes(m, &layouts)) {
                        selecting = Some((i, m));
                        clicked = true;
                    }
                }
            }
            // Range slider: move the window or its edges.
            if let (true, Some((i, grab, off)), Some(m)) = (down, slider_drag, mouse) {
                if let Some((_, map)) = layouts.get(i).and_then(|l| l.slider.clone()) {
                    let l = &layouts[i];
                    let (a, b) = (map.px(l.x.min), map.px(l.x.max));
                    let (na, nb) = match grab {
                        SliderGrab::Start => (m.0.min(b - 4.0), b),
                        SliderGrab::End => (a, m.0.max(a + 4.0)),
                        SliderGrab::Move => (m.0 - off, m.0 - off + (b - a)),
                    };
                    let (u, v) = (map.value(na), map.value(nb));
                    if u.is_finite()
                        && v.is_finite()
                        && (u - l.x.min).abs() + (v - l.x.max).abs() > 0.0
                    {
                        if let Some(ax) = self.cell_axes(i) {
                            ax.x.limits(u.min(v), u.max(v));
                            dirty = true;
                        }
                        remember(&mut views, i, self.axes_limits(i));
                        self.link_x(&layouts, i, &mut views);
                        self.apply_views(&views, auto);
                    }
                }
            }
            if !down {
                if let (Some((i, a)), Some(b)) = (selecting.take(), mouse) {
                    let l = &layouts[i];
                    let (x0, x1) = (l.x.value(a.0), l.x.value(b.0));
                    let (y0, y1) = (l.y.value(a.1), l.y.value(b.1));
                    if (a.0 - b.0).abs() > 3.0 && (a.1 - b.1).abs() > 3.0 {
                        let sel = Selection {
                            plot: i,
                            x: (x0.min(x1), x0.max(x1)),
                            y: (y0.min(y1), y0.max(y1)),
                        };
                        let axes = self.cell_axes(i);
                        message = Some((
                            format!(
                                "selected x {} … {}, y {} … {}",
                                axes.as_ref()
                                    .map_or(String::new(), |a| a.cursor_label(l, false, sel.x.0)),
                                axes.as_ref()
                                    .map_or(String::new(), |a| a.cursor_label(l, false, sel.x.1)),
                                super::series::short(sel.y.0),
                                super::series::short(sel.y.1)
                            ),
                            Instant::now(),
                        ));
                        self.selection = Some(sel);
                    }
                }
                if let (Some((i, a, None)), Some(b)) = (measure, mouse) {
                    measure = Some((i, a, Some(b)));
                }
                slider_drag = None;
                clicked = false;
            }
            was_down = down;
            // Left-drag pan.
            if down && !clicked {
                if let Some(m) = mouse {
                    match drag {
                        None => drag = in_axes(m, &layouts).map(|i| (m.0, m.1, i)),
                        Some((x, y, i)) if (m.0 - x).abs() + (m.1 - y).abs() > 0.5 => {
                            let lay = layouts[i].clone();
                            if let Some(a) = self.cell_axes(i) {
                                a.pan(&lay, m.0 - x, m.1 - y);
                                dirty = true;
                            }
                            remember(&mut views, i, self.axes_limits(i));
                            self.link_x(&layouts, i, &mut views);
                            self.apply_views(&views, auto);
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
                    remember(&mut views, i, self.axes_limits(i));
                    self.link_x(&layouts, i, &mut views);
                    self.apply_views(&views, auto);
                    box_start = None;
                }
                (false, Some(_), None) => box_start = None,
                _ => {}
            }
            if window.is_key_pressed(Key::R, KeyRepeat::No) {
                views.clear();
                self.selection = None;
                measure = None;
                self.restore(&original);
                if live {
                    // Let the data set the view again.
                    update(self);
                }
                dirty = true;
            }
            if window.is_key_pressed(Key::A, KeyRepeat::No) {
                auto = !auto;
                if !auto {
                    // Hold the y range that is visible now.
                    for (i, v) in views.iter_mut().enumerate() {
                        if let (Some(v), Some(l)) = (v.as_mut(), layouts.get(i)) {
                            let (y0, y1) = (l.y.value(l.y.p0), l.y.value(l.y.p1));
                            (v.2, v.3) = (Some(y0.min(y1)), Some(y0.max(y1)));
                            if let Some(m) = &l.y2 {
                                let (u, w) = (m.value(m.p0), m.value(m.p1));
                                (v.4, v.5) = (Some(u.min(w)), Some(u.max(w)));
                            }
                        }
                    }
                }
                self.apply_views(&views, auto);
                message = Some((
                    format!("Auto scale {}", if auto { "on" } else { "off" }),
                    Instant::now(),
                ));
                last_mouse = (f64::NAN, f64::NAN);
                dirty = true;
            }
            if window.is_key_pressed(Key::H, KeyRepeat::No) {
                hover = !hover;
                last_mouse = (f64::NAN, f64::NAN);
            }
            if window.is_key_pressed(Key::C, KeyRepeat::No) && !ctrl {
                cross = !cross;
                last_mouse = (f64::NAN, f64::NAN);
            }
            if window.is_key_pressed(Key::C, KeyRepeat::No) && ctrl {
                // Ctrl+C: the plots as PNG on the clipboard.
                message = Some((
                    match base
                        .encode(output::Format::Png)
                        .map_err(|e| e.to_string())
                        .and_then(|png| copy_png(&png))
                    {
                        Ok(()) => "copied to the clipboard".to_string(),
                        Err(e) => format!("could not copy: {e}"),
                    },
                    Instant::now(),
                ));
                last_mouse = (f64::NAN, f64::NAN);
            }
            if window.is_key_pressed(Key::S, KeyRepeat::No) && shift {
                let stem = sanitize(title);
                let mut saved = Vec::new();
                for ext in ["svg", "pdf"] {
                    let name = format!("{stem}.{ext}");
                    if self.save(&name).is_ok() {
                        saved.push(name);
                    }
                }
                message = Some((
                    if saved.is_empty() {
                        "could not save".to_string()
                    } else {
                        format!("saved {}", saved.join(", "))
                    },
                    Instant::now(),
                ));
                last_mouse = (f64::NAN, f64::NAN);
            } else if window.is_key_pressed(Key::S, KeyRepeat::No) {
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

            if window.is_key_pressed(Key::V, KeyRepeat::No) {
                message = Some((
                    match recording.take() {
                        Some(r) => r.stop(),
                        None => match Recording::start(title) {
                            Ok(r) => {
                                r.send(&base);
                                let text = format!("recording {} (V stops)", r.name);
                                recording = Some(r);
                                text
                            }
                            Err(e) => format!("cannot record: {e}"),
                        },
                    },
                    Instant::now(),
                ));
                last_mouse = (f64::NAN, f64::NAN);
            }

            if dirty {
                layouts = self.frame(&mut base)?;
                if let Some(r) = &recording {
                    r.send(&base);
                }
                base.write_0rgb(&mut plain);
                under.clear();
                under.extend(plain.iter().map(|&p| {
                    // 0RGB → opaque RGBA bytes (overlays draw over these).
                    u32::from_ne_bytes([(p >> 16) as u8, (p >> 8) as u8, p as u8, 255])
                }));
                dirty = false;
                last_mouse = (f64::NAN, f64::NAN);
            }

            // Overlays (hover tooltip, zoom box, messages) only when needed.
            let m = mouse.unwrap_or((f64::NAN, f64::NAN));
            let overlay_needed = m != last_mouse
                || box_start.is_some()
                || message.is_some()
                || recording.is_some()
                || selecting.is_some()
                || slider_drag.is_some();
            if overlay_needed {
                last_mouse = m;
                let fonts = self.fonts();
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
                }
                // Selection being drawn, or the last one.
                let sel_rect = match (selecting, mouse) {
                    (Some((i, a)), Some(b)) => Some((i, a, b)),
                    _ => self.selection.as_ref().and_then(|s| {
                        let l = layouts.get(s.plot)?;
                        Some((
                            s.plot,
                            (l.x.px(s.x.0), l.y.px(s.y.0)),
                            (l.x.px(s.x.1), l.y.px(s.y.1)),
                        ))
                    }),
                };
                if let Some((_, a, b)) = sel_rect {
                    nodes.push(Node::Rect {
                        x: a.0.min(b.0),
                        y: a.1.min(b.1),
                        w: (a.0 - b.0).abs(),
                        h: (a.1 - b.1).abs(),
                        rx: 0.0,
                        fill: Some(Color::rgba(255, 152, 0, 0.12)),
                        stroke: Some(
                            Stroke::new(Color::rgb(255, 152, 0), 1.0).dashed(vec![4.0, 3.0]),
                        ),
                    });
                }
                if let Some((i, a, end)) = measure {
                    if let Some(b) = end.or(mouse) {
                        self.measure_overlay(&fonts, &layouts, i, a, b, &mut nodes);
                    }
                }
                let mut crossed = false;
                let busy = measure.is_some_and(|m| m.2.is_none())
                    || selecting.is_some()
                    || slider_drag.is_some();
                if let (true, None, None, Some(m), false) = (cross, drag, box_start, mouse, busy) {
                    if let (Some(i), None) = (in_axes(m, &layouts), legend_at(&layouts, m)) {
                        crossed = self.crosshair(&layouts, i, m, hover, &mut nodes);
                    }
                }
                if hover && !crossed && drag.is_none() && box_start.is_none() {
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
                                        &fonts,
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
                if live && views.iter().any(Option::is_some) && message.is_none() {
                    tooltip(
                        &fonts,
                        &mut nodes,
                        if auto {
                            "View held while data arrives · auto scale on (A) · R: follow live data"
                        } else {
                            "View held while data arrives · auto scale off (A) · R: follow live data"
                        },
                        (10.0, h as f64 - 30.0),
                        w as f64,
                        h as f64,
                    );
                }
                if let Some(r) = &recording {
                    axis_box(
                        &fonts,
                        &mut nodes,
                        &format!("● REC {:.0} s", r.start.elapsed().as_secs_f64()),
                        w as f64 - 8.0,
                        16.0,
                        Anchor::End,
                    );
                }
                if let Some((text, at)) = &message {
                    if at.elapsed() < Duration::from_secs(2) {
                        tooltip(
                            &fonts,
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
                buffer.clear();
                if nodes.is_empty() {
                    buffer.extend_from_slice(&plain);
                } else {
                    // Draw the overlay straight over a copy of the frame.
                    buffer.extend_from_slice(&under);
                    let mut scene = Scene::new(w as f64, h as f64);
                    scene.background = None;
                    scene.nodes = nodes;
                    super::math::expand(&mut scene.nodes, &fonts);
                    super::raster::draw_over(
                        &scene,
                        &fonts,
                        as_bytes_mut(&mut buffer),
                        w as u32,
                        h as u32,
                    );
                    super::raster::premul_to_0rgb_in_place(&mut buffer, 1);
                }
            }
            window
                .update_with_buffer(&buffer, w, h)
                .map_err(|e| Error::Render(e.to_string()))?;
        }
        if let Some(r) = recording {
            r.stop();
        }
        Ok(())
    }
}

impl Figure {
    /// Gives the plots that showed the same x range as plot `i` its new x
    /// limits (linked zoom and pan).
    fn link_x(&mut self, layouts: &[Layout], i: usize, views: &mut Vec<Option<AxesLimits>>) {
        let Some(v) = self.axes_limits(i) else { return };
        for j in 0..layouts.len() {
            if j == i || !same_x(&layouts[i], &layouts[j]) {
                continue;
            }
            if let Some(a) = self.cell_axes(j) {
                if a.kind != AxesKind::Cartesian {
                    continue;
                }
                (a.x.min, a.x.max) = (v.0, v.1);
                if views.get(j).is_none_or(Option::is_none) {
                    // y of a plot that was following the data keeps doing so.
                    (a.y.min, a.y.max, a.y2.min, a.y2.max) = (None, None, None, None);
                }
                let l = self.axes_limits(j);
                remember(views, j, l);
            }
        }
    }
}

/// A GIF recording of the window's frames, encoded on another thread.
struct Recording {
    tx: std::sync::mpsc::SyncSender<(Option<Pixels>, Duration)>,
    worker: std::thread::JoinHandle<Result<usize, Error>>,
    start: Instant,
    name: String,
}

impl Recording {
    fn start(title: &str) -> Result<Self, Error> {
        let base = sanitize(title);
        let name = (1..)
            .map(|i| format!("{base}-{i}.gif"))
            .find(|n| !std::path::Path::new(n).exists())
            .unwrap_or_else(|| format!("{base}.gif"));
        Self::start_path(name)
    }

    fn start_path(name: String) -> Result<Self, Error> {
        let mut rec = super::anim::Recorder::create(&name, 50.0)?;
        // A few frames of slack; when encoding falls behind, frames are
        // dropped instead of slowing the window down.
        let (tx, rx) = std::sync::mpsc::sync_channel::<(Option<Pixels>, Duration)>(3);
        let worker = std::thread::spawn(move || {
            let mut last = None;
            for (px, at) in rx {
                match px {
                    Some(px) => {
                        rec.push_at(&px, at)?;
                        last = Some(px);
                    }
                    // End: the last frame stays until the recording stops.
                    None => {
                        if let Some(px) = &last {
                            rec.push_at(px, at)?;
                        }
                    }
                }
            }
            let n = rec.frames();
            rec.finish()?;
            Ok(n)
        });
        Ok(Self {
            tx,
            worker,
            start: Instant::now(),
            name,
        })
    }

    fn send(&self, frame: &Pixels) {
        let _ = self
            .tx
            .try_send((Some(frame.clone()), self.start.elapsed()));
    }

    /// Ends the recording; returns the message to show.
    fn stop(self) -> String {
        let _ = self.tx.send((None, self.start.elapsed()));
        drop(self.tx);
        match self.worker.join() {
            Ok(Ok(n)) => format!("saved {} ({n} frames)", self.name),
            Ok(Err(e)) => format!("could not save {}: {e}", self.name),
            Err(_) => format!("could not save {}", self.name),
        }
    }
}

/// Which part of a range slider window is dragged.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SliderGrab {
    Start,
    End,
    Move,
}

/// The range slider under `m`: plot, part, and the offset of `m` from the
/// window start (a click outside the window centres it there).
fn slider_at(layouts: &[Layout], m: (f64, f64)) -> Option<(usize, SliderGrab, f64)> {
    layouts.iter().enumerate().find_map(|(i, l)| {
        let (r, map) = l.slider.as_ref()?;
        if !r.contains(m.0, m.1) {
            return None;
        }
        let (a, b) = (map.px(l.x.min), map.px(l.x.max));
        Some(if (m.0 - a).abs() <= 6.0 {
            (i, SliderGrab::Start, 0.0)
        } else if (m.0 - b).abs() <= 6.0 {
            (i, SliderGrab::End, 0.0)
        } else if m.0 > a && m.0 < b {
            (i, SliderGrab::Move, m.0 - a)
        } else {
            (i, SliderGrab::Move, (b - a) / 2.0)
        })
    })
}

/// A box selected in a window with Ctrl+drag (data coordinates).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection {
    /// Index of the plot (in the order the plots were created).
    pub plot: usize,
    /// x range.
    pub x: (f64, f64),
    /// y range.
    pub y: (f64, f64),
}

impl Selection {
    /// Whether a data point lies in the box.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x.0 && x <= self.x.1 && y >= self.y.0 && y <= self.y.1
    }
}

impl Figure {
    /// The box the user selected with Ctrl+drag in a window (cleared by
    /// `R`), e.g. to filter data in the `show_live` update.
    pub fn selection(&self) -> Option<Selection> {
        self.selection
    }

    /// Measurement between canvas points `a` and `b` of plot `i`.
    fn measure_overlay(
        &mut self,
        fonts: &FontSet,
        layouts: &[Layout],
        i: usize,
        a: (f64, f64),
        b: (f64, f64),
        nodes: &mut Vec<Node>,
    ) {
        let Some(l) = layouts.get(i) else { return };
        let Some(ax) = self.cell_axes(i) else { return };
        let (x0, x1) = (l.x.value(a.0), l.x.value(b.0));
        let (y0, y1) = (l.y.value(a.1), l.y.value(b.1));
        let dy = y1 - y0;
        let up = dy >= 0.0;
        let tint = if up {
            Color::rgb(38, 166, 154)
        } else {
            Color::rgb(239, 83, 80)
        };
        nodes.push(Node::Rect {
            x: a.0.min(b.0),
            y: a.1.min(b.1),
            w: (a.0 - b.0).abs(),
            h: (a.1 - b.1).abs(),
            rx: 0.0,
            fill: Some(tint.with_alpha(0.12)),
            stroke: None,
        });
        nodes.push(Node::Path {
            cmds: vec![PathCmd::MoveTo(a.0, a.1), PathCmd::LineTo(b.0, b.1)],
            fill: None,
            stroke: Some(Stroke::new(tint, 1.5)),
        });
        let dx = if ax.x.scale == super::scale::Scale::Time {
            duration(x1 - x0)
        } else {
            super::series::short(x1 - x0)
        };
        let pct = if y0 != 0.0 {
            format!(" ({:+.2} %)", dy / y0.abs() * 100.0)
        } else {
            String::new()
        };
        let lines = [
            format!("Δx {dx}"),
            format!("Δy {}{pct}", super::series::short(dy)),
        ];
        let text: Vec<&str> = lines.iter().map(String::as_str).collect();
        tooltip_lines(fonts, nodes, &text, b, l.plot, &[]);
    }
}

/// A time span in seconds as text ("2 d 3 h", "4 min 5 s").
fn duration(s: f64) -> String {
    let sign = if s < 0.0 { "−" } else { "" };
    let s = s.abs();
    let (d, h, m) = (
        (s / 86400.0).floor(),
        (s % 86400.0 / 3600.0).floor(),
        (s % 3600.0 / 60.0).floor(),
    );
    let sec = s % 60.0;
    let t = if d >= 1.0 {
        format!("{d} d {h} h")
    } else if h >= 1.0 {
        format!("{h} h {m} min")
    } else if m >= 1.0 {
        format!("{m} min {:.0} s", sec)
    } else {
        format!("{} s", super::series::short(sec))
    };
    format!("{sign}{t}")
}

/// Puts a PNG on the clipboard with the system's clipboard tool.
fn copy_png(png: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let pipe = |cmd: &str, args: &[&str]| -> Result<(), String> {
        let mut child = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        child
            .stdin
            .take()
            .ok_or("no input")?
            .write_all(png)
            .map_err(|e| e.to_string())?;
        // Clipboard tools on X11 keep running to serve the data.
        Ok(())
    };
    if cfg!(target_os = "macos") || cfg!(target_os = "windows") {
        let path = std::env::temp_dir().join("dataviz-clipboard.png");
        std::fs::write(&path, png).map_err(|e| e.to_string())?;
        let p = path.to_string_lossy().replace('\'', "");
        let ok = if cfg!(target_os = "macos") {
            Command::new("osascript")
                .args([
                    "-e",
                    &format!("set the clipboard to (read (POSIX file \"{p}\") as «class PNGf»)"),
                ])
                .status()
        } else {
            Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!("Add-Type -AssemblyName System.Windows.Forms; [Windows.Forms.Clipboard]::SetImage([Drawing.Image]::FromFile('{p}'))"),
                ])
                .status()
        };
        return match ok {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => Err(format!("clipboard tool failed ({s})")),
            Err(e) => Err(e.to_string()),
        };
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && pipe("wl-copy", &["--type", "image/png"]).is_ok()
    {
        return Ok(());
    }
    pipe("xclip", &["-selection", "clipboard", "-t", "image/png"])
        .map_err(|_| "needs wl-copy (Wayland) or xclip (X11)".to_string())
}

/// Records the view of plot `i` chosen by the user.
fn remember(views: &mut Vec<Option<AxesLimits>>, i: usize, limits: Option<AxesLimits>) {
    if views.len() <= i {
        views.resize(i + 1, None);
    }
    views[i] = limits;
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
fn tooltip(fonts: &FontSet, nodes: &mut Vec<Node>, text: &str, at: (f64, f64), w: f64, h: f64) {
    let size = 12.0;
    let tw = fonts.width(text, size) + 16.0;
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
        italic: false,
    }));
}

/// The legend row under `m`: (plot, series index).
fn legend_at(layouts: &[Layout], m: (f64, f64)) -> Option<(usize, usize)> {
    layouts.iter().enumerate().find_map(|(i, l)| {
        l.legend_items
            .iter()
            .find(|(r, _)| r.contains(m.0, m.1))
            .map(|&(_, k)| (i, k))
    })
}

/// The pixels as bytes (RGBA in memory order).
fn as_bytes_mut(v: &mut [u32]) -> &mut [u8] {
    // SAFETY: u8 has no alignment or validity requirements and the length
    // covers exactly the same memory.
    unsafe { std::slice::from_raw_parts_mut(v.as_mut_ptr().cast::<u8>(), v.len() * 4) }
}

/// Whether two plots show (about) the same x range: the same kind of scale
/// and ranges overlapping by at least 90 % of their union. Such plots get a
/// linked cursor and are zoomed and panned together.
fn same_x(a: &Layout, b: &Layout) -> bool {
    use super::scale::Scale;
    let kind = |s: &Scale| match s {
        Scale::Time => 0,
        Scale::Log(_) | Scale::SymLog(_) => 1,
        _ => 2,
    };
    if kind(&a.x.scale) != kind(&b.x.scale) {
        return false;
    }
    let overlap = a.x.max.min(b.x.max) - a.x.min.max(b.x.min);
    let union = a.x.max.max(b.x.max) - a.x.min.min(b.x.min);
    union > 0.0 && overlap >= 0.9 * union
}

const CROSS: Color = Color::rgba(90, 90, 90, 0.8);
const LABEL_BG: Color = Color::rgba(50, 50, 55, 0.92);

/// A value box on an axis edge.
fn axis_box(fonts: &FontSet, nodes: &mut Vec<Node>, text: &str, x: f64, y: f64, anchor: Anchor) {
    let size = 11.0;
    let tw = fonts.width(text, size) + 10.0;
    let th = size + 8.0;
    let left = match anchor {
        Anchor::Start => x,
        Anchor::Middle => x - tw / 2.0,
        Anchor::End => x - tw,
    };
    nodes.push(Node::Rect {
        x: left,
        y: y - th / 2.0,
        w: tw,
        h: th,
        rx: 2.0,
        fill: Some(LABEL_BG),
        stroke: None,
    });
    nodes.push(Node::Text(TextNode {
        x: left + tw / 2.0,
        y,
        text: text.to_string(),
        size,
        color: Color::WHITE,
        anchor: Anchor::Middle,
        baseline: Baseline::Middle,
        rotate: 0.0,
        bold: false,
        italic: false,
    }));
}

impl Figure {
    /// Crosshair in plot `i` at `m` and linked cursors in the plots with
    /// the same x range.
    fn crosshair(
        &mut self,
        layouts: &[Layout],
        i: usize,
        m: (f64, f64),
        values: bool,
        nodes: &mut Vec<Node>,
    ) -> bool {
        let fonts = self.fonts();
        let Some(a) = self.cell_axes(i) else {
            return false;
        };
        if a.kind != AxesKind::Cartesian {
            return false;
        }
        let l = &layouts[i];
        let p = l.plot;
        let stroke = Stroke::new(CROSS, 1.0).dashed(vec![4.0, 3.0]);
        let line = |x0: f64, y0: f64, x1: f64, y1: f64| Node::Path {
            cmds: vec![PathCmd::MoveTo(x0, y0), PathCmd::LineTo(x1, y1)],
            fill: None,
            stroke: Some(stroke.clone()),
        };
        // Lines on pixel centres stay one pixel wide.
        let (cx, cy) = (m.0.floor() + 0.5, m.1.floor() + 0.5);
        nodes.push(line(cx, p.y, cx, p.bottom()));
        nodes.push(line(p.x, cy, p.right(), cy));
        let xv = l.x.value(m.0);
        let xs = a.cursor_label(l, false, xv);
        let ys = a.cursor_label(l, true, l.y.value(m.1));
        axis_box(&fonts, nodes, &xs, m.0, p.bottom() + 10.0, Anchor::Middle);
        axis_box(&fonts, nodes, &ys, p.x - 2.0, m.1, Anchor::End);
        if let Some(y2) = &l.y2 {
            let v = y2.value(m.1);
            let s = super::series::short(v);
            axis_box(&fonts, nodes, &s, p.right() + 2.0, m.1, Anchor::Start);
        }
        if values {
            let hits = a.hits_at_x(l, m.0, 30.0);
            if !hits.is_empty() {
                let text: Vec<&str> = hits.iter().map(|h| h.label.as_str()).collect();
                let at = (m.0, (m.1 - 40.0).max(p.y + 4.0));
                tooltip_lines(&fonts, nodes, &text, at, p, &hits);
            }
        }
        for (j, o) in layouts.iter().enumerate() {
            if j == i || !same_x(l, o) {
                continue;
            }
            let Some(b) = self.cell_axes(j) else { continue };
            if b.kind != AxesKind::Cartesian {
                continue;
            }
            let px = o.x.px(xv);
            if !px.is_finite() || px < o.plot.x || px > o.plot.right() {
                continue;
            }
            let x = px.floor() + 0.5;
            nodes.push(line(x, o.plot.y, x, o.plot.bottom()));
            if values {
                let hits = b.hits_at_x(o, px, 30.0);
                if !hits.is_empty() {
                    let text: Vec<&str> = hits.iter().map(|h| h.label.as_str()).collect();
                    tooltip_lines(&fonts, nodes, &text, (px, o.plot.y + 4.0), o.plot, &hits);
                }
            }
        }
        true
    }
}

/// A box with several lines of text next to `at`, kept inside `r`.
/// `dots` are drawn at the data points the lines describe.
fn tooltip_lines(
    fonts: &FontSet,
    nodes: &mut Vec<Node>,
    lines: &[&str],
    at: (f64, f64),
    r: Rect,
    dots: &[super::series::Hit],
) {
    let size = 12.0;
    let line_h = size + 4.0;
    let tw = lines
        .iter()
        .map(|t| fonts.width(t, size))
        .fold(0.0, f64::max)
        + 16.0;
    let th = lines.len() as f64 * line_h + 8.0;
    let x = if at.0 + 12.0 + tw <= r.right() {
        at.0 + 12.0
    } else {
        (at.0 - 12.0 - tw).max(r.x)
    };
    let y = at.1.min(r.bottom() - th).max(r.y);
    for d in dots {
        if r.contains(d.px, d.py) {
            nodes.push(Node::Circle {
                cx: d.px,
                cy: d.py,
                r: 4.0,
                fill: Some(Color::WHITE),
                stroke: Some(Stroke::new(Color::rgb(30, 30, 30), 1.5)),
            });
        }
    }
    nodes.push(Node::Rect {
        x,
        y,
        w: tw,
        h: th,
        rx: 4.0,
        fill: Some(Color::rgba(255, 255, 255, 0.95)),
        stroke: Some(Stroke::new(Color::rgb(120, 120, 120), 1.0)),
    });
    for (k, t) in lines.iter().enumerate() {
        nodes.push(Node::Text(TextNode {
            x: x + 8.0,
            y: y + 4.0 + line_h * (k as f64 + 0.5),
            text: t.to_string(),
            size,
            color: Color::rgb(20, 20, 20),
            anchor: Anchor::Start,
            baseline: Baseline::Middle,
            rotate: 0.0,
            bold: false,
            italic: false,
        }));
    }
}

#[cfg(test)]
mod tests {
    /// A view chosen by the user survives a data update that rebuilds or
    /// re-limits the figure.
    #[test]
    fn auto_scale_holds_only_x() {
        let mut fig = Figure::new(400, 300);
        fig.axes().line([0.0, 1.0, 2.0], [0.0, 10.0, 1.0]);
        fig.axes().x_limits(1.5, 2.0).y_limits(-5.0, 50.0);
        let mut views = Vec::new();
        remember(&mut views, 0, fig.axes_limits(0));
        fig.apply_views(&views, true);
        let a = fig.axes();
        assert_eq!(
            (a.x.min, a.x.max, a.y.min, a.y.max),
            (Some(1.5), Some(2.0), None, None)
        );
        assert!(a.auto_scales_y());
        // Held completely with auto scale off.
        fig.apply_views(&views, false);
        assert_eq!(fig.axes().y.max, Some(50.0));
    }

    #[test]
    fn user_view_survives_updates() {
        // The same code builds the figure on every update (as live
        // dashboards do), so plot indices are stable.
        let build = |n: f64| {
            let mut fig = Figure::new(400, 300);
            fig.grid(1, 2);
            fig.at(0, 0).line([0.0, n], [0.0, n]);
            fig.at(0, 1).line([0.0, n], [0.0, n]);
            fig.at(0, 1).x_limits(0.0, n);
            fig
        };
        let mut fig = build(1.0);
        fig.at(0, 1).x_limits(0.2, 0.4).y_limits(0.1, 0.3);
        let mut views = Vec::new();
        remember(&mut views, 1, fig.axes_limits(1));
        // New data: the figure is rebuilt with its own limits.
        fig = build(2.0);
        fig.apply_views(&views, false);
        let a = fig.at(0, 1);
        assert_eq!(
            (a.x.min, a.x.max, a.y.min, a.y.max),
            (Some(0.2), Some(0.4), Some(0.1), Some(0.3))
        );
        // The other plot keeps following the data.
        assert_eq!(fig.at(0, 0).x.min, None);
    }

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
    fn overlay_draws_over_the_frame() {
        // A black frame with a white half-transparent box drawn over it.
        let mut buf = vec![u32::from_ne_bytes([0, 0, 0, 255]); 16];
        let mut scene = Scene::new(4.0, 4.0);
        scene.background = None;
        scene.nodes.push(Node::Rect {
            x: 0.0,
            y: 0.0,
            w: 4.0,
            h: 4.0,
            rx: 0.0,
            fill: Some(Color::rgba(255, 255, 255, 0.5)),
            stroke: None,
        });
        let fonts = Figure::new(4, 4).fonts();
        crate::plot::raster::draw_over(&scene, &fonts, as_bytes_mut(&mut buf), 4, 4);
        crate::plot::raster::premul_to_0rgb_in_place(&mut buf, 1);
        assert!(
            buf.iter().all(|&p| (0x7f7f7f..=0x818181).contains(&p)),
            "{buf:x?}"
        );
    }

    #[test]
    fn legend_click_hides_a_series() {
        let mut f = Figure::new(400, 300);
        f.axes().line(vec![0.0, 1.0], vec![0.0, 1.0]).label("a");
        f.axes().line(vec![0.0, 1.0], vec![5.0, 9.0]).label("b");
        let (_, l) = f.scene_and_layouts();
        assert_eq!(l[0].legend_items.len(), 2);
        let (r, k) = l[0].legend_items[1];
        assert_eq!(
            legend_at(&l, (r.x + r.w / 2.0, r.y + r.h / 2.0)),
            Some((0, k))
        );
        f.axes().set_visible(k, false);
        let (_, l) = f.scene_and_layouts();
        // The hidden series no longer sets the y range.
        assert!(l[0].y.max < 5.0, "{}", l[0].y.max);
        assert_eq!(l[0].legend_items.len(), 2);
    }

    #[test]
    fn range_slider_parts() {
        let mut f = Figure::new(600, 400);
        let x: Vec<f64> = (0..100).map(|i| i as f64).collect();
        f.axes().line(&x, &x);
        f.axes().range_slider(true);
        f.axes().x_limits(20.0, 60.0);
        let (_, l) = f.scene_and_layouts();
        let (r, map) = l[0].slider.clone().unwrap();
        assert!(r.y > l[0].plot.bottom());
        let (a, b) = (map.px(20.0), map.px(60.0));
        let mid = r.y + r.h / 2.0;
        assert_eq!(slider_at(&l, (a, mid)).map(|s| s.1), Some(SliderGrab::Start));
        assert_eq!(slider_at(&l, (b, mid)).map(|s| s.1), Some(SliderGrab::End));
        assert_eq!(slider_at(&l, ((a + b) / 2.0, mid)).map(|s| s.1), Some(SliderGrab::Move));
        assert!(slider_at(&l, (a, r.y - 5.0)).is_none());
        // The slider covers all data, not just the shown range.
        assert!(map.min <= 0.0 && map.max >= 99.0);
    }

    #[test]
    fn measurement_and_selection() {
        assert_eq!(duration(3.0 * 3600.0 + 120.0), "3 h 2 min");
        assert_eq!(duration(-90.0), "−1 min 30 s");
        let s = Selection { plot: 0, x: (0.0, 1.0), y: (2.0, 3.0) };
        assert!(s.contains(0.5, 2.5) && !s.contains(1.5, 2.5));
        let mut f = Figure::new(400, 300);
        f.axes().line(vec![0.0, 10.0], vec![100.0, 110.0]);
        let (_, l) = f.scene_and_layouts();
        let fonts = f.fonts();
        let mut nodes = Vec::new();
        let (a, b) = ((l[0].x.px(0.0), l[0].y.px(100.0)), (l[0].x.px(10.0), l[0].y.px(110.0)));
        f.measure_overlay(&fonts, &l, 0, a, b, &mut nodes);
        let texts: Vec<String> = nodes
            .iter()
            .filter_map(|n| match n {
                Node::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t.contains("+10.00 %")), "{texts:?}");
    }

    #[test]
    fn recording_writes_a_gif() {
        let dir = std::env::temp_dir().join(format!("dataviz-rec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let name = dir.join("r.gif").to_string_lossy().into_owned();
        let mut f = Figure::new(120, 80);
        f.axes().line(vec![0.0, 1.0], vec![0.0, 1.0]);
        let r = Recording::start_path(name.clone()).unwrap();
        r.send(&f.to_pixels().unwrap());
        std::thread::sleep(Duration::from_millis(30));
        f.axes().line(vec![0.0, 1.0], vec![1.0, 0.0]);
        r.send(&f.to_pixels().unwrap());
        std::thread::sleep(Duration::from_millis(30));
        assert!(r.stop().contains("(2 frames)"));
        let bytes = std::fs::read(&name).unwrap();
        assert_eq!(&bytes[..6], b"GIF89a");
    }

    #[test]
    fn linked_zoom_moves_plots_with_the_same_x() {
        let mut f = Figure::new(600, 600);
        f.grid(3, 1);
        let t: Vec<f64> = (0..50).map(|i| 1.7e9 + 60.0 * i as f64 + 30.0).collect();
        let v: Vec<f64> = (0..50).map(|i| 100.0 + i as f64).collect();
        f.at(0, 0).candlestick(&t, &v, &v, &v, &v).width(45.0);
        f.at(1, 0).bar_xy(&t, &v).width(45.0);
        f.at(2, 0).line(vec![0.0, 1.0], vec![0.0, 1.0]);
        let (_, l) = f.scene_and_layouts();
        assert!(
            same_x(&l[0], &l[1]),
            "{:?} {:?}",
            (l[0].x.min, l[0].x.max),
            (l[1].x.min, l[1].x.max)
        );
        assert!(!same_x(&l[0], &l[2]));
        let mut views = Vec::new();
        let p = l[0].plot;
        f.cell_axes(0)
            .unwrap()
            .zoom_at(&l[0], p.x + p.w / 2.0, p.y + p.h / 2.0, 0.5);
        remember(&mut views, 0, f.axes_limits(0));
        f.link_x(&l, 0, &mut views);
        f.apply_views(&views, true);
        let (_, l2) = f.scene_and_layouts();
        assert!(same_x(&l2[0], &l2[1]));
        assert!(l2[0].x.max - l2[0].x.min < (l[0].x.max - l[0].x.min) * 0.6);
        assert_eq!((l2[2].x.min, l2[2].x.max), (l[2].x.min, l[2].x.max));
    }

    #[test]
    fn linked_cursor_finds_values_at_x() {
        let mut f = Figure::new(400, 400);
        f.grid(2, 1).share_x(true);
        f.at(0, 0)
            .line(vec![0.0, 1.0, 2.0], vec![1.0, 2.0, 3.0])
            .label("a");
        f.at(1, 0).bar(vec![0.0, 1.0, 2.0], vec![4.0, 5.0, 6.0]);
        let (_, l) = f.scene_and_layouts();
        assert!(same_x(&l[0], &l[1]), "{:?} {:?}", l[0].x, l[1].x);
        let px = l[1].x.px(1.0);
        let hits = f.cell_axes(1).unwrap().hits_at_x(&l[1], px, 30.0);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].label.contains('5'), "{}", hits[0].label);
        let mut nodes = Vec::new();
        f.crosshair(
            &l,
            0,
            (l[0].x.px(1.0), l[0].plot.y + 10.0),
            true,
            &mut nodes,
        );
        let texts: Vec<String> = nodes
            .iter()
            .filter_map(|n| match n {
                Node::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "1.000"), "{texts:?}");
        assert!(texts.iter().any(|t| t.contains('5')), "{texts:?}");
    }
}
