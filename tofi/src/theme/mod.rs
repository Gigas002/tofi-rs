//! Visual types and theme loading from a theme TOML file.
//!
//! All theme fields are `Option<T>` — unset means "use default".
//! Final resolution with defaults happens in [`crate::settings`].

use std::path::{Path, PathBuf};

use libtofi_rs::color::Color;
use serde::Deserialize;

// ── Resolved visual types (used in Settings) ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitValue {
    pub value: u32,
    pub is_percent: bool,
}

impl UnitValue {
    pub fn pixels(v: u32) -> Self {
        Self {
            value: v,
            is_percent: false,
        }
    }
    pub fn percent(v: u32) -> Self {
        Self {
            value: v,
            is_percent: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Directional {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

impl Directional {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorKind {
    Bar,
    Block,
    Underscore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    pub show: bool,
    pub kind: CursorKind,
    pub color: Option<Color>,
    pub text_color: Option<Color>,
    pub corner_radius: u32,
    pub thickness: Option<u32>,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            show: false,
            kind: CursorKind::Bar,
            color: None,
            text_color: None,
            corner_radius: 0,
            thickness: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextStyle {
    pub foreground_color: Option<Color>,
    pub background_color: Option<Color>,
    pub padding: Option<Directional>,
    pub background_corner_radius: Option<u32>,
}

// ── Theme — TOML serde struct (all optional) ──────────────────────────────────

#[derive(Debug, Default, Deserialize)]
pub struct Theme {
    #[serde(default)]
    pub prompt: PromptSection,
    #[serde(default)]
    pub results: ResultsSection,
    #[serde(default)]
    pub text: TextSection,
    #[serde(default)]
    pub font: FontSection,
    #[serde(default)]
    pub window: WindowSection,
    #[serde(default)]
    pub cursor: CursorSection,
    #[serde(default)]
    pub input: InputSection,
}

#[derive(Debug, Default, Deserialize)]
pub struct PromptSection {
    pub text: Option<String>,
    pub padding: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ResultsSection {
    pub count: Option<u32>,
    pub spacing: Option<u32>,
    pub mode: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TextSection {
    pub prompt: Option<String>,
    pub color: Option<String>,
    pub prompt_color: Option<String>,
    #[serde(alias = "input-color")]
    pub input_color: Option<String>,
    pub match_color: Option<String>,
    pub selection_color: Option<String>,
    pub selection_match_color: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct FontSection {
    pub name: Option<String>,
    pub size: Option<u32>,
    pub features: Option<String>,
    pub variations: Option<String>,
    pub hint: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct WindowSection {
    pub anchor: Option<String>,
    pub margin_top: Option<u32>,
    pub margin_bottom: Option<u32>,
    pub margin_left: Option<u32>,
    pub margin_right: Option<u32>,
    pub width: Option<Dim>,
    pub height: Option<Dim>,
    pub background_color: Option<String>,
    pub outline_width: Option<u32>,
    pub outline_color: Option<String>,
    pub border_width: Option<u32>,
    pub border_color: Option<String>,
    pub corner_radius: Option<u32>,
    pub padding_top: Option<Dim>,
    pub padding_bottom: Option<Dim>,
    pub padding_left: Option<Dim>,
    pub padding_right: Option<Dim>,
    pub clip_to_padding: Option<bool>,
    pub scale: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CursorSection {
    pub hide: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct InputSection {
    pub cursor: Option<bool>,
    pub cursor_style: Option<String>,
    pub cursor_color: Option<String>,
    pub cursor_background: Option<String>,
    pub cursor_corner_radius: Option<u32>,
    pub cursor_thickness: Option<u32>,
    pub hide: Option<bool>,
    pub hidden_character: Option<String>,
}

// ── Dim — TOML dimension: "45%" or 45 ────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Dim {
    Str(String),
    Int(u32),
}

/// `45` → 45 px, `"45%"` → 45 %, `"45"` → 45 px; `None` for anything else.
pub(crate) fn dim_to_unit(d: &Dim) -> Option<UnitValue> {
    match d {
        Dim::Int(n) => Some(UnitValue::pixels(*n)),
        Dim::Str(s) => match s.trim().strip_suffix('%') {
            Some(pct) => pct.trim().parse().ok().map(UnitValue::percent),
            None => s.trim().parse().ok().map(UnitValue::pixels),
        },
    }
}

// ── Path resolution ───────────────────────────────────────────────────────────

pub fn default_dir() -> Option<PathBuf> {
    crate::config::xdg_config_dir().map(|d| d.join("tofi/themes"))
}

/// Resolve a theme name/path to an absolute `PathBuf`.
///
/// - Absolute path → returned as-is.
/// - Contains `/` → resolved relative to `config_dir`.
/// - Plain filename → looked up in `~/.config/tofi/themes/`.
pub fn resolve_path(name: &str, config_dir: Option<&Path>) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_absolute() {
        return Some(p.to_path_buf());
    }
    if name.contains('/') {
        return config_dir.map(|d| d.join(name));
    }
    default_dir().map(|d| d.join(name))
}

// ── Loading ───────────────────────────────────────────────────────────────────

/// Load the theme at `path`, with any problems found; a file that can't be
/// read or parsed gives the defaults.
pub fn load(path: &Path) -> (Theme, Vec<crate::config::Problem>) {
    crate::config::load_toml(path)
}

// ── Helpers (used by settings::build) ────────────────────────────────────────

pub(crate) fn parse_hex_color(s: &str) -> Option<Color> {
    let s = s.trim().strip_prefix('#')?;
    let (r, g, b, a) = match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            (r, g, b, 255u8)
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };
    Some(Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: a as f32 / 255.0,
    })
}

pub(crate) fn parse_anchor(s: &str) -> Option<Anchor> {
    Some(match s.to_lowercase().replace('-', "_").as_str() {
        "top_left" => Anchor::TopLeft,
        "top" => Anchor::Top,
        "top_right" => Anchor::TopRight,
        "right" => Anchor::Right,
        "bottom_right" => Anchor::BottomRight,
        "bottom" => Anchor::Bottom,
        "bottom_left" => Anchor::BottomLeft,
        "left" => Anchor::Left,
        "center" => Anchor::Center,
        _ => return None,
    })
}

pub(crate) fn parse_cursor_kind(s: &str) -> Option<CursorKind> {
    Some(match s.to_lowercase().as_str() {
        "bar" => CursorKind::Bar,
        "block" => CursorKind::Block,
        "underscore" => CursorKind::Underscore,
        _ => return None,
    })
}

pub(crate) fn parse_algorithm(s: &str) -> Option<libtofi_rs::matching::MatchingAlgorithm> {
    use libtofi_rs::matching::MatchingAlgorithm;
    Some(match s.to_lowercase().as_str() {
        "normal" => MatchingAlgorithm::Normal,
        "fuzzy" => MatchingAlgorithm::Fuzzy,
        "prefix" => MatchingAlgorithm::Prefix,
        _ => return None,
    })
}

/// `[results].mode`: `true` for `"horizontal"`, `false` for `"vertical"`.
pub(crate) fn parse_horizontal(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "horizontal" => Some(true),
        "vertical" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
