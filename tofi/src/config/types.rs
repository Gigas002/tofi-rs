//! Plain data types for the tofi configuration.

use libtofi_rs::color::Color;
use libtofi_rs::matching::MatchingAlgorithm;

// ---------------------------------------------------------------------------
// Supporting value types
// ---------------------------------------------------------------------------

/// A value that can be expressed in absolute pixels or as a percentage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitValue {
    pub value: u32,
    pub is_percent: bool,
}

impl UnitValue {
    pub const fn pixels(value: u32) -> Self {
        Self {
            value,
            is_percent: false,
        }
    }

    pub const fn percent(value: u32) -> Self {
        Self {
            value,
            is_percent: true,
        }
    }
}

/// CSS-style directional insets (top / right / bottom / left).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Directional {
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

impl Directional {
    pub const fn uniform(v: i32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
}

/// Text-cursor drawing style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum CursorStyle {
    #[default]
    Bar,
    Block,
    Underscore,
}

/// Visual theme for the text cursor.
///
/// `None` fields mean "inherit from context".
#[derive(Debug, Clone, PartialEq)]
pub struct CursorTheme {
    /// Whether the cursor is visible.
    pub show: bool,
    pub style: CursorStyle,
    /// Foreground color; `None` → use `input_theme` foreground.
    pub color: Option<Color>,
    /// Text color under a block cursor; `None` → use `background_color`.
    pub text_color: Option<Color>,
    pub corner_radius: u32,
    /// Bar / block thickness in pixels.  `None` → font-dependent (underscore)
    /// or 2 (bar / block).
    pub thickness: Option<u32>,
}

impl Default for CursorTheme {
    fn default() -> Self {
        Self {
            show: false,
            style: CursorStyle::Bar,
            color: None,
            text_color: None,
            corner_radius: 0,
            // Default thickness = 2 (bar style); stored as None so callers can
            // distinguish "user did not set" from "user set to 2".
            thickness: None,
        }
    }
}

/// Visual theme for a text element (prompt, input, results, selection, …).
///
/// `None` means the element inherits from the parent / global setting.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextTheme {
    pub foreground_color: Option<Color>,
    pub background_color: Option<Color>,
    pub padding: Option<Directional>,
    pub background_corner_radius: Option<u32>,
}

/// Screen-edge anchor position for the launcher window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    #[default]
    Center,
}

/// The character shown in place of real input when `hide_input = true`.
///
/// `None` means input is completely hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiddenCharacter(pub Option<char>);

impl Default for HiddenCharacter {
    fn default() -> Self {
        Self(Some('*'))
    }
}

// ---------------------------------------------------------------------------
// Top-level config struct
// ---------------------------------------------------------------------------

/// Complete runtime configuration for one tofi instance.
///
/// This is **plain data** — no I/O, no Wayland, no file parsing.
/// Populated by [`crate::config`] (file + CLI) and passed into the engine.
#[derive(Debug, Clone, PartialEq)]
pub struct TofiConfig {
    // --- Window geometry -------------------------------------------------------
    /// Window width (pixels or percent of output width). Default: 1280 px.
    pub width: UnitValue,
    /// Window height (pixels or percent of output height). Default: 720 px.
    pub height: UnitValue,
    /// Edge to anchor the window to. Default: [`Anchor::Center`].
    pub anchor: Anchor,
    /// Exclusive zone: -1 = ignore others, 0 = avoid others, >0 = claim space.
    /// Default: -1.
    pub exclusive_zone: i32,
    /// Whether `exclusive_zone` was given as a percentage.
    pub exclusive_zone_is_percent: bool,
    pub margin_top: UnitValue,
    pub margin_bottom: UnitValue,
    pub margin_left: UnitValue,
    pub margin_right: UnitValue,
    /// Scale window by the output's scale factor. Default: `true`.
    pub scale: bool,
    /// Name of the output to appear on; empty = compositor default. Default: `""`.
    pub target_output: String,

    // --- Font ------------------------------------------------------------------
    /// Font name (Pango format) or path to a font file. Default: `"Sans"`.
    pub font: String,
    /// Font size in points. Default: `24`.
    pub font_size: u32,
    /// OpenType feature settings (comma-separated). Default: `""`.
    pub font_features: String,
    /// OpenType variation settings (comma-separated). Default: `""`.
    pub font_variations: String,
    /// Enable font hinting. Default: `true`.
    pub hint_font: bool,

    // --- Window colors / decoration --------------------------------------------
    /// Window background color. Default: `#1B1D1E` / `(0.106, 0.114, 0.118, 1.0)`.
    pub background_color: Color,
    /// Default text color. Default: `#FFFFFF`.
    pub foreground_color: Color,
    /// Border color. Default: `#F92672`.
    pub border_color: Color,
    /// Outline color (outside the border). Default: `#080800`.
    pub outline_color: Color,
    /// Highlighted matching portion of selected result. Default: `#00000000`.
    pub selection_highlight_color: Color,
    /// Window corner radius in pixels. Default: `0`.
    pub corner_radius: u32,
    /// Border width in pixels. Default: `12`.
    pub border_width: u32,
    /// Outline width in pixels. Default: `4`.
    pub outline_width: u32,

    // --- Padding ---------------------------------------------------------------
    pub padding_top: UnitValue,
    pub padding_bottom: UnitValue,
    pub padding_left: UnitValue,
    pub padding_right: UnitValue,
    /// Clip text drawing to the padded area. Default: `true`.
    pub clip_to_padding: bool,

    // --- Text layout -----------------------------------------------------------
    /// Prompt text displayed before the input. Default: `"run: "`.
    pub prompt_text: String,
    /// Extra horizontal gap between prompt and input field. Default: `0`.
    pub prompt_padding: u32,
    /// Placeholder shown when input is empty. Default: `""`.
    pub placeholder_text: String,
    /// Maximum number of results to display; 0 = fill window. Default: `0`.
    pub num_results: u32,
    /// Pixel spacing between result items (may be negative). Default: `0`.
    pub result_spacing: i32,
    /// Display results horizontally. Default: `false`.
    pub horizontal: bool,
    /// Minimum input field width in horizontal mode. Default: `0`.
    pub min_input_width: u32,

    // --- Per-element themes ----------------------------------------------------
    pub cursor_theme: CursorTheme,
    pub prompt_theme: TextTheme,
    pub input_theme: TextTheme,
    pub placeholder_theme: TextTheme,
    pub default_result_theme: TextTheme,
    /// `None` fields inherit from `default_result_theme`.
    pub alternate_result_theme: TextTheme,
    pub selection_theme: TextTheme,

    // --- Behaviour -------------------------------------------------------------
    /// Hide the mouse cursor. Default: `false`.
    pub hide_cursor: bool,
    /// Sort results by usage history in run / drun modes. Default: `true`.
    pub use_history: bool,
    /// Override path for the history file; `None` = XDG default. Default: `None`.
    pub history_file: Option<String>,
    /// Matching algorithm. Default: [`MatchingAlgorithm::Normal`].
    pub matching_algorithm: MatchingAlgorithm,
    /// Require a result match to confirm selection. Default: `true`.
    pub require_match: bool,
    /// Accept automatically when only one result remains. Default: `false`.
    pub auto_accept_single: bool,
    /// Hide typed input. Default: `false`.
    pub hide_input: bool,
    /// Character shown in place of input when `hide_input` is `true`.
    /// `None` = completely hidden. Default: `'*'`.
    pub hidden_character: HiddenCharacter,
    /// Use physical key positions for shortcuts regardless of layout.
    /// Default: `true`.
    pub physical_keybindings: bool,
    /// Print 1-based index instead of the selected entry. Default: `false`.
    pub print_index: bool,
    /// In drun mode: launch the app directly instead of printing the command.
    /// Default: `false`.
    pub drun_launch: bool,
    /// Terminal for launching terminal apps in drun mode; `None` = `$TERMINAL`.
    /// Default: `None`.
    pub default_terminal: Option<String>,
    /// Delay keyboard init until after first frame draw. Default: `false`.
    pub late_keyboard_init: bool,
    /// Allow multiple simultaneous instances. Default: `false`.
    pub multi_instance: bool,
    /// Assume ASCII input; disables some Unicode handling. Default: `false`.
    pub ascii_input: bool,
}
