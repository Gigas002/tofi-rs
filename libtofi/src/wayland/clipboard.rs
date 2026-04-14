//! Wayland clipboard / paste (`wl_data_device`). Feature **`clipboard`** (implies **`wayland`**).
//!
//! Tracks the current `wl_data_offer` announced by the compositor, negotiates
//! a plain-text MIME type, and provides [`ClipboardState::begin_paste`] to
//! open a pipe and request the compositor write clipboard data into it.
//!
//! # C reference
//!
//! `src/clipboard.c`, `src/clipboard.h`, and the clipboard listeners /
//! `paste` function in `src/main.c` / `src/input.c`.

use nix::{fcntl::OFlag, unistd::pipe2};
use std::os::fd::OwnedFd;
use wayland_client::protocol::wl_data_offer;

/// Preferred MIME type — UTF-8 plain text.
///
/// C reference: `mime_type_text_plain_utf8` in `src/main.c`.
pub const MIME_TEXT_UTF8: &str = "text/plain;charset=utf-8";

/// Fallback MIME type — accepted when the UTF-8 variant is not offered.
///
/// C reference: `mime_type_text_plain` in `src/main.c`.
pub const MIME_TEXT_PLAIN: &str = "text/plain";

/// Clipboard paste state — tracks the current `wl_data_offer` and the
/// read end of the pipe used to receive clipboard data from the compositor.
///
/// C reference: `struct clipboard` in `src/clipboard.h`.
#[derive(Default)]
pub struct ClipboardState {
    /// The current data offer from the compositor; `None` when no selection
    /// is active.  Dropping a `WlDataOffer` automatically sends `destroy`
    /// (it is a destructor-typed request in the protocol).
    pub offer: Option<wl_data_offer::WlDataOffer>,
    /// Negotiated MIME type for the current offer.
    pub mime_type: Option<String>,
    /// Read end of the pipe used to receive clipboard data.
    /// `None` when no paste is in progress; dropping closes the fd.
    pub read_fd: Option<OwnedFd>,
}

impl ClipboardState {
    /// Destroy the current offer and close any open clipboard pipe.
    ///
    /// Called when a new `wl_data_offer` arrives (replacing the previous one)
    /// or when the compositor clears the selection.
    ///
    /// C reference: `clipboard_reset` in `src/clipboard.c`.
    pub fn reset(&mut self) {
        // Dropping WlDataOffer sends the destructor request automatically.
        self.offer = None;
        // Dropping OwnedFd closes the file descriptor.
        self.read_fd = None;
        self.mime_type = None;
    }

    /// Close the clipboard pipe after paste is complete or on error.
    ///
    /// C reference: `clipboard_finish_paste` in `src/clipboard.c`.
    pub fn finish_paste(&mut self) {
        self.read_fd = None;
    }

    /// Start a paste: open a non-blocking pipe, ask the compositor to write
    /// clipboard data to the write end, and store the read end for the main
    /// loop to drain.
    ///
    /// Returns `true` when a paste was successfully started (an offer with a
    /// known MIME type is available), `false` otherwise (nothing queued).
    ///
    /// C reference: `paste` in `src/input.c`.
    pub fn begin_paste(&mut self) -> bool {
        if self.offer.is_none() || self.mime_type.is_none() {
            return false;
        }
        let mime_type = self.mime_type.clone().expect("checked above");

        let (read_fd, write_fd): (OwnedFd, OwnedFd) =
            match pipe2(OFlag::O_CLOEXEC | OFlag::O_NONBLOCK) {
                Ok(fds) => fds,
                Err(e) => {
                    tracing::error!("Failed to open pipe for clipboard: {e}");
                    return false;
                }
            };

        // Give the write end to the compositor; it will write clipboard data
        // to it and then close it, giving us EOF on the read end.
        use std::os::fd::AsFd as _;
        if let Some(offer) = self.offer.as_ref() {
            offer.receive(mime_type, write_fd.as_fd());
        }
        // Close the write end — the compositor holds its own copy.
        drop(write_fd);

        self.read_fd = Some(read_fd);
        tracing::debug!("Clipboard paste started");
        true
    }
}
