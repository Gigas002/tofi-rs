//! `tofi` binary — wires [`cli::Cli`] to the rest of the program.

mod cli;

use clap::Parser as _;

fn main() {
    cli::Cli::parse();
    libtofi_rs::noop();
}
