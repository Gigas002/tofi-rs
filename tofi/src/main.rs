//! `tofi` binary — wires [`cli::Cli`] to the rest of the program.
#![deny(unsafe_code)]

mod app;
mod cli;
#[cfg(feature = "completions")]
mod completions;
#[allow(dead_code)]
mod config;
#[cfg(feature = "history")]
#[allow(dead_code)]
mod history;
#[cfg(feature = "run-commands")]
#[allow(dead_code)]
mod run_commands;
mod submit;

use clap::Parser as _;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();

    // Handle --completions before anything else: print script and exit.
    #[cfg(feature = "completions")]
    if let Some(shell) = cli.completions {
        completions::generate_completions(shell);
        return;
    }

    #[cfg(feature = "drun")]
    let flag_drun = cli.drun;
    #[cfg(feature = "run-commands")]
    let flag_run = cli.run;

    #[allow(unused_variables)]
    let (config, _errors) = cli.into_config().expect("Failed to load config");

    let submitted = app::run(
        config,
        #[cfg(feature = "drun")]
        flag_drun,
        #[cfg(feature = "run-commands")]
        flag_run,
    );

    // Exit code 1 on cancel (Escape / close without selection), matching
    // upstream tofi behaviour.
    if !submitted {
        std::process::exit(1);
    }
}
