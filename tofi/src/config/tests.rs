use super::*;

#[test]
fn load_missing_file_gives_defaults() {
    let cfg = load(std::path::Path::new("/nonexistent/tofi/config.toml")).0; // reported, see below
    assert!(cfg.base.output.is_none());
    assert!(cfg.matching.algorithm.is_none());
    assert!(cfg.history.history.is_none());
}

#[test]
fn load_empty_file_gives_defaults() {
    let f = tempfile::NamedTempFile::new().unwrap();
    let cfg = load(f.path()).0;
    assert!(cfg.base.output.is_none());
    assert!(cfg.matching.require.is_none());
}

#[test]
fn load_matching_section() {
    use std::io::Write as _;
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"[matching]\nalgorithm = \"fuzzy\"\nrequire = false\nascii = true\n")
        .unwrap();
    let cfg = load(f.path()).0;
    assert_eq!(cfg.matching.algorithm.as_deref(), Some("fuzzy"));
    assert_eq!(cfg.matching.require, Some(false));
    assert_eq!(cfg.matching.ascii, Some(true));
}

#[test]
fn load_history_section() {
    use std::io::Write as _;
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"[history]\nhistory = false\npath = \"/tmp/hist\"\n")
        .unwrap();
    let cfg = load(f.path()).0;
    assert_eq!(cfg.history.history, Some(false));
    assert_eq!(cfg.history.path.as_deref(), Some("/tmp/hist"));
}

#[test]
fn load_base_section() {
    use std::io::Write as _;
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"[base]\noutput = \"HDMI-1\"\nterminal = \"alacritty\"\ntheme = \"Sweet.toml\"\n")
        .unwrap();
    let cfg = load(f.path()).0;
    assert_eq!(cfg.base.output.as_deref(), Some("HDMI-1"));
    assert_eq!(cfg.base.terminal.as_deref(), Some("alacritty"));
    assert_eq!(cfg.base.theme.as_deref(), Some("Sweet.toml"));
}

fn examples_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
}

#[test]
fn load_example_config_toml() {
    let path = examples_dir().join("config/config.toml");
    if !path.exists() {
        return;
    }
    let cfg = load(&path).0;
    // [matching] require = true, ascii = false
    assert_eq!(cfg.matching.require, Some(true));
    assert_eq!(cfg.matching.ascii, Some(false));
    // [history] history = true
    assert_eq!(cfg.history.history, Some(true));
}

// ── Problems ──────────────────────────────────────────────────────────────────

fn load_str(toml: &str) -> (Config, Vec<Problem>) {
    let f = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(f.path(), toml).unwrap();
    load(f.path())
}

#[test]
fn unknown_keys_are_reported_with_their_section() {
    let (cfg, problems) =
        load_str("[base]\nlauncher = \"run\"\n[matching]\nalgorithm = \"fuzzy\"\n");
    assert_eq!(cfg.matching.algorithm.as_deref(), Some("fuzzy"));
    assert_eq!(problems.len(), 1);
    let Problem::UnknownKey(msg) = &problems[0] else {
        panic!("{problems:?}");
    };
    assert!(
        msg.ends_with("unknown key `base.launcher` ignored"),
        "{msg}"
    );
}

#[test]
fn unknown_sections_are_reported() {
    let (_, problems) = load_str("[colours]\nx = 1\n");
    assert!(matches!(&problems[..], [Problem::UnknownKey(m)] if m.contains("`colours`")));
}

#[test]
fn invalid_toml_is_reported_and_gives_defaults() {
    let (cfg, problems) = load_str("[matching]\nrequire = \"yes\"\nalgorithm = \"fuzzy\"\n");
    assert!(cfg.matching.algorithm.is_none());
    let [Problem::Unusable(msg)] = &problems[..] else {
        panic!("{problems:?}");
    };
    assert!(msg.contains("none of it is used"), "{msg}");
    assert!(msg.contains("line 2"), "the error says where: {msg}");
}

#[test]
fn an_unreadable_file_is_reported() {
    let (_, problems) = load(std::path::Path::new("/nonexistent/tofi/config.toml"));
    assert!(matches!(&problems[..], [Problem::Unusable(m)] if m.contains("can't read")));
}

#[test]
fn a_clean_file_has_no_problems() {
    assert!(load_str("[base]\nmode = \"run\"\n").1.is_empty());
}

#[test]
fn the_example_files_have_no_problems() {
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    for name in [
        "config/config.toml",
        "config/maximal.toml",
        "config/minimal.toml",
    ] {
        let (_, problems) = load(&examples.join(name));
        assert!(problems.is_empty(), "{name}: {problems:?}");
    }
    for name in [
        "theme/theme.toml",
        "theme/maximal.toml",
        "theme/minimal.toml",
    ] {
        let (_, problems) = crate::theme::load(&examples.join(name));
        assert!(problems.is_empty(), "{name}: {problems:?}");
    }
}
