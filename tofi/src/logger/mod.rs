//! `tracing` subscriber setup (feature **`logging`**).
//!
//! Logs go to **stderr**: in `run` and `dmenu` mode, stdout carries the
//! selection that scripts read, so it must hold nothing else.

use std::io::IsTerminal as _;

use tracing_subscriber::EnvFilter;

/// The filter to use: `RUST_LOG` when it is set and valid, else `level`.
pub(crate) fn filter(level: &str) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level))
}

/// Install the global subscriber, writing to stderr at `level`.
pub fn init(level: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(filter(level))
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .init();
}

#[cfg(test)]
mod tests;
