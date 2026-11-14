use minifb::{Key, MouseMode, Window, WindowOptions};
use resvg::{
    tiny_skia::{self, Pixmap},
    usvg,
};
use std::time::{Duration, Instant};

use super::hover::Hover;
use crate::figure::{
    canvas::pixelcanvas::PixelCanvas,
    configuration::figureconfig::FigureConfig,
    drawers::drawer::Drawer,
    render::{font, pixel},
};

/// A utility struct for managing window operations and displaying graphics interactively.
pub struct Winop;

impl Default for Winop {
    fn default() -> Self {
        Self::new()
    }
}

impl Winop {
    /// Creates a new `Winop` instance.
    ///
    /// # Returns
    /// A new `Winop` instance.
    pub fn new() -> Self {
        Self
    }

    /// Rasterizes SVG text into `0xFFRRGGBB` pixels (composited on white).
    ///
    /// Text with the generic `sans-serif` family is rendered with
    /// `FigureConfig::font_label` when set, otherwise with an installed
    /// sans-serif font.
    ///
    /// # Errors
    /// Returns a message if the SVG cannot be parsed or has an empty size.
    pub(crate) fn rasterize_svg(
        svg_content: &str,
        figure_config: &FigureConfig,
    ) -> Result<(usize, usize, Vec<u32>), String> {
        let fontdb = font::svg_font_database(figure_config.font_label.as_deref());
        let opt = usvg::Options {
            fontdb: std::sync::Arc::new(fontdb),
            ..usvg::Options::default()
        };
        let tree = usvg::Tree::from_str(svg_content, &opt).map_err(|e| e.to_string())?;

        let size = tree.size().to_int_size();
        let (width, height) = (size.width(), size.height());
        let mut pixmap =
            Pixmap::new(width, height).ok_or_else(|| "SVG has an empty size".to_string())?;
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

        // Pixels are premultiplied: compositing on white is `c + (255 - a)`.
        let buffer = pixmap
            .pixels()
            .iter()
            .map(|p| {
                let bg = 255 - p.alpha() as u32;
                let r = p.red() as u32 + bg;
                let g = p.green() as u32 + bg;
                let b = p.blue() as u32 + bg;
                0xFF00_0000 | (r.min(255) << 16) | (g.min(255) << 8) | b.min(255)
            })
            .collect();
        Ok((width as usize, height as usize, buffer))
    }

    /// Displays an SVG image in a window until it is closed or Escape is
    /// pressed.
    ///
    /// # Parameters
    /// - `svg_content`: The SVG content as a string.
    /// - `window_title`: The title of the window.
    /// - `figure_config`: Its `font_label` (if set) is used for SVG text.
    ///
    /// # Panics
    /// - If the SVG content cannot be parsed.
    /// - If the window cannot be created.
    pub fn display_svg(svg_content: &str, window_title: &str, figure_config: &FigureConfig) {
        let (width, height, buffer) = Self::rasterize_svg(svg_content, figure_config)
            .unwrap_or_else(|e| panic!("Failed to render SVG: {e}"));

        let mut window = Window::new(
            window_title,
            width,
            height,
            WindowOptions {
                resize: false,
                scale: minifb::Scale::X1,
                ..WindowOptions::default()
            },
        )
        .expect("Unable to create window");
        window.set_target_fps(30);

        while window.is_open() && !window.is_key_down(Key::Escape) {
            window
                .update_with_buffer(&buffer, width, height)
                .expect("Failed to update buffer");
        }
    }

    /// Displays a plot in real-time with continuous updates.
    ///
    /// Keys: `C` toggles the hover tooltip, `H` toggles the key hints,
    /// `Escape` closes the window.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw on.
    /// - `plot`: The plot to be rendered, implementing `Hover` and `Drawer`.
    /// - `title`: The title of the window.
    /// - `update_data`: A closure to update the plot's data dynamically.
    /// - `fps`: Data updates per second (`0` is treated as `1`).
    ///
    /// # Panics
    /// - If the window cannot be created.
    pub fn display_real_time<T: Hover + Drawer>(
        canvas: &mut PixelCanvas,
        plot: &mut T,
        title: &str,
        mut update_data: impl FnMut(&mut T) + 'static,
        fps: u32,
    ) {
        let mut window = Self::open_window(canvas, title);
        let frame_duration = Duration::from_secs_f64(1.0 / fps.max(1) as f64);
        let mut last_frame_time: Option<Instant> = None;
        let mut state = InteractionState::default();

        while window.is_open() && !window.is_key_pressed(Key::Escape, minifb::KeyRepeat::No) {
            if last_frame_time.is_none_or(|t| t.elapsed() >= frame_duration) {
                update_data(plot);
                plot.draw(canvas);
                last_frame_time = Some(Instant::now());
            }
            let buffer = state.frame(&window, canvas, plot);
            state.handle_keys(&window);
            window
                .update_with_buffer(&buffer, canvas.width as usize, canvas.height as usize)
                .expect("Failed to update buffer");
        }
    }

    /// Converts the canvas buffer into a format compatible with minifb.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` whose buffer needs conversion.
    ///
    /// # Returns
    /// A vector of `u32` representing the pixel data in ARGB format.
    #[allow(dead_code)]
    fn canvas_to_buffer(canvas: &PixelCanvas) -> Vec<u32> {
        pixel::to_argb(canvas)
    }

    /// Displays a plot in an interactive window with hover functionality.
    ///
    /// Keys: `C` toggles the hover tooltip, `H` toggles the key hints,
    /// `Escape` closes the window.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw on (already drawn).
    /// - `plot`: The plot to be rendered, implementing `Hover`.
    /// - `title`: The title of the window.
    ///
    /// # Panics
    /// - If the window cannot be created.
    pub fn display_interactive<T: Hover>(canvas: &mut PixelCanvas, plot: &T, title: &str) {
        let mut window = Self::open_window(canvas, title);
        let mut state = InteractionState::default();

        while window.is_open() && !window.is_key_pressed(Key::Escape, minifb::KeyRepeat::No) {
            let buffer = state.frame(&window, canvas, plot);
            state.handle_keys(&window);
            window
                .update_with_buffer(&buffer, canvas.width as usize, canvas.height as usize)
                .expect("Failed to update buffer");
        }
    }

    fn open_window(canvas: &PixelCanvas, title: &str) -> Window {
        let mut window = Window::new(
            title,
            canvas.width as usize,
            canvas.height as usize,
            WindowOptions {
                resize: true,
                scale: minifb::Scale::X1,
                ..WindowOptions::default()
            },
        )
        .unwrap_or_else(|e| panic!("Unable to open Window: {e}"));
        // Without a frame limit the event loop spins at 100 % CPU.
        window.set_target_fps(60);
        window
    }

    /// Renders hints on the canvas for user guidance.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to display hints on.
    #[allow(dead_code)]
    fn render_hints(canvas: &mut PixelCanvas) {
        let buffer = hints_overlay(canvas);
        for (dst, px) in canvas.buffer.chunks_exact_mut(3).zip(buffer) {
            dst.copy_from_slice(&[(px >> 16) as u8, (px >> 8) as u8, px as u8]);
        }
    }
}

/// Toggle state shared by the interactive display loops.
#[derive(Default)]
struct InteractionState {
    hover_enabled: bool,
    show_hints: bool,
}

impl InteractionState {
    /// The buffer to show for this frame: the canvas, with the hover tooltip
    /// or the key hints on top when enabled.
    fn frame<T: Hover + ?Sized>(
        &self,
        window: &Window,
        canvas: &PixelCanvas,
        plot: &T,
    ) -> Vec<u32> {
        if self.hover_enabled {
            if let Some((x, y)) = window.get_mouse_pos(MouseMode::Discard) {
                if let Some(buffer) =
                    plot.handle_hover(x.max(0.0) as u32, y.max(0.0) as u32, canvas)
                {
                    return buffer;
                }
            }
        }
        if self.show_hints {
            return hints_overlay(canvas);
        }
        pixel::to_argb(canvas)
    }

    fn handle_keys(&mut self, window: &Window) {
        if window.is_key_pressed(Key::C, minifb::KeyRepeat::No) {
            self.hover_enabled = !self.hover_enabled;
        }
        if window.is_key_pressed(Key::H, minifb::KeyRepeat::No) {
            self.show_hints = !self.show_hints;
        }
    }
}

/// The canvas with a box listing the keyboard shortcuts.
fn hints_overlay(canvas: &PixelCanvas) -> Vec<u32> {
    pixel::tooltip(
        canvas,
        &FigureConfig::default(),
        None,
        (0, canvas.height),
        "C: hover info   H: hints   Esc: close",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{canvas::svgcanvas::SvgCanvas, figuretypes::piechart::PieChart};

    #[test]
    fn rasterizes_chart_svg_with_text() {
        let cfg = FigureConfig::default();
        let mut pie = PieChart::new("Market Share & More", cfg.clone());
        pie.add_slice("A", 1.0, [255, 0, 0]);
        let mut svg = SvgCanvas::new(200, 150, "white", 20);
        pie.draw_svg(&mut svg);

        let (w, h, buffer) = Winop::rasterize_svg(&svg.get_svg_as_text(), &cfg).unwrap();
        assert_eq!((w, h, buffer.len()), (200, 150, 200 * 150));
        assert!(buffer.iter().all(|p| p >> 24 == 0xFF));
        assert!(buffer.contains(&0xFFFF_0000), "pie slice missing");

        // The title is drawn when any system font is installed.
        let title_ink = buffer[..200 * 15].iter().any(|&p| p != 0xFFFF_FFFF);
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        assert_eq!(title_ink, !db.is_empty());
    }

    #[test]
    fn rasterize_reports_invalid_svg() {
        assert!(Winop::rasterize_svg("<svg", &FigureConfig::default()).is_err());
    }
}
