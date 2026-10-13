// ── run_commands helpers ──────────────────────────────────────────────────────

#[cfg(feature = "renderer")]
mod run_commands_tests {
    use super::super::run_utils::{commands_cached, load_cache, save_cache, scan};
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::Path;

    fn make_exe(dir: &Path, name: &str) {
        let path = dir.join(name);
        fs::write(&path, b"#!/bin/sh\n").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
    }

    fn make_file(dir: &Path, name: &str) {
        let path = dir.join(name);
        fs::write(&path, b"data").unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&path, perms).unwrap();
    }

    #[test]
    fn scan_returns_executable_names() {
        let dir = tempfile::tempdir().unwrap();
        make_exe(dir.path(), "foo");
        make_exe(dir.path(), "bar");
        let result = scan(dir.path().to_str().unwrap());
        assert!(result.contains(&"foo".to_owned()));
        assert!(result.contains(&"bar".to_owned()));
    }

    #[test]
    fn scan_excludes_non_executable_files() {
        let dir = tempfile::tempdir().unwrap();
        make_exe(dir.path(), "yes");
        make_file(dir.path(), "no");
        let result = scan(dir.path().to_str().unwrap());
        assert!(result.contains(&"yes".to_owned()));
        assert!(!result.contains(&"no".to_owned()));
    }

    #[test]
    fn scan_result_is_sorted() {
        let dir = tempfile::tempdir().unwrap();
        for name in &["zzz", "aaa", "mmm"] {
            make_exe(dir.path(), name);
        }
        let result = scan(dir.path().to_str().unwrap());
        let mut sorted = result.clone();
        sorted.sort();
        assert_eq!(result, sorted);
    }

    #[test]
    fn scan_deduplicates_across_path_dirs() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
        make_exe(dir.path(), "real");
        let path_var = format!("/nonexistent/path:{}", dir.path().display());
        let result = scan(&path_var);
        assert!(result.contains(&"real".to_owned()));
    }

    #[test]
    fn scan_empty_path_returns_empty() {
        assert!(scan("").is_empty());
    }

    #[test]
    fn cache_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("tofi-compgen");
        let commands = vec!["bash".to_owned(), "fish".to_owned(), "zsh".to_owned()];
        save_cache(&commands, &path).unwrap();
        let loaded = load_cache(&path).unwrap();
        assert_eq!(loaded, commands);
    }

    #[test]
    fn save_cache_creates_parent_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub/dir/tofi-compgen");
        save_cache(&[], &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn commands_cached_creates_cache_when_missing() {
        let exe_dir = tempfile::tempdir().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        make_exe(exe_dir.path(), "myapp");
        let cache_path = cache_dir.path().join("tofi-compgen");
        let result = commands_cached(exe_dir.path().to_str().unwrap(), &cache_path).unwrap();
        assert!(result.contains(&"myapp".to_owned()));
        assert!(cache_path.exists());
    }

    #[test]
    fn commands_cached_returns_result_after_update() {
        let exe_dir = tempfile::tempdir().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        let cache_path = cache_dir.path().join("tofi-compgen");
        make_exe(exe_dir.path(), "newapp");
        let result = commands_cached(exe_dir.path().to_str().unwrap(), &cache_path).unwrap();
        assert!(!result.is_empty());
    }
}

// ── launch_argv ──────────────────────────────────────────────────────────────

#[cfg(feature = "wayland")]
mod launch_argv_tests {
    use super::super::submit_utils::launch_argv;
    use libtofi_rs::drun::DesktopEntry;
    use std::path::PathBuf;

    fn entry(exec: &str, terminal: bool) -> DesktopEntry {
        DesktopEntry {
            id: "t.desktop".into(),
            name: "Test App".into(),
            path: PathBuf::from("/usr/share/applications/t.desktop"),
            keywords: String::new(),
            exec: exec.to_owned(),
            icon: String::new(),
            terminal,
        }
    }

    #[test]
    fn gui_app_ignores_terminal() {
        let e = entry(r#"chromium "--profile-directory=Profile 1""#, false);
        assert_eq!(
            launch_argv(&e, Some("foot")),
            ["chromium", "--profile-directory=Profile 1"]
        );
    }

    #[test]
    fn terminal_app_defaults_to_xterm() {
        let e = entry("htop", true);
        assert_eq!(launch_argv(&e, None), ["xterm", "htop"]);
    }

    #[test]
    fn terminal_app_empty_terminal_falls_back_to_xterm() {
        let e = entry("htop", true);
        assert_eq!(launch_argv(&e, Some("")), ["xterm", "htop"]);
    }

    #[test]
    fn terminal_app_terminal_with_args_and_quotes() {
        let e = entry(r#"vim "my file.txt""#, true);
        assert_eq!(
            launch_argv(&e, Some(r#""/opt/My Term/term" -e"#)),
            ["/opt/My Term/term", "-e", "vim", "my file.txt"]
        );
    }

    #[test]
    fn empty_exec_gives_empty_argv() {
        assert!(launch_argv(&entry("", false), None).is_empty());
    }
}
