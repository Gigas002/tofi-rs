//! Config data model for `tofi`.
//!
//! This module defines [`TofiConfig`] (and its supporting value types) as **plain data**.
//! There is **no** file I/O or argument parsing here — the file loader lives in
//! [`crate::config::load()`] and CLI parsing in [`crate::cli`].
//!
//! # Defaults
//!
//! [`TofiConfig::default()`] reflects the defaults documented in
//! [`doc/config`](../../../doc/config).

pub mod apply;
pub mod load;
pub mod types;

pub use apply::apply_key;
pub use load::{ParseError, default_config_path, load};
pub use types::*;

impl Default for types::TofiConfig {
    fn default() -> Self {
        use libtofi_rs::color::Color;
        use types::*;
        Self {
            // Window geometry
            width: UnitValue::pixels(1280),
            height: UnitValue::pixels(720),
            anchor: Anchor::Center,
            exclusive_zone: -1,
            exclusive_zone_is_percent: false,
            margin_top: UnitValue::pixels(0),
            margin_bottom: UnitValue::pixels(0),
            margin_left: UnitValue::pixels(0),
            margin_right: UnitValue::pixels(0),
            scale: true,
            target_output: String::new(),

            // Font
            font: String::from("Sans"),
            font_size: 24,
            font_features: String::new(),
            font_variations: String::new(),
            hint_font: true,

            // Colors / decoration
            background_color: Color {
                r: 0.106,
                g: 0.114,
                b: 0.118,
                a: 1.0,
            },
            foreground_color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            border_color: Color {
                r: 0.976,
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
            selection_highlight_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            },
            corner_radius: 0,
            border_width: 12,
            outline_width: 4,

            // Padding
            padding_top: UnitValue::pixels(8),
            padding_bottom: UnitValue::pixels(8),
            padding_left: UnitValue::pixels(8),
            padding_right: UnitValue::pixels(8),
            clip_to_padding: true,

            // Text layout
            prompt_text: String::from("run: "),
            prompt_padding: 0,
            placeholder_text: String::new(),
            num_results: 0,
            result_spacing: 0,
            horizontal: false,
            min_input_width: 0,

            // Themes: most None (inherit) — only pre-specified defaults set here
            cursor_theme: CursorTheme::default(),
            prompt_theme: TextTheme::default(),
            input_theme: TextTheme::default(),
            placeholder_theme: TextTheme {
                // placeholder-color = #FFFFFFA8 ≈ (1.0, 1.0, 1.0, 0.659)
                foreground_color: Some(Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 0.659,
                }),
                ..TextTheme::default()
            },
            default_result_theme: TextTheme::default(),
            alternate_result_theme: TextTheme::default(),
            selection_theme: TextTheme {
                // selection-color = #F92672
                foreground_color: Some(Color {
                    r: 0.976,
                    g: 0.149,
                    b: 0.447,
                    a: 1.0,
                }),
                ..TextTheme::default()
            },

            // Behaviour
            hide_cursor: false,
            use_history: true,
            history_file: None,
            matching_algorithm: libtofi_rs::matching::MatchingAlgorithm::Normal,
            require_match: true,
            auto_accept_single: false,
            hide_input: false,
            hidden_character: HiddenCharacter::default(),
            physical_keybindings: true,
            drun_launch: false,
            default_terminal: None,
            late_keyboard_init: false,
            multi_instance: false,
            ascii_input: false,
        }
    }
}

#[cfg(test)]
mod tests;
