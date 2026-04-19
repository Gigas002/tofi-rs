//! Behavioral config loaded from `config.toml`.
//!
//! All fields are `Option<T>` — unset means "use theme/default".
//! Merging with CLI overrides and defaults happens in [`crate::settings`].

use std::path::{Path, PathBuf};

use serde::Deserialize;

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub base: Base,
    #[serde(default)]
    pub matching: Matching,
    #[serde(default)]
    pub history: History,
    #[serde(default)]
    #[cfg_attr(not(feature = "logging"), allow(dead_code))]
    pub logging: Logging,
}

#[derive(Debug, Default, Deserialize)]
pub struct Base {
    pub output: Option<String>,
    pub terminal: Option<String>,
    pub theme: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Matching {
    pub algorithm: Option<String>,
    pub require: Option<bool>,
    pub ascii: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct History {
    pub history: Option<bool>,
    pub path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(not(feature = "logging"), allow(dead_code))]
pub struct Logging {
    pub level: Option<String>,
}

// ── Loading ───────────────────────────────────────────────────────────────────

pub fn default_path() -> Option<PathBuf> {
    xdg_config_dir().map(|d| d.join("tofi/config.toml"))
}

pub fn load(path: &Path) -> Config {
    let s = std::fs::read_to_string(path).unwrap_or_default();
    toml::from_str(&s).unwrap_or_default()
}

pub(crate) fn xdg_config_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(p));
    }
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".config"))
}

#[cfg(test)]
mod tests;
