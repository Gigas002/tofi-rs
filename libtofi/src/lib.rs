//! Tofi engine library.
//!
//! # Cargo features
//!
//! | Feature | Role |
//! | --- | --- |
//! | `wayland` | Core Wayland client, SHM, surfaces |
//! | `renderer` | Drawing / text layout (Cairo stack under [`crate::renderer`]) |
//! | `clipboard` | Paste (`wayland::clipboard`; implies **`wayland`**) |
//!
//! With `--no-default-features`, only [`noop`] is guaranteed; optional modules are omitted.

pub mod error;
pub use error::{Error, Result};

pub mod color;
pub mod drun;
pub mod input;
pub mod lock;
pub mod matching;
pub mod scale;
pub mod string_table;
pub mod unicode;

#[cfg(feature = "renderer")]
pub mod entry;
#[cfg(feature = "renderer")]
pub mod renderer;
#[cfg(feature = "wayland")]
pub mod shm;
#[cfg(feature = "wayland")]
pub mod wayland;

/// Placeholder until real APIs land; keeps the workspace linked and compiling.
pub fn noop() {}
