//! Interaction without a window: zoom, pan, hover tooltips and legend
//! clicks on a figure drawn by the application (GUI toolkits, WebAssembly,
//! custom windows). Positions are canvas pixels of the last render.
//!
//! ```
//! use dataviz::prelude::*;
//! let mut fig = Figure::new(600, 400);
//! fig.axes().line([0.0, 1.0, 2.0], [1.0, 3.0, 2.0]).label("a");
//! let mut px = Pixels::default();
//! fig.render_into(&mut px).unwrap();          // draw (and remember the layout)
//! fig.zoom_at(300.0, 200.0, 0.8);             // wheel: zoom in around the cursor
//! fig.pan_by(300.0, 200.0, 25.0, 0.0);        // drag by 25 px
//! if let Some(h) = fig.hover(300.0, 200.0) { println!("{}", h.text); }
//! fig.reset_view();
//! ```
//!
//! With [`Axes::auto_scale_y`] on, zooming and panning only change x and
//! the y-axes keep fitting the visible data.

use super::axes::{Axes, AxesKind, Layout};
use super::figure::Figure;

impl Axes {
    /// Zooms around a canvas position by `factor` (< 1 zooms in).
    pub(crate) fn zoom_at(&mut self, lay: &Layout, px: f64, py: f64, factor: f64) {
        if self.kind == AxesKind::ThreeD {
            self.zoom3d_by(factor);
            return;
        }
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
        // With auto scale the y-axes keep following the data.
        if !self.fit_y {
            self.y.limits(y0, y1);
            if let Some(m) = &lay.y2 {
                let (a, b) = zoom(m, py);
                self.y2.limits(a, b);
            }
        }
    }

    /// Pans by a canvas offset.
    pub(crate) fn pan(&mut self, lay: &Layout, dx: f64, dy: f64) {
        if self.kind == AxesKind::ThreeD {
            self.drag3d(dx, dy);
            return;
        }
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
        // With auto scale the y-axes keep following the data.
        if !self.fit_y {
            self.y.limits(y0, y1);
            if let Some(m) = &lay.y2 {
                let (a, b) = shift(m, dy);
                self.y2.limits(a, b);
            }
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
        // With auto scale the y-axes keep following the data.
        if !self.fit_y {
            self.y.limits(y0.min(y1), y0.max(y1));
            if let Some(m) = &lay.y2 {
                let (u, v) = (m.value(a.1), m.value(b.1));
                self.y2.limits(u.min(v), u.max(v));
            }
        }
    }
}

/// A hover readout: the nearest data point to a canvas position.
#[derive(Clone, Debug, PartialEq)]
pub struct Hover {
    /// Plot index.
    pub plot: usize,
    /// Canvas position of the data point.
    pub x: f64,
    /// Canvas position of the data point.
    pub y: f64,
    /// Text (series name and values).
    pub text: String,
}

impl Figure {
    /// The plot whose area contains the canvas position (last render).
    pub fn plot_at(&self, x: f64, y: f64) -> Option<usize> {
        self.last_layouts.iter().position(|l| l.plot.contains(x, y))
    }

    fn view_snapshot(&mut self) {
        if self.home.is_none() {
            self.home = Some(
                self.all_axes()
                    .map(|a| (a.x.min, a.x.max, a.y.min, a.y.max, a.y2.min, a.y2.max))
                    .collect(),
            );
        }
    }

    /// Zooms the plot under `(x, y)` around that point by `factor` (< 1
    /// zooms in); false if no plot is there. Takes effect at the next
    /// render.
    pub fn zoom_at(&mut self, x: f64, y: f64, factor: f64) -> bool {
        let Some(i) = self.plot_at(x, y) else {
            return false;
        };
        self.view_snapshot();
        let lay = self.last_layouts[i].clone();
        match self.cells_mut().get_mut(i) {
            Some(a) => {
                a.zoom_at(&lay, x, y, factor);
                true
            }
            None => false,
        }
    }

    /// Pans the plot under `(x, y)` by `(dx, dy)` canvas pixels (a drag).
    pub fn pan_by(&mut self, x: f64, y: f64, dx: f64, dy: f64) -> bool {
        let Some(i) = self.plot_at(x, y) else {
            return false;
        };
        self.view_snapshot();
        let lay = self.last_layouts[i].clone();
        match self.cells_mut().get_mut(i) {
            Some(a) => {
                a.pan(&lay, dx, dy);
                true
            }
            None => false,
        }
    }

    /// Zooms the plot under `a` to the rectangle between canvas points `a`
    /// and `b`.
    pub fn zoom_to_box(&mut self, a: (f64, f64), b: (f64, f64)) -> bool {
        let Some(i) = self.plot_at(a.0, a.1) else {
            return false;
        };
        self.view_snapshot();
        let lay = self.last_layouts[i].clone();
        match self.cells_mut().get_mut(i) {
            Some(ax) => {
                ax.zoom_box(&lay, a, b);
                true
            }
            None => false,
        }
    }

    /// Turns the 3D scene of plot `plot` by `deg` degrees around a data
    /// axis (false when it is not a 3D plot).
    pub fn rotate_view(
        &mut self,
        plot: usize,
        axis: super::series::three_d::Axis3,
        deg: f64,
    ) -> bool {
        match self.cells_mut().into_iter().nth(plot) {
            Some(a) if a.kind == AxesKind::ThreeD => {
                a.view3d.camera.rotate(axis, deg);
                true
            }
            _ => false,
        }
    }

    /// Back to the limits from before the first zoom or pan (and 3D scenes
    /// to their camera).
    pub fn reset_view(&mut self) {
        for a in self.all_axes() {
            a.reset_camera();
        }
        if let Some(home) = self.home.take() {
            for (a, l) in self.all_axes().zip(&home) {
                (a.x.min, a.x.max, a.y.min, a.y.max, a.y2.min, a.y2.max) = *l;
            }
        }
    }

    /// The data point nearest to a canvas position (within 40 px).
    pub fn hover(&mut self, x: f64, y: f64) -> Option<Hover> {
        let i = self.plot_at(x, y)?;
        let lay = self.last_layouts.get(i)?.clone();
        let ax = self.cells_mut().into_iter().nth(i)?;
        if ax.kind == AxesKind::Frameless {
            return None;
        }
        let h = ax.hit(&lay, x, y).filter(|h| h.distance < 40.0)?;
        Some(Hover {
            plot: i,
            x: h.px,
            y: h.py,
            text: h.label,
        })
    }

    /// Handles a click: a legend row hides or shows its series (true when
    /// something changed).
    pub fn click(&mut self, x: f64, y: f64) -> bool {
        let hit = self.last_layouts.iter().enumerate().find_map(|(i, l)| {
            l.legend_items
                .iter()
                .find(|(r, _)| r.contains(x, y))
                .map(|&(_, k)| (i, k))
        });
        let Some((i, k)) = hit else { return false };
        match self.cells_mut().into_iter().nth(i) {
            Some(a) => {
                let on = !a.is_visible(k);
                a.set_visible(k, on);
                true
            }
            None => false,
        }
    }

    /// Mutable plots in creation order.
    fn cells_mut(&mut self) -> Vec<&mut Axes> {
        self.all_axes().collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn zoom_pan_hover_click_without_a_window() {
        let mut fig = Figure::new(600, 400);
        let x: Vec<f64> = (0..=10).map(|v| v as f64).collect();
        fig.axes().line(&x, &x).label("a");
        fig.axes().line(&x, x.iter().map(|_| 5.0)).label("b");
        let mut px = Pixels::default();
        fig.render_into(&mut px).unwrap();
        let (_, l) = fig.scene_and_layouts();
        let p = l[0].plot;
        let (cx, cy) = (p.x + p.w / 2.0, p.y + p.h / 2.0);
        assert!(fig.zoom_at(cx, cy, 0.5));
        let (_, l2) = fig.scene_and_layouts();
        assert!(l2[0].x.max - l2[0].x.min < (l[0].x.max - l[0].x.min) * 0.6);
        assert!(fig.pan_by(cx, cy, 30.0, 0.0));
        fig.reset_view();
        let (_, l3) = fig.scene_and_layouts();
        assert_eq!((l3[0].x.min, l3[0].x.max), (l[0].x.min, l[0].x.max));
        let (hx, hy) = (l3[0].x.px(5.0), l3[0].y.px(5.0));
        let h = fig.hover(hx, hy).unwrap();
        assert!(h.text.contains('5'), "{}", h.text);
        let (r, _) = l3[0].legend_items[1];
        assert!(fig.click(r.x + 2.0, r.y + 2.0));
        assert!(!fig.axes().is_visible(1));
        assert!(fig.plot_at(1.0, 1.0).is_none());
    }
}
