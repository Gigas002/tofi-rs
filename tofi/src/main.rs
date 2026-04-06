//! `tofi` binary — wires [`cli::Cli`] to the rest of the program.

mod cli;
#[allow(dead_code)]
mod config;
#[cfg(feature = "history")]
#[allow(dead_code)]
mod history;
#[cfg(feature = "run-commands")]
#[allow(dead_code)]
mod run_commands;

use clap::Parser as _;

fn main() {
    // Initialise tracing; verbosity controlled by RUST_LOG (e.g. RUST_LOG=debug).
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();
    let (_config, _errors) = cli.into_config().expect("Failed to load config");

    #[cfg(feature = "wayland")]
    {
        // Step 4.2: bind globals and log; drop state + queue to disconnect cleanly.
        let (_state, _event_queue) =
            libtofi_rs::wayland::connect().expect("Failed to initialize Wayland");
    }

    libtofi_rs::noop();
}
