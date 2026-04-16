//! Pango/Cairo drawing backend for [`super::Entry`].
//!
//! Implements: prompt, input field (with cursor), result list, selection
//! highlight.  All drawing uses **logical** pixel coordinates (the Cairo
//! device scale handles physical-pixel mapping).

use std::f64;

use cairo::Context;
use pango::{FontDescription, prelude::FontExt};

use crate::{Error, Result};

use super::{
    CursorStyle, Entry, EntryConfig, ResolvedCursorTheme, ResolvedTextTheme, rounded_rectangle,
};

// ── PangoBackend ──────────────────────────────────────────────────────────────

/// Pango state held by [`Entry`] for the lifetime of the widget.
pub struct PangoBackend {
    /// Kept alive as a lifetime anchor for the layout (GObject refcount).
    /// The Pango layout internally holds a strong ref to its context, so this
    /// field is deliberately not read after construction.
    _context: pango::Context,
    pub(crate) layout: pango::Layout,
}

impl PangoBackend {
    /// Initialise the Pango context and layout; compute cursor metrics.
    ///
    /// Returns `(PangoBackend, ResolvedCursorTheme)`.
    pub(crate) fn init(
        cr: &Context,
        config: &EntryConfig,
        resolved_input: &ResolvedTextTheme,
        _default_fg: crate::color::Color,
        default_bg: crate::color::Color,
    ) -> Result<(Self, ResolvedCursorTheme)> {
        let context = pangocairo::functions::create_context(cr);

        let mut font_desc = FontDescription::from_string(&config.font_name);
        font_desc.set_size(config.font_size as i32 * pango::SCALE);

        if !config.font_variations.is_empty() {
            font_desc.set_variations(Some(&config.font_variations));
        }

        context.set_font_description(Some(&font_desc));

        let layout = pango::Layout::new(&context);

        // Font features attribute.
        if !config.font_features.is_empty() {
            let attr = pango::AttrFontFeatures::new(&config.font_features);
            let attr_list = pango::AttrList::new();
            attr_list.insert(attr);
            layout.set_attributes(Some(&attr_list));
        }

        // Measure font metrics for cursor sizing.
        let font = context
            .load_font(&font_desc)
            .ok_or_else(|| Error::Renderer("Pango: failed to load font".into()))?;
        let metrics = font.metrics(None);

        // em_width: try HarfBuzz glyph advance for 'm'; fall back to
        // approximate_char_width (the C code does the same).
        // We always use the fallback here since we don't link harfbuzz directly.
        let em_width = metrics.approximate_char_width() as f64 / pango::SCALE as f64;

        let underline_depth =
            (metrics.ascent() - metrics.underline_position()) as f64 / pango::SCALE as f64;

        // Cursor thickness: use font underline thickness for UNDERSCORE style
        // when not overridden; default 2 px otherwise.
        let thickness = match &config.cursor_theme.thickness {
            Some(t) => *t,
            None => {
                if config.cursor_theme.style == CursorStyle::Underscore {
                    (metrics.underline_thickness() / pango::SCALE) as u32
                } else {
                    2
                }
            }
        };

        // Resolve cursor colors.
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
            "PangoBackend::init font={:?} em_width={em_width:.2} underline_depth={underline_depth:.2}",
            config.font_name,
        );

        Ok((
            Self {
                _context: context,
                layout,
            },
            resolved_cursor,
        ))
    }
}

// ── ClipRect ──────────────────────────────────────────────────────────────────

/// Clip rectangle passed to drawing helpers to reduce their argument count.
#[derive(Clone, Copy)]
struct ClipRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

// ── update ────────────────────────────────────────────────────────────────────

/// Render all text elements into `cr`.
///
/// Called by [`Entry::pango_update`] with the current back-buffer context.
pub(crate) fn update(cr: &Context, entry: &mut Entry) {
    // Clone layout (GObject refcount) to avoid a split-borrow conflict with
    // the later `&mut entry` access for `num_results_drawn`.
    let layout = entry.pango.layout.clone();

    cr.save().unwrap_or_default();

    // ── Prompt ───────────────────────────────────────────────────────────────
    let mut ink = pango::Rectangle::new(0, 0, 0, 0);
    let mut logical = pango::Rectangle::new(0, 0, 0, 0);

    let clip = ClipRect {
        x: entry.clip_x,
        y: entry.clip_y,
        width: entry.clip_width,
        height: entry.clip_height,
    };

    render_text_themed(
        cr,
        &layout,
        &entry.config.prompt_text.clone(),
        &entry.resolved_prompt_theme,
        clip,
        &mut ink,
        &mut logical,
    );

    // Advance past prompt.
    // logical comes from pixel_extents() — already in device pixels, no SCALE division.
    let prompt_adv = (logical.width() + logical.x()) as f64;
    cr.translate(prompt_adv + entry.config.prompt_padding as f64, 0.0);

    // ── Input field ──────────────────────────────────────────────────────────
    if entry.input.is_empty() {
        // Show placeholder.
        let placeholder = entry.config.placeholder_text.clone();
        let placeholder_theme = entry.resolved_placeholder_theme;
        let cursor = entry.resolved_cursor;
        render_input(
            cr,
            &layout,
            &placeholder,
            placeholder.chars().count(),
            &placeholder_theme,
            0,
            &cursor,
            &mut ink,
            &mut logical,
        );
    } else if entry.config.hide_input {
        // Replace each character with the hidden character.
        let hidden_char = entry.config.hidden_character.clone();
        let n_chars = entry.input.chars().count();
        let hidden: String = hidden_char.repeat(n_chars);
        let input_theme = entry.resolved_input_theme;
        let cursor_pos = entry.cursor_position;
        let cursor = entry.resolved_cursor;
        render_input(
            cr,
            &layout,
            &hidden,
            n_chars,
            &input_theme,
            cursor_pos,
            &cursor,
            &mut ink,
            &mut logical,
        );
    } else {
        let input_str = entry.input.clone();
        let n_chars = input_str.chars().count();
        let input_theme = entry.resolved_input_theme;
        let cursor_pos = entry.cursor_position;
        let cursor = entry.resolved_cursor;
        render_input(
            cr,
            &layout,
            &input_str,
            n_chars,
            &input_theme,
            cursor_pos,
            &cursor,
            &mut ink,
            &mut logical,
        );
    }

    // Enforce minimum input width (horizontal mode).
    // input_width is in logical pixels; logical.width() from pixel_extents() is also pixels.
    let min_w = entry.config.input_width as i32;
    if logical.width() < min_w {
        logical.set_width(min_w);
    }

    // ── Result list ──────────────────────────────────────────────────────────
    let fg = entry.config.foreground_color;
    cr.set_source_rgba(fg.r as f64, fg.g as f64, fg.b as f64, fg.a as f64);

    let num_results_cap = if entry.config.num_results == 0 {
        entry.results.len()
    } else {
        entry.config.num_results as usize
    };

    let horizontal = entry.config.horizontal;
    let result_spacing = entry.config.result_spacing as f64;
    let first = entry.first_result;
    let selection = entry.selection;
    let clip = ClipRect {
        x: entry.clip_x,
        y: entry.clip_y,
        width: entry.clip_width,
        height: entry.clip_height,
    };
    let selection_highlight_color = entry.config.selection_highlight_color;
    let num_results_fixed = entry.config.num_results;

    // We need owned copies to avoid borrow issues with &mut entry below.
    let results: Vec<String> = entry.results.clone();
    let default_result_theme = entry.resolved_default_result_theme;
    let alternate_result_theme = entry.resolved_alternate_result_theme;
    let sel_theme = entry.resolved_selection_theme;
    let input_str = entry.input.clone();

    let mut i = 0usize;
    while i < num_results_cap {
        // Translate to the next result position.
        if horizontal {
            let adv = (logical.x() + logical.width()) as f64 + result_spacing;
            cr.translate(adv, 0.0);
        } else {
            let adv = logical.height() as f64 + result_spacing;
            cr.translate(0.0, adv);
        }

        // Overflow check when num_results == 0 (auto-fit).
        if num_results_fixed == 0 && size_overflows(cr, clip, horizontal, 0.0, 0.0) {
            break;
        }

        let result_index = i + first;
        if result_index >= results.len() {
            break;
        }

        let str = &results[result_index];

        if i != selection || selection_highlight_color.a == 0.0 {
            // Normal or alternate-row theme.
            let theme = if i == selection {
                &sel_theme
            } else if result_index % 2 == 1 {
                &alternate_result_theme
            } else {
                &default_result_theme
            };

            if num_results_fixed > 0 {
                render_text_themed(cr, &layout, str, theme, clip, &mut ink, &mut logical);
            } else if !horizontal {
                let h_px = logical.height() as f64;
                if size_overflows(cr, clip, false, 0.0, h_px) {
                    entry.num_results_drawn = i;
                    cr.restore().unwrap_or_default();
                    return;
                }
                render_text_themed(cr, &layout, str, theme, clip, &mut ink, &mut logical);
            } else {
                // Horizontal auto-fit: speculatively render, then discard if overflow.
                cr.push_group();
                render_text_themed(cr, &layout, str, theme, clip, &mut ink, &mut logical);
                let group = cr.pop_group().unwrap();
                let w_px = logical.width() as f64;
                if size_overflows(cr, clip, true, w_px, 0.0) {
                    entry.num_results_drawn = i;
                    // Discard the group — do not paint.
                    cr.restore().unwrap_or_default();
                    return;
                }
                cr.save().unwrap_or_default();
                cr.set_source(&group).unwrap_or_default();
                cr.paint().unwrap_or_default();
                cr.restore().unwrap_or_default();
            }
        } else {
            // Selected result with input-match highlight.
            render_selected_result(
                cr,
                &layout,
                str,
                &input_str,
                &sel_theme,
                selection_highlight_color,
                &mut ink,
                &mut logical,
            );
        }

        i += 1;
    }

    entry.num_results_drawn = i;
    tracing::debug!("pango_backend::update drew {} results", i);

    cr.restore().unwrap_or_default();
}

// ── render_text_themed ────────────────────────────────────────────────────────

/// Render `text` at the current CTM origin using `theme`; update `ink` and
/// `logical` with the pixel extents.
///
/// If the theme has a non-transparent background, paint a rounded rectangle
/// behind the text first.
fn render_text_themed(
    cr: &Context,
    layout: &pango::Layout,
    text: &str,
    theme: &ResolvedTextTheme,
    clip: ClipRect,
    ink: &mut pango::Rectangle,
    logical: &mut pango::Rectangle,
) {
    let fg = theme.foreground_color;
    cr.set_source_rgba(fg.r as f64, fg.g as f64, fg.b as f64, fg.a as f64);

    layout.set_text(text);
    pangocairo::functions::update_layout(cr, layout);
    pangocairo::functions::show_layout(cr, layout);

    let (pi, pl) = layout.pixel_extents();
    *ink = pi;
    *logical = pl;

    if theme.background_color.a == 0.0 {
        return;
    }

    let padding = theme.padding;
    let mat = cr.matrix();
    let base_x = mat.x0() - clip.x as f64 + ink.x() as f64;
    let base_y = mat.y0() - clip.y as f64;

    let mut pad_left = padding.left as f64;
    let mut pad_right = padding.right as f64;
    let mut pad_top = padding.top as f64;
    let mut pad_bottom = padding.bottom as f64;

    if pad_left < 0.0 {
        pad_left = base_x;
    }
    if pad_right < 0.0 {
        pad_right = clip.width as f64 - ink.width() as f64 - base_x;
    }
    if pad_top < 0.0 {
        pad_top = base_y;
    }
    if pad_bottom < 0.0 {
        pad_bottom = clip.height as f64 - logical.height() as f64 - base_y;
    }

    cr.save().unwrap_or_default();
    let bg = theme.background_color;
    cr.set_source_rgba(bg.r as f64, bg.g as f64, bg.b as f64, bg.a as f64);
    cr.translate(-pad_left + ink.x() as f64, -pad_top);
    rounded_rectangle(
        cr,
        (ink.width() as f64 + pad_left + pad_right).ceil(),
        (logical.height() as f64 + pad_top + pad_bottom).ceil(),
        theme.background_corner_radius as f64,
    );
    cr.fill().unwrap_or_default();
    cr.restore().unwrap_or_default();

    // Re-draw text on top of background.
    cr.set_source_rgba(fg.r as f64, fg.g as f64, fg.b as f64, fg.a as f64);
    pangocairo::functions::show_layout(cr, layout);
}

// ── render_input ──────────────────────────────────────────────────────────────

/// Render the input field text and cursor at the current CTM origin.
#[allow(clippy::too_many_arguments)]
fn render_input(
    cr: &Context,
    layout: &pango::Layout,
    text: &str,
    text_char_len: usize,
    theme: &ResolvedTextTheme,
    cursor_position: usize,
    cursor: &ResolvedCursorTheme,
    ink: &mut pango::Rectangle,
    logical: &mut pango::Rectangle,
) {
    let fg = theme.foreground_color;
    cr.set_source_rgba(fg.r as f64, fg.g as f64, fg.b as f64, fg.a as f64);

    layout.set_text(text);
    pangocairo::functions::update_layout(cr, layout);
    pangocairo::functions::show_layout(cr, layout);
    let (pi, pl) = layout.pixel_extents();
    *ink = pi;
    *logical = pl;

    // Extra advance when the cursor is at the end (bar / block / underscore).
    let mut extra_cursor_advance = 0.0f64;
    if cursor_position == text_char_len && cursor.show {
        extra_cursor_advance = match cursor.style {
            CursorStyle::Bar => cursor.thickness as f64,
            CursorStyle::Block | CursorStyle::Underscore => cursor.em_width,
        };
        // Account for logical width vs ink width difference.
        extra_cursor_advance += logical.width() as f64 - logical.x() as f64 - ink.width() as f64;
    }

    // Background rectangle for the input field.
    if theme.background_color.a != 0.0 {
        let padding = theme.padding;
        cr.save().unwrap_or_default();
        let bg = theme.background_color;
        cr.set_source_rgba(bg.r as f64, bg.g as f64, bg.b as f64, bg.a as f64);
        cr.translate(
            f64::floor(-padding.left as f64 + ink.x() as f64),
            -padding.top as f64,
        );
        rounded_rectangle(
            cr,
            (ink.width() as f64
                + extra_cursor_advance
                + padding.left as f64
                + padding.right as f64)
                .ceil(),
            (logical.height() as f64 + padding.top as f64 + padding.bottom as f64).ceil(),
            theme.background_corner_radius as f64,
        );
        cr.fill().unwrap_or_default();
        cr.restore().unwrap_or_default();

        cr.set_source_rgba(fg.r as f64, fg.g as f64, fg.b as f64, fg.a as f64);
        pangocairo::functions::show_layout(cr, layout);
    }

    if !cursor.show {
        return;
    }

    // ── Cursor ────────────────────────────────────────────────────────────────
    let (cursor_x, cursor_width) = if cursor_position == text_char_len {
        let x = logical.width() as f64 + logical.x() as f64;
        (x, cursor.em_width)
    } else {
        // Convert char index to byte index for Pango.
        let mut byte_index = 0usize;
        for (ci, (bi, _ch)) in text.char_indices().enumerate() {
            if ci == cursor_position {
                byte_index = bi;
                break;
            }
        }
        let end_byte = text[byte_index..]
            .char_indices()
            .nth(1)
            .map(|(bi, _)| byte_index + bi)
            .unwrap_or(text.len());

        let (start_rect, _) = layout.cursor_pos(byte_index as i32);
        let (end_rect, _) = layout.cursor_pos(end_byte as i32);
        let cx = start_rect.x() as f64 / pango::SCALE as f64;
        let cw = (end_rect.x() - start_rect.x()) as f64 / pango::SCALE as f64;
        (cx, cw)
    };

    cr.save().unwrap_or_default();
    let cc = cursor.color;
    cr.set_source_rgba(cc.r as f64, cc.g as f64, cc.b as f64, cc.a as f64);
    cr.translate(cursor_x, 0.0);

    match cursor.style {
        CursorStyle::Bar => {
            rounded_rectangle(
                cr,
                cursor.thickness as f64,
                logical.height() as f64,
                cursor.corner_radius as f64,
            );
            cr.fill().unwrap_or_default();
        }
        CursorStyle::Block => {
            rounded_rectangle(
                cr,
                cursor_width,
                logical.height() as f64,
                cursor.corner_radius as f64,
            );
            cr.fill_preserve().unwrap_or_default();
            cr.clip();
            cr.translate(-cursor_x, 0.0);
            let tc = cursor.text_color;
            cr.set_source_rgba(tc.r as f64, tc.g as f64, tc.b as f64, tc.a as f64);
            pangocairo::functions::show_layout(cr, layout);
        }
        CursorStyle::Underscore => {
            cr.translate(0.0, cursor.underline_depth);
            rounded_rectangle(
                cr,
                cursor_width,
                cursor.thickness as f64,
                cursor.corner_radius as f64,
            );
            cr.fill().unwrap_or_default();
        }
    }

    logical.set_width(logical.width() + extra_cursor_advance as i32);
    cr.restore().unwrap_or_default();
}

// ── render_selected_result ───────────────────────────────────────────────────

/// Render the selected result with optional input-match highlight colouring.
///
/// Drawn in two passes: first the background rectangle, then the foreground
/// text (with differently-coloured match segment).
#[allow(clippy::too_many_arguments)]
fn render_selected_result(
    cr: &Context,
    layout: &pango::Layout,
    text: &str,
    input: &str,
    sel_theme: &ResolvedTextTheme,
    highlight_color: crate::color::Color,
    ink: &mut pango::Rectangle,
    logical: &mut pango::Rectangle,
) {
    // Find the position of the input match inside `text` (case-insensitive).
    let match_info = if !input.is_empty() && highlight_color.a > 0.0 {
        find_match_position(text, input)
    } else {
        None
    };

    // Two-pass render: pass 0 = draw background rect, pass 1 = draw text.
    // On pass 0 we gather extents; on pass 1 we paint text on top.
    for pass in 0..2 {
        cr.save().unwrap_or_default();

        let mut combined_ink = pango::Rectangle::new(0, 0, 0, 0);
        let mut combined_logical = pango::Rectangle::new(0, 0, 0, 0);
        let mut first_segment = true;

        // Draw each segment (pre-match, match, post-match).
        let segments: Vec<(&str, crate::color::Color)> = match match_info {
            None => vec![(text, sel_theme.foreground_color)],
            Some((pre_end, match_end)) => {
                let mut segs = Vec::new();
                if pre_end > 0 {
                    segs.push((&text[..pre_end], sel_theme.foreground_color));
                }
                segs.push((&text[pre_end..match_end], highlight_color));
                if match_end < text.len() {
                    segs.push((&text[match_end..], sel_theme.foreground_color));
                }
                segs
            }
        };

        for (seg_text, seg_color) in &segments {
            let c = *seg_color;
            cr.set_source_rgba(c.r as f64, c.g as f64, c.b as f64, c.a as f64);
            layout.set_text(seg_text);
            pangocairo::functions::update_layout(cr, layout);
            pangocairo::functions::show_layout(cr, layout);

            let (seg_ink, seg_logical) = layout.pixel_extents();

            if first_segment {
                combined_ink = seg_ink;
                combined_logical = seg_logical;
                first_segment = false;
            } else {
                // Extend combined extents horizontally.
                let new_iw =
                    combined_logical.width() - combined_ink.x() + seg_ink.x() + seg_ink.width();
                combined_ink.set_width(new_iw);
                combined_logical
                    .set_width(combined_logical.width() + seg_logical.x() + seg_logical.width());
            }

            // Advance CTM to the right for the next segment.
            let adv = seg_logical.x() as f64 + seg_logical.width() as f64;
            cr.translate(adv / pango::SCALE as f64, 0.0);
        }

        cr.restore().unwrap_or_default();
        *ink = combined_ink;
        *logical = combined_logical;

        // On pass 0, draw background rectangle if the selection theme has one.
        if pass == 0 {
            if sel_theme.background_color.a == 0.0 {
                break; // No background — single pass suffices.
            }
            let padding = sel_theme.padding;
            cr.save().unwrap_or_default();
            let bg = sel_theme.background_color;
            cr.set_source_rgba(bg.r as f64, bg.g as f64, bg.b as f64, bg.a as f64);
            cr.translate(
                f64::floor(-padding.left as f64 + ink.x() as f64),
                -(padding.top as f64),
            );
            rounded_rectangle(
                cr,
                (ink.width() as f64 + padding.left as f64 + padding.right as f64).ceil(),
                (logical.height() as f64 + padding.top as f64 + padding.bottom as f64).ceil(),
                sel_theme.background_corner_radius as f64,
            );
            cr.fill().unwrap_or_default();
            cr.restore().unwrap_or_default();
        }
    }
}

// ── size_overflows ────────────────────────────────────────────────────────────

/// Return `true` if a rectangle of `extra_w` × `extra_h` would overflow the
/// clip region in the current direction.
fn size_overflows(
    cr: &Context,
    clip: ClipRect,
    horizontal: bool,
    extra_w: f64,
    extra_h: f64,
) -> bool {
    let mat = cr.matrix();
    if horizontal {
        mat.x0() + extra_w > (clip.x + clip.width) as f64
    } else {
        mat.y0() + extra_h > (clip.y + clip.height) as f64
    }
}

// ── find_match_position ───────────────────────────────────────────────────────

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
