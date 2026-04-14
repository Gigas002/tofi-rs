//! Library-wide error type (`libtofi::Error`) and `Result` alias.
#![deny(unsafe_code)]
//!
//! # Design
//!
//! - Uses `thiserror` for the library; no `anyhow` at the public API boundary.
//! - Wayland-specific variants are added in Phase 4 behind `#[cfg(feature = "wayland")]`.
//! - C reference: `src/log.c` — maps `log_error` / `log_warning` categories to enum variants.

use thiserror::Error;

/// All errors that `libtofi-rs` may produce.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A Wayland protocol or connection error.
    ///
    /// Populated in Phase 4 when the `wayland` feature is enabled; kept as a
    /// placeholder so callers can pattern-match without a future breaking change.
    #[cfg(feature = "wayland")]
    #[error("Wayland error: {0}")]
    Wayland(String),

    /// An I/O error (config files, history, lock, cache, etc.).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A UTF-8 / string encoding error.
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// A value was out of the expected range or otherwise invalid.
    #[error("Invalid value: {0}")]
    InvalidValue(String),

    /// A Cairo / Pango rendering error (feature **`renderer`**).
    #[cfg(feature = "renderer")]
    #[error("Renderer error: {0}")]
    Renderer(String),

    /// An operation is not supported in the current build configuration.
    #[error("Not supported: {0}")]
    NotSupported(String),
}

/// Convenience alias — mirrors `std::result::Result` with `crate::Error`.
pub type Result<T> = std::result::Result<T, Error>;
