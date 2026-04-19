//! CLI parameters and parsing (`clap`) only — no app logic here.
//!
//! Options are **argv-only** (no `#[arg(env = …)]`).
//!
//! # Two-pass design
//!
//! 1. **Config file pass** — `--config <path>` (or the default XDG path) is
//!    loaded first.  `--include <path>` applies an extra file on top.
//! 2. **Override pass** — every config-key flag that was supplied on the
//!    command line is applied via [`crate::config::apply_key`], overriding
//!    whatever the file set.
//!
//! Call [`Cli::into_config`] after [`Cli::parse`] to get a [`TofiConfig`].

use std::io;
use std::path::PathBuf;

use clap::Parser;

use crate::config::{ParseError, TofiConfig, apply_key, default_config_path, load};

// ---------------------------------------------------------------------------
// Cli struct
// ---------------------------------------------------------------------------

/// Wayland application launcher.
///
/// Reads `$XDG_CONFIG_HOME/tofi/config` (or `$HOME/.config/tofi/config`)
/// unless `--config` is given.  Command-line flags override the config file.
#[derive(Parser, Debug)]
#[command(name = "tofi", version, about)]
pub struct Cli {
    // -----------------------------------------------------------------------
    // Meta / special
    // -----------------------------------------------------------------------
    /// Print a shell completion script to stdout and exit.
    ///
    /// Supported shells: bash, zsh, fish, nushell.
    /// Redirect the output to the appropriate location for your shell.
    #[cfg(feature = "completions")]
    #[arg(long, value_name = "SHELL")]
    pub completions: Option<crate::completions::CompletionShell>,

    /// Path to config file (overrides the default XDG path).
    #[arg(short = 'c', long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Load an additional config file on top of the main one.
    #[arg(long, value_name = "FILE")]
    pub include: Option<PathBuf>,

    /// Run in drun mode: list desktop applications from XDG data dirs.
    /// Equivalent to invoking the binary as `tofi-drun`.
    #[cfg(feature = "drun")]
    #[arg(long, conflicts_with = "run")]
    pub drun: bool,

    /// Run in run mode: list executables from $PATH.
    /// Equivalent to invoking the binary as `tofi-run`.
    #[cfg(feature = "run-commands")]
    #[arg(long, conflicts_with = "drun")]
    pub run: bool,

    // -----------------------------------------------------------------------
    // Window positioning
    // -----------------------------------------------------------------------
    /// Edge to anchor the window to: top-left, top, top-right, right,
    /// bottom-right, bottom, bottom-left, left, center.
    #[arg(long, value_name = "ANCHOR")]
    pub anchor: Option<String>,

    /// Exclusive zone size (-1, 0, or positive pixels/percent).
    #[arg(long, value_name = "VALUE")]
    pub exclusive_zone: Option<String>,

    /// Output to appear on (empty = compositor default).
    #[arg(long, value_name = "NAME")]
    pub output: Option<String>,

    /// Window width in pixels or percent (e.g. `1280` or `80%`).
    #[arg(long, value_name = "VALUE")]
    pub width: Option<String>,

    /// Window height in pixels or percent.
    #[arg(long, value_name = "VALUE")]
    pub height: Option<String>,

    /// Top margin in pixels or percent.
    #[arg(long, value_name = "VALUE")]
    pub margin_top: Option<String>,

    /// Bottom margin in pixels or percent.
    #[arg(long, value_name = "VALUE")]
    pub margin_bottom: Option<String>,

    /// Left margin in pixels or percent.
    #[arg(long, value_name = "VALUE")]
    pub margin_left: Option<String>,

    /// Right margin in pixels or percent.
    #[arg(long, value_name = "VALUE")]
    pub margin_right: Option<String>,

    // -----------------------------------------------------------------------
    // Window decoration
    // -----------------------------------------------------------------------
    /// Window background color (hex: `#RRGGBB` or `#RRGGBBAA`).
    #[arg(long, value_name = "COLOR")]
    pub background_color: Option<String>,

    /// Window corner radius in pixels.
    #[arg(long, value_name = "PX")]
    pub corner_radius: Option<String>,

    /// Border width in pixels.
    #[arg(long, value_name = "PX")]
    pub border_width: Option<String>,

    /// Border color.
    #[arg(long, value_name = "COLOR")]
    pub border_color: Option<String>,

    /// Outline width in pixels.
    #[arg(long, value_name = "PX")]
    pub outline_width: Option<String>,

    /// Outline color.
    #[arg(long, value_name = "COLOR")]
    pub outline_color: Option<String>,

    // -----------------------------------------------------------------------
    // Padding
    // -----------------------------------------------------------------------
    /// Top padding in pixels or percent.
    #[arg(long, value_name = "VALUE")]
    pub padding_top: Option<String>,

    /// Bottom padding in pixels or percent.
    #[arg(long, value_name = "VALUE")]
    pub padding_bottom: Option<String>,

    /// Left padding in pixels or percent.
    #[arg(long, value_name = "VALUE")]
    pub padding_left: Option<String>,

    /// Right padding in pixels or percent.
    #[arg(long, value_name = "VALUE")]
    pub padding_right: Option<String>,

    /// Clip text drawing to the padded area (true/false).
    #[arg(long, value_name = "BOOL")]
    pub clip_to_padding: Option<String>,

    // -----------------------------------------------------------------------
    // Font
    // -----------------------------------------------------------------------
    /// Font name (Pango format) or path to a font file.
    #[arg(long, value_name = "FONT")]
    pub font: Option<String>,

    /// Font size in points.
    #[arg(long, value_name = "PT")]
    pub font_size: Option<String>,

    /// OpenType font feature settings (comma-separated).
    #[arg(long, value_name = "FEATURES")]
    pub font_features: Option<String>,

    /// OpenType font variation settings (comma-separated).
    #[arg(long, value_name = "VARIATIONS")]
    pub font_variations: Option<String>,

    /// Enable font hinting (true/false).
    #[arg(long, value_name = "BOOL")]
    pub hint_font: Option<String>,

    // -----------------------------------------------------------------------
    // Text layout
    // -----------------------------------------------------------------------
    /// Prompt text.
    #[arg(long, value_name = "TEXT")]
    pub prompt_text: Option<String>,

    /// Extra horizontal gap between prompt and input field (pixels).
    #[arg(long, value_name = "PX")]
    pub prompt_padding: Option<String>,

    /// Placeholder input text.
    #[arg(long, value_name = "TEXT")]
    pub placeholder_text: Option<String>,

    /// Default text color.
    #[arg(long, value_name = "COLOR")]
    pub text_color: Option<String>,

    /// Maximum results to display (0 = fill window).
    #[arg(long, value_name = "N")]
    pub num_results: Option<String>,

    /// Pixel spacing between results (can be negative).
    #[arg(long, value_name = "PX")]
    pub result_spacing: Option<String>,

    /// List results horizontally (true/false).
    #[arg(long, value_name = "BOOL")]
    pub horizontal: Option<String>,

    /// Minimum input field width in horizontal mode (pixels).
    #[arg(long, value_name = "PX")]
    pub min_input_width: Option<String>,

    // -----------------------------------------------------------------------
    // Cursor theme
    // -----------------------------------------------------------------------
    /// Show text cursor in the input field (true/false).
    #[arg(long, value_name = "BOOL")]
    pub text_cursor: Option<String>,

    /// Text cursor style: bar, block, underscore.
    #[arg(long, value_name = "STYLE")]
    pub text_cursor_style: Option<String>,

    /// Text cursor color.
    #[arg(long, value_name = "COLOR")]
    pub text_cursor_color: Option<String>,

    /// Text color behind a block cursor.
    #[arg(long, value_name = "COLOR")]
    pub text_cursor_background: Option<String>,

    /// Text cursor corner radius in pixels.
    #[arg(long, value_name = "PX")]
    pub text_cursor_corner_radius: Option<String>,

    /// Text cursor thickness in pixels.
    #[arg(long, value_name = "PX")]
    pub text_cursor_thickness: Option<String>,

    // -----------------------------------------------------------------------
    // Per-element themes
    // -----------------------------------------------------------------------
    /// Prompt text color.
    #[arg(long, value_name = "COLOR")]
    pub prompt_color: Option<String>,

    /// Prompt background color.
    #[arg(long, value_name = "COLOR")]
    pub prompt_background: Option<String>,

    /// Prompt background padding (CSS-style, comma-separated).
    #[arg(long, value_name = "PADDING")]
    pub prompt_background_padding: Option<String>,

    /// Prompt background corner radius in pixels.
    #[arg(long, value_name = "PX")]
    pub prompt_background_corner_radius: Option<String>,

    /// Placeholder text color.
    #[arg(long, value_name = "COLOR")]
    pub placeholder_color: Option<String>,

    /// Placeholder background color.
    #[arg(long, value_name = "COLOR")]
    pub placeholder_background: Option<String>,

    /// Placeholder background padding (CSS-style).
    #[arg(long, value_name = "PADDING")]
    pub placeholder_background_padding: Option<String>,

    /// Placeholder background corner radius in pixels.
    #[arg(long, value_name = "PX")]
    pub placeholder_background_corner_radius: Option<String>,

    /// Input text color.
    #[arg(long, value_name = "COLOR")]
    pub input_color: Option<String>,

    /// Input background color.
    #[arg(long, value_name = "COLOR")]
    pub input_background: Option<String>,

    /// Input background padding (CSS-style).
    #[arg(long, value_name = "PADDING")]
    pub input_background_padding: Option<String>,

    /// Input background corner radius in pixels.
    #[arg(long, value_name = "PX")]
    pub input_background_corner_radius: Option<String>,

    /// Default result text color.
    #[arg(long, value_name = "COLOR")]
    pub default_result_color: Option<String>,

    /// Default result background color.
    #[arg(long, value_name = "COLOR")]
    pub default_result_background: Option<String>,

    /// Default result background padding (CSS-style).
    #[arg(long, value_name = "PADDING")]
    pub default_result_background_padding: Option<String>,

    /// Default result background corner radius in pixels.
    #[arg(long, value_name = "PX")]
    pub default_result_background_corner_radius: Option<String>,

    /// Alternate result text color.
    #[arg(long, value_name = "COLOR")]
    pub alternate_result_color: Option<String>,

    /// Alternate result background color.
    #[arg(long, value_name = "COLOR")]
    pub alternate_result_background: Option<String>,

    /// Alternate result background padding (CSS-style).
    #[arg(long, value_name = "PADDING")]
    pub alternate_result_background_padding: Option<String>,

    /// Alternate result background corner radius in pixels.
    #[arg(long, value_name = "PX")]
    pub alternate_result_background_corner_radius: Option<String>,

    /// Selection text color.
    #[arg(long, value_name = "COLOR")]
    pub selection_color: Option<String>,

    /// Highlighted match color within selected result.
    #[arg(long, value_name = "COLOR")]
    pub selection_match_color: Option<String>,

    /// Selection background color.
    #[arg(long, value_name = "COLOR")]
    pub selection_background: Option<String>,

    /// Selection background padding (CSS-style).
    #[arg(long, value_name = "PADDING")]
    pub selection_background_padding: Option<String>,

    /// Selection background corner radius in pixels.
    #[arg(long, value_name = "PX")]
    pub selection_background_corner_radius: Option<String>,

    /// \[Deprecated\] Use --selection-background-padding instead.
    #[arg(long, value_name = "PX", hide = true)]
    pub selection_padding: Option<String>,

    // -----------------------------------------------------------------------
    // Behaviour
    // -----------------------------------------------------------------------
    /// Hide the mouse cursor (true/false).
    #[arg(long, value_name = "BOOL")]
    pub hide_cursor: Option<String>,

    /// Scale window by the output's scale factor (true/false).
    #[arg(long, value_name = "BOOL")]
    pub scale: Option<String>,

    /// Sort results by usage history in run/drun mode (true/false).
    #[arg(long, value_name = "BOOL")]
    pub history: Option<String>,

    /// Custom history file path.
    #[arg(long, value_name = "FILE")]
    pub history_file: Option<String>,

    /// Matching algorithm: normal, prefix, fuzzy.
    #[arg(long, value_name = "ALGO")]
    pub matching_algorithm: Option<String>,

    /// \[Deprecated\] Use --matching-algorithm=fuzzy instead.
    #[arg(long, value_name = "BOOL", hide = true)]
    pub fuzzy_match: Option<String>,

    /// Require a result match to confirm selection (true/false).
    #[arg(long, value_name = "BOOL")]
    pub require_match: Option<String>,

    /// Accept automatically when only one result remains (true/false).
    #[arg(long, value_name = "BOOL")]
    pub auto_accept_single: Option<String>,

    /// Hide typed input (true/false).
    #[arg(long, value_name = "BOOL")]
    pub hide_input: Option<String>,

    /// Character to show in place of input when hide-input is true
    /// (empty string = completely hidden).
    #[arg(long, value_name = "CHAR")]
    pub hidden_character: Option<String>,

    /// Use physical key positions for shortcuts (true/false).
    #[arg(long, value_name = "BOOL")]
    pub physical_keybindings: Option<String>,

    /// Launch apps directly in drun mode (true/false).
    #[arg(long, value_name = "BOOL")]
    pub drun_launch: Option<String>,

    /// \[Deprecated\] drun always prints exec now; option is ignored.
    #[arg(long, value_name = "BOOL", hide = true)]
    pub drun_print_exec: Option<String>,

    /// Terminal for launching terminal apps in drun mode.
    #[arg(long, value_name = "TERM")]
    pub terminal: Option<String>,

    /// Allow multiple simultaneous instances (true/false).
    #[arg(long, value_name = "BOOL")]
    pub multi_instance: Option<String>,

    /// Assume plain ASCII input (true/false).
    #[arg(long, value_name = "BOOL")]
    pub ascii_input: Option<String>,

    /// Delay keyboard init until after first frame draw.
    /// Pass without a value to enable, or `true`/`false` explicitly.
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        value_name = "BOOL"
    )]
    pub late_keyboard_init: Option<String>,
}

// ---------------------------------------------------------------------------
// Two-pass config build
// ---------------------------------------------------------------------------

impl Cli {
    /// Build a [`TofiConfig`] from the CLI arguments:
    ///
    /// 1. Load the config file (`--config` or the XDG default).
    /// 2. Apply `--include` if given.
    /// 3. Apply any config-key flags supplied on the command line (overrides).
    ///
    /// Returns the populated config and any non-fatal parse warnings from
    /// the config file.  Fatal CLI value errors are returned as `Err`.
    pub fn into_config(self) -> io::Result<(TofiConfig, Vec<ParseError>)> {
        let mut cfg = TofiConfig::default();
        let mut errors: Vec<ParseError> = Vec::new();

        // Pass 1 — config file.
        if let Some(ref path) = self.config {
            errors.extend(load(path, &mut cfg)?);
        } else if let Some(default) = default_config_path()
            && default.exists()
        {
            errors.extend(load(&default, &mut cfg)?);
        }

        // Pass 2 — --include.
        if let Some(ref path) = self.include {
            errors.extend(load(path, &mut cfg)?);
        }

        // Pass 3 — per-flag overrides via apply_key.
        macro_rules! apply {
            ($field:expr, $key:literal) => {
                if let Some(ref val) = $field {
                    apply_key(&mut cfg, $key, val).map_err(io::Error::other)?;
                }
            };
        }

        apply!(self.anchor, "anchor");
        apply!(self.exclusive_zone, "exclusive-zone");
        apply!(self.output, "output");
        apply!(self.width, "width");
        apply!(self.height, "height");
        apply!(self.margin_top, "margin-top");
        apply!(self.margin_bottom, "margin-bottom");
        apply!(self.margin_left, "margin-left");
        apply!(self.margin_right, "margin-right");
        apply!(self.background_color, "background-color");
        apply!(self.corner_radius, "corner-radius");
        apply!(self.border_width, "border-width");
        apply!(self.border_color, "border-color");
        apply!(self.outline_width, "outline-width");
        apply!(self.outline_color, "outline-color");
        apply!(self.padding_top, "padding-top");
        apply!(self.padding_bottom, "padding-bottom");
        apply!(self.padding_left, "padding-left");
        apply!(self.padding_right, "padding-right");
        apply!(self.clip_to_padding, "clip-to-padding");
        apply!(self.font, "font");
        apply!(self.font_size, "font-size");
        apply!(self.font_features, "font-features");
        apply!(self.font_variations, "font-variations");
        apply!(self.hint_font, "hint-font");
        apply!(self.prompt_text, "prompt-text");
        apply!(self.prompt_padding, "prompt-padding");
        apply!(self.placeholder_text, "placeholder-text");
        apply!(self.text_color, "text-color");
        apply!(self.num_results, "num-results");
        apply!(self.result_spacing, "result-spacing");
        apply!(self.horizontal, "horizontal");
        apply!(self.min_input_width, "min-input-width");
        apply!(self.text_cursor, "text-cursor");
        apply!(self.text_cursor_style, "text-cursor-style");
        apply!(self.text_cursor_color, "text-cursor-color");
        apply!(self.text_cursor_background, "text-cursor-background");
        apply!(self.text_cursor_corner_radius, "text-cursor-corner-radius");
        apply!(self.text_cursor_thickness, "text-cursor-thickness");
        apply!(self.prompt_color, "prompt-color");
        apply!(self.prompt_background, "prompt-background");
        apply!(self.prompt_background_padding, "prompt-background-padding");
        apply!(
            self.prompt_background_corner_radius,
            "prompt-background-corner-radius"
        );
        apply!(self.placeholder_color, "placeholder-color");
        apply!(self.placeholder_background, "placeholder-background");
        apply!(
            self.placeholder_background_padding,
            "placeholder-background-padding"
        );
        apply!(
            self.placeholder_background_corner_radius,
            "placeholder-background-corner-radius"
        );
        apply!(self.input_color, "input-color");
        apply!(self.input_background, "input-background");
        apply!(self.input_background_padding, "input-background-padding");
        apply!(
            self.input_background_corner_radius,
            "input-background-corner-radius"
        );
        apply!(self.default_result_color, "default-result-color");
        apply!(self.default_result_background, "default-result-background");
        apply!(
            self.default_result_background_padding,
            "default-result-background-padding"
        );
        apply!(
            self.default_result_background_corner_radius,
            "default-result-background-corner-radius"
        );
        apply!(self.alternate_result_color, "alternate-result-color");
        apply!(
            self.alternate_result_background,
            "alternate-result-background"
        );
        apply!(
            self.alternate_result_background_padding,
            "alternate-result-background-padding"
        );
        apply!(
            self.alternate_result_background_corner_radius,
            "alternate-result-background-corner-radius"
        );
        apply!(self.selection_color, "selection-color");
        apply!(self.selection_match_color, "selection-match-color");
        apply!(self.selection_background, "selection-background");
        apply!(
            self.selection_background_padding,
            "selection-background-padding"
        );
        apply!(
            self.selection_background_corner_radius,
            "selection-background-corner-radius"
        );
        apply!(self.selection_padding, "selection-padding");
        apply!(self.hide_cursor, "hide-cursor");
        apply!(self.scale, "scale");
        apply!(self.history, "history");
        apply!(self.history_file, "history-file");
        apply!(self.matching_algorithm, "matching-algorithm");
        apply!(self.fuzzy_match, "fuzzy-match");
        apply!(self.require_match, "require-match");
        apply!(self.auto_accept_single, "auto-accept-single");
        apply!(self.hide_input, "hide-input");
        apply!(self.hidden_character, "hidden-character");
        apply!(self.physical_keybindings, "physical-keybindings");
        apply!(self.drun_launch, "drun-launch");
        apply!(self.drun_print_exec, "drun-print-exec");
        apply!(self.terminal, "terminal");
        apply!(self.multi_instance, "multi-instance");
        apply!(self.ascii_input, "ascii-input");
        apply!(self.late_keyboard_init, "late-keyboard-init");

        Ok((cfg, errors))
    }
}

#[cfg(test)]
mod tests;
