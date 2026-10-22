//! Entry layout engine.
//!
//! Frames are rasterized with [`vello_cpu`] and text is laid out with
//! [`parley`]; see [`text`] for the text drawing backend.
//!
//! # Safety
//!
//! [`Entry::new`] is `unsafe` because it wraps raw SHM buffer memory.
//! See the function's doc-comment for invariants.

use vello_cpu::kurbo::{Affine, Rect, RoundedRect, Shape, Stroke};
use vello_cpu::{RenderContext, Resources};

use crate::Result;
use crate::color::Color;
use crate::renderer::{canvas_size, paint, rasterize};
use crate::scale::scale_apply_inverse;

#[cfg(test)]
mod tests;
pub mod text;

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
    /// Distance from the top of the line to the underline top (font-computed).
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
/// Renders into two frames occupying consecutive halves of an SHM buffer,
/// and holds the text backend, resolved themes, and all runtime state.
pub struct Entry {
    /// Start of the two-frame SHM mapping.
    data: *mut u8,
    /// Physical frame dimensions.
    width: u16,
    height: u16,
    scale: f64,
    /// Logical frame dimensions (physical ÷ scale).
    logical_width: u32,
    logical_height: u32,

    // Scene builder and rasterizer resources, reused across frames.
    ctx: RenderContext,
    resources: Resources,
    /// Which buffer is next to be drawn into.
    pub index: usize,

    /// Clip rectangle in logical pixels, before truncation.
    clip_rect: Rect,
    /// Where text drawing starts, in logical pixels.
    text_origin: (f64, f64),

    // Clip rectangle in logical pixels (set by `Entry::new`).
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
    pub(crate) text: text::TextBackend,
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
        let (w16, h16) = canvas_size(width, height)?;

        tracing::debug!("Entry::new {width}×{height} physical, scale={scale:.4}");

        // Logical dimensions (physical ÷ scale).
        let logical_width = scale_apply_inverse(width, scale_numerator);
        let logical_height = scale_apply_inverse(height, scale_numerator);

        // Clip region (border inset + optional padding + corner inset) and
        // the point where text drawing starts.
        let clip = compute_clip(&config, logical_width, logical_height);

        // ── Resolve text themes against global defaults ──────────────────────
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
        // alternate_result inherits from default_result.
        let alt_fallback = resolved_default_result_theme;
        let resolved_alternate_result_theme = config.alternate_result_theme.resolve(&alt_fallback);
        let resolved_selection_theme = config.selection_theme.resolve(&default_fallback);

        // ── Initialise text backend ──────────────────────────────────────────
        let (text, resolved_cursor) =
            text::TextBackend::init(&config, &resolved_input_theme, default_bg)?;

        // ── Build Entry and render the first frame ───────────────────────────
        let mut entry = Self {
            data,
            width: w16,
            height: h16,
            scale,
            logical_width,
            logical_height,
            ctx: RenderContext::new(w16, h16),
            resources: Resources::new(),
            index: 0,
            clip_rect: clip.rect,
            text_origin: clip.text_origin,
            clip_x: clip.x,
            clip_y: clip.y,
            clip_width: clip.width,
            clip_height: clip.height,
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

        entry.update();

        Ok(entry)
    }

    /// Redraw the entry into the current back buffer and flip the index.
    ///
    /// Must be called whenever the input, selection, or results change.
    pub fn update(&mut self) {
        tracing::debug!("Entry::update");

        // The whole scene is rebuilt each frame; vello_cpu only rasterizes
        // what is drawn, so this is cheap compared to the text layout.
        self.ctx.reset();
        self.ctx.set_transform(Affine::scale(self.scale));
        draw_background_and_border(
            &mut self.ctx,
            &self.config,
            self.logical_width,
            self.logical_height,
        );

        self.ctx.push_clip_rect(&self.clip_rect);
        text::update(self);
        self.ctx.pop_clip();

        let frame_bytes = self.width as usize * self.height as usize * 4;
        // SAFETY: `Entry::new`'s contract guarantees `data` is valid for two
        // consecutive frames of `frame_bytes` and is not aliased while `self`
        // is alive; `index` is always 0 or 1.
        let frame = unsafe {
            std::slice::from_raw_parts_mut(self.data.add(self.index * frame_bytes), frame_bytes)
        };
        rasterize(
            &mut self.ctx,
            &mut self.resources,
            frame,
            self.width,
            self.height,
        );

        self.index ^= 1;
    }

    /// No-op kept for API compatibility.
    ///
    /// [`Entry::update`] now writes finished pixels into the SHM buffer.
    #[deprecated(note = "`Entry::update` writes finished pixels; flushing is no longer needed")]
    pub fn flush(&self) {}

    /// Returns the index of the frame that is ready to be committed to the
    /// compositor (the one most recently drawn into, before the flip).
    pub fn ready_index(&self) -> usize {
        self.index ^ 1
    }

    // ── Selection / scrolling ─────────────────────────────────────────────────

    /// Reset selection to the first result and scroll back to the top.
    ///
    /// Should be called when the input changes (results list is rebuilt).
    pub fn reset_selection(&mut self) {
        self.selection = 0;
        self.first_result = 0;
    }

    /// Move the selection one item forward, scrolling the visible window when
    /// the selection reaches the end of the currently drawn results.
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

    /// Move the selection one item backward, scrolling the visible window when
    /// the selection reaches the beginning of the currently drawn results.
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

    /// Jump forward one page (skip `num_results_drawn` items).
    pub fn select_next_page(&mut self) {
        self.first_result += self.num_results_drawn;
        if self.first_result >= self.results.len() {
            self.first_result = 0;
        }
        self.selection = 0;
        self.last_num_results_drawn = self.num_results_drawn;
    }

    /// Jump backward one page (skip back `last_num_results_drawn` items).
    pub fn select_prev_page(&mut self) {
        if self.first_result >= self.last_num_results_drawn {
            self.first_result -= self.last_num_results_drawn;
        } else {
            self.first_result = 0;
        }
        self.selection = 0;
        self.last_num_results_drawn = self.num_results_drawn;
    }
}

// ── Drawing helpers ───────────────────────────────────────────────────────────

/// Paint background + border + outline inside the rounded window shape.
///
/// Everything outside the rounded rectangle stays transparent.
fn draw_background_and_border(ctx: &mut RenderContext, cfg: &EntryConfig, width: u32, height: u32) {
    let rect = Rect::new(0.0, 0.0, width as f64, height as f64);
    let shape = RoundedRect::from_rect(rect, cfg.corner_radius as f64).to_path(0.1);
    let bw = cfg.border_width as f64;
    let ow = cfg.outline_width as f64;

    // Strokes are centered on the window edge; clipping to the shape keeps
    // only their inner half, leaving outline / border / outline bands.
    ctx.push_clip_layer(&shape);

    ctx.set_paint(paint(cfg.background_color));
    ctx.fill_rect(&rect);

    for (line_width, color) in [
        (4.0 * ow + 2.0 * bw, cfg.outline_color),
        (2.0 * ow + 2.0 * bw, cfg.border_color),
        (2.0 * ow, cfg.outline_color),
    ] {
        if line_width > 0.0 {
            ctx.set_stroke(Stroke::new(line_width));
            ctx.set_paint(paint(color));
            ctx.stroke_path(&shape);
        }
    }

    ctx.pop_layer();
}

/// Clip region and text origin computed by [`compute_clip`].
struct ClipLayout {
    /// Clip rectangle in logical pixels.
    rect: Rect,
    /// `rect`, truncated to whole pixels.
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    /// Where text drawing starts, in logical pixels.
    text_origin: (f64, f64),
}

/// Compute the clip region (border inset + optional padding + corner inset)
/// and the text drawing origin.
fn compute_clip(cfg: &EntryConfig, width: u32, height: u32) -> ClipLayout {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut w = width as f64;
    let mut h = height as f64;
    let bw = cfg.border_width as f64;
    let ow = cfg.outline_width as f64;

    // Inset inside the border+outline.
    let dx = 2.0 * ow + bw;
    x += dx;
    y += dx;
    w -= 2.0 * dx;
    h -= 2.0 * dx;

    // Optionally include padding in the clip region.
    if cfg.clip_to_padding {
        x += cfg.padding_left as f64;
        y += cfg.padding_top as f64;
        w -= (cfg.padding_left + cfg.padding_right) as f64;
        h -= (cfg.padding_top + cfg.padding_bottom) as f64;
    }

    // Further inset for rounded corner clearance.
    let inner_r = (cfg.corner_radius as f64 - dx).max(0.0);
    let corner_dx = (inner_r * (1.0 - 1.0 / std::f64::consts::SQRT_2)).ceil();
    x += corner_dx;
    y += corner_dx;
    w -= 2.0 * corner_dx;
    h -= 2.0 * corner_dx;

    let rect = Rect::new(x, y, x + w.max(0.0), y + h.max(0.0));

    // If padding is *not* part of the clip region, still offset past it so
    // text starts at the right place.
    let text_origin = if cfg.clip_to_padding {
        (x, y)
    } else {
        (x + cfg.padding_left as f64, y + cfg.padding_top as f64)
    };

    ClipLayout {
        rect,
        x: x as u32,
        y: y as u32,
        width: w as u32,
        height: h as u32,
        text_origin,
    }
}
