//! parley/vello_cpu text backend for [`super::Entry`].
//!
//! Implements: prompt, input field (with cursor), result list, selection
//! highlight.  All drawing uses **logical** pixel coordinates (the render
//! context transform handles physical-pixel mapping).
//!
//! Background boxes are sized from each layout's advance width and line
//! height; parley exposes no ink rectangle, so unlike the old Pango backend
//! glyph side bearings are not trimmed.

use std::borrow::Cow;
use std::ops::Range;

use parley::setting::Tag;
use parley::{
    Affinity, Cursor, FontContext, FontFamily, FontFamilyName, FontFeature, FontFeatures,
    FontStyle, FontVariation, FontVariations, FontWeight, FontWidth, GenericFamily, Layout,
    LayoutContext, LineHeight, PositionedLayoutItem, StyleProperty,
};
use vello_cpu::kurbo::{Rect, RoundedRect, Shape};
use vello_cpu::{RenderContext, Resources};

use crate::Result;
use crate::color::Color;
use crate::renderer::{PT_TO_PX, draw_layout, paint};

use super::{CursorStyle, Entry, EntryConfig, ResolvedCursorTheme, ResolvedTextTheme};

// ── FontSpec ──────────────────────────────────────────────────────────────────

/// Font selection parsed from a Pango-style description string.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FontSpec {
    pub families: Vec<FontFamilyName<'static>>,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub width: FontWidth,
}

/// Parse a Pango-style font description: `"FAMILY-LIST [STYLE-OPTIONS] [SIZE]"`.
///
/// The family list is comma-separated.  Trailing style words (`Bold`,
/// `Italic`, `Condensed`, …) set weight / style / width; a trailing size is
/// ignored because the font size is configured separately.  Generic names
/// (`Sans`, `Serif`, `Monospace`, …) map to the CSS generic families, which
/// fontique resolves through fontconfig.
pub(super) fn parse_font_description(desc: &str) -> FontSpec {
    let mut weight = FontWeight::NORMAL;
    let mut style = FontStyle::Normal;
    let mut width = FontWidth::NORMAL;

    let mut words: Vec<&str> = desc.split_whitespace().collect();
    // Consume options from the end, always leaving at least one family word.
    while words.len() > 1 {
        let word = words[words.len() - 1];
        if word.ends_with(',') {
            break;
        }
        let lower = word.to_ascii_lowercase();
        let lower = lower.as_str();
        if lower.trim_end_matches("px").parse::<f32>().is_ok() {
            // Size: configured separately via `font_size`.
        } else if let Some(w) = parse_weight(lower) {
            weight = w;
        } else if let Some(w) = parse_width(lower) {
            width = w;
        } else {
            match lower {
                "italic" => style = FontStyle::Italic,
                "oblique" => style = FontStyle::Oblique(None),
                "normal" | "roman" | "regular" => {}
                _ => break,
            }
        }
        words.pop();
    }

    let families = words
        .join(" ")
        .split(',')
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .map(family_name)
        .collect();

    FontSpec {
        families,
        weight,
        style,
        width,
    }
}

fn parse_weight(word: &str) -> Option<FontWeight> {
    Some(match word {
        "thin" => FontWeight::THIN,
        "ultra-light" | "ultralight" | "extra-light" | "extralight" => FontWeight::EXTRA_LIGHT,
        "light" => FontWeight::LIGHT,
        "semi-light" | "semilight" | "demi-light" | "demilight" => FontWeight::SEMI_LIGHT,
        "book" => FontWeight::new(380.0),
        "medium" => FontWeight::MEDIUM,
        "semi-bold" | "semibold" | "demi-bold" | "demibold" => FontWeight::SEMI_BOLD,
        "bold" => FontWeight::BOLD,
        "ultra-bold" | "ultrabold" | "extra-bold" | "extrabold" => FontWeight::EXTRA_BOLD,
        "heavy" | "black" => FontWeight::BLACK,
        "ultra-heavy" | "ultraheavy" | "ultra-black" | "ultrablack" | "extra-black"
        | "extrablack" => FontWeight::EXTRA_BLACK,
        _ => return None,
    })
}

fn parse_width(word: &str) -> Option<FontWidth> {
    Some(match word {
        "ultra-condensed" | "ultracondensed" => FontWidth::ULTRA_CONDENSED,
        "extra-condensed" | "extracondensed" => FontWidth::EXTRA_CONDENSED,
        "condensed" => FontWidth::CONDENSED,
        "semi-condensed" | "semicondensed" => FontWidth::SEMI_CONDENSED,
        "semi-expanded" | "semiexpanded" => FontWidth::SEMI_EXPANDED,
        "expanded" => FontWidth::EXPANDED,
        "extra-expanded" | "extraexpanded" => FontWidth::EXTRA_EXPANDED,
        "ultra-expanded" | "ultraexpanded" => FontWidth::ULTRA_EXPANDED,
        _ => return None,
    })
}

fn family_name(name: &str) -> FontFamilyName<'static> {
    let generic = match name.to_ascii_lowercase().as_str() {
        "sans" | "sans-serif" => Some(GenericFamily::SansSerif),
        "serif" => Some(GenericFamily::Serif),
        "mono" | "monospace" => Some(GenericFamily::Monospace),
        "cursive" => Some(GenericFamily::Cursive),
        "fantasy" => Some(GenericFamily::Fantasy),
        "system-ui" => Some(GenericFamily::SystemUi),
        "emoji" => Some(GenericFamily::Emoji),
        "math" => Some(GenericFamily::Math),
        _ => None,
    };
    match generic {
        Some(g) => FontFamilyName::Generic(g),
        None => FontFamilyName::Named(Cow::Owned(name.to_owned())),
    }
}

// ── OpenType settings ─────────────────────────────────────────────────────────

/// Split a `"tag=value, tag value, …"` list into `(tag, value)` pairs.
///
/// A bare value is `None`.  Invalid tags are logged and skipped.
fn parse_settings(list: &str) -> Vec<(Tag, Option<&str>)> {
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .filter_map(|item| {
            let (tag, value) = match item.split_once(['=', ' ']) {
                Some((t, v)) => (t.trim(), Some(v.trim())),
                None => (item, None),
            };
            let tag = tag.trim_matches(['"', '\'']);
            let parsed = Tag::parse(tag);
            if parsed.is_none() {
                tracing::warn!("ignoring invalid OpenType tag {tag:?} in {list:?}");
            }
            parsed.map(|t| (t, value))
        })
        .collect()
}

/// Parse a font feature list such as `"liga=0, calt, -kern, +smcp"`.
///
/// `tag` and `+tag` enable a feature, `-tag` disables it, and `tag=N` (or
/// `tag N`) sets an explicit value; `on`/`off` are accepted as values.
pub(super) fn parse_font_features(list: &str) -> Vec<FontFeature> {
    let list = list.trim();
    let mut out = Vec::new();
    for item in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (sign, rest) = match item.as_bytes()[0] {
            b'-' => (Some(0), &item[1..]),
            b'+' => (Some(1), &item[1..]),
            _ => (None, item),
        };
        for (tag, value) in parse_settings(rest) {
            let value = match (sign, value) {
                (Some(v), _) => Some(v),
                (None, None | Some("on" | "true")) => Some(1),
                (None, Some("off" | "false")) => Some(0),
                (None, Some(v)) => v.parse::<u16>().ok(),
            };
            match value {
                Some(v) => out.push(FontFeature::new(tag, v)),
                None => tracing::warn!("ignoring invalid font feature {item:?}"),
            }
        }
    }
    out
}

/// Parse a variation list such as `"wght=700, wdth=75"`.
pub(super) fn parse_font_variations(list: &str) -> Vec<FontVariation> {
    parse_settings(list)
        .into_iter()
        .filter_map(|(tag, value)| {
            let parsed = value.and_then(|v| v.parse::<f32>().ok());
            if parsed.is_none() {
                tracing::warn!("ignoring font variation {tag} without a numeric value");
            }
            parsed.map(|v| FontVariation::new(tag, v))
        })
        .collect()
}

// ── TextBackend ───────────────────────────────────────────────────────────────

/// Font database, layout scratch space and resolved font settings, held by
/// [`Entry`] for the lifetime of the widget.
pub struct TextBackend {
    font_cx: FontContext,
    layout_cx: LayoutContext<Color>,
    font: FontSpec,
    font_size: f32,
    /// Fixed line height; `None` until measured in [`TextBackend::init`].
    line_height: Option<f32>,
    features: Vec<FontFeature>,
    variations: Vec<FontVariation>,
}

impl TextBackend {
    /// Discover system fonts, resolve the configured font, and compute
    /// cursor metrics.
    ///
    /// Returns `(TextBackend, ResolvedCursorTheme)`.
    pub(crate) fn init(
        config: &EntryConfig,
        resolved_input: &ResolvedTextTheme,
        default_bg: Color,
    ) -> Result<(Self, ResolvedCursorTheme)> {
        let mut backend = Self {
            font_cx: FontContext::new(),
            layout_cx: LayoutContext::new(),
            font: parse_font_description(&config.font_name),
            font_size: config.font_size as f32 * PT_TO_PX,
            line_height: None,
            features: parse_font_features(&config.font_features),
            variations: parse_font_variations(&config.font_variations),
        };

        // Measure font metrics for cursor sizing from a laid-out 'm', whose
        // advance doubles as the block / underscore cursor width.
        let probe = backend.layout("m", Color::default(), None);
        let metrics = probe
            .lines()
            .flat_map(|line| line.items())
            .find_map(|item| match item {
                PositionedLayoutItem::GlyphRun(run) => Some(*run.run().font_metrics()),
                PositionedLayoutItem::InlineBox(_) => None,
            })
            .unwrap_or_else(|| {
                tracing::warn!("no usable font found for {:?}", config.font_name);
                parley::FontMetrics::fallback(backend.font_size)
            });

        // Match Pango's line height: hinted (whole-pixel) ascent + descent,
        // without the font's line gap.  This keeps rows the same height for
        // every font fallback, as they were with Pango.
        backend.line_height = Some(metrics.ascent.ceil() + metrics.descent.ceil());
        let probe = backend.layout("m", Color::default(), None);
        let baseline = probe
            .lines()
            .next()
            .map_or(metrics.ascent, |line| line.metrics().baseline);

        let em_width = probe.width() as f64;
        // `underline_offset` is negative below the baseline.
        let underline_depth = (baseline - metrics.underline_offset) as f64;

        // Cursor thickness: use font underline thickness for UNDERSCORE style
        // when not overridden; default 2 px otherwise.
        let thickness = match &config.cursor_theme.thickness {
            Some(t) => *t,
            None => {
                if config.cursor_theme.style == CursorStyle::Underscore {
                    (metrics.underline_size.round() as u32).max(1)
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
            "TextBackend::init font={:?} em_width={em_width:.2} underline_depth={underline_depth:.2}",
            config.font_name,
        );

        Ok((backend, resolved_cursor))
    }

    /// Lay out a single line of `text` in `color`, optionally painting the
    /// byte range of `highlight` in a second color.
    pub(crate) fn layout(
        &mut self,
        text: &str,
        color: Color,
        highlight: Option<(Range<usize>, Color)>,
    ) -> Layout<Color> {
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(FontFamily::List(Cow::Borrowed(
            &self.font.families,
        ))));
        builder.push_default(StyleProperty::FontSize(self.font_size));
        if let Some(height) = self.line_height {
            builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(height)));
        }
        builder.push_default(StyleProperty::FontWeight(self.font.weight));
        builder.push_default(StyleProperty::FontStyle(self.font.style));
        builder.push_default(StyleProperty::FontWidth(self.font.width));
        builder.push_default(StyleProperty::Brush(color));
        if !self.features.is_empty() {
            builder.push_default(StyleProperty::FontFeatures(FontFeatures::List(
                Cow::Borrowed(&self.features),
            )));
        }
        if !self.variations.is_empty() {
            builder.push_default(StyleProperty::FontVariations(FontVariations::List(
                Cow::Borrowed(&self.variations),
            )));
        }
        if let Some((range, hl)) = highlight {
            builder.push(StyleProperty::Brush(hl), range);
        }

        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout
    }
}

// ── Painter ───────────────────────────────────────────────────────────────────

/// Clip rectangle in logical pixels, as recorded by [`Entry::new`].
#[derive(Clone, Copy)]
struct ClipRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl ClipRect {
    /// Return `true` if a box of `extra_w` × `extra_h` at `pen` would
    /// overflow the clip region in the current direction.
    fn overflows(&self, pen: (f64, f64), horizontal: bool, extra_w: f64, extra_h: f64) -> bool {
        if horizontal {
            pen.0 + extra_w > self.x + self.width
        } else {
            pen.1 + extra_h > self.y + self.height
        }
    }
}

/// Logical size of a drawn element.
#[derive(Clone, Copy, Default)]
struct Extents {
    width: f64,
    height: f64,
}

impl Extents {
    /// Size of `layout`, rounded up to whole pixels as Pango's pixel extents
    /// were.  This keeps row spacing unchanged and every pen position (and so
    /// every baseline) on the pixel grid.
    fn of(layout: &Layout<Color>) -> Self {
        Self {
            width: f64::from(layout.width()).ceil(),
            height: f64::from(layout.height()).ceil(),
        }
    }
}

/// Mutable drawing state borrowed from [`Entry`] for one frame.
struct Painter<'a> {
    ctx: &'a mut RenderContext,
    resources: &'a mut Resources,
    text: &'a mut TextBackend,
    clip: ClipRect,
}

impl Painter<'_> {
    fn fill_rounded_rect(&mut self, rect: Rect, radius: f64, color: Color) {
        self.ctx.set_paint(paint(color));
        if radius <= 0.0 {
            self.ctx.fill_rect(&rect);
        } else {
            self.ctx
                .fill_path(&RoundedRect::from_rect(rect, radius).to_path(0.1));
        }
    }

    fn draw(&mut self, layout: &Layout<Color>, pen: (f64, f64), color_override: Option<Color>) {
        draw_layout(self.ctx, self.resources, layout, pen, color_override);
    }

    /// Draw `layout` at `pen` using `theme`'s background box.
    ///
    /// Negative theme padding means "extend to the clip edge" on that side.
    fn render_text_themed(
        &mut self,
        layout: &Layout<Color>,
        theme: &ResolvedTextTheme,
        pen: (f64, f64),
    ) -> Extents {
        let ext = Extents::of(layout);

        if theme.background_color.a != 0.0 {
            let clip = self.clip;
            let padding = theme.padding;
            let base_x = pen.0 - clip.x;
            let base_y = pen.1 - clip.y;

            let mut pad_left = padding.left as f64;
            let mut pad_right = padding.right as f64;
            let mut pad_top = padding.top as f64;
            let mut pad_bottom = padding.bottom as f64;

            if pad_left < 0.0 {
                pad_left = base_x;
            }
            if pad_right < 0.0 {
                pad_right = clip.width - ext.width - base_x;
            }
            if pad_top < 0.0 {
                pad_top = base_y;
            }
            if pad_bottom < 0.0 {
                pad_bottom = clip.height - ext.height - base_y;
            }

            let (x, y) = (pen.0 - pad_left, pen.1 - pad_top);
            let rect = Rect::new(
                x,
                y,
                x + (ext.width + pad_left + pad_right).ceil(),
                y + (ext.height + pad_top + pad_bottom).ceil(),
            );
            self.fill_rounded_rect(
                rect,
                theme.background_corner_radius as f64,
                theme.background_color,
            );
        }

        self.draw(layout, pen, None);
        ext
    }

    /// Render the input field text and cursor at `pen`.
    fn render_input(
        &mut self,
        text: &str,
        text_char_len: usize,
        theme: &ResolvedTextTheme,
        cursor_position: usize,
        cursor: &ResolvedCursorTheme,
        pen: (f64, f64),
    ) -> Extents {
        let layout = self.text.layout(text, theme.foreground_color, None);
        let mut ext = Extents::of(&layout);

        // Extra advance when the cursor is at the end (bar / block / underscore).
        let mut extra_cursor_advance = 0.0f64;
        if cursor_position == text_char_len && cursor.show {
            extra_cursor_advance = match cursor.style {
                CursorStyle::Bar => cursor.thickness as f64,
                CursorStyle::Block | CursorStyle::Underscore => cursor.em_width,
            };
        }

        // Background rectangle for the input field.
        if theme.background_color.a != 0.0 {
            let padding = theme.padding;
            let x = (pen.0 - padding.left as f64).floor();
            let y = pen.1 - padding.top as f64;
            let rect = Rect::new(
                x,
                y,
                x + (ext.width + extra_cursor_advance + (padding.left + padding.right) as f64)
                    .ceil(),
                y + (ext.height + (padding.top + padding.bottom) as f64).ceil(),
            );
            self.fill_rounded_rect(
                rect,
                theme.background_corner_radius as f64,
                theme.background_color,
            );
        }

        self.draw(&layout, pen, None);

        if !cursor.show {
            return ext;
        }

        // ── Cursor ────────────────────────────────────────────────────────────
        let (cursor_x, cursor_width) = if cursor_position == text_char_len {
            (ext.width, cursor.em_width)
        } else {
            // Convert char index to byte index for parley.
            let byte_index = text
                .char_indices()
                .nth(cursor_position)
                .map_or(0, |(bi, _)| bi);
            let end_byte = text[byte_index..]
                .char_indices()
                .nth(1)
                .map_or(text.len(), |(bi, _)| byte_index + bi);

            let x_at = |index| {
                Cursor::from_byte_index(&layout, index, Affinity::Downstream)
                    .geometry(&layout, 0.0)
                    .x0
            };
            let start = x_at(byte_index);
            (start, x_at(end_byte) - start)
        };

        let x = pen.0 + cursor_x;
        let radius = cursor.corner_radius as f64;
        match cursor.style {
            CursorStyle::Bar => {
                let rect = Rect::new(x, pen.1, x + cursor.thickness as f64, pen.1 + ext.height);
                self.fill_rounded_rect(rect, radius, cursor.color);
            }
            CursorStyle::Block => {
                let rect = Rect::new(x, pen.1, x + cursor_width, pen.1 + ext.height);
                self.fill_rounded_rect(rect, radius, cursor.color);
                // Re-draw the text under the block in the cursor text color.
                self.ctx
                    .push_clip_path(&RoundedRect::from_rect(rect, radius).to_path(0.1));
                self.draw(&layout, pen, Some(cursor.text_color));
                self.ctx.pop_clip();
            }
            CursorStyle::Underscore => {
                let y = pen.1 + cursor.underline_depth;
                let rect = Rect::new(x, y, x + cursor_width, y + cursor.thickness as f64);
                self.fill_rounded_rect(rect, radius, cursor.color);
            }
        }

        ext.width += extra_cursor_advance;
        ext
    }

    /// Render the selected result with optional input-match highlight colouring.
    fn render_selected_result(
        &mut self,
        text: &str,
        input: &str,
        sel_theme: &ResolvedTextTheme,
        highlight_color: Color,
        pen: (f64, f64),
    ) -> Extents {
        // Find the position of the input match inside `text` (case-insensitive).
        let highlight = if !input.is_empty() && highlight_color.a > 0.0 {
            find_match_position(text, input)
                .filter(|&(start, end)| {
                    end <= text.len() && text.is_char_boundary(start) && text.is_char_boundary(end)
                })
                .map(|(start, end)| (start..end, highlight_color))
        } else {
            None
        };

        let layout = self
            .text
            .layout(text, sel_theme.foreground_color, highlight);
        let ext = Extents::of(&layout);

        if sel_theme.background_color.a != 0.0 {
            let padding = sel_theme.padding;
            let x = (pen.0 - padding.left as f64).floor();
            let y = pen.1 - padding.top as f64;
            let rect = Rect::new(
                x,
                y,
                x + (ext.width + (padding.left + padding.right) as f64).ceil(),
                y + (ext.height + (padding.top + padding.bottom) as f64).ceil(),
            );
            self.fill_rounded_rect(
                rect,
                sel_theme.background_corner_radius as f64,
                sel_theme.background_color,
            );
        }

        self.draw(&layout, pen, None);
        ext
    }
}

// ── update ────────────────────────────────────────────────────────────────────

/// Render all text elements into the entry's render context.
///
/// Called by [`Entry::update`] after the background and clip are set up.
pub(crate) fn update(entry: &mut Entry) {
    let clip = ClipRect {
        x: entry.clip_x as f64,
        y: entry.clip_y as f64,
        width: entry.clip_width as f64,
        height: entry.clip_height as f64,
    };
    let mut p = Painter {
        ctx: &mut entry.ctx,
        resources: &mut entry.resources,
        text: &mut entry.text,
        clip,
    };
    let mut pen = entry.text_origin;

    // ── Prompt ───────────────────────────────────────────────────────────────
    let prompt_theme = entry.resolved_prompt_theme;
    let layout = p.text.layout(
        &entry.config.prompt_text,
        prompt_theme.foreground_color,
        None,
    );
    let ext = p.render_text_themed(&layout, &prompt_theme, pen);

    // Advance past prompt.
    pen.0 += ext.width + entry.config.prompt_padding as f64;

    // ── Input field ──────────────────────────────────────────────────────────
    let cursor = entry.resolved_cursor;
    let mut ext = if entry.input.is_empty() {
        // Show placeholder.
        let placeholder = &entry.config.placeholder_text;
        p.render_input(
            placeholder,
            placeholder.chars().count(),
            &entry.resolved_placeholder_theme,
            0,
            &cursor,
            pen,
        )
    } else if entry.config.hide_input {
        // Replace each character with the hidden character.
        let n_chars = entry.input.chars().count();
        let hidden = entry.config.hidden_character.repeat(n_chars);
        p.render_input(
            &hidden,
            n_chars,
            &entry.resolved_input_theme,
            entry.cursor_position,
            &cursor,
            pen,
        )
    } else {
        p.render_input(
            &entry.input,
            entry.input.chars().count(),
            &entry.resolved_input_theme,
            entry.cursor_position,
            &cursor,
            pen,
        )
    };

    // Enforce minimum input width (horizontal mode).
    ext.width = ext.width.max(entry.config.input_width as f64);

    // ── Result list ──────────────────────────────────────────────────────────
    let num_results_cap = if entry.config.num_results == 0 {
        entry.results.len()
    } else {
        entry.config.num_results as usize
    };

    let horizontal = entry.config.horizontal;
    let result_spacing = entry.config.result_spacing as f64;
    let num_results_fixed = entry.config.num_results;
    let selection_highlight_color = entry.config.selection_highlight_color;

    let mut i = 0usize;
    while i < num_results_cap {
        // Advance to the next result position.
        if horizontal {
            pen.0 += ext.width + result_spacing;
        } else {
            pen.1 += ext.height + result_spacing;
        }

        // Overflow check when num_results == 0 (auto-fit).
        if num_results_fixed == 0 && clip.overflows(pen, horizontal, 0.0, 0.0) {
            break;
        }

        let result_index = i + entry.first_result;
        let Some(result) = entry.results.get(result_index) else {
            break;
        };

        if i != entry.selection || selection_highlight_color.a == 0.0 {
            // Normal or alternate-row theme.
            let theme = if i == entry.selection {
                entry.resolved_selection_theme
            } else if result_index % 2 == 1 {
                entry.resolved_alternate_result_theme
            } else {
                entry.resolved_default_result_theme
            };

            // Auto-fit: stop before a result that would not fit.  Vertical
            // mode checks the previous row's height, horizontal mode the
            // new result's width.
            if num_results_fixed == 0 && !horizontal && clip.overflows(pen, false, 0.0, ext.height)
            {
                break;
            }
            let layout = p.text.layout(result, theme.foreground_color, None);
            if num_results_fixed == 0
                && horizontal
                && clip.overflows(pen, true, layout.width() as f64, 0.0)
            {
                break;
            }
            ext = p.render_text_themed(&layout, &theme, pen);
        } else {
            // Selected result with input-match highlight.
            ext = p.render_selected_result(
                result,
                &entry.input,
                &entry.resolved_selection_theme,
                selection_highlight_color,
                pen,
            );
        }

        i += 1;
    }

    entry.num_results_drawn = i;
    tracing::debug!("text::update drew {} results", i);
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
