//! The figure: canvas size, theme, title and a grid of axes.

use super::axes::{Axes, Layout};
use super::output::{self, Backend, Error, Format, Pixels};
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

/// Limits of one plot: x, y and secondary y (min, max).
pub(crate) type AxesLimitsSnapshot = (
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
);

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
    #[cfg(feature = "window")]
    pub(crate) selection: Option<super::window::Selection>,
    /// Layouts of the last render (for `interact`).
    pub(crate) last_layouts: Vec<super::axes::Layout>,
    /// Limits before the first interactive zoom or pan.
    pub(crate) home: Option<Vec<AxesLimitsSnapshot>>,
    /// Pixel buffer reused between renders (real-time redraws).
    pub(crate) pixmap: Option<resvg::tiny_skia::Pixmap>,
    pub(crate) backend: Backend,
}

impl Drop for Figure {
    fn drop(&mut self) {
        if let Some(pm) = self.pixmap.take() {
            super::render::recycle(pm);
        }
    }
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
            #[cfg(feature = "window")]
            selection: None,
            last_layouts: Vec::new(),
            home: None,
            pixmap: None,
            backend: Backend::Auto,
        }
    }

    /// A small axis-less line chart (sparkline) of `values`, with the last
    /// value and the extremes marked.
    ///
    /// ```
    /// use dataviz::prelude::*;
    /// let mut s = Figure::sparkline([3.0, 5.0, 2.0, 8.0, 6.0], 160, 40);
    /// assert!(s.to_svg().contains("<circle"));
    /// ```
    pub fn sparkline(values: impl super::series::IntoValues, width: u32, height: u32) -> Self {
        let mut fig = Figure::new(width, height);
        let mut theme = Theme {
            spines: super::theme::Spines::NONE,
            grid_x: false,
            grid_y: false,
            padding: 3.0,
            tick_length: 0.0,
            ..Theme::default()
        };
        theme.line_width = 1.5;
        fig.theme(theme);
        let v = values.into_values();
        let x: Vec<f64> = (0..v.len()).map(|i| i as f64).collect();
        let ax = fig.axes();
        ax.x_axis().hidden().margin(0.02);
        ax.y_axis().hidden().margin(0.12);
        ax.legend(super::LegendPosition::Hidden);
        let color = ax.next_color();
        ax.line(&x, &v).color(color);
        let finite: Vec<(usize, f64)> = v
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, y)| y.is_finite())
            .collect();
        if let (Some(min), Some(max), Some(last)) = (
            finite.iter().min_by(|a, b| a.1.total_cmp(&b.1)),
            finite.iter().max_by(|a, b| a.1.total_cmp(&b.1)),
            finite.last(),
        ) {
            ax.scatter([min.0 as f64], [min.1])
                .color(super::Color::RED)
                .size(4.0);
            ax.scatter([max.0 as f64], [max.1])
                .color(super::Color::GREEN)
                .size(4.0);
            ax.scatter([last.0 as f64], [last.1]).color(color).size(5.0);
        }
        fig
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

    /// The plot created `i`-th (grid order of creation).
    pub(crate) fn axes_at(&self, i: usize) -> Option<&Axes> {
        self.cells.get(i).map(|c| &c.axes)
    }

    /// The figure title.
    pub(crate) fn title_text(&self) -> Option<String> {
        self.title.clone()
    }

    /// Lines up the plot areas of a grid: plots starting in the same
    /// column get the same left edge, plots ending in the same column the
    /// same right edge, and likewise for rows.
    fn align_cells(&mut self, rects: &[Rect], fonts: &FontSet) {
        for c in &mut self.cells {
            c.axes.align = None;
        }
        if self.cells.len() < 2 {
            return;
        }
        let insets: Vec<Option<[f64; 4]>> = self
            .cells
            .iter()
            .zip(rects)
            .map(|(c, r)| {
                (c.axes.kind == super::AxesKind::Cartesian).then(|| {
                    let p = c.axes.layout(*r, &self.theme, fonts).0.plot;
                    [
                        p.x - r.x,
                        r.right() - p.right(),
                        p.y - r.y,
                        r.bottom() - p.bottom(),
                    ]
                })
            })
            .collect();
        let keys: Vec<[usize; 4]> = self
            .cells
            .iter()
            .map(|c| [c.col, c.col + c.colspan, c.row, c.row + c.rowspan])
            .collect();
        for i in 0..self.cells.len() {
            if insets[i].is_none() {
                continue;
            }
            let mut a = [0.0f64; 4];
            for (j, inset) in insets.iter().enumerate() {
                if let Some(v) = inset {
                    for k in 0..4 {
                        if keys[j][k] == keys[i][k] {
                            a[k] = a[k].max(v[k]);
                        }
                    }
                }
            }
            self.cells[i].axes.align = Some(a);
        }
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
            fig.align_cells(&rects, &fonts);
            let mut layouts = Vec::new();
            for (i, (c, r)) in fig.cells.iter().zip(rects).enumerate() {
                let mut nodes: Vec<Node> = Vec::new();
                layouts.push(c.axes.render(r, &fig.theme, &fonts, &mut nodes));
                prefix_ids(&mut nodes, i);
                scene.nodes.extend(nodes);
            }
            super::math::expand(&mut scene.nodes, &fonts);
            fig.last_layouts = layouts.clone();
            (scene, layouts)
        })
    }

    /// The figure as an SVG document.
    pub fn to_svg(&mut self) -> String {
        self.to_scene().to_svg()
    }

    /// Rasterizes the figure (at [`Figure::scale`]).
    pub fn to_pixels(&mut self) -> Result<Pixels, Error> {
        let mut px = Pixels {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
        self.render_into(&mut px)?;
        Ok(px)
    }

    /// Rasterizes the figure into `out`, reusing its allocation and the
    /// figure's internal pixel buffer. Use this for real-time redraws.
    pub fn render_into(&mut self, out: &mut Pixels) -> Result<(), Error> {
        self.render_out(super::render::Out::Pixels(out))
    }

    /// Rasterizes the figure into `0x00RRGGBB` pixels composited on white
    /// (the layout of window frame buffers), reusing `out`'s allocation.
    pub fn render_0rgb(&mut self, out: &mut Vec<u32>) -> Result<(), Error> {
        self.render_out(super::render::Out::Orgb(out))
    }

    pub(crate) fn render_out(&mut self, out: super::render::Out<'_>) -> Result<(), Error> {
        let scene = self.to_scene();
        let fonts = self.fonts();
        super::render::render_to(
            &scene,
            &fonts,
            self.scale,
            self.backend,
            &mut self.pixmap,
            out,
        )
    }

    /// Where raster output is computed (default [`Backend::Auto`]).
    pub fn backend(&mut self, backend: Backend) -> &mut Self {
        self.backend = backend;
        self
    }

    /// Encodes the figure in a format.
    pub fn encode(&mut self, format: Format) -> Result<Vec<u8>, Error> {
        match format {
            Format::Html => Ok(self.to_html().into_bytes()),
            Format::Svg | Format::Pdf => {
                let scene = self.to_scene();
                output::encode(&scene, &self.fonts(), format, self.scale)
            }
            Format::Png => {
                let scene = self.to_scene();
                let fonts = self.fonts();
                super::render::render(&scene, &fonts, self.scale, self.backend, &mut self.pixmap)?;
                let pm = self
                    .pixmap
                    .as_ref()
                    .ok_or_else(|| Error::Render("empty image".into()))?;
                output::encode_png_premul(pm.data(), pm.width(), pm.height())
            }
            raster => self.to_pixels()?.encode(raster),
        }
    }

    /// Saves the figure; the format follows the extension (`.svg`, `.png`,
    /// `.jpg`, `.webp`, `.bmp`, `.html` (interactive), `.pdf` with the `pdf`
    /// feature).
    pub fn save(&mut self, path: impl AsRef<Path>) -> Result<(), Error> {
        let bytes = self.encode(Format::from_path(path.as_ref())?)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    #[cfg(feature = "window")]
    pub(crate) fn cell_axes(&mut self, i: usize) -> Option<&mut Axes> {
        self.cells.get_mut(i).map(|c| &mut c.axes)
    }
}

/// Series group ids `s<k>` of plot `i` become `p<i>-s<k>`.
fn prefix_ids(nodes: &mut [Node], i: usize) {
    for n in nodes {
        if let Node::Group { children, id, .. } = n {
            if let Some(s) = id.as_mut().filter(|s| s.starts_with('s')) {
                *s = format!("p{i}-{s}");
            }
            prefix_ids(children, i);
        }
    }
}
