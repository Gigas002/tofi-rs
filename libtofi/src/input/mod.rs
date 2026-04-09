//! Input handling — text editing helpers and XKB keyboard state.
//!
//! # Structure
//!
//! - **Text editing** (this module): pure functions operating on `(&mut String,
//!   &mut usize)` — no Wayland or renderer dependency, fully unit-testable.
//! - **[`keyboard`]**: [`keyboard::KeyboardState`] wraps `libxkbcommon` for
//!   keymap parsing, modifier tracking, and key-repeat accounting.  Gated by
//!   the **`wayland`** feature because keymaps are received from the compositor.
//!
//! # C reference
//!
//! `src/input.c`, `src/input.h`.

#[cfg(feature = "wayland")]
pub mod keyboard;
#[cfg(test)]
mod tests;

// ── Linux evdev key codes ─────────────────────────────────────────────────────
// Source: `linux/input-event-codes.h`.  Listed here so callers do not need a
// separate `input-linux` crate.

pub const KEY_ESC: u32 = 1;
pub const KEY_BACKSPACE: u32 = 14;
pub const KEY_TAB: u32 = 15;
pub const KEY_ENTER: u32 = 28;
pub const KEY_LEFTBRACE: u32 = 26;
pub const KEY_B: u32 = 48;
pub const KEY_C: u32 = 46;
pub const KEY_F: u32 = 33;
pub const KEY_G: u32 = 34;
pub const KEY_H: u32 = 35;
pub const KEY_J: u32 = 36;
pub const KEY_K: u32 = 37;
pub const KEY_L: u32 = 38;
pub const KEY_M: u32 = 50;
pub const KEY_N: u32 = 49;
pub const KEY_P: u32 = 25;
pub const KEY_U: u32 = 22;
pub const KEY_V: u32 = 47;
pub const KEY_W: u32 = 17;
pub const KEY_HOME: u32 = 102;
pub const KEY_UP: u32 = 103;
pub const KEY_PAGEUP: u32 = 104;
pub const KEY_LEFT: u32 = 105;
pub const KEY_RIGHT: u32 = 106;
pub const KEY_DOWN: u32 = 108;
pub const KEY_PAGEDOWN: u32 = 109;
pub const KEY_KPENTER: u32 = 96;

// ── Text editing helpers ──────────────────────────────────────────────────────
// Pure functions that operate on (input: &mut String, cursor: &mut usize) so
// they can be tested without any Wayland or renderer dependency.

/// Insert a printable `ch` at `cursor` and advance the cursor by one.
///
/// Does nothing if `input` already contains
/// [`crate::entry::MAX_INPUT_LENGTH`] codepoints.
///
/// C reference: `add_character` in `src/input.c`.
pub fn add_char(input: &mut String, cursor: &mut usize, ch: char, max_len: usize) {
    if input.chars().count() >= max_len {
        return;
    }
    let byte_pos = input
        .char_indices()
        .nth(*cursor)
        .map(|(i, _)| i)
        .unwrap_or(input.len());
    input.insert(byte_pos, ch);
    *cursor += 1;
}

/// Delete the character immediately before `cursor` (backspace behaviour).
///
/// Does nothing when the cursor is at position 0.
///
/// C reference: `delete_character` in `src/input.c`.
pub fn delete_char(input: &mut String, cursor: &mut usize) {
    if *cursor == 0 || input.is_empty() {
        return;
    }
    let byte_pos = input
        .char_indices()
        .nth(*cursor - 1)
        .map(|(i, _)| i)
        .expect("cursor within string bounds");
    input.remove(byte_pos);
    *cursor -= 1;
}

/// Delete from the cursor back to the start of the previous word (Ctrl+W).
///
/// Mirrors the C `delete_word`: skip trailing spaces, then skip non-spaces.
///
/// C reference: `delete_word` in `src/input.c`.
pub fn delete_word(input: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }

    // Collect codepoints up to the cursor so we can do index arithmetic.
    let prefix: Vec<char> = input.chars().take(*cursor).collect();
    let total = *cursor;

    let mut new_cursor = total;
    // Skip trailing whitespace.
    while new_cursor > 0 && prefix[new_cursor - 1].is_whitespace() {
        new_cursor -= 1;
    }
    // Skip the word.
    while new_cursor > 0 && !prefix[new_cursor - 1].is_whitespace() {
        new_cursor -= 1;
    }

    // Remove the codepoints in [new_cursor, cursor).
    let remove_count = total - new_cursor;
    if remove_count == 0 {
        return;
    }

    // Find byte offsets.
    let byte_start = input
        .char_indices()
        .nth(new_cursor)
        .map(|(i, _)| i)
        .unwrap_or(input.len());
    let byte_end = input
        .char_indices()
        .nth(total)
        .map(|(i, _)| i)
        .unwrap_or(input.len());

    input.drain(byte_start..byte_end);
    *cursor = new_cursor;
}

/// Clear the entire input and reset the cursor to 0 (Ctrl+U).
///
/// C reference: `clear_input` in `src/input.c`.
pub fn clear_input(input: &mut String, cursor: &mut usize) {
    input.clear();
    *cursor = 0;
}
