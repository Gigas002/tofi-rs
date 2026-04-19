//! Selection history file (opt-in via the `history` feature).
//!
//! # File format
//!
//! Each entry occupies one line:
//!
//! ```text
//! {run_count} {name}\n
//! ```
//!
//! Entries are stored in descending order by `run_count` (most-used first).
//!
//! # Path resolution
//!
//! | Condition | Path |
//! |---|---|
//! | `$XDG_STATE_HOME` is set | `$XDG_STATE_HOME/tofi[-drun]-history` |
//! | Fallback | `$HOME/.local/state/tofi[-drun]-history` |

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

/// Maximum accepted history file size (10 MiB).
const MAX_HISTFILE_SIZE: u64 = 10 * 1024 * 1024;

const HISTFILE_BASENAME: &str = "tofi-history";
const DRUN_HISTFILE_BASENAME: &str = "tofi-drun-history";

/// A single program entry in the history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppHistoryEntry {
    /// Program name (or desktop entry id for drun mode).
    pub name: String,
    /// Number of times this program has been launched.
    pub run_count: usize,
}

/// In-memory history, sorted descending by [`AppHistoryEntry::run_count`].
#[derive(Debug, Default, Clone)]
pub struct AppHistory {
    entries: Vec<AppHistoryEntry>,
}

impl AppHistory {
    /// Create an empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Iterate over entries in descending run-count order.
    pub fn entries(&self) -> &[AppHistoryEntry] {
        &self.entries
    }

    /// Add `name` to the history.
    ///
    /// - If already present: increment `run_count`, then bubble the entry
    ///   upward as long as its count exceeds the preceding entry (stable with
    ///   respect to equal counts).
    /// - If not present: append with `run_count = 1`.
    pub fn add(&mut self, name: &str) {
        if let Some(i) = self.entries.iter().position(|p| p.name == name) {
            self.entries[i].run_count += 1;
            let count = self.entries[i].run_count;
            if i == 0 || count <= self.entries[i - 1].run_count {
                return;
            }
            let mut j = i;
            while j > 0 && count > self.entries[j - 1].run_count {
                j -= 1;
            }
            let entry = self.entries.remove(i);
            self.entries.insert(j, entry);
        } else {
            self.entries.push(AppHistoryEntry {
                name: name.to_owned(),
                run_count: 1,
            });
        }
    }

    /// Remove the entry with the given name (if present).
    pub fn remove(&mut self, name: &str) {
        if let Some(i) = self.entries.iter().position(|p| p.name == name) {
            self.entries.remove(i);
        }
    }
}

/// Resolve the default history file path.
///
/// Uses `$XDG_STATE_HOME` when set; otherwise falls back to
/// `$HOME/.local/state/`. Returns `None` when neither variable is available.
pub fn default_history_path(drun: bool) -> Option<PathBuf> {
    resolve_history_path(
        std::env::var_os("XDG_STATE_HOME"),
        std::env::var_os("HOME"),
        drun,
    )
}

/// Pure path-resolution logic, extracted for testing without env mutation.
fn resolve_history_path(
    xdg_state_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
    drun: bool,
) -> Option<PathBuf> {
    let basename = if drun {
        DRUN_HISTFILE_BASENAME
    } else {
        HISTFILE_BASENAME
    };

    if let Some(state) = xdg_state_home {
        Some(PathBuf::from(state).join(basename))
    } else {
        Some(PathBuf::from(home?).join(".local/state").join(basename))
    }
}

/// Load a [`AppHistory`] from `path`.
///
/// Returns an empty history (not an error) when the file does not exist.
/// Returns an error for I/O failures other than `NotFound`.
pub fn load(path: &Path) -> io::Result<AppHistory> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(AppHistory::new()),
        Err(e) => return Err(e),
    };

    if bytes.len() as u64 > MAX_HISTFILE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "history file too large (> {} MiB): {}",
                MAX_HISTFILE_SIZE / 1024 / 1024,
                path.display()
            ),
        ));
    }

    let text =
        std::str::from_utf8(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut history = AppHistory::new();

    for line in text.lines() {
        // Format: "{run_count} {name}"
        let mut parts = line.splitn(2, ' ');
        let (Some(count_str), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(run_count) = count_str.parse::<usize>() else {
            continue;
        };
        if !name.is_empty() {
            history.entries.push(AppHistoryEntry {
                name: name.to_owned(),
                run_count,
            });
        }
    }

    Ok(history)
}

/// Save `history` to `path`, creating intermediate directories as needed.
///
/// The file is written with mode `0600` (owner read/write only).
pub fn save(history: &AppHistory, path: &Path) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;

    let mut writer = io::BufWriter::new(file);
    for entry in &history.entries {
        writeln!(writer, "{} {}", entry.run_count, entry.name)?;
    }
    writer.flush()?;
    Ok(())
}

/// Load history from the platform-default path.
pub fn load_default(drun: bool) -> io::Result<AppHistory> {
    match default_history_path(drun) {
        Some(path) => load(&path),
        None => Ok(AppHistory::new()),
    }
}

/// Save history to the platform-default path.
pub fn save_default(history: &AppHistory, drun: bool) -> io::Result<()> {
    match default_history_path(drun) {
        Some(path) => save(history, &path),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests;
