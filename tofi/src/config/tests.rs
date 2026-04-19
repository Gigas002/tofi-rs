use super::*;

#[test]
fn load_missing_file_gives_defaults() {
    let cfg = load(std::path::Path::new("/nonexistent/tofi/config.toml"));
    assert!(cfg.base.output.is_none());
    assert!(cfg.matching.algorithm.is_none());
    assert!(cfg.history.history.is_none());
}

#[test]
fn load_empty_file_gives_defaults() {
    let f = tempfile::NamedTempFile::new().unwrap();
    let cfg = load(f.path());
    assert!(cfg.base.output.is_none());
    assert!(cfg.matching.require.is_none());
}

#[test]
fn load_matching_section() {
    use std::io::Write as _;
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"[matching]\nalgorithm = \"fuzzy\"\nrequire = false\nascii = true\n")
        .unwrap();
    let cfg = load(f.path());
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
    let cfg = load(f.path());
    assert_eq!(cfg.history.history, Some(false));
    assert_eq!(cfg.history.path.as_deref(), Some("/tmp/hist"));
}

#[test]
fn load_base_section() {
    use std::io::Write as _;
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"[base]\noutput = \"HDMI-1\"\nterminal = \"alacritty\"\ntheme = \"Sweet.toml\"\n")
        .unwrap();
    let cfg = load(f.path());
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
    let cfg = load(&path);
    // [matching] require = true, ascii = false
    assert_eq!(cfg.matching.require, Some(true));
    assert_eq!(cfg.matching.ascii, Some(false));
    // [history] history = true
    assert_eq!(cfg.history.history, Some(true));
}
