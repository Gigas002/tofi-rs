//! Config file loading.
//!
//! Public surface:
//! - [`load`] — read a config file and merge into [`TofiConfig`]
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

use super::apply::apply_key;
use super::types::TofiConfig;

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

// ---------------------------------------------------------------------------
// Internal: file parsing
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
