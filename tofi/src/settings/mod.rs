//! Final merged settings: CLI > config > default, theme > default.
//!
//! [`build`] combines parsed [`Config`], [`Theme`], and [`Cli`] into a
//! concrete [`Settings`] struct that the rest of the application uses.

use libtofi_rs::color::Color;
use libtofi_rs::matching::MatchingAlgorithm;

use crate::cli::Cli;
use crate::config::Config;
use crate::theme::{
    self, Anchor, Cursor, CursorKind, TextStyle, Theme, UnitValue, dim_to_unit, parse_anchor,
    parse_cursor_kind, parse_hex_color,
};

// ── LaunchMode ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    Run,
    Drun,
    Dmenu,
}

pub(crate) fn detect_mode(mode: Option<&str>) -> LaunchMode {
    match mode {
        Some("run") => LaunchMode::Run,
        Some("dmenu") => LaunchMode::Dmenu,
        Some("drun") => LaunchMode::Drun,
        None => LaunchMode::Drun,
        _ => LaunchMode::Drun,
    }
}

// ── Settings ──────────────────────────────────────────────────────────────────

/// Fully resolved runtime settings passed to the Wayland engine.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    // Behavioral (cli > config > default)
    pub mode: LaunchMode,
    pub target_output: String,
    pub default_terminal: Option<String>,
    pub algorithm: MatchingAlgorithm,
    pub require_match: bool,
    pub ascii_input: bool,
    pub use_history: bool,
    pub history_file: Option<String>,

    // Visual (theme > default)
    pub width: UnitValue,
    pub height: UnitValue,
    pub anchor: Anchor,
    pub margin_top: UnitValue,
    pub margin_bottom: UnitValue,
    pub margin_left: UnitValue,
    pub margin_right: UnitValue,
    pub scale: bool,

    pub font: String,
    pub font_size: u32,
    pub font_features: String,
    pub font_variations: String,
    pub hint_font: bool,

    pub background_color: Color,
    pub foreground_color: Color,
    pub border_color: Color,
    pub outline_color: Color,
    pub selection_highlight_color: Color,
    pub corner_radius: u32,
    pub border_width: u32,
    pub outline_width: u32,

    pub padding_top: UnitValue,
    pub padding_bottom: UnitValue,
    pub padding_left: UnitValue,
    pub padding_right: UnitValue,
    pub clip_to_padding: bool,

    pub prompt_text: String,
    pub prompt_padding: u32,
    pub num_results: u32,
    pub result_spacing: u32,
    pub horizontal: bool,

    pub hide_cursor: bool,
    pub hide_input: bool,
    pub hidden_character: char,

    pub cursor: Cursor,
    pub prompt_style: TextStyle,
    pub input_style: TextStyle,
    pub default_result_style: TextStyle,
    pub alternate_result_style: TextStyle,
    pub selection_style: TextStyle,
}

// ── Build ─────────────────────────────────────────────────────────────────────

/// Merge CLI overrides, config file, and theme into concrete [`Settings`].
pub fn build(cli: &Cli, config: &Config, theme: &Theme) -> Settings {
    // ── Visual defaults ───────────────────────────────────────────────────────
    let width = theme
        .window
        .width
        .as_ref()
        .map(dim_to_unit)
        .unwrap_or(UnitValue::pixels(1280));
    let height = theme
        .window
        .height
        .as_ref()
        .map(dim_to_unit)
        .unwrap_or(UnitValue::pixels(720));
    let anchor = theme
        .window
        .anchor
        .as_deref()
        .map(parse_anchor)
        .unwrap_or(Anchor::Center);
    let margin_top = theme
        .window
        .margin_top
        .map(UnitValue::pixels)
        .unwrap_or(UnitValue::pixels(0));
    let margin_bottom = theme
        .window
        .margin_bottom
        .map(UnitValue::pixels)
        .unwrap_or(UnitValue::pixels(0));
    let margin_left = theme
        .window
        .margin_left
        .map(UnitValue::pixels)
        .unwrap_or(UnitValue::pixels(0));
    let margin_right = theme
        .window
        .margin_right
        .map(UnitValue::pixels)
        .unwrap_or(UnitValue::pixels(0));
    let scale = theme.window.scale.unwrap_or(true);

    let font = theme.font.name.clone().unwrap_or_else(|| "Sans".to_owned());
    let font_size = theme.font.size.unwrap_or(24);
    let font_features = theme.font.features.clone().unwrap_or_default();
    let font_variations = theme.font.variations.clone().unwrap_or_default();
    let hint_font = theme.font.hint.unwrap_or(true);

    let background_color = theme
        .window
        .background_color
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(Color {
            r: 0.106,
            g: 0.114,
            b: 0.118,
            a: 1.0,
        });
    let foreground_color = theme
        .text
        .color
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });
    let border_color = theme
        .window
        .border_color
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(Color {
            r: 0.976,
            g: 0.149,
            b: 0.447,
            a: 1.0,
        });
    let outline_color = theme
        .window
        .outline_color
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(Color {
            r: 0.031,
            g: 0.031,
            b: 0.0,
            a: 1.0,
        });
    let selection_highlight_color = theme
        .text
        .selection_match_color
        .as_deref()
        .and_then(parse_hex_color)
        .unwrap_or(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
    let corner_radius = theme.window.corner_radius.unwrap_or(0);
    let border_width = theme.window.border_width.unwrap_or(12);
    let outline_width = theme.window.outline_width.unwrap_or(4);

    let padding_top = theme
        .window
        .padding_top
        .as_ref()
        .map(dim_to_unit)
        .unwrap_or(UnitValue::pixels(8));
    let padding_bottom = theme
        .window
        .padding_bottom
        .as_ref()
        .map(dim_to_unit)
        .unwrap_or(UnitValue::pixels(8));
    let padding_left = theme
        .window
        .padding_left
        .as_ref()
        .map(dim_to_unit)
        .unwrap_or(UnitValue::pixels(8));
    let padding_right = theme
        .window
        .padding_right
        .as_ref()
        .map(dim_to_unit)
        .unwrap_or(UnitValue::pixels(8));
    let clip_to_padding = theme.window.clip_to_padding.unwrap_or(true);

    let prompt_text = theme
        .prompt
        .text
        .clone()
        .or_else(|| theme.text.prompt.clone())
        .unwrap_or_else(|| "run: ".to_owned());
    let prompt_padding = theme.prompt.padding.unwrap_or(0);
    let num_results = theme.results.count.unwrap_or(0);
    let result_spacing = theme.results.spacing.unwrap_or(0);
    let horizontal = theme
        .results
        .mode
        .as_deref()
        .map(|m| m.eq_ignore_ascii_case("horizontal"))
        .unwrap_or(false);

    let hide_cursor = theme.cursor.hide.unwrap_or(false);
    let hide_input = theme.input.hide.unwrap_or(false);
    let hidden_character = theme
        .input
        .hidden_character
        .as_deref()
        .and_then(|s| s.chars().next())
        .unwrap_or('*');

    let cursor = Cursor {
        show: theme.input.cursor.unwrap_or(false),
        kind: theme
            .input
            .cursor_style
            .as_deref()
            .map(parse_cursor_kind)
            .unwrap_or(CursorKind::Bar),
        color: theme
            .input
            .cursor_color
            .as_deref()
            .and_then(parse_hex_color),
        text_color: theme
            .input
            .cursor_background
            .as_deref()
            .and_then(parse_hex_color),
        corner_radius: theme.input.cursor_corner_radius.unwrap_or(0),
        thickness: theme.input.cursor_thickness,
    };

    let prompt_style = TextStyle {
        foreground_color: theme.text.prompt_color.as_deref().and_then(parse_hex_color),
        ..TextStyle::default()
    };
    let input_style = TextStyle {
        foreground_color: theme.text.input_color.as_deref().and_then(parse_hex_color),
        ..TextStyle::default()
    };
    let default_result_style = TextStyle {
        foreground_color: theme.text.match_color.as_deref().and_then(parse_hex_color),
        ..TextStyle::default()
    };
    let alternate_result_style = TextStyle {
        foreground_color: theme.text.match_color.as_deref().and_then(parse_hex_color),
        ..TextStyle::default()
    };
    let selection_style = TextStyle {
        foreground_color: theme
            .text
            .selection_color
            .as_deref()
            .and_then(parse_hex_color)
            .or(Some(Color {
                r: 0.976,
                g: 0.149,
                b: 0.447,
                a: 1.0,
            })),
        ..TextStyle::default()
    };

    // ── Behavioral: cli > config > default ────────────────────────────────────
    let mode = detect_mode(cli.mode.as_deref().or(config.base.mode.as_deref()));

    let target_output = cli
        .output
        .clone()
        .or_else(|| config.base.output.clone().filter(|s| !s.is_empty()))
        .unwrap_or_default();

    let default_terminal = cli
        .terminal
        .clone()
        .or_else(|| config.base.terminal.clone().filter(|s| !s.is_empty()));

    let algorithm = cli
        .algorithm
        .as_deref()
        .or(config.matching.algorithm.as_deref())
        .map(theme::parse_algorithm)
        .unwrap_or(MatchingAlgorithm::Normal);

    let require_match = config.matching.require.unwrap_or(true);

    let ascii_input = config.matching.ascii.unwrap_or(false);

    let use_history = cli.history.or(config.history.history).unwrap_or(true);

    let history_file = config.history.path.clone();

    Settings {
        mode,
        target_output,
        default_terminal,
        algorithm,
        require_match,
        ascii_input,
        use_history,
        history_file,
        width,
        height,
        anchor,
        margin_top,
        margin_bottom,
        margin_left,
        margin_right,
        scale,
        font,
        font_size,
        font_features,
        font_variations,
        hint_font,
        background_color,
        foreground_color,
        border_color,
        outline_color,
        selection_highlight_color,
        corner_radius,
        border_width,
        outline_width,
        padding_top,
        padding_bottom,
        padding_left,
        padding_right,
        clip_to_padding,
        prompt_text,
        prompt_padding,
        num_results,
        result_spacing,
        horizontal,
        hide_cursor,
        hide_input,
        hidden_character,
        cursor,
        prompt_style,
        input_style,
        default_result_style,
        alternate_result_style,
        selection_style,
    }
}

#[cfg(test)]
mod tests;
