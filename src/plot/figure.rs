//! The figure: canvas size, theme, title and a grid of axes.

use super::axes::{Axes, Layout};
use super::output::{self, Error, Format, Pixels};
use super::scene::{Anchor, Baseline, Node, Scene};
use super::series::Rect;
use super::text::FontSet;
use super::theme::Theme;
use std::path::Path;
use std::sync::Arc;

struct Cell {
    row: usize,
    col: usize,
    rowspan: usize,
    colspan: usize,
    axes: Axes,
}

/// A figure with one or more plots.
///
/// ```
/// use dataviz::prelude::*;
///
/// let mut fig = Figure::new(800, 500);
/// fig.title("Demo");
/// let ax = fig.axes();
/// ax.line([1, 2, 3, 4], [1.0, 4.0, 9.0, 16.0]).label("x²").marker(Marker::Circle);
/// ax.x_label("x").y_label("y");
/// let svg = fig.to_svg();
/// assert!(svg.starts_with("<svg"));
/// // fig.save("demo.png")?;  // .svg .png .jpg .webp .bmp (.pdf with the `pdf` feature)
/// ```
pub struct Figure {
    width: f64,
    height: f64,
    theme: Theme,
    title: Option<String>,
    rows: usize,
    cols: usize,
    cells: Vec<Cell>,
    scale: f64,
    spacing: f64,
    share_x: bool,
    share_y: bool,
}

impl Figure {
    /// A figure of `width` × `height` pixels with the default theme.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1) as f64,
            height: height.max(1) as f64,
            theme: Theme::default(),
            title: None,
            rows: 1,
            cols: 1,
            cells: Vec::new(),
            scale: 1.0,
            spacing: 16.0,
            share_x: false,
            share_y: false,
        }
    }

    /// Uses a theme. Set it before adding data: colors are assigned from the
    /// theme palette when series are added.
    pub fn theme(&mut self, theme: Theme) -> &mut Self {
        for c in &mut self.cells {
            c.axes.defaults = super::axes::Defaults::from_theme(&theme);
        }
        self.theme = theme;
        self
    }

    /// The current theme (mutable).
    pub fn theme_mut(&mut self) -> &mut Theme {
        &mut self.theme
    }

    /// Figure title.
    pub fn title(&mut self, title: impl Into<String>) -> &mut Self {
        self.title = Some(title.into());
        self
    }

    /// Font file for all text (TTF/OTF).
    pub fn font(&mut self, path: impl Into<String>) -> &mut Self {
        self.theme.font = Some(path.into());
        self
    }

    /// Splits the figure into a `rows` × `cols` grid of plots.
    pub fn grid(&mut self, rows: usize, cols: usize) -> &mut Self {
        self.rows = rows.max(1);
        self.cols = cols.max(1);
        self
    }

    /// Space between grid cells in pixels.
    pub fn spacing(&mut self, px: f64) -> &mut Self {
        self.spacing = px.max(0.0);
        self
    }

    /// Same x limits in every plot.
    pub fn share_x(&mut self, on: bool) -> &mut Self {
        self.share_x = on;
        self
    }

    /// Same y limits in every plot.
    pub fn share_y(&mut self, on: bool) -> &mut Self {
        self.share_y = on;
        self
    }

    /// Raster output scale (2.0 for high-DPI / print).
    pub fn scale(&mut self, scale: f64) -> &mut Self {
        self.scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        self
    }

    /// The first plot (created on first use).
    pub fn axes(&mut self) -> &mut Axes {
        self.at(0, 0)
    }

    /// The plot in grid cell (`row`, `col`), created on first use.
    pub fn at(&mut self, row: usize, col: usize) -> &mut Axes {
        self.span(row, col, 1, 1)
    }

    /// A plot spanning several grid cells.
    pub fn span(&mut self, row: usize, col: usize, rowspan: usize, colspan: usize) -> &mut Axes {
        self.rows = self.rows.max(row + rowspan);
        self.cols = self.cols.max(col + colspan);
        let i = match self.cells.iter().position(|c| c.row == row && c.col == col) {
            Some(i) => {
                self.cells[i].rowspan = rowspan.max(1);
                self.cells[i].colspan = colspan.max(1);
                i
            }
            None => {
                self.cells.push(Cell {
                    row,
                    col,
                    rowspan: rowspan.max(1),
                    colspan: colspan.max(1),
                    axes: Axes::new(&self.theme),
                });
                self.cells.len() - 1
            }
        };
        &mut self.cells[i].axes
    }

    /// All plots in creation order.
    pub fn all_axes(&mut self) -> impl Iterator<Item = &mut Axes> {
        self.cells.iter_mut().map(|c| &mut c.axes)
    }

    /// Width and height in pixels.
    pub fn size(&self) -> (f64, f64) {
        (self.width, self.height)
    }

    pub(crate) fn fonts(&self) -> Arc<FontSet> {
        FontSet::resolve(self.theme.font.as_deref())
    }

    fn cell_rects(&self, fonts: &FontSet) -> Vec<Rect> {
        let pad = self.theme.padding;
        let mut top = pad;
        if self.title.is_some() {
            top += fonts.height(self.theme.title_size) + 8.0;
        }
        let area = Rect {
            x: pad,
            y: top,
            w: (self.width - 2.0 * pad).max(1.0),
            h: (self.height - top - pad).max(1.0),
        };
        let cw = (area.w - self.spacing * (self.cols - 1) as f64) / self.cols as f64;
        let ch = (area.h - self.spacing * (self.rows - 1) as f64) / self.rows as f64;
        self.cells
            .iter()
            .map(|c| Rect {
                x: area.x + c.col as f64 * (cw + self.spacing),
                y: area.y + c.row as f64 * (ch + self.spacing),
                w: (cw * c.colspan as f64 + self.spacing * (c.colspan - 1) as f64).max(1.0),
                h: (ch * c.rowspan as f64 + self.spacing * (c.rowspan - 1) as f64).max(1.0),
            })
            .collect()
    }

    /// Applies shared limits by fixing them on a temporary copy.
    fn with_shared_limits<R>(&mut self, fonts: &FontSet, f: impl FnOnce(&mut Self) -> R) -> R {
        if !(self.share_x || self.share_y) || self.cells.len() < 2 {
            return f(self);
        }
        let rects = self.cell_rects(fonts);
        let mut xs = (f64::INFINITY, f64::NEG_INFINITY);
        let mut ys = (f64::INFINITY, f64::NEG_INFINITY);
        for (c, r) in self.cells.iter().zip(&rects) {
            let (lay, _) = c.axes.layout(*r, &self.theme, fonts);
            xs = (xs.0.min(lay.x.min), xs.1.max(lay.x.max));
            ys = (ys.0.min(lay.y.min), ys.1.max(lay.y.max));
        }
        let saved: Vec<_> = self
            .cells
            .iter()
            .map(|c| (c.axes.x.min, c.axes.x.max, c.axes.y.min, c.axes.y.max))
            .collect();
        for c in &mut self.cells {
            if self.share_x {
                c.axes.x.min = Some(xs.0);
                c.axes.x.max = Some(xs.1);
            }
            if self.share_y {
                c.axes.y.min = Some(ys.0);
                c.axes.y.max = Some(ys.1);
            }
        }
        let r = f(self);
        for (c, s) in self.cells.iter_mut().zip(saved) {
            (c.axes.x.min, c.axes.x.max, c.axes.y.min, c.axes.y.max) = s;
        }
        r
    }

    /// Builds the backend-independent scene.
    pub fn to_scene(&mut self) -> Scene {
        self.scene_and_layouts().0
    }

    pub(crate) fn scene_and_layouts(&mut self) -> (Scene, Vec<Layout>) {
        let fonts = self.fonts();
        self.with_shared_limits(&fonts, |fig| {
            let mut scene = Scene::new(fig.width, fig.height);
            scene.background = Some(fig.theme.background);
            scene.font_family = fonts.family.clone();
            if let Some(t) = &fig.title {
                let h = fonts.height(fig.theme.title_size);
                scene.push(super::axes::text(
                    fig.width / 2.0,
                    fig.theme.padding + h / 2.0,
                    t,
                    fig.theme.title_size,
                    fig.theme.text,
                    Anchor::Middle,
                    Baseline::Middle,
                    true,
                ));
            }
            let rects = fig.cell_rects(&fonts);
            let mut layouts = Vec::new();
            for (c, r) in fig.cells.iter().zip(rects) {
                let mut nodes: Vec<Node> = Vec::new();
                layouts.push(c.axes.render(r, &fig.theme, &fonts, &mut nodes));
                scene.nodes.extend(nodes);
            }
            (scene, layouts)
        })
    }

    /// The figure as an SVG document.
    pub fn to_svg(&mut self) -> String {
        self.to_scene().to_svg()
    }

    /// Rasterizes the figure (at [`Figure::scale`]).
    pub fn to_pixels(&mut self) -> Result<Pixels, Error> {
        let scene = self.to_scene();
        output::rasterize(&scene, &self.fonts(), self.scale)
    }

    /// Encodes the figure in a format.
    pub fn encode(&mut self, format: Format) -> Result<Vec<u8>, Error> {
        let scene = self.to_scene();
        output::encode(&scene, &self.fonts(), format, self.scale)
    }

    /// Saves the figure; the format follows the extension (`.svg`, `.png`,
    /// `.jpg`, `.webp`, `.bmp`, `.pdf` with the `pdf` feature).
    pub fn save(&mut self, path: impl AsRef<Path>) -> Result<(), Error> {
        let scene = self.to_scene();
        output::save(&scene, &self.fonts(), path.as_ref(), self.scale)
    }

    /// Layouts of all plots (in creation order) and the index of the plot
    /// containing a canvas position.
    pub(crate) fn hit_axes(&mut self, px: f64, py: f64) -> Option<(usize, Layout)> {
        let (_, layouts) = self.scene_and_layouts();
        layouts
            .into_iter()
            .enumerate()
            .find(|(_, l)| l.plot.contains(px, py))
    }

    pub(crate) fn cell_axes(&mut self, i: usize) -> Option<&mut Axes> {
        self.cells.get_mut(i).map(|c| &mut c.axes)
    }
}
