//! `tofi` binary — wires [`cli::Cli`] to the rest of the program.

mod cli;
#[allow(dead_code)]
mod config;

use clap::Parser as _;

fn main() {
    let cli = cli::Cli::parse();
    let (_config, _errors) = cli.into_config().expect("Failed to load config");
    libtofi_rs::noop();
}
