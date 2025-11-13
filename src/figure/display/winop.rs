use minifb::{Key, MouseMode, Window, WindowOptions};
use resvg::{
    tiny_skia::{self, Pixmap},
    usvg::{self, fontdb},
};
use std::time::{Duration, Instant};

use super::hover::Hover;
use crate::figure::{
    canvas::pixelcanvas::PixelCanvas, configuration::figureconfig::FigureConfig,
    drawers::drawer::Drawer,
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

    /// Displays an SVG image in a window.
    ///
    /// # Parameters
    /// - `svg_content`: The SVG content as a string.
    /// - `window_title`: The title of the window.
    ///
    /// # Panics
    /// - If the SVG content cannot be parsed.
    /// - If the window cannot be created.
    #[allow(clippy::field_reassign_with_default)]
    pub fn display_svg(svg_content: &str, window_title: &str, figure_config: &FigureConfig) {
        // Initialize a font database.
        let mut fontdb = fontdb::Database::new();
        fontdb.load_system_fonts();
        if figure_config.validate().is_ok() {
            fontdb.load_font_data(figure_config.font_label.clone().unwrap().into_bytes());
        }

        // Parse the SVG content.
        let mut opt = usvg::Options::default();
        opt.fontdb = fontdb.into();
        let tree = usvg::Tree::from_str(svg_content, &opt).expect("Failed to parse SVG");

        // Get the dimensions of the SVG from the view box.
        let size = tree.size();
        let width = size.width() as usize;
        let height = size.height() as usize;

        // Render the SVG into a pixmap.
        let mut pixmap = Pixmap::new(width as u32, height as u32).expect("Failed to create pixmap");
        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

        // Convert the pixmap into a pixel buffer.
        let buffer: Vec<u32> = pixmap
            .pixels()
            .iter()
            .map(|pixel| {
                let r = pixel.red();
                let g = pixel.green();
                let b = pixel.blue();
                let a = pixel.alpha();
                ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
            })
            .collect();

        // Create a minifb window.
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

        // Display the image in the window.
        while window.is_open() && !window.is_key_down(Key::Escape) {
            window
                .update_with_buffer(&buffer, width, height)
                .expect("Failed to update buffer");
        }
    }

    /// Displays a plot in real-time with continuous updates.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw on.
    /// - `plot`: The plot to be rendered, implementing `Hover` and `Drawer`.
    /// - `title`: The title of the window.
    /// - `update_data`: A closure to update the plot's data dynamically.
    /// - `fps`: Frames per second for rendering updates.
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
        let width = canvas.width as usize;
        let height = canvas.height as usize;

        let mut window = Window::new(
            title,
            width,
            height,
            WindowOptions {
                resize: true,
                scale: minifb::Scale::X1,
                ..WindowOptions::default()
            },
        )
        .unwrap_or_else(|e| panic!("Unable to open Window: {e}"));

        let frame_duration = Duration::from_secs_f32(1.0 / fps as f32);
        let mut last_frame_time = Instant::now();

        let mut hover_enabled = false;
        let mut show_hints = false;

        while window.is_open() && !window.is_key_pressed(Key::Escape, minifb::KeyRepeat::No) {
            // Update data for real-time rendering.
            if last_frame_time.elapsed() >= frame_duration {
                update_data(plot);
                plot.draw(canvas);
                last_frame_time = Instant::now();
            }

            // Render the canvas to a buffer.
            let mut buffer: Vec<u32> = Self::canvas_to_buffer(canvas);

            if hover_enabled {
                if let Some(mouse_pos) = window.get_mouse_pos(MouseMode::Pass) {
                    let (mouse_x, mouse_y) = (mouse_pos.0 as u32, mouse_pos.1 as u32);

                    if let Some(updated_buffer) = plot.handle_hover(mouse_x, mouse_y, canvas) {
                        buffer = updated_buffer;
                    }
                }
            }

            if show_hints {
                Self::render_hints(canvas);
            }

            if window.is_key_pressed(Key::C, minifb::KeyRepeat::No) {
                hover_enabled = !hover_enabled;
            }

            if window.is_key_pressed(Key::H, minifb::KeyRepeat::No) {
                show_hints = !show_hints;
            }

            window.update_with_buffer(&buffer, width, height).unwrap();
        }
    }

    /// Converts the canvas buffer into a format compatible with minifb.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` whose buffer needs conversion.
    ///
    /// # Returns
    /// A vector of `u32` representing the pixel data in ARGB format.
    fn canvas_to_buffer(canvas: &PixelCanvas) -> Vec<u32> {
        canvas
            .buffer
            .chunks_exact(3)
            .map(|rgb| {
                let r = rgb[0] as u32;
                let g = rgb[1] as u32;
                let b = rgb[2] as u32;
                0xFF000000 | (r << 16) | (g << 8) | b
            })
            .collect()
    }

    /// Displays a plot in an interactive window with hover functionality.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to draw on.
    /// - `plot`: The plot to be rendered, implementing `Hover`.
    /// - `title`: The title of the window.
    ///
    /// # Panics
    /// - If the window cannot be created.
    pub fn display_interactive<T: Hover>(canvas: &mut PixelCanvas, plot: &T, title: &str) {
        let width = canvas.width as usize;
        let height = canvas.height as usize;

        let mut window = Window::new(
            title,
            width,
            height,
            WindowOptions {
                resize: true,
                scale: minifb::Scale::X1,
                ..WindowOptions::default()
            },
        )
        .unwrap_or_else(|e| panic!("Unable to open Window: {e}"));

        let mut hover_enabled = false;
        let mut show_hints = false;

        while window.is_open() && !window.is_key_pressed(Key::Escape, minifb::KeyRepeat::No) {
            // Render the canvas to a buffer.
            let mut buffer: Vec<u32> = Self::canvas_to_buffer(canvas);

            if show_hints {
                Self::render_hints(canvas);
            }

            if hover_enabled {
                if let Some(mouse_pos) = window.get_mouse_pos(MouseMode::Pass) {
                    let (mouse_x, mouse_y) = (mouse_pos.0 as u32, mouse_pos.1 as u32);

                    if let Some(updated_buffer) = plot.handle_hover(mouse_x, mouse_y, canvas) {
                        buffer = updated_buffer;
                    }
                }
            }

            if window.is_key_pressed(Key::H, minifb::KeyRepeat::No) {
                show_hints = !show_hints;
            }

            if window.is_key_pressed(Key::C, minifb::KeyRepeat::No) {
                hover_enabled = !hover_enabled;
            }

            window.update_with_buffer(&buffer, width, height).unwrap();
        }
    }

    /// Renders hints on the canvas for user guidance.
    ///
    /// # Parameters
    /// - `canvas`: The `PixelCanvas` to display hints on.
    ///
    /// # Note
    /// This function is a placeholder and needs to be implemented for specific hint rendering.
    fn render_hints(_canvas: &mut PixelCanvas) {
        // todo!();
    }
}
