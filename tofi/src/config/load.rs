//! Config file loading and key application.
//!
//! Public surface:
//! - [`load`] — read a config file and merge into [`TofiConfig`]
//! - [`apply_key`] — apply one `key = value` pair (used by both file loader and CLI)
//! - [`default_config_path`] — resolve the default config file path
//!
//! # Format
//!
//! - Lines starting with `#`, `;`, or `[` (after stripping leading whitespace) are comments.
//! - Options are `key = value` (whitespace stripped; surrounding `"…"` stripped from value).
//! - `include = /path/to/other.conf` recursively loads another file (max depth 32).
//! - Parse errors are non-fatal; loading stops after 5 errors per file.
//! - Only `true` and `false` are accepted for boolean values (case-insensitive).

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use libtofi_rs::color::Color;
use libtofi_rs::matching::MatchingAlgorithm;

use super::{Anchor, CursorStyle, Directional, HiddenCharacter, TextTheme, TofiConfig, UnitValue};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_ERRORS: usize = 5;
const MAX_RECURSION: u8 = 32;
const MAX_CONFIG_SIZE: u64 = 10 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// A non-fatal parse error from a config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub file: PathBuf,
    /// 1-based line number; 0 if not from a file line.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(
                f,
                "{}: line {}: {}",
                self.file.display(),
                self.line,
                self.message
            )
        } else {
            write!(f, "{}: {}", self.file.display(), self.message)
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns the default config file path.
///
/// Resolves `$XDG_CONFIG_HOME/tofi/config` or `$HOME/.config/tofi/config`.
pub fn default_config_path() -> Option<PathBuf> {
    if let Some(base) = std::env::var_os("XDG_CONFIG_HOME") {
        let mut p = PathBuf::from(base);
        p.push("tofi/config");
        return Some(p);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".config/tofi/config");
        return Some(p);
    }
    None
}

/// Load a config file and merge its settings into `cfg`.
///
/// Non-fatal parse errors are collected and returned.  Only I/O failures
/// cause an `Err` return.  If the file does not exist and `path` was not
/// explicitly requested (i.e. it is the default path), the caller should
/// treat `Ok([])` with a missing file as "no config" rather than an error.
pub fn load(path: &Path, cfg: &mut TofiConfig) -> std::io::Result<Vec<ParseError>> {
    let mut errors = Vec::new();
    load_inner(path, cfg, 0, &mut errors)?;
    Ok(errors)
}

/// Apply a single `key = value` pair (already stripped) to `cfg`.
///
/// Returns `Ok(())` on success.  Returns `Err(message)` when the value
/// cannot be parsed for the given key, or when the key is unknown.
/// Deprecated keys emit a message via `eprintln!` but still return `Ok(())`.
pub fn apply_key(cfg: &mut TofiConfig, key: &str, value: &str) -> Result<(), String> {
    match key.to_ascii_lowercase().as_str() {
        // --- window geometry ---
        "anchor" => {
            cfg.anchor =
                parse_anchor(value).ok_or_else(|| format!("Invalid anchor \"{value}\""))?;
        }
        "output" => {
            cfg.target_output = value.to_owned();
        }
        "exclusive-zone" => {
            if value == "-1" {
                cfg.exclusive_zone = -1;
                cfg.exclusive_zone_is_percent = false;
            } else {
                let uv = parse_unit_value(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
                cfg.exclusive_zone = uv.value as i32;
                cfg.exclusive_zone_is_percent = uv.is_percent;
            }
        }
        "width" => {
            cfg.width = parse_unit_value(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "height" => {
            cfg.height = parse_unit_value(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "margin-top" => {
            cfg.margin_top = parse_unit_value(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "margin-bottom" => {
            cfg.margin_bottom = parse_unit_value(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "margin-left" => {
            cfg.margin_left = parse_unit_value(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "margin-right" => {
            cfg.margin_right = parse_unit_value(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "scale" => {
            cfg.scale =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }

        // --- font ---
        "font" => {
            cfg.font = expand_home(value);
        }
        "font-size" => {
            let v = parse_u32(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
            if v == 0 {
                return Err("Option \"font-size\" must be greater than 0".to_string());
            }
            cfg.font_size = v;
        }
        "font-features" => {
            cfg.font_features = value.to_owned();
        }
        "font-variations" => {
            cfg.font_variations = value.to_owned();
        }
        "hint-font" => {
            cfg.hint_font =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }

        // --- window colors / decoration ---
        "background-color" => {
            cfg.background_color = parse_color(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?;
        }
        "text-color" => {
            cfg.foreground_color = parse_color(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?;
        }
        "border-color" => {
            cfg.border_color = parse_color(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?;
        }
        "outline-color" => {
            cfg.outline_color = parse_color(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?;
        }
        "selection-match-color" => {
            cfg.selection_highlight_color = parse_color(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?;
        }
        "corner-radius" => {
            cfg.corner_radius = parse_u32(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "border-width" => {
            cfg.border_width = parse_u32(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "outline-width" => {
            cfg.outline_width = parse_u32(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }

        // --- padding ---
        "padding-top" => {
            cfg.padding_top = parse_unit_value(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "padding-bottom" => {
            cfg.padding_bottom = parse_unit_value(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "padding-left" => {
            cfg.padding_left = parse_unit_value(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "padding-right" => {
            cfg.padding_right = parse_unit_value(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "clip-to-padding" => {
            cfg.clip_to_padding =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }

        // --- text layout ---
        "prompt-text" => {
            cfg.prompt_text = value.to_owned();
        }
        "prompt-padding" => {
            cfg.prompt_padding = parse_u32(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "placeholder-text" => {
            cfg.placeholder_text = value.to_owned();
        }
        "num-results" => {
            cfg.num_results = parse_u32(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "result-spacing" => {
            cfg.result_spacing =
                parse_i32(value).ok_or_else(|| format!("Failed to parse \"{value}\" as int"))?;
        }
        "horizontal" => {
            cfg.horizontal =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "min-input-width" => {
            cfg.min_input_width = parse_u32(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }

        // --- cursor theme ---
        "text-cursor" => {
            cfg.cursor_theme.show =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "text-cursor-style" => {
            cfg.cursor_theme.style = parse_cursor_style(value)
                .ok_or_else(|| format!("Invalid cursor style \"{value}\""))?;
        }
        "text-cursor-color" => {
            cfg.cursor_theme.color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "text-cursor-background" => {
            cfg.cursor_theme.text_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "text-cursor-corner-radius" => {
            cfg.cursor_theme.corner_radius = parse_u32(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?;
        }
        "text-cursor-thickness" => {
            cfg.cursor_theme.thickness = Some(
                parse_u32(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?,
            );
        }

        // --- prompt theme ---
        "prompt-color" => {
            cfg.prompt_theme.foreground_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "prompt-background" => {
            cfg.prompt_theme.background_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "prompt-background-padding" => {
            cfg.prompt_theme.padding = Some(
                parse_directional(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as directional"))?,
            );
        }
        "prompt-background-corner-radius" => {
            cfg.prompt_theme.background_corner_radius = Some(
                parse_u32(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?,
            );
        }

        // --- input theme ---
        "input-color" => {
            cfg.input_theme.foreground_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "input-background" => {
            cfg.input_theme.background_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "input-background-padding" => {
            cfg.input_theme.padding = Some(
                parse_directional(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as directional"))?,
            );
        }
        "input-background-corner-radius" => {
            cfg.input_theme.background_corner_radius = Some(
                parse_u32(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?,
            );
        }

        // --- placeholder theme ---
        "placeholder-color" => {
            cfg.placeholder_theme.foreground_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "placeholder-background" => {
            cfg.placeholder_theme.background_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "placeholder-background-padding" => {
            cfg.placeholder_theme.padding = Some(
                parse_directional(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as directional"))?,
            );
        }
        "placeholder-background-corner-radius" => {
            cfg.placeholder_theme.background_corner_radius = Some(
                parse_u32(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?,
            );
        }

        // --- default-result theme ---
        "default-result-color" => {
            cfg.default_result_theme.foreground_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "default-result-background" => {
            cfg.default_result_theme.background_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "default-result-background-padding" => {
            cfg.default_result_theme.padding = Some(
                parse_directional(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as directional"))?,
            );
        }
        "default-result-background-corner-radius" => {
            cfg.default_result_theme.background_corner_radius = Some(
                parse_u32(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?,
            );
        }

        // --- alternate-result theme ---
        "alternate-result-color" => {
            cfg.alternate_result_theme.foreground_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "alternate-result-background" => {
            cfg.alternate_result_theme.background_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "alternate-result-background-padding" => {
            cfg.alternate_result_theme.padding = Some(
                parse_directional(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as directional"))?,
            );
        }
        "alternate-result-background-corner-radius" => {
            cfg.alternate_result_theme.background_corner_radius = Some(
                parse_u32(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?,
            );
        }

        // --- selection theme ---
        "selection-color" => {
            cfg.selection_theme.foreground_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "selection-background" => {
            cfg.selection_theme.background_color = Some(
                parse_color(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as a color"))?,
            );
        }
        "selection-background-padding" => {
            cfg.selection_theme.padding = Some(
                parse_directional(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as directional"))?,
            );
        }
        "selection-background-corner-radius" => {
            cfg.selection_theme.background_corner_radius = Some(
                parse_u32(value)
                    .ok_or_else(|| format!("Failed to parse \"{value}\" as unsigned int"))?,
            );
        }
        "selection-padding" => {
            // Deprecated: left+right only (old single-value form).
            eprintln!(
                "Warning: \"selection-padding\" is deprecated. \
                 Use \"selection-background-padding\" instead."
            );
            let v =
                parse_i32(value).ok_or_else(|| format!("Failed to parse \"{value}\" as int"))?;
            let d = cfg
                .selection_theme
                .padding
                .get_or_insert_with(Directional::default);
            d.left = v;
            d.right = v;
        }

        // --- behaviour ---
        "hide-cursor" => {
            cfg.hide_cursor =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "history" => {
            cfg.use_history =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "history-file" => {
            cfg.history_file = Some(value.to_owned());
        }
        "matching-algorithm" => {
            cfg.matching_algorithm = parse_matching_algorithm(value)
                .ok_or_else(|| format!("Invalid matching algorithm \"{value}\""))?;
        }
        "fuzzy-match" => {
            // Deprecated alias for matching-algorithm = fuzzy.
            eprintln!(
                "Warning: \"fuzzy-match\" is deprecated. \
                 Use \"matching-algorithm = fuzzy\" instead."
            );
            let v =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
            if v {
                cfg.matching_algorithm = MatchingAlgorithm::Fuzzy;
            }
        }
        "require-match" => {
            cfg.require_match =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "auto-accept-single" => {
            cfg.auto_accept_single =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "print-index" => {
            cfg.print_index =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "hide-input" => {
            cfg.hide_input =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "hidden-character" => {
            cfg.hidden_character = parse_hidden_character(value)
                .ok_or_else(|| format!("Failed to parse \"{value}\" as a character"))?;
        }
        "physical-keybindings" => {
            cfg.physical_keybindings =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "drun-launch" => {
            cfg.drun_launch =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "drun-print-exec" => {
            // Deprecated: always true now; ignore silently after warning.
            eprintln!(
                "Warning: \"drun-print-exec\" is deprecated and has no effect \
                 (exec is always printed). This option may be removed in future."
            );
        }
        "terminal" => {
            cfg.default_terminal = Some(value.to_owned());
        }
        "late-keyboard-init" => {
            cfg.late_keyboard_init =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "multi-instance" => {
            cfg.multi_instance =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }
        "ascii-input" => {
            cfg.ascii_input =
                parse_bool(value).ok_or_else(|| format!("Invalid boolean value \"{value}\""))?;
        }

        _ => return Err(format!("Unknown option \"{key}\"")),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal: file loading
// ---------------------------------------------------------------------------

fn load_inner(
    path: &Path,
    cfg: &mut TofiConfig,
    depth: u8,
    errors: &mut Vec<ParseError>,
) -> std::io::Result<()> {
    if depth > MAX_RECURSION {
        errors.push(ParseError {
            file: path.to_owned(),
            line: 0,
            message: format!(
                "Refusing to load {}: recursion too deep (> {} layers)",
                path.display(),
                MAX_RECURSION
            ),
        });
        return Ok(());
    }

    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if metadata.len() > MAX_CONFIG_SIZE {
        return Err(std::io::Error::other(format!(
            "Config file too big (> {} MiB): {}",
            MAX_CONFIG_SIZE / 1024 / 1024,
            path.display()
        )));
    }

    let content = fs::read_to_string(path)?;
    let mut num_errors: usize = 0;

    for (lineno_0, raw) in content.lines().enumerate() {
        let lineno = lineno_0 + 1;
        if num_errors >= MAX_ERRORS {
            errors.push(ParseError {
                file: path.to_owned(),
                line: lineno,
                message: format!("Too many errors (> {}), giving up", MAX_ERRORS),
            });
            break;
        }

        // Strip the line; skip blank lines and comment/section-header lines.
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some('#' | ';' | '[') = line.chars().next() {
            continue;
        }

        // Split on the first `=`.
        let Some(eq) = line.find('=') else {
            errors.push(ParseError {
                file: path.to_owned(),
                line: lineno,
                message: format!("Config option missing value in: {line}"),
            });
            num_errors += 1;
            continue;
        };

        if eq == 0 {
            errors.push(ParseError {
                file: path.to_owned(),
                line: lineno,
                message: "Missing option (line starts with '=')".to_owned(),
            });
            num_errors += 1;
            continue;
        }

        let key = strip(&line[..eq]);
        // strip() for values returns None only for all-whitespace; Some("") is
        // valid (e.g. `font-features = ""`).
        let val = strip(&line[eq + 1..]);

        let (key, val) = match (key, val) {
            (Some(k), Some(v)) if !k.is_empty() => (k, v),
            _ => {
                errors.push(ParseError {
                    file: path.to_owned(),
                    line: lineno,
                    message: format!("Missing key or value in: {line}"),
                });
                num_errors += 1;
                continue;
            }
        };

        // Handle `include` specially — not forwarded to apply_key.
        if key.eq_ignore_ascii_case("include") {
            let include_path = if val.starts_with('/') {
                PathBuf::from(&val)
            } else {
                // Relative to the current file's directory.
                let dir = path.parent().unwrap_or(Path::new("."));
                dir.join(&val)
            };
            load_inner(&include_path, cfg, depth + 1, errors)?;
            continue;
        }

        if let Err(msg) = apply_key(cfg, &key, &val) {
            errors.push(ParseError {
                file: path.to_owned(),
                line: lineno,
                message: msg,
            });
            num_errors += 1;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// strip
// ---------------------------------------------------------------------------

/// Trim leading/trailing whitespace; strip surrounding `"…"` if both present.
///
/// Returns `None` only when the input is entirely whitespace (no content).
/// An explicitly empty value `""` returns `Some("")`.
fn strip(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Strip surrounding double-quotes when both are present.
    let inner = if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    };
    Some(inner.to_owned())
}

// ---------------------------------------------------------------------------
// Value parsers
// ---------------------------------------------------------------------------

/// `true` / `false` (case-insensitive).
fn parse_bool(s: &str) -> Option<bool> {
    match s.to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Non-negative 32-bit integer.
fn parse_u32(s: &str) -> Option<u32> {
    s.parse::<u32>().ok()
}

/// Signed 32-bit integer.
fn parse_i32(s: &str) -> Option<i32> {
    s.trim().parse::<i32>().ok()
}

/// Non-negative integer with optional `%` suffix.
fn parse_unit_value(s: &str) -> Option<UnitValue> {
    let s = s.trim();
    if let Some(digits) = s.strip_suffix('%') {
        let v = digits.trim().parse::<u32>().ok()?;
        Some(UnitValue::percent(v))
    } else {
        let v = s.parse::<u32>().ok()?;
        Some(UnitValue::pixels(v))
    }
}

/// CSS-style directional value (1–4 comma-separated `i32`s).
fn parse_directional(s: &str) -> Option<Directional> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() > 4 || parts.is_empty() {
        return None;
    }
    let nums: Vec<i32> = parts.iter().map(|p| parse_i32(p)).collect::<Option<_>>()?;
    let d = match nums.as_slice() {
        [a] => Directional::uniform(*a),
        [tb, lr] => Directional {
            top: *tb,
            right: *lr,
            bottom: *tb,
            left: *lr,
        },
        [t, lr, b] => Directional {
            top: *t,
            right: *lr,
            bottom: *b,
            left: *lr,
        },
        [t, r, b, l] => Directional {
            top: *t,
            right: *r,
            bottom: *b,
            left: *l,
        },
        _ => return None,
    };
    Some(d)
}

/// Parse a color via [`libtofi_rs::color`].
fn parse_color(s: &str) -> Option<Color> {
    s.parse::<Color>().ok()
}

/// Parse an anchor string.
fn parse_anchor(s: &str) -> Option<Anchor> {
    match s.to_ascii_lowercase().as_str() {
        "top-left" => Some(Anchor::TopLeft),
        "top" => Some(Anchor::Top),
        "top-right" => Some(Anchor::TopRight),
        "right" => Some(Anchor::Right),
        "bottom-right" => Some(Anchor::BottomRight),
        "bottom" => Some(Anchor::Bottom),
        "bottom-left" => Some(Anchor::BottomLeft),
        "left" => Some(Anchor::Left),
        "center" => Some(Anchor::Center),
        _ => None,
    }
}

/// Parse a cursor style.
fn parse_cursor_style(s: &str) -> Option<CursorStyle> {
    match s.to_ascii_lowercase().as_str() {
        "bar" => Some(CursorStyle::Bar),
        "block" => Some(CursorStyle::Block),
        "underscore" => Some(CursorStyle::Underscore),
        _ => None,
    }
}

/// Parse a matching algorithm.
fn parse_matching_algorithm(s: &str) -> Option<MatchingAlgorithm> {
    match s.to_ascii_lowercase().as_str() {
        "normal" => Some(MatchingAlgorithm::Normal),
        "prefix" => Some(MatchingAlgorithm::Prefix),
        "fuzzy" => Some(MatchingAlgorithm::Fuzzy),
        _ => None,
    }
}

/// Parse a single Unicode character (or empty string → `None` = completely hidden).
fn parse_hidden_character(s: &str) -> Option<HiddenCharacter> {
    if s.is_empty() {
        return Some(HiddenCharacter(None));
    }
    let mut chars = s.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        // More than one character — error.
        return None;
    }
    Some(HiddenCharacter(Some(ch)))
}

/// Expand a leading `~/` to `$HOME/`.
fn expand_home(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}/{rest}");
    }
    s.to_owned()
}

// Ensure TextTheme is usable in apply_key without importing it separately.
impl TextTheme {}
