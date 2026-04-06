//! Tofi engine library (Rust port).
//!
//! # Cargo features (§4.1)
//!
//! | Feature | Role |
//! | --- | --- |
//! | `wayland` | Core Wayland client, SHM, surfaces |
//! | `renderer-cairo` | Cairo + Pango + HarfBuzz drawing |
//! | `drun` | `.desktop` scanning / `tofi-drun` |
//! | `run-command-cache` | Cached PATH command list for `tofi-run` |
//! | `clipboard-wayland` | Wayland paste |
//! | `history` | History file |
//! | `single-instance-lock` | Single-instance lock file |
//!
//! With `--no-default-features`, only [`noop`] is guaranteed; optional modules are omitted.

#[cfg(feature = "clipboard-wayland")]
pub mod clipboard_wayland;
#[cfg(feature = "drun")]
pub mod drun;
#[cfg(feature = "history")]
pub mod history;
#[cfg(feature = "single-instance-lock")]
pub mod lock;
#[cfg(feature = "renderer-cairo")]
pub mod renderer_cairo;
#[cfg(feature = "run-command-cache")]
pub mod run_command_cache;
#[cfg(feature = "wayland")]
pub mod wayland;

/// Placeholder until real APIs land; keeps the workspace linked and compiling.
pub fn noop() {}
