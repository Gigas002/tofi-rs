use clap::{CommandFactory, Parser, error::ErrorKind};

use super::Cli;

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

#[test]
fn help_mentions_config_flag() {
    let mut cmd = Cli::command();
    assert!(cmd.render_long_help().to_string().contains("--config"));
}

#[test]
fn help_mentions_theme_flag() {
    let mut cmd = Cli::command();
    assert!(cmd.render_long_help().to_string().contains("--theme"));
}

#[test]
fn unknown_long_flag_rejected() {
    assert!(Cli::try_parse_from(["tofi", "--not-a-real-option", "val"]).is_err());
}

#[test]
fn unknown_short_flag_rejected() {
    assert!(Cli::try_parse_from(["tofi", "-z"]).is_err());
}

#[test]
fn short_config_flag() {
    let cli = Cli::try_parse_from(["tofi", "-c", "/tmp/tofi.conf"]).unwrap();
    assert_eq!(
        cli.config.as_deref().unwrap().to_str().unwrap(),
        "/tmp/tofi.conf"
    );
}

#[test]
fn theme_flag_parsed() {
    let cli = Cli::try_parse_from(["tofi", "--theme", "/etc/tofi/Sweet.toml"]).unwrap();
    assert_eq!(
        cli.theme.as_deref().unwrap().to_str().unwrap(),
        "/etc/tofi/Sweet.toml"
    );
}

#[test]
fn algorithm_flag_parsed() {
    let cli = Cli::try_parse_from(["tofi", "--algorithm", "fuzzy"]).unwrap();
    assert_eq!(cli.algorithm.as_deref(), Some("fuzzy"));
}

#[test]
fn history_flag_enables() {
    let cli = Cli::try_parse_from(["tofi", "--history"]).unwrap();
    assert_eq!(cli.history, Some(true));
}

#[test]
fn history_flag_explicit_false() {
    let cli = Cli::try_parse_from(["tofi", "--history", "false"]).unwrap();
    assert_eq!(cli.history, Some(false));
}

#[test]
fn history_absent_is_none() {
    let cli = Cli::try_parse_from(["tofi"]).unwrap();
    assert_eq!(cli.history, None);
}

#[test]
fn output_flag_parsed() {
    let cli = Cli::try_parse_from(["tofi", "--output", "HDMI-1"]).unwrap();
    assert_eq!(cli.output.as_deref(), Some("HDMI-1"));
}

#[test]
fn short_theme_flag() {
    let cli = Cli::try_parse_from(["tofi", "-t", "/etc/tofi/Sweet.toml"]).unwrap();
    assert_eq!(
        cli.theme.as_deref().unwrap().to_str().unwrap(),
        "/etc/tofi/Sweet.toml"
    );
}

#[test]
fn short_output_flag() {
    let cli = Cli::try_parse_from(["tofi", "-o", "DP-1"]).unwrap();
    assert_eq!(cli.output.as_deref(), Some("DP-1"));
}

#[test]
fn short_algorithm_flag() {
    let cli = Cli::try_parse_from(["tofi", "-a", "fuzzy"]).unwrap();
    assert_eq!(cli.algorithm.as_deref(), Some("fuzzy"));
}

#[test]
fn short_history_flag_enables() {
    let cli = Cli::try_parse_from(["tofi", "-h"]).unwrap();
    assert_eq!(cli.history, Some(true));
}

#[test]
fn mode_flag_drun() {
    let cli = Cli::try_parse_from(["tofi", "--mode", "drun"]).unwrap();
    assert_eq!(cli.mode.as_deref(), Some("drun"));
}

#[test]
fn mode_flag_run() {
    let cli = Cli::try_parse_from(["tofi", "--mode", "run"]).unwrap();
    assert_eq!(cli.mode.as_deref(), Some("run"));
}

#[test]
fn mode_flag_dmenu() {
    let cli = Cli::try_parse_from(["tofi", "--mode", "dmenu"]).unwrap();
    assert_eq!(cli.mode.as_deref(), Some("dmenu"));
}

#[test]
fn mode_absent_is_none() {
    let cli = Cli::try_parse_from(["tofi"]).unwrap();
    assert_eq!(cli.mode, None);
}
