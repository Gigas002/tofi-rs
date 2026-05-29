//! Entry layout engine.
//!
//! # Safety
//!
//! [`Entry::new`] is `unsafe` because it wraps raw SHM buffer memory.
//! See the function's doc-comment for invariants.

use std::f64::consts::SQRT_2;

use tiny_skia::Pixmap;

use crate::color::Color;
use crate::entry::pixel_format::{fill_rgba_premul, rgba_premul_to_argb8888};
use crate::scale::scale_apply_inverse;
use crate::{Error, Result};

pub mod canvas;
pub mod pixel_format;
#[cfg(test)]
mod tests;
pub mod text_backend;

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum number of Unicode codepoints in the input buffer.
pub const MAX_INPUT_LENGTH: usize = 256;

// ── Support types ─────────────────────────────────────────────────────────────

/// Cursor drawing style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyle {
    /// Thin vertical bar (default).
    #[default]
    Bar,
    /// Full character-width filled block.
    Block,
    /// Horizontal underline.
    Underscore,
}

/// Per-side insets in logical pixels.
///
/// Negative values in [`TextTheme::padding`] are treated as "fill to edge"
/// by the text backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct Directional {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

/// Text element theme — `None` fields inherit from the global defaults.
///
/// Call `TextTheme::resolve` with the fallback to obtain a
/// `ResolvedTextTheme` before drawing.
#[derive(Debug, Clone, Default)]
pub struct TextTheme {
    /// `None` → use entry foreground color.
    pub foreground_color: Option<Color>,
    /// `None` → transparent (α = 0), matching the C default.
    pub background_color: Option<Color>,
    /// `None` → all-zero padding.
    pub padding: Option<Directional>,
    /// `None` → 0 radius (square corners).
    pub background_corner_radius: Option<u32>,
}

/// Text theme with every field populated (after fallback resolution).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedTextTheme {
    pub foreground_color: Color,
    pub background_color: Color,
    pub padding: Directional,
    pub background_corner_radius: u32,
}

impl TextTheme {
    /// Resolve `self` against `fallback`, filling in any `None` fields.
    pub(crate) fn resolve(&self, fallback: &ResolvedTextTheme) -> ResolvedTextTheme {
        ResolvedTextTheme {
            foreground_color: self.foreground_color.unwrap_or(fallback.foreground_color),
            background_color: self.background_color.unwrap_or(fallback.background_color),
            padding: self.padding.unwrap_or(fallback.padding),
            background_corner_radius: self
                .background_corner_radius
                .unwrap_or(fallback.background_corner_radius),
        }
    }
}

/// Cursor theme before backend metrics are applied.
///
/// `None` fields are resolved during [`Entry::new`].
#[derive(Debug, Clone)]
pub struct CursorTheme {
    /// Cursor color; `None` → use input foreground color.
    pub color: Option<Color>,
    /// Text color under a block cursor; `None` → use entry background color.
    pub text_color: Option<Color>,
    pub style: CursorStyle,
    pub corner_radius: u32,
    /// Bar / underscore thickness in pixels; `None` → font-computed default.
    pub thickness: Option<u32>,
    /// Show the cursor.
    pub show: bool,
}

impl Default for CursorTheme {
    fn default() -> Self {
        Self {
            color: None,
            text_color: None,
            style: CursorStyle::Bar,
            corner_radius: 0,
            thickness: None,
            show: true,
        }
    }
}

/// Cursor theme with all fields resolved by the text backend.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedCursorTheme {
    pub color: Color,
    pub text_color: Color,
    pub style: CursorStyle,
    pub corner_radius: u32,
    pub thickness: u32,
    /// Distance from text baseline to the underline top.
    pub underline_depth: f64,
    /// Width of the 'm' glyph, used for block/underscore cursor width.
    pub em_width: f64,
    pub show: bool,
}

// ── EntryConfig ───────────────────────────────────────────────────────────────

/// All configuration for an [`Entry`], passed to [`Entry::new`].
#[derive(Debug, Clone)]
pub struct EntryConfig {
    // Font
    pub font_name: String,
    pub font_size: u32,
    pub font_features: String,
    pub font_variations: String,

    // Colors
    pub foreground_color: Color,
    pub background_color: Color,
    pub selection_highlight_color: Color,
    pub border_color: Color,
    pub outline_color: Color,

    // Border / geometry
    pub corner_radius: u32,
    pub border_width: u32,
    pub outline_width: u32,
    pub padding_top: u32,
    pub padding_bottom: u32,
    pub padding_left: u32,
    pub padding_right: u32,
    pub clip_to_padding: bool,

    // Text layout
    pub prompt_text: String,
    pub prompt_padding: u32,
    pub placeholder_text: String,
    pub num_results: u32,
    pub result_spacing: i32,
    pub horizontal: bool,
    /// Minimum input field width in horizontal mode.
    pub input_width: u32,

    // Per-element themes
    pub cursor_theme: CursorTheme,
    pub prompt_theme: TextTheme,
    pub input_theme: TextTheme,
    pub placeholder_theme: TextTheme,
    pub default_result_theme: TextTheme,
    pub alternate_result_theme: TextTheme,
    pub selection_theme: TextTheme,

    // Input behaviour
    pub hide_input: bool,
    /// UTF-8 string displayed per character when `hide_input` is `true`.
    /// Empty string = fully hidden.
    pub hidden_character: String,
}

impl Default for EntryConfig {
    fn default() -> Self {
        Self {
            font_name: "Sans".into(),
            font_size: 24,
            font_features: String::new(),
            font_variations: String::new(),
            foreground_color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            background_color: Color {
                r: 0.106,
                g: 0.114,
                b: 0.118,
                a: 1.0,
            },
            selection_highlight_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            border_color: Color {
                r: 0.973,
                g: 0.149,
                b: 0.447,
                a: 1.0,
            },
            outline_color: Color {
                r: 0.031,
                g: 0.031,
                b: 0.0,
                a: 1.0,
            },
            corner_radius: 0,
            border_width: 12,
            outline_width: 4,
            padding_top: 8,
            padding_bottom: 8,
            padding_left: 8,
            padding_right: 8,
            clip_to_padding: true,
            prompt_text: "run: ".into(),
            prompt_padding: 0,
            placeholder_text: String::new(),
            num_results: 0,
            result_spacing: 0,
            horizontal: false,
            input_width: 0,
            cursor_theme: CursorTheme::default(),
            prompt_theme: TextTheme::default(),
            input_theme: TextTheme::default(),
            placeholder_theme: TextTheme::default(),
            default_result_theme: TextTheme::default(),
            alternate_result_theme: TextTheme::default(),
            selection_theme: TextTheme::default(),
            hide_input: false,
            hidden_character: "*".into(),
        }
    }
}

// ── Entry ─────────────────────────────────────────────────────────────────────

/// Double-buffered entry widget.
///
/// Holds two tiny-skia pixmaps backed by consecutive halves of an SHM buffer,
/// a cosmic-text backend, resolved themes, and all runtime state.
pub struct Entry {
    // Double-buffered RGBA pixmaps (index 0 and 1).
    pixmaps: [Pixmap; 2],
    /// Which buffer is next to be drawn into.
    pub index: usize,
    /// Fractional scale numerator (denominator 120).
    scale_numerator: u32,
    /// Raw SHM pointer for flush conversion.
    shm_data: *mut u8,
    frame_bytes: usize,

    // Clip rectangle in physical / device pixels (set by entry_init).
    pub clip_x: u32,
    pub clip_y: u32,
    pub clip_width: u32,
    pub clip_height: u32,

    // Runtime state (mutated by the caller between frames).
    /// UTF-8 input string.
    pub input: String,
    /// Cursor position in Unicode codepoints.
    pub cursor_position: usize,
    /// Index of the currently selected result (0-based).
    pub selection: usize,
    /// Index of the first visible result (for scrolling).
    pub first_result: usize,
    /// Result strings to display.
    pub results: Vec<String>,
    /// Number of results actually drawn in the last update (set by backend).
    pub num_results_drawn: usize,
    /// Number of results drawn in the *previous* update — used for page scrolling.
    pub last_num_results_drawn: usize,

    // Config (immutable after init).
    pub config: EntryConfig,

    // Resolved themes (set during init after fallback application).
    pub(crate) resolved_prompt_theme: ResolvedTextTheme,
    pub(crate) resolved_input_theme: ResolvedTextTheme,
    pub(crate) resolved_placeholder_theme: ResolvedTextTheme,
    pub(crate) resolved_default_result_theme: ResolvedTextTheme,
    pub(crate) resolved_alternate_result_theme: ResolvedTextTheme,
    pub(crate) resolved_selection_theme: ResolvedTextTheme,
    pub(crate) resolved_cursor: ResolvedCursorTheme,

    // Text backend (lives for the lifetime of the Entry).
    pub(crate) text: text_backend::TextBackend,
}

impl Entry {
    /// Create an [`Entry`] backed by a raw SHM buffer.
    ///
    /// # Arguments
    ///
    /// * `data` — pointer to the start of the SHM mapping; must be at least
    ///   `width × height × 4 × 2` bytes (two ARGB32 frames).
    /// * `width` / `height` — physical dimensions in pixels.
    /// * `scale_numerator` — fractional scale (denominator 120).
    /// * `config` — rendering configuration.
    ///
    /// # Safety
    ///
    /// `data` must remain valid, writable, and not aliased for the entire
    /// lifetime of the returned `Entry`.
    pub unsafe fn new(
        data: *mut u8,
        width: u32,
        height: u32,
        scale_numerator: u32,
        config: EntryConfig,
    ) -> Result<Self> {
        let scale = scale_numerator as f64 / 120.0;
        let frame_bytes = (width * height * 4) as usize;

        tracing::debug!("Entry::new {width}×{height} physical, scale={scale:.4}");

        let mut pixmap0 = Pixmap::new(width, height)
            .ok_or_else(|| Error::Renderer("entry pixmap[0]: allocation failed".into()))?;
        let mut pixmap1 = Pixmap::new(width, height)
            .ok_or_else(|| Error::Renderer("entry pixmap[1]: allocation failed".into()))?;

        let logical_width = scale_apply_inverse(width, scale_numerator);
        let logical_height = scale_apply_inverse(height, scale_numerator);

        draw_background_and_border(&mut pixmap0, scale, &config, logical_width, logical_height)?;
        draw_background_and_border(&mut pixmap1, scale, &config, logical_width, logical_height)?;

        let (clip_x, clip_y, clip_w, clip_h, _tx, _ty) =
            compute_clip(&config, logical_width, logical_height);

        let transparent = Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        let default_fg = config.foreground_color;
        let default_bg = config.background_color;

        let default_fallback = ResolvedTextTheme {
            foreground_color: default_fg,
            background_color: transparent,
            padding: Directional::default(),
            background_corner_radius: 0,
        };

        let resolved_prompt_theme = config.prompt_theme.resolve(&default_fallback);
        let resolved_input_theme = config.input_theme.resolve(&default_fallback);
        let resolved_placeholder_theme = config.placeholder_theme.resolve(&default_fallback);
        let resolved_default_result_theme = config.default_result_theme.resolve(&default_fallback);
        let alt_fallback = resolved_default_result_theme;
        let resolved_alternate_result_theme = config.alternate_result_theme.resolve(&alt_fallback);
        let resolved_selection_theme = config.selection_theme.resolve(&default_fallback);

        let (text, resolved_cursor) = text_backend::TextBackend::init(
            &config,
            &resolved_input_theme,
            default_fg,
            default_bg,
        )?;

        let mut entry = Self {
            pixmaps: [pixmap0, pixmap1],
            index: 0,
            scale_numerator,
            shm_data: data,
            frame_bytes,
            clip_x,
            clip_y,
            clip_width: clip_w,
            clip_height: clip_h,
            input: String::new(),
            cursor_position: 0,
            selection: 0,
            first_result: 0,
            results: Vec::new(),
            num_results_drawn: 0,
            last_num_results_drawn: 0,
            config,
            resolved_prompt_theme,
            resolved_input_theme,
            resolved_placeholder_theme,
            resolved_default_result_theme,
            resolved_alternate_result_theme,
            resolved_selection_theme,
            resolved_cursor,
            text,
        };

        entry.text_update();
        entry.index = 1;

        Ok(entry)
    }

    /// Redraw the entry into the current back buffer and flip the index.
    ///
    /// Must be called whenever the input, selection, or results change.
    pub fn update(&mut self) {
        tracing::debug!("Entry::update");
        let idx = self.index;
        let c = self.config.background_color;
        fill_rgba_premul(
            self.pixmaps[idx].data_mut(),
            (c.r * 255.0).round() as u8,
            (c.g * 255.0).round() as u8,
            (c.b * 255.0).round() as u8,
            (c.a * 255.0).round() as u8,
        );

        self.text_update();
        self.index ^= 1;
    }

    /// Convert premultiplied RGBA pixmaps into the ARGB8888 SHM buffer.
    pub fn flush(&self) {
        // SAFETY: caller guarantees shm_data validity for 2 × frame_bytes.
        unsafe {
            let base = self.shm_data;
            for i in 0..2 {
                rgba_premul_to_argb8888(
                    self.pixmaps[i].data(),
                    std::slice::from_raw_parts_mut(
                        base.add(i * self.frame_bytes),
                        self.frame_bytes,
                    ),
                );
            }
        }
    }

    /// Returns the index of the frame that is ready to be committed to the
    /// compositor (the one most recently drawn into, before the flip).
    pub fn ready_index(&self) -> usize {
        self.index ^ 1
    }

    /// Reset selection to the first result and scroll back to the top.
    pub fn reset_selection(&mut self) {
        self.selection = 0;
        self.first_result = 0;
    }

    pub fn select_next(&mut self) {
        let nsel = self.num_results_drawn.min(self.results.len()).max(1);

        self.selection += 1;
        if self.selection >= nsel {
            self.selection -= nsel;
            if !self.results.is_empty() {
                self.first_result = (self.first_result + nsel) % self.results.len();
            } else {
                self.first_result = 0;
            }
            self.last_num_results_drawn = self.num_results_drawn;
        }
    }

    pub fn select_prev(&mut self) {
        if self.selection > 0 {
            self.selection -= 1;
            return;
        }

        let nsel = self.num_results_drawn.min(self.results.len()).max(1);

        if self.first_result > nsel {
            self.first_result -= self.last_num_results_drawn;
            self.selection = self.last_num_results_drawn.saturating_sub(1);
        } else if self.first_result > 0 {
            self.selection = self.first_result - 1;
            self.first_result = 0;
        }
    }

    pub fn select_next_page(&mut self) {
        self.first_result += self.num_results_drawn;
        if self.first_result >= self.results.len() {
            self.first_result = 0;
        }
        self.selection = 0;
        self.last_num_results_drawn = self.num_results_drawn;
    }

    pub fn select_prev_page(&mut self) {
        if self.first_result >= self.last_num_results_drawn {
            self.first_result -= self.last_num_results_drawn;
        } else {
            self.first_result = 0;
        }
        self.selection = 0;
        self.last_num_results_drawn = self.num_results_drawn;
    }

    fn text_update(&mut self) {
        let scale = self.scale_numerator as f64 / 120.0;
        let idx = self.index;
        let ctx = self.text_update_context();
        let (tx, ty) = clip_origin(
            &self.config,
            scale_apply_inverse(self.pixmaps[idx].width(), self.scale_numerator),
            scale_apply_inverse(self.pixmaps[idx].height(), self.scale_numerator),
        );
        let clip_w = self.clip_width as f64;
        let clip_h = self.clip_height as f64;

        let mut pixmap_mut = self.pixmaps[idx].as_mut();
        let mut canvas = canvas::Canvas::new(&mut pixmap_mut, scale);
        canvas.translate(tx, ty);
        canvas.set_clip_logical(0.0, 0.0, clip_w, clip_h);
        self.num_results_drawn = text_backend::update(&mut canvas, &mut self.text, &ctx);
    }

    fn text_update_context(&self) -> text_backend::TextUpdateContext {
        text_backend::TextUpdateContext {
            config: self.config.clone(),
            input: self.input.clone(),
            cursor_position: self.cursor_position,
            first_result: self.first_result,
            selection: self.selection,
            results: self.results.clone(),
            clip_x: self.clip_x,
            clip_y: self.clip_y,
            clip_width: self.clip_width,
            clip_height: self.clip_height,
            resolved_prompt_theme: self.resolved_prompt_theme,
            resolved_input_theme: self.resolved_input_theme,
            resolved_placeholder_theme: self.resolved_placeholder_theme,
            resolved_default_result_theme: self.resolved_default_result_theme,
            resolved_alternate_result_theme: self.resolved_alternate_result_theme,
            resolved_selection_theme: self.resolved_selection_theme,
            resolved_cursor: self.resolved_cursor,
        }
    }
}

// ── Drawing helpers ───────────────────────────────────────────────────────────

fn draw_background_and_border(
    pixmap: &mut Pixmap,
    scale: f64,
    cfg: &EntryConfig,
    width: u32,
    height: u32,
) -> Result<()> {
    let mut pixmap_mut = pixmap.as_mut();
    let mut canvas = canvas::Canvas::new(&mut pixmap_mut, scale);
    let (w, h) = (width as f64, height as f64);
    let r = cfg.corner_radius as f64;
    let bw = cfg.border_width as f64;
    let ow = cfg.outline_width as f64;

    canvas.paint_solid(cfg.background_color);

    canvas.stroke_rounded_rect_preserve(w, h, r, 4.0 * ow + 2.0 * bw, cfg.outline_color);
    canvas.stroke_rounded_rect_preserve(w, h, r, 2.0 * ow + 2.0 * bw, cfg.border_color);
    canvas.stroke_rounded_rect_preserve(w, h, r, 2.0 * ow, cfg.outline_color);
    canvas.fill_even_odd_clear(w, h, r);

    Ok(())
}

fn compute_clip(cfg: &EntryConfig, width: u32, height: u32) -> (u32, u32, u32, u32, f64, f64) {
    let (tx, ty, clip_w, clip_h) = clip_geometry(cfg, width as f64, height as f64);
    let clip_x = tx.round() as u32;
    let clip_y = ty.round() as u32;
    (clip_x, clip_y, clip_w as u32, clip_h as u32, tx, ty)
}

fn clip_origin(cfg: &EntryConfig, width: u32, height: u32) -> (f64, f64) {
    let (tx, ty, _, _) = clip_geometry(cfg, width as f64, height as f64);
    (tx, ty)
}

fn clip_geometry(cfg: &EntryConfig, mut w: f64, mut h: f64) -> (f64, f64, f64, f64) {
    let bw = cfg.border_width as f64;
    let ow = cfg.outline_width as f64;

    let mut tx = 2.0 * ow + bw;
    let mut ty = 2.0 * ow + bw;
    w -= 2.0 * tx;
    h -= 2.0 * ty;

    if cfg.clip_to_padding {
        tx += cfg.padding_left as f64;
        ty += cfg.padding_top as f64;
        w -= (cfg.padding_left + cfg.padding_right) as f64;
        h -= (cfg.padding_top + cfg.padding_bottom) as f64;
    }

    let inner_r = (cfg.corner_radius as f64 - (2.0 * ow + bw)).max(0.0);
    let corner_dx = (inner_r * (1.0 - 1.0 / SQRT_2)).ceil();
    tx += corner_dx;
    ty += corner_dx;
    w -= 2.0 * corner_dx;
    h -= 2.0 * corner_dx;

    if !cfg.clip_to_padding {
        tx += cfg.padding_left as f64;
        ty += cfg.padding_top as f64;
    }

    (tx, ty, w, h)
}
