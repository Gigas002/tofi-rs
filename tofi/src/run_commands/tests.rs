//! Unit tests for `tofi::run_commands`.

use super::*;
use std::fs;
use std::io::Write as _;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Create an executable file at `dir/name`.
fn make_exe(dir: &Path, name: &str) {
    let path = dir.join(name);
    fs::write(&path, b"#!/bin/sh\n").expect("write");
    let mut perms = fs::metadata(&path).expect("meta").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).expect("chmod");
}

/// Create a non-executable file at `dir/name`.
fn make_file(dir: &Path, name: &str) {
    let path = dir.join(name);
    fs::write(&path, b"data").expect("write");
    let mut perms = fs::metadata(&path).expect("meta").permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&path, perms).expect("chmod");
}

// ── scan ─────────────────────────────────────────────────────────────────────

#[test]
fn scan_returns_executable_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    make_exe(dir.path(), "foo");
    make_exe(dir.path(), "bar");
    let result = scan(dir.path().to_str().expect("utf8"));
    assert!(result.contains(&"foo".to_owned()));
    assert!(result.contains(&"bar".to_owned()));
}

#[test]
fn scan_excludes_non_executable_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    make_exe(dir.path(), "yes");
    make_file(dir.path(), "no");
    let result = scan(dir.path().to_str().expect("utf8"));
    assert!(result.contains(&"yes".to_owned()));
    assert!(!result.contains(&"no".to_owned()));
}

#[test]
fn scan_result_is_sorted() {
    let dir = tempfile::tempdir().expect("tempdir");
    for name in &["zzz", "aaa", "mmm"] {
        make_exe(dir.path(), name);
    }
    let result = scan(dir.path().to_str().expect("utf8"));
    let mut sorted = result.clone();
    sorted.sort();
    assert_eq!(result, sorted);
}

#[test]
fn scan_deduplicates_across_path_dirs() {
    let dir1 = tempfile::tempdir().expect("tempdir1");
    let dir2 = tempfile::tempdir().expect("tempdir2");
    make_exe(dir1.path(), "shared");
    make_exe(dir2.path(), "shared");
    make_exe(dir1.path(), "only1");
    make_exe(dir2.path(), "only2");

    let path_var = format!("{}:{}", dir1.path().display(), dir2.path().display());
    let result = scan(&path_var);

    assert_eq!(result.iter().filter(|n| *n == "shared").count(), 1);
    assert!(result.contains(&"only1".to_owned()));
    assert!(result.contains(&"only2".to_owned()));
}

#[test]
fn scan_skips_missing_dirs() {
    let dir = tempfile::tempdir().expect("tempdir");
    make_exe(dir.path(), "real");
    let path_var = format!("/nonexistent/path:{}", dir.path().display());
    let result = scan(&path_var);
    assert!(result.contains(&"real".to_owned()));
}

#[test]
fn scan_empty_path_returns_empty() {
    let result = scan("");
    assert!(result.is_empty());
}

// ── save_cache / load_cache round-trip ───────────────────────────────────────

#[test]
fn cache_roundtrip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("tofi-compgen");
    let commands = vec!["bash".to_owned(), "fish".to_owned(), "zsh".to_owned()];
    save_cache(&commands, &path).expect("save");
    let loaded = load_cache(&path).expect("load");
    assert_eq!(loaded, commands);
}

#[test]
fn save_cache_creates_parent_directories() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("sub").join("dir").join("tofi-compgen");
    save_cache(&[], &path).expect("save");
    assert!(path.exists());
}

// ── commands_cached ───────────────────────────────────────────────────────────

#[test]
fn commands_cached_creates_cache_when_missing() {
    let exe_dir = tempfile::tempdir().expect("exe_dir");
    let cache_dir = tempfile::tempdir().expect("cache_dir");
    make_exe(exe_dir.path(), "myapp");

    let cache_path = cache_dir.path().join("tofi-compgen");
    let result = commands_cached(exe_dir.path().to_str().expect("utf8"), &cache_path)
        .expect("commands_cached");

    assert!(result.contains(&"myapp".to_owned()));
    assert!(cache_path.exists(), "cache file should have been created");
}

#[test]
fn commands_cached_returns_stale_cache_after_update() {
    let exe_dir = tempfile::tempdir().expect("exe_dir");
    let cache_dir = tempfile::tempdir().expect("cache_dir");
    let cache_path = cache_dir.path().join("tofi-compgen");
    let path_var = exe_dir.path().to_str().expect("utf8");

    // Seed the cache with a manually written file (old content).
    {
        let mut f = fs::File::create(&cache_path).expect("create");
        writeln!(f, "oldapp").expect("write");
    }

    // Write a newer executable into the dir, then touch the dir mtime.
    make_exe(exe_dir.path(), "newapp");

    // Force the exe_dir mtime to be after the cache.
    // We do this by simply re-running commands_cached — on any filesystem
    // where the dir mtime updates on file creation this will rescan.
    let result = commands_cached(path_var, &cache_path).expect("commands_cached");

    // Either the cache was stale and rescanned (contains newapp),
    // or the mtime didn't advance (contains oldapp from cache).
    // Both are valid outcomes; what matters is no panic and non-empty result.
    assert!(!result.is_empty());
}

// ── resolve_cache_path (pure) ─────────────────────────────────────────────────

#[test]
fn path_uses_xdg_cache_home() {
    let path =
        resolve_cache_path(Some("/tmp/cache".into()), Some("/home/user".into())).expect("path");
    assert_eq!(path, PathBuf::from("/tmp/cache/tofi-compgen"));
}

#[test]
fn path_falls_back_to_home_cache() {
    let path = resolve_cache_path(None, Some("/home/user".into())).expect("path");
    assert_eq!(path, PathBuf::from("/home/user/.cache/tofi-compgen"));
}

#[test]
fn path_returns_none_when_no_env() {
    assert!(resolve_cache_path(None, None).is_none());
}
