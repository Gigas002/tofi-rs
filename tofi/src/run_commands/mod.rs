//! Cached `$PATH` executable list for **`tofi-run`** (feature **`run-commands`**).
//!
//! Scans `$PATH` directories directly for executable regular files,
//! caches the sorted, deduplicated list, and invalidates the cache when any
//! `$PATH` directory is newer than the cache file.
//!
//! # Cache path resolution
//!
//! | Condition | Cache file |
//! |---|---|
//! | `$XDG_CACHE_HOME` is set | `$XDG_CACHE_HOME/tofi-compgen` |
//! | Fallback | `$HOME/.cache/tofi-compgen` |

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const CACHE_BASENAME: &str = "tofi-compgen";

/// Scan every directory in `path_var` (colon-separated) and return a sorted,
/// deduplicated list of executable regular-file names.
pub fn scan(path_var: &str) -> Vec<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();

    for dir in path_var.split(':') {
        if dir.is_empty() {
            continue;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            if meta.permissions().mode() & 0o111 == 0 {
                continue;
            }
            if let Some(name) = entry.file_name().to_str().map(str::to_owned) {
                names.insert(name);
            }
        }
    }

    names.into_iter().collect()
}

/// Write `commands` (one name per line) to `path`, creating parent dirs as
/// needed.
pub fn save_cache(commands: &[String], path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let content: String = commands.iter().map(|s| format!("{s}\n")).collect();
    fs::write(path, content)
}

/// Load `commands` from a cache file (one name per line, empty lines skipped).
pub fn load_cache(path: &Path) -> io::Result<Vec<String>> {
    let text = fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

/// Return the most-recent mtime among the directories in `path_var`, or
/// `None` if none of them could be stat'd.
fn path_dirs_mtime(path_var: &str) -> Option<SystemTime> {
    path_var
        .split(':')
        .filter(|s| !s.is_empty())
        .filter_map(|dir| fs::metadata(dir).ok()?.modified().ok())
        .max()
}

/// Return the commands list, using a cache when valid.
///
/// - If the cache file does not exist → scan and write it.
/// - If any `$PATH` directory is newer than the cache → rescan and overwrite.
/// - Otherwise → load and return the cache.
pub fn commands_cached(path_var: &str, cache_path: &Path) -> io::Result<Vec<String>> {
    match fs::metadata(cache_path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // Cache doesn't exist — build and save it.
            let commands = scan(path_var);
            let _ = save_cache(&commands, cache_path); // non-fatal if it fails
            Ok(commands)
        }
        Err(e) => Err(e),
        Ok(cache_meta) => {
            let cache_mtime = cache_meta.modified()?;
            let is_stale = path_dirs_mtime(path_var)
                .map(|dir_mtime| dir_mtime > cache_mtime)
                .unwrap_or(false);

            if is_stale {
                let commands = scan(path_var);
                let _ = save_cache(&commands, cache_path);
                Ok(commands)
            } else {
                load_cache(cache_path)
            }
        }
    }
}

/// Resolve the default cache file path.
pub fn default_cache_path() -> Option<PathBuf> {
    resolve_cache_path(std::env::var_os("XDG_CACHE_HOME"), std::env::var_os("HOME"))
}

/// Pure path-resolution logic, extracted for testing without env mutation.
fn resolve_cache_path(
    xdg_cache_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    if let Some(cache) = xdg_cache_home {
        Some(PathBuf::from(cache).join(CACHE_BASENAME))
    } else {
        Some(PathBuf::from(home?).join(".cache").join(CACHE_BASENAME))
    }
}

#[cfg(test)]
mod tests;
