//! Wayland client, SHM, surfaces (feature **`wayland`**).
//!
//! Paste lives here too — see [`clipboard`] (feature **`clipboard`**).
//!
//! # Step 4.1
//!
//! [`connect`] establishes a connection to the Wayland compositor via
//! `WAYLAND_DISPLAY` (or the socket default). Registry binding and surface
//! creation follow in later steps.
//!
//! C reference: `src/main.c` — `wl_display_connect(NULL)`.

use wayland_client::Connection;

use crate::{Error, Result};

#[cfg(feature = "clipboard")]
pub mod clipboard;

#[cfg(test)]
mod tests;

/// A live connection to the Wayland compositor.
///
/// Dropping this struct disconnects from the compositor cleanly.
pub struct WaylandConnection {
    // Used in Step 4.2+ (registry binding, event loop).
    #[allow(dead_code)]
    pub(crate) connection: Connection,
}

/// Connect to the Wayland compositor identified by `$WAYLAND_DISPLAY`.
///
/// Returns [`Error::Wayland`] if no compositor socket is found or the
/// handshake fails. The returned [`WaylandConnection`] disconnects on drop.
///
/// C reference: `wl_display_connect(NULL)` in `src/main.c`.
pub fn connect() -> Result<WaylandConnection> {
    let connection = Connection::connect_to_env().map_err(|e| Error::Wayland(e.to_string()))?;
    Ok(WaylandConnection { connection })
}
