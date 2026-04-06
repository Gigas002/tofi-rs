use std::io::Write as _;

use clap::{CommandFactory, Parser, error::ErrorKind};

use super::Cli;
use crate::config::{Anchor, TofiConfig, UnitValue};

// ---------------------------------------------------------------------------
// Basic CLI identity
// ---------------------------------------------------------------------------

#[test]
fn parse_empty_succeeds() {
    Cli::try_parse_from(["tofi"]).unwrap();
}

#[test]
fn version_requests_display() {
    match Cli::try_parse_from(["tofi", "--version"]) {
        Err(e) => assert_eq!(e.kind(), ErrorKind::DisplayVersion),
        Ok(_) => panic!("expected --version to stop parsing"),
    }
}

#[test]
fn help_requests_display() {
    match Cli::try_parse_from(["tofi", "--help"]) {
        Err(e) => assert_eq!(e.kind(), ErrorKind::DisplayHelp),
        Ok(_) => panic!("expected --help to stop parsing"),
    }
}

#[test]
fn command_metadata() {
    let mut cmd = Cli::command();
    assert_eq!(cmd.get_name(), "tofi");
    let help = cmd.render_long_help().to_string();
    assert!(help.contains("tofi") || help.contains("Wayland"));
}

// ---------------------------------------------------------------------------
// Help text lists key flags
// ---------------------------------------------------------------------------

#[test]
fn help_mentions_config_flag() {
    let mut cmd = Cli::command();
    let help = cmd.render_long_help().to_string();
    assert!(help.contains("--config"), "help should list --config");
}

#[test]
fn help_mentions_font_size() {
    let mut cmd = Cli::command();
    let help = cmd.render_long_help().to_string();
    assert!(help.contains("--font-size"), "help should list --font-size");
}

#[test]
fn help_mentions_anchor() {
    let mut cmd = Cli::command();
    let help = cmd.render_long_help().to_string();
    assert!(help.contains("--anchor"), "help should list --anchor");
}

// ---------------------------------------------------------------------------
// Unknown flags are rejected
// ---------------------------------------------------------------------------

#[test]
fn unknown_long_flag_rejected() {
    let result = Cli::try_parse_from(["tofi", "--not-a-real-option", "val"]);
    assert!(result.is_err());
}

#[test]
fn unknown_short_flag_rejected() {
    let result = Cli::try_parse_from(["tofi", "-z"]);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// --config short form
// ---------------------------------------------------------------------------

#[test]
fn short_config_flag() {
    let cli = Cli::try_parse_from(["tofi", "-c", "/tmp/tofi.conf"]).unwrap();
    assert_eq!(
        cli.config.as_deref().unwrap().to_str().unwrap(),
        "/tmp/tofi.conf"
    );
}

// ---------------------------------------------------------------------------
// Flag parsing — individual fields
// ---------------------------------------------------------------------------

#[test]
fn flag_font_size() {
    let cli = Cli::try_parse_from(["tofi", "--font-size", "36"]).unwrap();
    assert_eq!(cli.font_size.as_deref(), Some("36"));
}

#[test]
fn flag_anchor() {
    let cli = Cli::try_parse_from(["tofi", "--anchor", "top-left"]).unwrap();
    assert_eq!(cli.anchor.as_deref(), Some("top-left"));
}

#[test]
fn flag_width_percent() {
    let cli = Cli::try_parse_from(["tofi", "--width", "80%"]).unwrap();
    assert_eq!(cli.width.as_deref(), Some("80%"));
}

#[test]
fn flag_late_keyboard_init_no_value() {
    // --late-keyboard-init with no value should default to "true".
    let cli = Cli::try_parse_from(["tofi", "--late-keyboard-init"]).unwrap();
    assert_eq!(cli.late_keyboard_init.as_deref(), Some("true"));
}

#[test]
fn flag_late_keyboard_init_explicit_false() {
    let cli = Cli::try_parse_from(["tofi", "--late-keyboard-init", "false"]).unwrap();
    assert_eq!(cli.late_keyboard_init.as_deref(), Some("false"));
}

// ---------------------------------------------------------------------------
// into_config — two-pass integration
// ---------------------------------------------------------------------------

#[test]
fn into_config_empty_args_gives_default() {
    // /dev/null as config avoids loading the real XDG default (which may exist
    // on the developer's machine and would change the expected values).
    let cli = Cli::try_parse_from(["tofi", "--config", "/dev/null"]).unwrap();
    let (cfg, errs) = cli.into_config().unwrap();
    assert!(errs.is_empty());
    assert_eq!(cfg, TofiConfig::default());
}

#[test]
fn into_config_cli_flag_overrides_default() {
    let cli = Cli::try_parse_from(["tofi", "--font-size", "48"]).unwrap();
    let (cfg, _) = cli.into_config().unwrap();
    assert_eq!(cfg.font_size, 48);
}

#[test]
fn into_config_anchor_override() {
    let cli = Cli::try_parse_from(["tofi", "--anchor", "top"]).unwrap();
    let (cfg, _) = cli.into_config().unwrap();
    assert_eq!(cfg.anchor, Anchor::Top);
}

#[test]
fn into_config_width_percent() {
    let cli = Cli::try_parse_from(["tofi", "--width", "50%"]).unwrap();
    let (cfg, _) = cli.into_config().unwrap();
    assert_eq!(cfg.width, UnitValue::percent(50));
}

#[test]
fn into_config_invalid_flag_value_returns_err() {
    let cli = Cli::try_parse_from(["tofi", "--font-size", "0"]).unwrap();
    assert!(cli.into_config().is_err());
}

#[test]
fn into_config_config_file_then_cli_override() {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"font-size = 20\n").unwrap();
    let path = f.path().to_str().unwrap().to_owned();

    // CLI override (36) should win over the file value (20).
    let cli = Cli::try_parse_from(["tofi", "--config", &path, "--font-size", "36"]).unwrap();
    let (cfg, errs) = cli.into_config().unwrap();
    assert!(errs.is_empty());
    assert_eq!(cfg.font_size, 36);
}

#[test]
fn into_config_config_file_applied() {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"font-size = 99\n").unwrap();
    let path = f.path().to_str().unwrap().to_owned();

    let cli = Cli::try_parse_from(["tofi", "--config", &path]).unwrap();
    let (cfg, errs) = cli.into_config().unwrap();
    assert!(errs.is_empty());
    assert_eq!(cfg.font_size, 99);
}

#[test]
fn into_config_late_keyboard_init_flag_only() {
    let cli = Cli::try_parse_from(["tofi", "--late-keyboard-init"]).unwrap();
    let (cfg, _) = cli.into_config().unwrap();
    assert!(cfg.late_keyboard_init);
}
