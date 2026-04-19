//! CLI argument parsing (`clap` only).
//!
//! Parsing produces a [`Cli`] with all optional overrides.
//! Merging with config and theme happens in [`crate::settings::build`].

use std::path::PathBuf;

use clap::Parser;

// ── Cli ───────────────────────────────────────────────────────────────────────

/// Wayland application launcher.
///
/// Reads `$XDG_CONFIG_HOME/tofi/config.toml` unless `--config` is given.
/// Theme is resolved from `[base].theme` in config, or overridden with `--theme`.
#[derive(Parser, Debug)]
#[command(name = "tofi", version, about, disable_help_flag = true)]
pub struct Cli {
    /// Print help information.
    #[arg(long, action = clap::ArgAction::Help)]
    pub help: Option<bool>,

    /// Print a shell completion script to stdout and exit.
    #[cfg(feature = "completions")]
    #[arg(long, value_name = "SHELL")]
    pub completions: Option<crate::completions::CompletionShell>,

    /// Path to the config TOML file (overrides the default XDG path).
    #[arg(short = 'c', long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Path to the theme TOML file (overrides `[base].theme` in config).
    #[arg(short = 't', long, value_name = "FILE")]
    pub theme: Option<PathBuf>,

    /// Launch mode: drun (desktop apps) or run (PATH executables). Defaults to drun.
    #[arg(long, value_name = "MODE")]
    pub mode: Option<String>,

    /// Output to appear on (empty = compositor default).
    #[arg(short = 'o', long, value_name = "NAME")]
    pub output: Option<String>,

    /// Terminal for launching terminal apps in drun mode.
    #[arg(long, value_name = "NAME")]
    pub terminal: Option<String>,

    /// Matching algorithm: normal, prefix, fuzzy.
    #[arg(short = 'a', long, value_name = "ALGORITHM")]
    pub algorithm: Option<String>,

    /// Enable or disable history sorting.
    #[arg(
        short = 'h',
        long,
        value_name = "BOOL",
        default_missing_value = "true",
        num_args = 0..=1,
        require_equals = false
    )]
    pub history: Option<bool>,
}

#[cfg(test)]
mod tests;
