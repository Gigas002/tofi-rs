//! Entry layout engine.
//!
//! # Safety
//!
//! [`Entry::new`] is `unsafe` because it wraps raw SHM buffer memory.
//! See the function's doc-comment for invariants.

use std::f64::consts::{FRAC_PI_2, PI};

use cairo::{Context, FillRule, Format, ImageSurface, Operator};

use crate::color::Color;
use crate::scale::scale_apply_inverse;
use crate::{Error, Result};

pub mod pango_backend;
#[cfg(test)]
mod tests;

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
/// by the pango backend.
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

/// Cursor theme with all fields resolved by the Pango backend.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedCursorTheme {
    pub color: Color,
    pub text_color: Color,
    pub style: CursorStyle,
    pub corner_radius: u32,
    pub thickness: u32,
    /// Distance from text baseline to the underline top (Pango-computed).
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
/// Holds two Cairo surfaces backed by consecutive halves of an SHM buffer,
/// a Pango backend, resolved themes, and all runtime state.
pub struct Entry {
    // Double-buffered Cairo surfaces (index 0 and 1).
    surfaces: [ImageSurface; 2],
    contexts: [Context; 2],
    /// Which buffer is next to be drawn into.
    pub index: usize,

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

    // Pango backend (lives for the lifetime of the Entry).
    pub(crate) pango: pango_backend::PangoBackend,
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
        let stride = width as i32 * 4;
        let frame_bytes = (width * height * 4) as usize;

        tracing::debug!("Entry::new {width}×{height} physical, scale={scale:.4}");

        // ── Create two Cairo surfaces (double buffering) ─────────────────────
        // SAFETY: caller guarantees validity.
        let surface0 = unsafe {
            ImageSurface::create_for_data_unsafe(
                data,
                Format::ARgb32,
                width as i32,
                height as i32,
                stride,
            )
        }
        .map_err(|e| Error::Renderer(format!("entry surface[0]: {e}")))?;

        let surface1 = unsafe {
            ImageSurface::create_for_data_unsafe(
                data.add(frame_bytes),
                Format::ARgb32,
                width as i32,
                height as i32,
                stride,
            )
        }
        .map_err(|e| Error::Renderer(format!("entry surface[1]: {e}")))?;

        surface0.set_device_scale(scale, scale);
        surface1.set_device_scale(scale, scale);

        let cr0 = Context::new(&surface0)
            .map_err(|e| Error::Renderer(format!("entry context[0]: {e}")))?;
        let cr1 = Context::new(&surface1)
            .map_err(|e| Error::Renderer(format!("entry context[1]: {e}")))?;

        // Logical dimensions (physical ÷ scale).
        let logical_width = scale_apply_inverse(width, scale_numerator);
        let logical_height = scale_apply_inverse(height, scale_numerator);

        // ── Draw static background + border on both frames ───────────────────
        // C draws only on cairo[0] in entry_init, deferring the memcpy to
        // cairo[1] until just after the first frame is shown.  We draw on both
        // frames here to avoid the deferred-copy complexity; the result is
        // identical.
        for cr in [&cr0, &cr1] {
            draw_background_and_border(cr, &config, logical_width, logical_height)?;
        }

        // ── Build clip transform on context[0], then mirror to context[1] ────
        // Returns the clip rect and the final translation applied inside the
        // clip so that text draws in the padded area.
        let (clip_x, clip_y, clip_w, clip_h) =
            setup_clip(&cr0, &config, logical_width, logical_height);
        setup_clip(&cr1, &config, logical_width, logical_height);

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

        // ── Initialise Pango backend ─────────────────────────────────────────
        let (pango, resolved_cursor) = pango_backend::PangoBackend::init(
            &cr0,
            &config,
            &resolved_input_theme,
            default_fg,
            default_bg,
        )?;

        // ── Build Entry and perform the initial text render ──────────────────
        let mut entry = Self {
            surfaces: [surface0, surface1],
            contexts: [cr0, cr1],
            index: 0,
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
            pango,
        };

        entry.pango_update();
        entry.index = 1;

        Ok(entry)
    }

    /// Redraw the entry into the current back buffer and flip the index.
    ///
    /// Must be called whenever the input, selection, or results change.
    pub fn update(&mut self) {
        tracing::debug!("Entry::update");
        let cr = &self.contexts[self.index];

        // Clear the interior with background color.
        let c = self.config.background_color;
        cr.set_source_rgba(c.r as f64, c.g as f64, c.b as f64, c.a as f64);
        cr.save().unwrap_or_default();
        cr.set_operator(Operator::Source);
        cr.paint().unwrap_or_default();
        cr.restore().unwrap_or_default();

        self.pango_update();
        self.index ^= 1;
    }

    /// Flush both Cairo surfaces so pixel writes are visible in the SHM buffer.
    pub fn flush(&self) {
        self.surfaces[0].flush();
        self.surfaces[1].flush();
    }

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

    // ── Internal helpers ─────────────────────────────────────────────────────

    /// Invoke the Pango backend update on the current context.
    fn pango_update(&mut self) {
        // Clone the Context (GObject refcount — zero-cost for the surface).
        // This avoids a split-borrow conflict: `self.contexts[idx]` is an
        // immutable borrow of `self`, but `update` also needs `&mut self`.
        let cr = self.contexts[self.index].clone();
        pango_backend::update(&cr, self);
    }
}

// ── Drawing helpers ───────────────────────────────────────────────────────────

/// Draw a rounded rectangle path.
///
/// When `r == 0` the result is a plain rectangle.
pub(crate) fn rounded_rectangle(cr: &Context, width: f64, height: f64, r: f64) {
    cr.new_path();
    if r <= 0.0 {
        cr.rectangle(0.0, 0.0, width, height);
        return;
    }
    // Top-left
    cr.arc(r, r, r, -PI, -FRAC_PI_2);
    // Top-right
    cr.arc(width - r, r, r, -FRAC_PI_2, 0.0);
    // Bottom-right
    cr.arc(width - r, height - r, r, 0.0, FRAC_PI_2);
    // Bottom-left
    cr.arc(r, height - r, r, FRAC_PI_2, PI);
    cr.close_path();
}

/// Paint background + border + outline, and clear pixel artifacts outside
/// rounded corners.
fn draw_background_and_border(
    cr: &Context,
    cfg: &EntryConfig,
    width: u32,
    height: u32,
) -> Result<()> {
    let (w, h) = (width as f64, height as f64);
    let r = cfg.corner_radius as f64;
    let bw = cfg.border_width as f64;
    let ow = cfg.outline_width as f64;

    // Fill with background color.
    let c = cfg.background_color;
    cr.set_source_rgba(c.r as f64, c.g as f64, c.b as f64, c.a as f64);
    cr.set_operator(Operator::Source);
    cr.paint()
        .map_err(|e| Error::Renderer(format!("background paint: {e}")))?;

    // Outer outline stroke.
    cr.set_line_width(4.0 * ow + 2.0 * bw);
    rounded_rectangle(cr, w, h, r);
    let c = cfg.outline_color;
    cr.set_source_rgba(c.r as f64, c.g as f64, c.b as f64, c.a as f64);
    cr.stroke_preserve()
        .map_err(|e| Error::Renderer(format!("outline stroke: {e}")))?;

    // Border stroke.
    let c = cfg.border_color;
    cr.set_source_rgba(c.r as f64, c.g as f64, c.b as f64, c.a as f64);
    cr.set_line_width(2.0 * ow + 2.0 * bw);
    cr.stroke_preserve()
        .map_err(|e| Error::Renderer(format!("border stroke: {e}")))?;

    // Inner outline stroke.
    let c = cfg.outline_color;
    cr.set_source_rgba(c.r as f64, c.g as f64, c.b as f64, c.a as f64);
    cr.set_line_width(2.0 * ow);
    cr.stroke_preserve()
        .map_err(|e| Error::Renderer(format!("inner outline stroke: {e}")))?;

    // Clear pixels outside rounded corners using EVEN_ODD fill rule.
    // The +1 prevents 1-pixel artifacts at certain fractional scale factors.
    cr.rectangle(0.0, 0.0, w + 1.0, h + 1.0);
    cr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
    cr.save()
        .map_err(|e| Error::Renderer(format!("save: {e}")))?;
    cr.set_fill_rule(FillRule::EvenOdd);
    cr.set_operator(Operator::Clear);
    cr.fill()
        .map_err(|e| Error::Renderer(format!("corner clear: {e}")))?;
    cr.restore()
        .map_err(|e| Error::Renderer(format!("restore: {e}")))?;

    cr.set_operator(Operator::Over);

    Ok(())
}

/// Apply the clip transform (border inset + optional padding + corner inset)
/// and return the clip rectangle `(clip_x, clip_y, clip_width, clip_height)`.
///
/// The context is left in a state where the current-transform-matrix (CTM)
/// origin is at the text drawing start, and a clip rectangle is active.
fn setup_clip(cr: &Context, cfg: &EntryConfig, width: u32, height: u32) -> (u32, u32, u32, u32) {
    let mut w = width as f64;
    let mut h = height as f64;
    let bw = cfg.border_width as f64;
    let ow = cfg.outline_width as f64;

    // Translate inside the border+outline inset.
    let dx = 2.0 * ow + bw;
    cr.translate(dx, dx);
    w -= 2.0 * dx;
    h -= 2.0 * dx;

    // Optionally include padding in the clip region.
    if cfg.clip_to_padding {
        cr.translate(cfg.padding_left as f64, cfg.padding_top as f64);
        w -= (cfg.padding_left + cfg.padding_right) as f64;
        h -= (cfg.padding_top + cfg.padding_bottom) as f64;
    }

    // Further inset for rounded corner clearance.
    let inner_r = (cfg.corner_radius as f64 - dx).max(0.0);
    let corner_dx = (inner_r * (1.0 - 1.0 / std::f64::consts::SQRT_2)).ceil();
    cr.translate(corner_dx, corner_dx);
    w -= 2.0 * corner_dx;
    h -= 2.0 * corner_dx;

    cr.rectangle(0.0, 0.0, w, h);
    cr.clip();

    // Record clip rect position from the CTM.
    let mat = cr.matrix();
    let clip_x = mat.x0() as u32;
    let clip_y = mat.y0() as u32;
    let clip_w = w as u32;
    let clip_h = h as u32;

    // If padding is *not* part of the clip region, still translate past it
    // so text starts at the right place.
    if !cfg.clip_to_padding {
        cr.translate(cfg.padding_left as f64, cfg.padding_top as f64);
    }

    (clip_x, clip_y, clip_w, clip_h)
}
