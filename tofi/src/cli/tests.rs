use super::Cli;
use clap::{CommandFactory, Parser, error::ErrorKind};

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
