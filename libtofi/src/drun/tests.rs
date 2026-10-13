//! Unit tests for `libtofi::drun`.
// The parent module has #![deny(unsafe_code)]; override here because tests
// need unsafe {{std::env::set_var}} / {{remove_var}} (made unsafe in Rust 1.87).
#![allow(unsafe_code)]

use super::*;
use std::fs;
use std::sync::Mutex;

// -- helpers ------------------------------------------------------------------

/// Global mutex that every test touching environment variables must hold for
/// its entire duration.  Because std::env is process-global state, tests
/// that mutate it must not run concurrently.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn write_desktop(dir: &Path, filename: &str, content: &str) -> PathBuf {
    let path = dir.join(filename);
    fs::write(&path, content).expect("write desktop file");
    path
}

fn basic_desktop(name: &str) -> String {
    format!(
        "[Desktop Entry]
Type=Application
Name={name}
Exec=app
"
    )
}

/// Build a HashMap<String,String> from a slice of (key, value) pairs.
fn locale_map(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// -- parse_entry --------------------------------------------------------------

#[test]
fn parse_basic_entry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "test.desktop",
        "[Desktop Entry]
Type=Application
Name=My App
Exec=myapp %u
Icon=myapp
Keywords=util;tool;
",
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
        "[Desktop Entry]
Name=Hidden
Exec=x
Hidden=true
",
    );
    assert!(parse_entry("h.desktop", &path).is_none());
}

#[test]
fn parse_no_display_returns_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "nodisplay.desktop",
        "[Desktop Entry]
Name=NoDisp
Exec=x
NoDisplay=true
",
    );
    assert!(parse_entry("nodisplay.desktop", &path).is_none());
}

#[test]
fn parse_terminal_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "term.desktop",
        "[Desktop Entry]
Name=Term App
Exec=myapp
Terminal=true
",
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
        "[Desktop Entry]
Exec=myapp
",
    );
    assert!(parse_entry("noname.desktop", &path).is_none());
}

#[test]
fn parse_only_desktop_entry_section_is_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "sec.desktop",
        "[Other Section]
Name=Wrong
[Desktop Entry]
Name=Correct
Exec=x
",
    );
    let entry = parse_entry("sec.desktop", &path).expect("parse");
    assert_eq!(entry.name, "Correct");
}

// -- scan ---------------------------------------------------------------------

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
        "[Desktop Entry]
Name=Hidden
Exec=x
Hidden=true
",
    );
    let entries = scan(&[dir.path().to_owned()]);
    assert!(entries.is_empty());
}

// -- cache round-trip ---------------------------------------------------------

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

// -- exec_command -------------------------------------------------------------

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
#[allow(deprecated)]
fn exec_plain_command_unchanged() {
    let e = entry_for_exec("myapp --flag");
    assert_eq!(exec_command(&e), "myapp --flag");
}

#[test]
#[allow(deprecated)]
fn exec_file_codes_removed() {
    let e = entry_for_exec("myapp %f");
    assert_eq!(exec_command(&e), "myapp");
    let e = entry_for_exec("myapp %F %u %U");
    assert_eq!(exec_command(&e), "myapp");
}

#[test]
#[allow(deprecated)]
fn exec_percent_percent_becomes_literal() {
    let e = entry_for_exec("echo %%");
    assert_eq!(exec_command(&e), "echo %");
}

#[test]
#[allow(deprecated)]
fn exec_icon_expansion() {
    let e = entry_for_exec("myapp %i");
    assert_eq!(exec_command(&e), "myapp --icon test-icon");
}

#[test]
#[allow(deprecated)]
fn exec_name_expansion() {
    let e = entry_for_exec("myapp %c");
    assert_eq!(exec_command(&e), "myapp Test App");
}

#[test]
#[allow(deprecated)]
fn exec_path_expansion() {
    let e = entry_for_exec("myapp %k");
    assert_eq!(exec_command(&e), "myapp /usr/share/applications/t.desktop");
}

#[test]
#[allow(deprecated)]
fn exec_no_icon_field_drops_percent_i() {
    let mut e = entry_for_exec("myapp %i");
    e.icon = String::new();
    assert_eq!(exec_command(&e), "myapp");
}

// -- resolve_cache_path (pure) ------------------------------------------------

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

// -- best_locale (pure, no env) -----------------------------------------------

#[test]
fn best_locale_exact_lang_territory_preferred() {
    // When both "en" and "en_US" keys are present the full key wins.
    let map = locale_map(&[("", "Default"), ("en", "English"), ("en_US", "English US")]);
    assert_eq!(best_locale(&map, "en", "US"), Some("English US"));
}

#[test]
fn best_locale_lang_only_when_no_territory_variant() {
    // "en_US" key absent -> fall back to "en".
    let map = locale_map(&[("", "Default"), ("en", "English")]);
    assert_eq!(best_locale(&map, "en", "US"), Some("English"));
}

#[test]
fn best_locale_fallback_to_unlocalized() {
    // Neither "en" nor "en_US" present -> use unlocalized empty key.
    let map = locale_map(&[("", "Default"), ("fr", "French")]);
    assert_eq!(best_locale(&map, "en", "US"), Some("Default"));
}

#[test]
fn best_locale_empty_map_returns_none() {
    let map: HashMap<String, String> = HashMap::new();
    assert!(best_locale(&map, "en", "US").is_none());
}

// -- current_locale -----------------------------------------------------------

#[test]
fn current_locale_lang_with_territory() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::remove_var("LANGUAGE");
        std::env::set_var("LANG", "en_US.UTF-8");
    }
    let (lang, territory) = current_locale();
    unsafe { std::env::remove_var("LANG") };
    assert_eq!(lang, "en");
    assert_eq!(territory, "US");
}

#[test]
fn current_locale_lang_without_territory() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::remove_var("LANGUAGE");
        std::env::set_var("LANG", "fr.UTF-8");
    }
    let (lang, territory) = current_locale();
    unsafe { std::env::remove_var("LANG") };
    assert_eq!(lang, "fr");
    assert_eq!(territory, "");
}

#[test]
fn current_locale_both_unset_returns_empty() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        std::env::remove_var("LANG");
        std::env::remove_var("LANGUAGE");
    }
    let (lang, territory) = current_locale();
    assert_eq!(lang, "");
    assert_eq!(territory, "");
}

// -- matches_current_desktop --------------------------------------------------

#[test]
fn matches_desktop_unset_returns_false() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::remove_var("XDG_CURRENT_DESKTOP") };
    assert!(!matches_current_desktop(&["GNOME".to_owned()]));
}

#[test]
fn matches_desktop_exact_match() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("XDG_CURRENT_DESKTOP", "GNOME") };
    let result = matches_current_desktop(&["GNOME".to_owned()]);
    unsafe { std::env::remove_var("XDG_CURRENT_DESKTOP") };
    assert!(result);
}

#[test]
fn matches_desktop_colon_separated_multi_value() {
    // "GNOME:KDE" -- searching for "KDE" must succeed.
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("XDG_CURRENT_DESKTOP", "GNOME:KDE") };
    let result = matches_current_desktop(&["KDE".to_owned()]);
    unsafe { std::env::remove_var("XDG_CURRENT_DESKTOP") };
    assert!(result);
}

#[test]
fn matches_desktop_no_overlap_returns_false() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("XDG_CURRENT_DESKTOP", "GNOME") };
    let result = matches_current_desktop(&["KDE".to_owned()]);
    unsafe { std::env::remove_var("XDG_CURRENT_DESKTOP") };
    assert!(!result);
}

// -- parse_entry: OnlyShowIn / NotShowIn --------------------------------------

#[test]
fn parse_only_show_in_matching_desktop_returns_some() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("XDG_CURRENT_DESKTOP", "GNOME") };
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "only_gnome.desktop",
        "[Desktop Entry]
Name=OnlyGnome
Exec=x
OnlyShowIn=GNOME;
",
    );
    let result = parse_entry("only_gnome.desktop", &path);
    unsafe { std::env::remove_var("XDG_CURRENT_DESKTOP") };
    assert!(result.is_some());
}

#[test]
fn parse_only_show_in_non_matching_returns_none() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("XDG_CURRENT_DESKTOP", "GNOME") };
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "only_kde.desktop",
        "[Desktop Entry]
Name=OnlyKDE
Exec=x
OnlyShowIn=KDE;
",
    );
    let result = parse_entry("only_kde.desktop", &path);
    unsafe { std::env::remove_var("XDG_CURRENT_DESKTOP") };
    assert!(result.is_none());
}

#[test]
fn parse_not_show_in_matching_desktop_returns_none() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("XDG_CURRENT_DESKTOP", "GNOME") };
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "not_gnome.desktop",
        "[Desktop Entry]
Name=NotGnome
Exec=x
NotShowIn=GNOME;
",
    );
    let result = parse_entry("not_gnome.desktop", &path);
    unsafe { std::env::remove_var("XDG_CURRENT_DESKTOP") };
    assert!(result.is_none());
}

#[test]
fn parse_not_show_in_non_matching_returns_some() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe { std::env::set_var("XDG_CURRENT_DESKTOP", "GNOME") };
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "not_kde.desktop",
        "[Desktop Entry]
Name=NotKDE
Exec=x
NotShowIn=KDE;
",
    );
    let result = parse_entry("not_kde.desktop", &path);
    unsafe { std::env::remove_var("XDG_CURRENT_DESKTOP") };
    assert!(result.is_some());
}

// -- split_args ---------------------------------------------------------------

#[test]
fn split_args_plain_whitespace() {
    assert_eq!(split_args("  app  -a\t-b "), ["app", "-a", "-b"]);
}

#[test]
fn split_args_empty_input() {
    assert!(split_args("").is_empty());
    assert!(split_args("   ").is_empty());
}

#[test]
fn split_args_double_quoted_space_is_one_arg() {
    assert_eq!(
        split_args(r#"chromium "--profile-directory=Profile 1" --app-id=abc"#),
        ["chromium", "--profile-directory=Profile 1", "--app-id=abc"]
    );
}

#[test]
fn split_args_quoted_program_path() {
    assert_eq!(
        split_args(r#""/opt/My App/bin/app" --flag"#),
        ["/opt/My App/bin/app", "--flag"]
    );
}

#[test]
fn split_args_quotes_join_adjacent_text() {
    assert_eq!(split_args(r#"--name="a b"c"#), ["--name=a bc"]);
}

#[test]
fn split_args_double_quote_escapes() {
    assert_eq!(
        split_args(r#""a\"b" "\`x\`" "\$HOME" "c\\d""#),
        ["a\"b", "`x`", "$HOME", "c\\d"]
    );
}

#[test]
fn split_args_other_backslash_in_double_quotes_kept() {
    assert_eq!(split_args(r#""a\nb""#), ["a\\nb"]);
}

#[test]
fn split_args_empty_quoted_arg_preserved() {
    assert_eq!(split_args(r#"app "" ''"#), ["app", "", ""]);
}

#[test]
fn split_args_single_quotes_lenient() {
    assert_eq!(split_args(r#"app 'a b' 'c\d'"#), ["app", "a b", "c\\d"]);
}

#[test]
fn split_args_backslash_outside_quotes_escapes_next() {
    assert_eq!(split_args(r"app a\ b"), ["app", "a b"]);
}

#[test]
fn split_args_unterminated_quote_runs_to_end() {
    assert_eq!(split_args(r#"app "a b"#), ["app", "a b"]);
}

#[test]
fn split_args_no_shell_expansion() {
    assert_eq!(split_args("echo $HOME *"), ["echo", "$HOME", "*"]);
}

// -- exec_args ----------------------------------------------------------------

/// Regression test for issue #61: Chromium PWA in a secondary profile.
#[test]
fn exec_args_quoted_arg_with_space() {
    let e = entry_for_exec(
        r#"/opt/google/chrome/chrome "--profile-directory=Profile 1" --app-id=abc %U"#,
    );
    assert_eq!(
        exec_args(&e),
        [
            "/opt/google/chrome/chrome",
            "--profile-directory=Profile 1",
            "--app-id=abc"
        ]
    );
}

#[test]
fn exec_args_drops_file_and_url_codes() {
    let e = entry_for_exec("app %f %F %u %U %d %D %n %N %v %m -x");
    assert_eq!(exec_args(&e), ["app", "-x"]);
}

#[test]
fn exec_args_drops_quoted_file_code() {
    let e = entry_for_exec(r#"app "%u""#);
    assert_eq!(exec_args(&e), ["app"]);
}

#[test]
fn exec_args_keeps_literal_empty_arg() {
    let e = entry_for_exec(r#"app """#);
    assert_eq!(exec_args(&e), ["app", ""]);
}

#[test]
fn exec_args_icon_expands_to_two_args() {
    let e = entry_for_exec("app %i");
    assert_eq!(exec_args(&e), ["app", "--icon", "test-icon"]);
}

#[test]
fn exec_args_icon_with_space_stays_one_arg() {
    let mut e = entry_for_exec("app %i");
    e.icon = "/home/u/My Icons/a.png".to_owned();
    assert_eq!(exec_args(&e), ["app", "--icon", "/home/u/My Icons/a.png"]);
}

#[test]
fn exec_args_icon_dropped_when_empty() {
    let mut e = entry_for_exec("app %i -x");
    e.icon = String::new();
    assert_eq!(exec_args(&e), ["app", "-x"]);
}

#[test]
fn exec_args_name_with_space_stays_one_arg() {
    let e = entry_for_exec("app --caption %c");
    assert_eq!(exec_args(&e), ["app", "--caption", "Test App"]);
}

#[test]
fn exec_args_name_with_quote_chars_is_literal() {
    let mut e = entry_for_exec(r#"app --caption %c "%c""#);
    e.name = r#"Don't "Starve""#.to_owned();
    assert_eq!(
        exec_args(&e),
        ["app", "--caption", r#"Don't "Starve""#, r#"Don't "Starve""#]
    );
}

#[test]
fn exec_args_path_code_in_place() {
    let mut e = entry_for_exec("app --desktop-file=%k");
    e.path = PathBuf::from("/home/u/My Apps/test.desktop");
    assert_eq!(
        exec_args(&e),
        ["app", "--desktop-file=/home/u/My Apps/test.desktop"]
    );
}

#[test]
fn exec_args_percent_escape() {
    let e = entry_for_exec("app 100%% %%u");
    assert_eq!(exec_args(&e), ["app", "100%", "%u"]);
}

#[test]
fn exec_args_general_string_escapes() {
    // `\s` is a space after unescaping, so quoting keeps it in one argument.
    let e = entry_for_exec(r#"app "a\sb""#);
    assert_eq!(exec_args(&e), ["app", "a b"]);
}

#[test]
fn exec_args_whitespace_string_escapes() {
    let e = entry_for_exec(r#"app "a\nb" "c\td" "e\rf""#);
    assert_eq!(exec_args(&e), ["app", "a\nb", "c\td", "e\rf"]);
}

#[test]
fn exec_args_escaped_quote_in_quotes() {
    // `\"` is not a general string escape, so it must reach the quote parser.
    let e = entry_for_exec(r#"sh -c "echo \"hi there\"""#);
    assert_eq!(exec_args(&e), ["sh", "-c", r#"echo "hi there""#]);
}

#[test]
fn exec_args_trailing_lone_percent_kept() {
    let e = entry_for_exec("app 50%");
    assert_eq!(exec_args(&e), ["app", "50%"]);
}

#[test]
fn exec_args_double_level_backslash_unescape() {
    // In the file, a literal backslash inside a quoted arg is written `\\\\`:
    // string unescape gives `\\`, quote unescape gives `\`.
    let e = entry_for_exec(r#"app "C:\\\\dir""#);
    assert_eq!(exec_args(&e), ["app", r"C:\dir"]);
}

#[test]
fn exec_args_escaped_dollar_in_quotes() {
    let e = entry_for_exec(r#"sh -c "echo \\$HOME""#);
    assert_eq!(exec_args(&e), ["sh", "-c", "echo $HOME"]);
}

#[test]
fn exec_args_empty_exec() {
    assert!(exec_args(&entry_for_exec("")).is_empty());
}

/// `exec_args` must work on the raw value stored by `parse_entry`.
#[test]
fn exec_args_from_parsed_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_desktop(
        dir.path(),
        "pwa.desktop",
        r#"[Desktop Entry]
Type=Application
Name=PWA
Exec=chromium "--profile-directory=Profile 1" --app-id=abc %U
"#,
    );
    let entry = parse_entry("pwa.desktop", &path).expect("parse");
    assert_eq!(
        exec_args(&entry),
        ["chromium", "--profile-directory=Profile 1", "--app-id=abc"]
    );
}

// -- application_dirs ---------------------------------------------------------

#[test]
fn application_dirs_includes_xdg_data_home() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let saved = std::env::var_os("XDG_DATA_HOME");
    unsafe { std::env::set_var("XDG_DATA_HOME", "/tmp/datahome") };
    let dirs = application_dirs();
    unsafe {
        match saved {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
    }
    assert!(
        dirs.contains(&PathBuf::from("/tmp/datahome/applications")),
        "expected /tmp/datahome/applications in {dirs:?}"
    );
}

#[test]
fn application_dirs_includes_xdg_data_dirs_entries() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let saved_data_home = std::env::var_os("XDG_DATA_HOME");
    let saved_data_dirs = std::env::var_os("XDG_DATA_DIRS");
    unsafe {
        std::env::remove_var("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_DIRS", "/a:/b");
    }
    let dirs = application_dirs();
    unsafe {
        if let Some(v) = saved_data_home {
            std::env::set_var("XDG_DATA_HOME", v)
        }
        match saved_data_dirs {
            Some(v) => std::env::set_var("XDG_DATA_DIRS", v),
            None => std::env::remove_var("XDG_DATA_DIRS"),
        }
    }
    assert!(
        dirs.contains(&PathBuf::from("/a/applications")),
        "expected /a/applications in {dirs:?}"
    );
    assert!(
        dirs.contains(&PathBuf::from("/b/applications")),
        "expected /b/applications in {dirs:?}"
    );
}

// -- entries_cached: wrong-header cache ---------------------------------------

/// When the cache file exists but starts with the wrong first line (e.g. a
/// stale C-tofi cache), entries_cached must detect the bad header via
/// load_cache -> InvalidData, discard the old cache, rescan the application
/// directories, and rewrite the cache file with the correct header.
#[test]
fn entries_cached_wrong_header_rescans_and_rewrites_cache() {
    let tmp = tempfile::tempdir().expect("tempdir");

    // Create an applications sub-directory containing one .desktop file.
    let appdir = tmp.path().join("applications");
    fs::create_dir(&appdir).expect("create appdir");
    write_desktop(&appdir, "foo.desktop", &basic_desktop("Foo"));

    // Write the cache file after creating the desktop file so its mtime is
    // >= the directory mtime.  This steers the code away from the "stale"
    // branch and towards the wrong-header branch (load_cache -> InvalidData).
    // Even if the filesystem assigns identical timestamps to both files and
    // the stale branch fires instead, the end-state assertions still hold.
    let cache_path = tmp.path().join("tofi-drun-test");
    fs::write(
        &cache_path,
        "#stale-c-tofi-cache
some-app Some App /path  app  1
",
    )
    .expect("write bad cache");

    let entries = entries_cached(&[appdir], &cache_path).expect("entries_cached");

    assert_eq!(
        entries.len(),
        1,
        "expected exactly one entry, got {entries:?}"
    );
    assert_eq!(entries[0].name, "Foo");

    // Cache must have been rewritten with the correct header.
    let first_line = fs::read_to_string(&cache_path)
        .expect("read rewritten cache")
        .lines()
        .next()
        .unwrap_or("")
        .to_owned();
    assert_eq!(
        first_line, "#tofi-rs-drun-v1",
        "cache header wrong after rewrite: {first_line:?}"
    );
}
