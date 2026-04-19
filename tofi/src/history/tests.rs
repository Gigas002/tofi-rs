//! Unit tests for `tofi::history`.

use super::*;

// ── helpers ──────────────────────────────────────────────────────────────────

fn tempfile() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("history");
    (dir, path)
}

// ── AppHistory::add ─────────────────────────────────────────────────────────────

#[test]
fn add_new_entry_gets_count_one() {
    let mut h = AppHistory::new();
    h.add("foo");
    assert_eq!(h.entries().len(), 1);
    assert_eq!(h.entries()[0].name, "foo");
    assert_eq!(h.entries()[0].run_count, 1);
}

#[test]
fn add_same_entry_increments_count() {
    let mut h = AppHistory::new();
    h.add("foo");
    h.add("foo");
    assert_eq!(h.entries().len(), 1);
    assert_eq!(h.entries()[0].run_count, 2);
}

#[test]
fn add_bubbles_up_when_count_exceeds_predecessor() {
    let mut h = AppHistory::new();
    h.add("a"); // count 1
    h.add("b"); // count 1  → order: a, b
    h.add("b"); // count 2  → b bubbles past a → order: b, a
    assert_eq!(h.entries()[0].name, "b");
    assert_eq!(h.entries()[1].name, "a");
}

#[test]
fn add_does_not_reorder_on_equal_counts() {
    // If a has count 2 and b reaches count 2, b should NOT jump past a
    // (bubble stops when count <= predecessor's count).
    let mut h = AppHistory::new();
    h.add("a");
    h.add("a"); // a→2
    h.add("b");
    h.add("b"); // b→2 (equal, no swap)
    assert_eq!(h.entries()[0].name, "a");
    assert_eq!(h.entries()[1].name, "b");
}

#[test]
fn add_multiple_distinct_entries() {
    let mut h = AppHistory::new();
    for name in &["c", "a", "b"] {
        h.add(name);
    }
    assert_eq!(h.entries().len(), 3);
    // All have count 1, insertion order preserved.
    let names: Vec<&str> = h.entries().iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["c", "a", "b"]);
}

#[test]
fn add_bubbles_to_front() {
    let mut h = AppHistory::new();
    h.add("x");
    h.add("x");
    h.add("x"); // x→3
    h.add("y");
    h.add("y"); // y→2
    h.add("z"); // z→1
    // Expected: x(3), y(2), z(1)
    assert_eq!(h.entries()[0].name, "x");
    assert_eq!(h.entries()[1].name, "y");
    assert_eq!(h.entries()[2].name, "z");

    // Now run z four times → z(5) should jump to front.
    h.add("z");
    h.add("z");
    h.add("z");
    h.add("z"); // z→5
    assert_eq!(h.entries()[0].name, "z");
}

// ── AppHistory::remove ───────────────────────────────────────────────────────────

#[test]
fn remove_existing_entry() {
    let mut h = AppHistory::new();
    h.add("foo");
    h.add("bar");
    h.remove("foo");
    assert_eq!(h.entries().len(), 1);
    assert_eq!(h.entries()[0].name, "bar");
}

#[test]
fn remove_nonexistent_is_noop() {
    let mut h = AppHistory::new();
    h.add("foo");
    h.remove("bar"); // should not panic
    assert_eq!(h.entries().len(), 1);
}

// ── save / load round-trip ────────────────────────────────────────────────────

#[test]
fn roundtrip_single_entry() {
    let (_dir, path) = tempfile();
    let mut h = AppHistory::new();
    h.add("bash");
    save(&h, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(loaded.entries(), h.entries());
}

#[test]
fn roundtrip_multiple_entries_preserves_order_and_counts() {
    let (_dir, path) = tempfile();
    let mut h = AppHistory::new();
    // Build an ordered list: foo(3), bar(2), baz(1).
    for _ in 0..3 {
        h.add("foo");
    }
    for _ in 0..2 {
        h.add("bar");
    }
    h.add("baz");

    save(&h, &path).expect("save");
    let loaded = load(&path).expect("load");

    assert_eq!(loaded.entries().len(), 3);
    assert_eq!(
        loaded.entries()[0],
        AppHistoryEntry {
            name: "foo".into(),
            run_count: 3
        }
    );
    assert_eq!(
        loaded.entries()[1],
        AppHistoryEntry {
            name: "bar".into(),
            run_count: 2
        }
    );
    assert_eq!(
        loaded.entries()[2],
        AppHistoryEntry {
            name: "baz".into(),
            run_count: 1
        }
    );
}

#[test]
fn load_missing_file_returns_empty_history() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("no-such-file");
    let h = load(&path).expect("load of missing file should succeed");
    assert!(h.entries().is_empty());
}

#[test]
fn save_creates_parent_directories() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a").join("b").join("history");
    let h = AppHistory::new(); // empty is fine
    save(&h, &path).expect("save should create parents");
    assert!(path.exists());
}

#[test]
fn file_permissions_are_0600() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, path) = tempfile();
    save(&AppHistory::new(), &path).expect("save");
    let meta = std::fs::metadata(&path).expect("metadata");
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
}

// ── load: file format edge cases ─────────────────────────────────────────────

#[test]
fn load_handles_entries_with_spaces_in_name() {
    use std::io::Write as _;
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("history");
    {
        let mut f = std::fs::File::create(&path).expect("create");
        writeln!(f, "5 my program").expect("write");
    }
    let h = load(&path).expect("load");
    assert_eq!(h.entries().len(), 1);
    assert_eq!(h.entries()[0].name, "my program");
    assert_eq!(h.entries()[0].run_count, 5);
}

#[test]
fn load_skips_malformed_lines() {
    use std::io::Write as _;
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("history");
    {
        let mut f = std::fs::File::create(&path).expect("create");
        // Good line, a bad line with no space, and another good line.
        writeln!(f, "3 firefox").expect("write");
        writeln!(f, "badline").expect("write");
        writeln!(f, "1 alacritty").expect("write");
    }
    let h = load(&path).expect("load");
    assert_eq!(h.entries().len(), 2);
    assert_eq!(h.entries()[0].name, "firefox");
    assert_eq!(h.entries()[1].name, "alacritty");
}

// ── resolve_history_path (pure) ──────────────────────────────────────────────

#[test]
fn path_uses_xdg_state_home_when_set() {
    let path = resolve_history_path(
        Some("/tmp/xdg_state".into()),
        Some("/home/user".into()),
        false,
    )
    .expect("path");
    assert_eq!(
        path,
        std::path::PathBuf::from("/tmp/xdg_state/tofi-history")
    );
}

#[test]
fn path_falls_back_to_home_local_state() {
    let path = resolve_history_path(None, Some("/home/user".into()), false).expect("path");
    assert_eq!(
        path,
        std::path::PathBuf::from("/home/user/.local/state/tofi-history")
    );
}

#[test]
fn path_drun_uses_drun_basename() {
    let path = resolve_history_path(
        Some("/tmp/xdg_state".into()),
        Some("/home/user".into()),
        true,
    )
    .expect("path");
    assert_eq!(
        path,
        std::path::PathBuf::from("/tmp/xdg_state/tofi-drun-history")
    );
}

#[test]
fn path_returns_none_when_neither_env_set() {
    let path = resolve_history_path(None, None, false);
    assert!(path.is_none());
}
