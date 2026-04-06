//! Tofi engine library (Rust port).
//!
//! # Cargo features (§4.1)
//!
//! | Feature | Role |
//! | --- | --- |
//! | `wayland` | Core Wayland client, SHM, surfaces |
//! | `renderer` | Drawing / text layout (Cairo stack under [`crate::renderer`]) |
//! | `drun` | `.desktop` scanning / `tofi-drun` |
//! | `run-commands` | Cached `$PATH` executable list for `tofi-run` |
//! | `clipboard` | Paste (`wayland::clipboard`; implies **`wayland`**) |
//! | `history` | History file |
//! | `single-instance-lock` | Single-instance lock file |
//!
//! With `--no-default-features`, only [`noop`] is guaranteed; optional modules are omitted.

#[cfg(feature = "drun")]
pub mod drun;
#[cfg(feature = "history")]
pub mod history;
#[cfg(feature = "single-instance-lock")]
pub mod lock;
#[cfg(feature = "renderer")]
pub mod renderer;
#[cfg(feature = "run-commands")]
pub mod run_commands;
#[cfg(feature = "wayland")]
pub mod wayland;

/// Placeholder until real APIs land; keeps the workspace linked and compiling.
pub fn noop() {}
