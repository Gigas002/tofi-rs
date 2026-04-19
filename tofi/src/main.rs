//! `tofi` binary — wires CLI, config, theme, and settings into the app.
#![deny(unsafe_code)]

mod app;
mod cli;
#[cfg(feature = "completions")]
mod completions;
mod config;
#[cfg(feature = "history")]
#[allow(dead_code)]
mod history;
mod settings;
mod theme;

use clap::Parser as _;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();

    let _lock =
        libtofi_rs::lock::try_acquire_default().expect("Failed to check single-instance lock");
    if _lock.is_none() {
        tracing::warn!("Another tofi instance is already running");
        std::process::exit(1);
    }

    #[cfg(feature = "completions")]
    if let Some(shell) = cli.completions {
        completions::generate_completions(shell);
        return;
    }

    // Load config file.
    let config_path = cli.config.clone().or_else(config::default_path);
    let config = config_path
        .as_deref()
        .filter(|p| p.exists())
        .map(config::load)
        .unwrap_or_default();

    // Resolve and load theme.
    let theme_path = cli.theme.clone().map(|p| p.to_path_buf()).or_else(|| {
        config.base.theme.as_deref().and_then(|name| {
            let config_dir = config_path.as_deref().and_then(|p| p.parent());
            theme::resolve_path(name, config_dir)
        })
    });
    let theme = theme_path
        .as_deref()
        .filter(|p| p.exists())
        .map(theme::load)
        .unwrap_or_default();

    let settings = settings::build(&cli, &config, &theme);

    let submitted = app::run(settings);

    if !submitted {
        std::process::exit(1);
    }
}
