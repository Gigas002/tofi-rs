//! Behavioral config loaded from `config.toml`.
//!
//! All fields are `Option<T>` — unset means "use theme/default".
//! Merging with CLI overrides and defaults happens in [`crate::settings`].

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

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
    /// Launch mode: `drun`, `run`, or `dmenu`.
    pub mode: Option<String>,
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

/// Load the config at `path`, with any [`Problem`]s found; a file that can't
/// be read or parsed gives the defaults.
pub fn load(path: &Path) -> (Config, Vec<Problem>) {
    load_toml(path)
}

// ── Problems ──────────────────────────────────────────────────────────────────

/// Something wrong with a config or theme file. Loading happens before
/// logging is set up, so these are returned for the caller to log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The file could not be read, or is not valid TOML: none of it is used.
    Unusable(String),
    /// A key nothing reads, e.g. a typo: only that key is ignored.
    UnknownKey(String),
}

impl Problem {
    /// Log the problem: an unusable file as an error, the rest as warnings.
    pub fn log(&self) {
        match self {
            Self::Unusable(msg) => tracing::error!("{msg}"),
            Self::UnknownKey(msg) => tracing::warn!("{msg}"),
        }
    }
}

/// Read and parse a TOML file, noting unknown keys. On any error the file
/// counts as empty, so every setting takes its default.
pub(crate) fn load_toml<T: DeserializeOwned + Default>(path: &Path) -> (T, Vec<Problem>) {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => {
            let msg = format!("{}: can't read it ({e}); using defaults", path.display());
            return (T::default(), vec![Problem::Unusable(msg)]);
        }
    };
    let mut problems = Vec::new();
    let parsed = toml::Deserializer::parse(&raw).and_then(|de| {
        serde_ignored::deserialize(de, |key| {
            problems.push(Problem::UnknownKey(format!(
                "{}: unknown key `{}` ignored",
                path.display(),
                key_path(&key)
            )));
        })
    });
    match parsed {
        Ok(value) => (value, problems),
        Err(e) => {
            let msg = format!(
                "{}: invalid, so none of it is used: {}",
                path.display(),
                e.to_string().trim_end()
            );
            (T::default(), vec![Problem::Unusable(msg)])
        }
    }
}

/// `window.blur`: the key as written in the file.
fn key_path(path: &serde_ignored::Path<'_>) -> String {
    use serde_ignored::Path;
    match path {
        Path::Root => String::new(),
        Path::Seq { parent, index } => format!("{}[{index}]", key_path(parent)),
        Path::Map { parent, key } => match key_path(parent) {
            p if p.is_empty() => key.clone(),
            p => format!("{p}.{key}"),
        },
        Path::Some { parent }
        | Path::NewtypeStruct { parent }
        | Path::NewtypeVariant { parent } => key_path(parent),
    }
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
