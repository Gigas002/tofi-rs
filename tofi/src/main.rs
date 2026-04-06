//! `tofi` binary — wires [`cli::Cli`] to the rest of the program.

mod cli;
#[allow(dead_code)]
mod config;

use clap::Parser as _;

fn main() {
    cli::Cli::parse();
    libtofi_rs::noop();
}
