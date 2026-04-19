//! Desktop-entry discovery and launching for `tofi --drun`.
#![deny(unsafe_code)]
//!
//! # Responsibilities
//!
//! - Discover `.desktop` files under `$XDG_DATA_HOME/applications/` and every
//!   `$XDG_DATA_DIRS` entry with `/applications/` appended.
//! - Parse each file: locale-aware `Name`, `Keywords`, `Hidden`/`NoDisplay`,
//!   `OnlyShowIn`/`NotShowIn`, `Exec`, `Terminal`.
//! - Enforce the Desktop Entry Specification uniqueness rule: the highest-
//!   precedence (first) file with a given ID wins.
//! - Cache the result; invalidate when any application directory is newer.
//! - Expand `Exec` field codes for launch.
//!
//! # Cache path
//!
//! | Condition | Path |
//! |---|---|
//! | `$XDG_CACHE_HOME` is set | `$XDG_CACHE_HOME/tofi-drun` |
//! | Fallback | `$HOME/.cache/tofi-drun` |

use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use walkdir::WalkDir;

use crate::unicode::utf8_normalize;

const CACHE_BASENAME: &str = "tofi-drun";
const DEFAULT_DATA_DIRS: &str = "/usr/local/share/:/usr/share/";
/// Magic header written as the first line of every cache file.
///
/// If the file starts with a different line (e.g. a stale C-tofi cache), we
/// treat it as invalid and trigger a fresh scan.
const CACHE_HEADER: &str = "#tofi-rs-drun-v1";

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A parsed desktop application entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    /// Desktop file ID (relative path with `/` replaced by `-`).
    pub id: String,
    /// Locale-aware application name, NFC-normalised.
    pub name: String,
    /// Absolute path to the `.desktop` file.
    pub path: PathBuf,
    /// Raw keyword string (semicolon-separated), kept as-is for matching.
    pub keywords: String,
    /// Raw `Exec` field value.
    pub exec: String,
    /// Icon name (for `%i` field-code expansion).
    pub icon: String,
    /// Whether the application requires a terminal emulator.
    pub terminal: bool,
}

// ---------------------------------------------------------------------------
// Desktop entry parsing
// ---------------------------------------------------------------------------

/// Parse locale strings from `LANG` / `LANGUAGE`.
///
/// Returns `(lang, territory)` where territory may be empty, e.g.
/// `("en", "US")` from `en_US.UTF-8`.
fn current_locale() -> (String, String) {
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LANGUAGE"))
        .unwrap_or_default();

    let base = lang.split('.').next().unwrap_or("").to_owned();
    let mut parts = base.splitn(2, '_');
    let language = parts.next().unwrap_or("").to_owned();
    let territory = parts.next().unwrap_or("").to_owned();
    (language, territory)
}

/// Pick the best localised value from a map of `key[locale] → value`.
///
/// Priority: `lang_territory` > `lang` > unlocalized (`""`).
fn best_locale<'a>(
    values: &'a HashMap<String, String>,
    lang: &str,
    territory: &str,
) -> Option<&'a str> {
    let full = format!("{lang}_{territory}");
    if !territory.is_empty()
        && let Some(v) = values.get(&full)
    {
        return Some(v);
    }
    if !lang.is_empty()
        && let Some(v) = values.get(lang)
    {
        return Some(v);
    }
    values.get("").map(String::as_str)
}

/// Parse a single `.desktop` file.
///
/// Returns `None` if the entry should be skipped (Hidden, NoDisplay, or
/// filtered by OnlyShowIn/NotShowIn).
pub fn parse_entry(id: &str, path: &Path) -> Option<DesktopEntry> {
    let content = fs::read_to_string(path).ok()?;
    let (lang, territory) = current_locale();

    // We only care about the [Desktop Entry] section.
    let mut in_section = false;

    // Collect potentially-localised fields.
    let mut names: HashMap<String, String> = HashMap::new();
    let mut keywords_map: HashMap<String, String> = HashMap::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut terminal = false;
    let mut hidden = false;
    let mut no_display = false;
    let mut only_show_in: Vec<String> = Vec::new();
    let mut not_show_in: Vec<String> = Vec::new();

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('[') {
            in_section = line == "[Desktop Entry]";
            continue;
        }
        if !in_section || line.starts_with('#') || line.is_empty() {
            continue;
        }

        let Some((raw_key, value)) = line.split_once('=') else {
            continue;
        };
        let raw_key = raw_key.trim();
        let value = value.trim();

        // Split key and optional locale: `Name[en_US]` → key="Name", locale="en_US"
        let (key, locale) = if let Some(i) = raw_key.find('[') {
            let k = &raw_key[..i];
            let l = raw_key[i + 1..].trim_end_matches(']');
            (k, l.to_owned())
        } else {
            (raw_key, String::new())
        };

        match key {
            "Name" => {
                names.insert(locale, value.to_owned());
            }
            "Keywords" => {
                keywords_map.insert(locale, value.to_owned());
            }
            "Exec" => exec = value.to_owned(),
            "Icon" => icon = value.to_owned(),
            "Terminal" => terminal = value.eq_ignore_ascii_case("true"),
            "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
            "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
            "OnlyShowIn" => {
                only_show_in = value
                    .split(';')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            "NotShowIn" => {
                not_show_in = value
                    .split(';')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect();
            }
            _ => {}
        }
    }

    if hidden || no_display {
        return None;
    }

    // OnlyShowIn / NotShowIn filtering.
    if !only_show_in.is_empty() && !matches_current_desktop(&only_show_in) {
        return None;
    }
    if !not_show_in.is_empty() && matches_current_desktop(&not_show_in) {
        return None;
    }

    let raw_name = best_locale(&names, &lang, &territory)?.to_owned();
    let name = utf8_normalize(&raw_name);
    let keywords = best_locale(&keywords_map, &lang, &territory)
        .unwrap_or("")
        .to_owned();

    Some(DesktopEntry {
        id: id.to_owned(),
        name,
        path: path.to_owned(),
        keywords,
        exec,
        icon,
        terminal,
    })
}

/// Check whether any of `desktops` matches `$XDG_CURRENT_DESKTOP`.
fn matches_current_desktop(desktops: &[String]) -> bool {
    let current = match std::env::var("XDG_CURRENT_DESKTOP") {
        Ok(v) => v,
        Err(_) => return false,
    };
    current
        .split(':')
        .any(|d| desktops.iter().any(|entry| entry == d))
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Resolve the list of `…/applications/` directories to scan.
pub fn application_dirs() -> Vec<PathBuf> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
        });

    let data_dirs_str =
        std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| DEFAULT_DATA_DIRS.to_owned());

    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(home) = data_home {
        dirs.push(home.join("applications"));
    }
    for d in data_dirs_str.split(':').filter(|s| !s.is_empty()) {
        dirs.push(PathBuf::from(d).join("applications"));
    }
    dirs
}

/// Scan `dirs` for `.desktop` files and return parsed entries.
///
/// Enforces uniqueness: the first (highest-precedence) file with a given ID
/// wins, matching the Desktop Entry Specification.
pub fn scan(dirs: &[PathBuf]) -> Vec<DesktopEntry> {
    // id → entry; insertion order maintained via a vec for sorting.
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut entries: Vec<DesktopEntry> = Vec::new();

    for dir in dirs {
        let prefix_len = dir.as_os_str().len() + 1; // +1 for separator
        for result in WalkDir::new(dir).follow_links(true).into_iter().flatten() {
            let path = result.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }

            // Build the desktop file ID: relative path with '/' replaced by '-'.
            let full = path.as_os_str().to_str().unwrap_or("");
            if full.len() <= prefix_len {
                continue;
            }
            let id = full[prefix_len..].replace('/', "-");

            if seen.contains_key(&id) {
                continue;
            }
            seen.insert(id.clone(), ());

            if let Some(entry) = parse_entry(&id, path) {
                entries.push(entry);
            }
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

/// Save `entries` to `path` in a null-byte–separated format.
///
/// The first line is always `CACHE_HEADER` so that stale C-tofi caches can
/// be detected and discarded.
///
/// Record layout (after header, one per line):
/// `{id}\0{name}\0{path}\0{keywords}\0{exec}\0{icon}\0{terminal}\n`
pub fn save_cache(entries: &[DesktopEntry], path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(path)?;
    let mut w = io::BufWriter::new(file);
    writeln!(w, "{CACHE_HEADER}")?;
    for e in entries {
        writeln!(
            w,
            "{}\0{}\0{}\0{}\0{}\0{}\0{}",
            e.id,
            e.name,
            e.path.display(),
            e.keywords,
            e.exec,
            e.icon,
            e.terminal as u8,
        )?;
    }
    w.flush()
}

/// Load entries from a cache file produced by [`save_cache`].
///
/// Returns `Err(InvalidData)` when the file does not start with the expected
/// `CACHE_HEADER`, allowing callers to detect a stale C-tofi cache and
/// trigger a fresh scan.
pub fn load_cache(path: &Path) -> io::Result<Vec<DesktopEntry>> {
    let file = fs::File::open(path)?;
    let mut reader = io::BufReader::new(file);
    let mut header = String::new();
    reader.read_line(&mut header)?;
    if header.trim_end() != CACHE_HEADER {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cache header mismatch — stale or foreign cache format",
        ));
    }
    let mut entries = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let fields: Vec<&str> = line.splitn(7, '\0').collect();
        if fields.len() < 7 {
            continue;
        }
        entries.push(DesktopEntry {
            id: fields[0].to_owned(),
            name: fields[1].to_owned(),
            path: PathBuf::from(fields[2]),
            keywords: fields[3].to_owned(),
            exec: fields[4].to_owned(),
            icon: fields[5].to_owned(),
            terminal: fields[6] == "1",
        });
    }
    Ok(entries)
}

/// Return the most-recent mtime among `dirs`, or `None`.
fn dirs_mtime(dirs: &[PathBuf]) -> Option<SystemTime> {
    dirs.iter()
        .filter_map(|d| fs::metadata(d).ok()?.modified().ok())
        .max()
}

/// Return the desktop entry list, using a cache when valid.
///
/// A missing cache, a stale cache (application dirs newer), or a cache with
/// the wrong header (e.g. a stale C-tofi cache) all trigger a fresh scan and
/// re-write of the cache file.
pub fn entries_cached(dirs: &[PathBuf], cache_path: &Path) -> io::Result<Vec<DesktopEntry>> {
    match fs::metadata(cache_path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            tracing::debug!("drun: no cache found, scanning");
            let entries = scan(dirs);
            let _ = save_cache(&entries, cache_path);
            Ok(entries)
        }
        Err(e) => Err(e),
        Ok(cache_meta) => {
            let cache_mtime = cache_meta.modified()?;
            let stale = dirs_mtime(dirs).map(|m| m > cache_mtime).unwrap_or(false);
            if stale {
                tracing::debug!("drun: cache is stale, rescanning");
                let entries = scan(dirs);
                let _ = save_cache(&entries, cache_path);
                Ok(entries)
            } else {
                match load_cache(cache_path) {
                    Ok(entries) => Ok(entries),
                    Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                        tracing::warn!(
                            "drun: cache at {} has wrong format ({}), rescanning",
                            cache_path.display(),
                            e
                        );
                        let entries = scan(dirs);
                        let _ = save_cache(&entries, cache_path);
                        Ok(entries)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

/// Resolve the default cache file path.
pub fn default_cache_path() -> Option<PathBuf> {
    resolve_cache_path(std::env::var_os("XDG_CACHE_HOME"), std::env::var_os("HOME"))
}

fn resolve_cache_path(
    xdg_cache_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    if let Some(c) = xdg_cache_home {
        Some(PathBuf::from(c).join(CACHE_BASENAME))
    } else {
        Some(PathBuf::from(home?).join(".cache").join(CACHE_BASENAME))
    }
}

// ---------------------------------------------------------------------------
// Exec field-code expansion
// ---------------------------------------------------------------------------

/// Expand freedesktop.org `Exec` field codes and return the command string.
///
/// Handles `%i`, `%c`, `%k`; drops `%f`/`%F`/`%u`/`%U` and deprecated codes;
/// turns `%%` into a literal `%`.
pub fn exec_command(entry: &DesktopEntry) -> String {
    let mut out = String::with_capacity(entry.exec.len());
    let mut chars = entry.exec.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('i') if !entry.icon.is_empty() => {
                out.push_str("--icon ");
                out.push_str(&entry.icon);
            }
            Some('c') => out.push_str(&entry.name),
            Some('k') => out.push_str(&entry.path.display().to_string()),
            // %f/%F/%u/%U and deprecated codes are dropped (no files/URLs).
            Some(_) => {}
            None => {}
        }
    }

    out.trim().to_owned()
}

#[cfg(test)]
mod tests;
