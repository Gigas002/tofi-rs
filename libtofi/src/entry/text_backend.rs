//! cosmic-text drawing backend for [`super::Entry`].
//!
//! Implements: prompt, input field (with cursor), result list, selection
//! highlight. All drawing uses **logical** pixel coordinates (physical mapping
//! is handled by [`super::canvas::Canvas`] scale).

use cosmic_text::{
    Attrs, Buffer, Color as CTColor, Cursor, Family, FeatureTag, FontFeatures, FontSystem, Metrics,
    Shaping, SwashCache, Wrap,
};

use crate::Result;

use super::canvas::Canvas;
use super::{CursorStyle, EntryConfig, ResolvedCursorTheme, ResolvedTextTheme};

// ── TextExtents ───────────────────────────────────────────────────────────────

/// Ink and logical bounding boxes for laid-out text (Pango-compatible fields).
#[derive(Clone, Copy, Default)]
pub(crate) struct TextExtents {
    pub ink_x: i32,
    pub ink_width: i32,
    pub logical_x: i32,
    pub logical_width: i32,
    pub logical_height: i32,
}

impl TextExtents {
    fn set_logical_width(&mut self, w: i32) {
        self.logical_width = w;
    }
}

// ── TextBackend ───────────────────────────────────────────────────────────────

/// Text layout state held by [`Entry`] for the lifetime of the widget.
pub struct TextBackend {
    pub(crate) font_system: FontSystem,
    swash_cache: SwashCache,
    buffer: Buffer,
    attrs: Attrs<'static>,
}

impl TextBackend {
    /// Initialise font system and layout; compute cursor metrics.
    pub(crate) fn init(
        config: &EntryConfig,
        resolved_input: &ResolvedTextTheme,
        default_fg: crate::color::Color,
        default_bg: crate::color::Color,
    ) -> Result<(Self, ResolvedCursorTheme)> {
        let mut font_system = FontSystem::new();
        let font_size = config.font_size as f32;
        let line_height = font_size * 1.2;
        let metrics = Metrics::new(font_size, line_height);

        let mut attrs = build_attrs(config);
        attrs = attrs.color(ct_color(default_fg));

        let mut buffer = Buffer::new(&mut font_system, metrics);
        buffer.set_size(Some(10_000.0), Some(line_height));
        buffer.set_wrap(Wrap::None);

        buffer.set_text("m", &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);
        let probe = measure_extents(&mut buffer, &mut font_system);

        let em_width = if probe.ink_width > 0 {
            probe.ink_width as f64
        } else {
            font_size as f64 * 0.6
        };

        let underline_depth = (line_height * 0.85) as f64;
        let underline_thickness = (font_size * 0.08).max(1.0) as u32;

        let thickness = match &config.cursor_theme.thickness {
            Some(t) => *t,
            None => {
                if config.cursor_theme.style == CursorStyle::Underscore {
                    underline_thickness
                } else {
                    2
                }
            }
        };

        let resolved_cursor = ResolvedCursorTheme {
            color: config
                .cursor_theme
                .color
                .unwrap_or(resolved_input.foreground_color),
            text_color: config.cursor_theme.text_color.unwrap_or(default_bg),
            style: config.cursor_theme.style,
            corner_radius: config.cursor_theme.corner_radius,
            thickness,
            underline_depth,
            em_width,
            show: config.cursor_theme.show,
        };

        tracing::debug!(
            "TextBackend::init font={:?} em_width={em_width:.2} underline_depth={underline_depth:.2}",
            config.font_name,
        );

        Ok((
            Self {
                font_system,
                swash_cache: SwashCache::new(),
                buffer,
                attrs,
            },
            resolved_cursor,
        ))
    }

    pub(crate) fn layout(&mut self, text: &str) -> TextExtents {
        self.buffer
            .set_text(text, &self.attrs, Shaping::Advanced, None);
        self.buffer.shape_until_scroll(&mut self.font_system, false);
        measure_extents(&mut self.buffer, &mut self.font_system)
    }

    fn draw_text(&mut self, canvas: &mut Canvas<'_>, text: &str, color: crate::color::Color) {
        let _extents = self.layout(text);
        let ct = ct_color(color);
        self.buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            ct,
            |x, y, w, h, c| {
                canvas.fill_pixels(x, y, w, h, |_lx, _ly| c.as_rgba());
            },
        );
    }

    fn cursor_x_for_char(&mut self, text: &str, char_index: usize) -> f64 {
        let byte_index = char_index_to_byte(text, char_index);
        self.layout(text);
        cursor_x_for_byte(&mut self.buffer, &mut self.font_system, byte_index)
    }

    fn cursor_width_for_char(&mut self, text: &str, char_index: usize) -> f64 {
        let byte_start = char_index_to_byte(text, char_index);
        let byte_end = text[byte_start..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| byte_start + i)
            .unwrap_or(text.len());
        self.layout(text);
        let x0 = cursor_x_for_byte(&mut self.buffer, &mut self.font_system, byte_start);
        let x1 = cursor_x_for_byte(&mut self.buffer, &mut self.font_system, byte_end);
        (x1 - x0).max(1.0)
    }
}

fn build_attrs(config: &EntryConfig) -> Attrs<'static> {
    let family = if config.font_name.is_empty() {
        Family::SansSerif
    } else {
        Family::Name(Box::leak(config.font_name.clone().into_boxed_str()))
    };

    let mut attrs = Attrs::new().family(family);

    if !config.font_features.is_empty() {
        attrs = attrs.font_features(parse_font_features(&config.font_features));
    }

    // OpenType variations are not yet exposed on cosmic-text Attrs; font_variations
    // from theme config is accepted but ignored until upstream adds support.

    attrs
}

fn parse_font_features(raw: &str) -> FontFeatures {
    let mut features = FontFeatures::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((tag, value)) = part.split_once('=') {
            let mut bytes = [b' '; 4];
            for (i, b) in tag.trim().bytes().take(4).enumerate() {
                bytes[i] = b;
            }
            let val: u32 = value.trim().parse().unwrap_or(1);
            features.set(FeatureTag::new(&bytes), val);
        } else {
            let mut bytes = [b' '; 4];
            for (i, b) in part.bytes().take(4).enumerate() {
                bytes[i] = b;
            }
            features.enable(FeatureTag::new(&bytes));
        }
    }
    features
}

fn measure_extents(buffer: &mut Buffer, font_system: &mut FontSystem) -> TextExtents {
    buffer.shape_until_scroll(font_system, false);
    let mut extents = TextExtents::default();

    if let Some(run) = buffer.layout_runs().next() {
        extents.logical_width = run.line_w.ceil() as i32;
        extents.logical_height = run.line_height.ceil() as i32;

        if run.glyphs.is_empty() {
            return extents;
        }

        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        for glyph in run.glyphs {
            min_x = min_x.min(glyph.x);
            max_x = max_x.max(glyph.x + glyph.w);
        }
        extents.ink_x = min_x.floor() as i32;
        extents.ink_width = (max_x - min_x).ceil() as i32;
    }

    extents
}

fn cursor_x_for_byte(buffer: &mut Buffer, font_system: &mut FontSystem, byte_index: usize) -> f64 {
    let cursor = Cursor::new(0, byte_index);
    let layout_cursor = buffer.layout_cursor(font_system, cursor);
    let Some(layout) = buffer.line_layout(font_system, 0) else {
        return 0.0;
    };

    let Some(lc) = layout_cursor else {
        return layout.first().map(|l| l.w as f64).unwrap_or(0.0);
    };

    let layout_line = &layout[lc.layout];
    if lc.glyph >= layout_line.glyphs.len() {
        layout_line.w as f64
    } else {
        layout_line.glyphs[lc.glyph].x as f64
    }
}

fn char_index_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}

fn ct_color(c: crate::color::Color) -> CTColor {
    CTColor::rgba(
        (c.r * 255.0).round() as u8,
        (c.g * 255.0).round() as u8,
        (c.b * 255.0).round() as u8,
        (c.a * 255.0).round() as u8,
    )
}

// ── TextUpdateContext ─────────────────────────────────────────────────────────

/// Snapshot of entry state needed for one text draw pass.
pub(crate) struct TextUpdateContext {
    pub config: EntryConfig,
    pub input: String,
    pub cursor_position: usize,
    pub first_result: usize,
    pub selection: usize,
    pub results: Vec<String>,
    pub clip_x: u32,
    pub clip_y: u32,
    pub clip_width: u32,
    pub clip_height: u32,
    pub resolved_prompt_theme: ResolvedTextTheme,
    pub resolved_input_theme: ResolvedTextTheme,
    pub resolved_placeholder_theme: ResolvedTextTheme,
    pub resolved_default_result_theme: ResolvedTextTheme,
    pub resolved_alternate_result_theme: ResolvedTextTheme,
    pub resolved_selection_theme: ResolvedTextTheme,
    pub resolved_cursor: ResolvedCursorTheme,
}

// ── ClipRect ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct ClipRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

// ── update ────────────────────────────────────────────────────────────────────

/// Render all text elements into `canvas`.
pub(crate) fn update(
    canvas: &mut Canvas<'_>,
    text: &mut TextBackend,
    ctx: &TextUpdateContext,
) -> usize {
    canvas.save();

    let mut ink = TextExtents::default();
    let mut logical = TextExtents::default();

    let clip = ClipRect {
        x: ctx.clip_x,
        y: ctx.clip_y,
        width: ctx.clip_width,
        height: ctx.clip_height,
    };

    render_text_themed(
        canvas,
        text,
        &ctx.config.prompt_text,
        &ctx.resolved_prompt_theme,
        clip,
        &mut ink,
        &mut logical,
    );

    let prompt_adv = (logical.logical_width + logical.logical_x) as f64;
    canvas.translate(prompt_adv + ctx.config.prompt_padding as f64, 0.0);

    if ctx.input.is_empty() {
        render_input(
            canvas,
            text,
            &ctx.config.placeholder_text,
            ctx.config.placeholder_text.chars().count(),
            &ctx.resolved_placeholder_theme,
            0,
            &ctx.resolved_cursor,
            &mut ink,
            &mut logical,
        );
    } else if ctx.config.hide_input {
        let hidden_char = ctx.config.hidden_character.clone();
        let n_chars = ctx.input.chars().count();
        let hidden: String = hidden_char.repeat(n_chars);
        render_input(
            canvas,
            text,
            &hidden,
            n_chars,
            &ctx.resolved_input_theme,
            ctx.cursor_position,
            &ctx.resolved_cursor,
            &mut ink,
            &mut logical,
        );
    } else {
        let n_chars = ctx.input.chars().count();
        render_input(
            canvas,
            text,
            &ctx.input,
            n_chars,
            &ctx.resolved_input_theme,
            ctx.cursor_position,
            &ctx.resolved_cursor,
            &mut ink,
            &mut logical,
        );
    }

    let min_w = ctx.config.input_width as i32;
    if logical.logical_width < min_w {
        logical.set_logical_width(min_w);
    }

    let num_results_cap = if ctx.config.num_results == 0 {
        ctx.results.len()
    } else {
        ctx.config.num_results as usize
    };

    let horizontal = ctx.config.horizontal;
    let result_spacing = ctx.config.result_spacing as f64;
    let first = ctx.first_result;
    let selection = ctx.selection;
    let selection_highlight_color = ctx.config.selection_highlight_color;
    let num_results_fixed = ctx.config.num_results;

    let mut i = 0usize;
    while i < num_results_cap {
        if horizontal {
            let adv = (logical.logical_x + logical.logical_width) as f64 + result_spacing;
            canvas.translate(adv, 0.0);
        } else {
            let adv = logical.logical_height as f64 + result_spacing;
            canvas.translate(0.0, adv);
        }

        if num_results_fixed == 0 && size_overflows(canvas, clip, horizontal, 0.0, 0.0) {
            break;
        }

        let result_index = i + first;
        if result_index >= ctx.results.len() {
            break;
        }

        let str = &ctx.results[result_index];

        if i != selection || selection_highlight_color.a == 0.0 {
            let theme = if i == selection {
                &ctx.resolved_selection_theme
            } else if result_index % 2 == 1 {
                &ctx.resolved_alternate_result_theme
            } else {
                &ctx.resolved_default_result_theme
            };

            if num_results_fixed > 0 {
                render_text_themed(canvas, text, str, theme, clip, &mut ink, &mut logical);
            } else if !horizontal {
                let h_px = logical.logical_height as f64;
                if size_overflows(canvas, clip, false, 0.0, h_px) {
                    canvas.restore();
                    return i;
                }
                render_text_themed(canvas, text, str, theme, clip, &mut ink, &mut logical);
            } else {
                let extents = text.layout(str);
                let w_px = extents.logical_width as f64;
                if size_overflows(canvas, clip, true, w_px, 0.0) {
                    canvas.restore();
                    return i;
                }
                render_text_themed(canvas, text, str, theme, clip, &mut ink, &mut logical);
            }
        } else {
            render_selected_result(
                canvas,
                text,
                str,
                &ctx.input,
                &ctx.resolved_selection_theme,
                selection_highlight_color,
                &mut ink,
                &mut logical,
            );
        }

        i += 1;
    }

    tracing::debug!("text_backend::update drew {} results", i);
    canvas.restore();
    i
}

fn render_text_themed(
    canvas: &mut Canvas<'_>,
    text: &mut TextBackend,
    text_str: &str,
    theme: &ResolvedTextTheme,
    clip: ClipRect,
    ink: &mut TextExtents,
    logical: &mut TextExtents,
) {
    let fg = theme.foreground_color;
    text.draw_text(canvas, text_str, fg);
    *ink = text.layout(text_str);
    *logical = *ink;

    if theme.background_color.a == 0.0 {
        return;
    }

    let padding = theme.padding;
    let base_x = canvas.matrix_x0() - clip.x as f64 + ink.ink_x as f64;
    let base_y = canvas.matrix_y0() - clip.y as f64;

    let mut pad_left = padding.left as f64;
    let mut pad_right = padding.right as f64;
    let mut pad_top = padding.top as f64;
    let mut pad_bottom = padding.bottom as f64;

    if pad_left < 0.0 {
        pad_left = base_x;
    }
    if pad_right < 0.0 {
        pad_right = clip.width as f64 - ink.ink_width as f64 - base_x;
    }
    if pad_top < 0.0 {
        pad_top = base_y;
    }
    if pad_bottom < 0.0 {
        pad_bottom = clip.height as f64 - logical.logical_height as f64 - base_y;
    }

    canvas.save();
    canvas.translate(-pad_left + ink.ink_x as f64, -pad_top);
    canvas.fill_rounded_rect(
        (ink.ink_width as f64 + pad_left + pad_right).ceil(),
        (logical.logical_height as f64 + pad_top + pad_bottom).ceil(),
        theme.background_corner_radius as f64,
        theme.background_color,
    );
    canvas.restore();

    text.draw_text(canvas, text_str, fg);
}

#[allow(clippy::too_many_arguments)]
fn render_input(
    canvas: &mut Canvas<'_>,
    text: &mut TextBackend,
    text_str: &str,
    text_char_len: usize,
    theme: &ResolvedTextTheme,
    cursor_position: usize,
    cursor: &ResolvedCursorTheme,
    ink: &mut TextExtents,
    logical: &mut TextExtents,
) {
    let fg = theme.foreground_color;
    text.draw_text(canvas, text_str, fg);
    *ink = text.layout(text_str);
    *logical = *ink;

    let mut extra_cursor_advance = 0.0f64;
    if cursor_position == text_char_len && cursor.show {
        extra_cursor_advance = match cursor.style {
            CursorStyle::Bar => cursor.thickness as f64,
            CursorStyle::Block | CursorStyle::Underscore => cursor.em_width,
        };
        extra_cursor_advance +=
            logical.logical_width as f64 - logical.logical_x as f64 - ink.ink_width as f64;
    }

    if theme.background_color.a != 0.0 {
        let padding = theme.padding;
        canvas.save();
        canvas.translate(
            f64::floor(-padding.left as f64 + ink.ink_x as f64),
            -padding.top as f64,
        );
        canvas.fill_rounded_rect(
            (ink.ink_width as f64
                + extra_cursor_advance
                + padding.left as f64
                + padding.right as f64)
                .ceil(),
            (logical.logical_height as f64 + padding.top as f64 + padding.bottom as f64).ceil(),
            theme.background_corner_radius as f64,
            theme.background_color,
        );
        canvas.restore();
        text.draw_text(canvas, text_str, fg);
    }

    if !cursor.show {
        return;
    }

    let (cursor_x, cursor_width) = if cursor_position == text_char_len {
        let x = logical.logical_width as f64 + logical.logical_x as f64;
        (x, cursor.em_width)
    } else {
        let cx = text.cursor_x_for_char(text_str, cursor_position);
        let cw = text.cursor_width_for_char(text_str, cursor_position);
        (cx, cw)
    };

    canvas.save();
    canvas.translate(cursor_x, 0.0);

    match cursor.style {
        CursorStyle::Bar => {
            canvas.fill_rounded_rect(
                cursor.thickness as f64,
                logical.logical_height as f64,
                cursor.corner_radius as f64,
                cursor.color,
            );
        }
        CursorStyle::Block => {
            canvas.fill_rounded_rect(
                cursor_width,
                logical.logical_height as f64,
                cursor.corner_radius as f64,
                cursor.color,
            );
            canvas.set_clip_logical(0.0, 0.0, cursor_width, logical.logical_height as f64);
            canvas.translate(-cursor_x, 0.0);
            text.draw_text(canvas, text_str, cursor.text_color);
        }
        CursorStyle::Underscore => {
            canvas.translate(0.0, cursor.underline_depth);
            canvas.fill_rounded_rect(
                cursor_width,
                cursor.thickness as f64,
                cursor.corner_radius as f64,
                cursor.color,
            );
        }
    }

    logical.set_logical_width(logical.logical_width + extra_cursor_advance as i32);
    canvas.restore();
}

#[allow(clippy::too_many_arguments)]
fn render_selected_result(
    canvas: &mut Canvas<'_>,
    text: &mut TextBackend,
    text_str: &str,
    input: &str,
    sel_theme: &ResolvedTextTheme,
    highlight_color: crate::color::Color,
    ink: &mut TextExtents,
    logical: &mut TextExtents,
) {
    let match_info = if !input.is_empty() && highlight_color.a > 0.0 {
        find_match_position(text_str, input)
    } else {
        None
    };

    for pass in 0..2 {
        canvas.save();

        let mut combined_ink = TextExtents::default();
        let mut combined_logical = TextExtents::default();
        let mut first_segment = true;

        let segments: Vec<(&str, crate::color::Color)> = match match_info {
            None => vec![(text_str, sel_theme.foreground_color)],
            Some((pre_end, match_end)) => {
                let mut segs = Vec::new();
                if pre_end > 0 {
                    segs.push((&text_str[..pre_end], sel_theme.foreground_color));
                }
                segs.push((&text_str[pre_end..match_end], highlight_color));
                if match_end < text_str.len() {
                    segs.push((&text_str[match_end..], sel_theme.foreground_color));
                }
                segs
            }
        };

        for (seg_text, seg_color) in &segments {
            text.draw_text(canvas, *seg_text, *seg_color);
            let seg = text.layout(seg_text);

            if first_segment {
                combined_ink = seg;
                combined_logical = seg;
                first_segment = false;
            } else {
                let new_iw =
                    combined_logical.logical_width - combined_ink.ink_x + seg.ink_x + seg.ink_width;
                combined_ink.ink_width = new_iw;
                combined_logical.logical_width =
                    combined_logical.logical_width + seg.logical_x + seg.logical_width;
            }

            let adv = seg.logical_x as f64 + seg.logical_width as f64;
            canvas.translate(adv, 0.0);
        }

        canvas.restore();
        *ink = combined_ink;
        *logical = combined_logical;

        if pass == 0 {
            if sel_theme.background_color.a == 0.0 {
                break;
            }
            let padding = sel_theme.padding;
            canvas.save();
            canvas.translate(
                f64::floor(-padding.left as f64 + ink.ink_x as f64),
                -(padding.top as f64),
            );
            canvas.fill_rounded_rect(
                (ink.ink_width as f64 + padding.left as f64 + padding.right as f64).ceil(),
                (logical.logical_height as f64 + padding.top as f64 + padding.bottom as f64).ceil(),
                sel_theme.background_corner_radius as f64,
                sel_theme.background_color,
            );
            canvas.restore();
        }
    }
}

fn size_overflows(
    canvas: &Canvas<'_>,
    clip: ClipRect,
    horizontal: bool,
    extra_w: f64,
    extra_h: f64,
) -> bool {
    if horizontal {
        canvas.matrix_x0() + extra_w > (clip.x + clip.width) as f64
    } else {
        canvas.matrix_y0() + extra_h > (clip.y + clip.height) as f64
    }
}

/// Case-insensitive substring search.
///
/// Returns `Some((pre_end_byte, match_end_byte))` when `needle` is found in
/// `haystack`, where both values are byte offsets.
pub(super) fn find_match_position(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return None;
    }
    let hay_lower = haystack.to_lowercase();
    let need_lower = needle.to_lowercase();
    let pos = hay_lower.find(&need_lower)?;
    Some((pos, pos + needle.len()))
}
