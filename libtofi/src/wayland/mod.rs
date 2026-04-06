//! Wayland client, SHM, surfaces (feature **`wayland`**).
//!
//! Paste lives here too — see [`clipboard`] (feature **`clipboard`**).

#[cfg(feature = "clipboard")]
pub mod clipboard;

#[cfg(test)]
mod tests;
