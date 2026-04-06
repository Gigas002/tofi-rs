//! Unit tests for `libtofi::drun`.

use super::*;
use std::fs;

// ── helpers ───────────────────────────────────────────────────────────────────

fn write_desktop(dir: &Path, filename: &str, content: &str) -> PathBuf {
    let path = dir.join(filename);
    fs::write(&path, content).expect("write desktop file");
    path
}

fn basic_desktop(name: &str) -> String {
    format!("[Desktop Entry]\nType=Application\nName={name}\nExec=app\n")
}

// ── parse_entry ───────────────────────────────────────────────────────────────

#[test]
fn parse_basic_entry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "test.desktop",
        "[Desktop Entry]\nType=Application\nName=My App\nExec=myapp %u\nIcon=myapp\nKeywords=util;tool;\n",
    );
    let entry = parse_entry("test.desktop", &path).expect("parse");
    assert_eq!(entry.name, "My App");
    assert_eq!(entry.exec, "myapp %u");
    assert_eq!(entry.icon, "myapp");
    assert_eq!(entry.keywords, "util;tool;");
    assert!(!entry.terminal);
}

#[test]
fn parse_hidden_returns_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "h.desktop",
        "[Desktop Entry]\nName=Hidden\nExec=x\nHidden=true\n",
    );
    assert!(parse_entry("h.desktop", &path).is_none());
}

#[test]
fn parse_no_display_returns_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "nd.desktop",
        "[Desktop Entry]\nName=NoDisp\nExec=x\nNoDisplay=true\n",
    );
    assert!(parse_entry("nd.desktop", &path).is_none());
}

#[test]
fn parse_terminal_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "term.desktop",
        "[Desktop Entry]\nName=Term App\nExec=myapp\nTerminal=true\n",
    );
    let entry = parse_entry("term.desktop", &path).expect("parse");
    assert!(entry.terminal);
}

#[test]
fn parse_missing_name_returns_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "noname.desktop",
        "[Desktop Entry]\nExec=myapp\n",
    );
    assert!(parse_entry("noname.desktop", &path).is_none());
}

#[test]
fn parse_only_desktop_entry_section_is_read() {
    // A stray Name in another section must not be picked up.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "sec.desktop",
        "[Other Section]\nName=Wrong\n[Desktop Entry]\nName=Correct\nExec=x\n",
    );
    let entry = parse_entry("sec.desktop", &path).expect("parse");
    assert_eq!(entry.name, "Correct");
}

// ── scan ─────────────────────────────────────────────────────────────────────

#[test]
fn scan_finds_desktop_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_desktop(dir.path(), "a.desktop", &basic_desktop("Alpha"));
    write_desktop(dir.path(), "b.desktop", &basic_desktop("Beta"));
    let entries = scan(&[dir.path().to_owned()]);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Alpha"));
    assert!(names.contains(&"Beta"));
}

#[test]
fn scan_result_is_sorted_by_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_desktop(dir.path(), "z.desktop", &basic_desktop("Zebra"));
    write_desktop(dir.path(), "a.desktop", &basic_desktop("Apple"));
    let entries = scan(&[dir.path().to_owned()]);
    assert_eq!(entries[0].name, "Apple");
    assert_eq!(entries[1].name, "Zebra");
}

#[test]
fn scan_higher_precedence_dir_wins() {
    let high = tempfile::tempdir().expect("high");
    let low = tempfile::tempdir().expect("low");
    // Same ID in both dirs — first dir has higher precedence.
    write_desktop(high.path(), "app.desktop", &basic_desktop("HighApp"));
    write_desktop(low.path(), "app.desktop", &basic_desktop("LowApp"));
    let entries = scan(&[high.path().to_owned(), low.path().to_owned()]);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "HighApp");
}

#[test]
fn scan_skips_hidden_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_desktop(
        dir.path(),
        "h.desktop",
        "[Desktop Entry]\nName=Hidden\nExec=x\nHidden=true\n",
    );
    let entries = scan(&[dir.path().to_owned()]);
    assert!(entries.is_empty());
}

// ── cache round-trip ──────────────────────────────────────────────────────────

#[test]
fn cache_roundtrip_preserves_all_fields() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = dir.path().join("tofi-drun");
    let entries = vec![DesktopEntry {
        id: "foo.desktop".into(),
        name: "Foo".into(),
        path: PathBuf::from("/usr/share/applications/foo.desktop"),
        keywords: "util;".into(),
        exec: "foo %u".into(),
        icon: "foo-icon".into(),
        terminal: false,
    }];
    save_cache(&entries, &cache).expect("save");
    let loaded = load_cache(&cache).expect("load");
    assert_eq!(loaded, entries);
}

#[test]
fn cache_roundtrip_terminal_true() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = dir.path().join("tofi-drun");
    let entries = vec![DesktopEntry {
        id: "term.desktop".into(),
        name: "Terminal App".into(),
        path: PathBuf::from("/usr/share/applications/term.desktop"),
        keywords: String::new(),
        exec: "myapp".into(),
        icon: String::new(),
        terminal: true,
    }];
    save_cache(&entries, &cache).expect("save");
    let loaded = load_cache(&cache).expect("load");
    assert!(loaded[0].terminal);
}

// ── exec_command ──────────────────────────────────────────────────────────────

fn entry_for_exec(exec: &str) -> DesktopEntry {
    DesktopEntry {
        id: "t.desktop".into(),
        name: "Test App".into(),
        path: PathBuf::from("/usr/share/applications/t.desktop"),
        keywords: String::new(),
        exec: exec.to_owned(),
        icon: "test-icon".into(),
        terminal: false,
    }
}

#[test]
fn exec_plain_command_unchanged() {
    let e = entry_for_exec("myapp --flag");
    assert_eq!(exec_command(&e), "myapp --flag");
}

#[test]
fn exec_file_codes_removed() {
    let e = entry_for_exec("myapp %f");
    assert_eq!(exec_command(&e), "myapp");
    let e = entry_for_exec("myapp %F %u %U");
    assert_eq!(exec_command(&e), "myapp");
}

#[test]
fn exec_percent_percent_becomes_literal() {
    let e = entry_for_exec("echo %%");
    assert_eq!(exec_command(&e), "echo %");
}

#[test]
fn exec_icon_expansion() {
    let e = entry_for_exec("myapp %i");
    assert_eq!(exec_command(&e), "myapp --icon test-icon");
}

#[test]
fn exec_name_expansion() {
    let e = entry_for_exec("myapp %c");
    assert_eq!(exec_command(&e), "myapp Test App");
}

#[test]
fn exec_path_expansion() {
    let e = entry_for_exec("myapp %k");
    assert_eq!(exec_command(&e), "myapp /usr/share/applications/t.desktop");
}

#[test]
fn exec_no_icon_field_drops_percent_i() {
    let mut e = entry_for_exec("myapp %i");
    e.icon = String::new();
    assert_eq!(exec_command(&e), "myapp");
}

// ── resolve_cache_path (pure) ─────────────────────────────────────────────────

#[test]
fn path_uses_xdg_cache_home() {
    let p = resolve_cache_path(Some("/tmp/cache".into()), Some("/home/user".into())).expect("path");
    assert_eq!(p, PathBuf::from("/tmp/cache/tofi-drun"));
}

#[test]
fn path_falls_back_to_home_cache() {
    let p = resolve_cache_path(None, Some("/home/user".into())).expect("path");
    assert_eq!(p, PathBuf::from("/home/user/.cache/tofi-drun"));
}

#[test]
fn path_returns_none_without_env() {
    assert!(resolve_cache_path(None, None).is_none());
}
