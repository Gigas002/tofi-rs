//! Input handling — text editing helpers, key binding dispatch, and XKB
#![deny(unsafe_code)]
//! keyboard state.
//!
//! # Structure
//!
//! - **Text editing** (this module): pure functions operating on `(&mut String,
//!   &mut usize)` — no Wayland or renderer dependency, fully unit-testable.
//! - **[`KeyAction`] / [`classify_keypress`]**: pure key-binding dispatch —
//!   maps `(ctrl, alt, shift, key, ch)` to a named action; fully unit-testable.
//! - **[`keyboard`]**: [`keyboard::KeyboardState`] wraps `libxkbcommon` for
//!   keymap parsing, modifier tracking, and key-repeat accounting.  Gated by
//!   the **`wayland`** feature because keymaps are received from the compositor.
//!
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

// ── KeyAction ─────────────────────────────────────────────────────────────────

/// The logical action produced by a single key event.
///
/// Returned by [`classify_keypress`] and consumed by `handle_keypress` in the
/// Wayland layer.  All variants are pure data — no Wayland or renderer
/// dependency — making [`classify_keypress`] fully unit-testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    /// Insert a printable character at the cursor.
    InsertChar(char),
    /// Delete the character immediately before the cursor (Backspace / Ctrl+H).
    DeleteChar,
    /// Delete the word before the cursor (Ctrl+W / Ctrl+Backspace).
    DeleteWord,
    /// Clear the entire input field (Ctrl+U).
    ClearInput,
    /// Paste from the clipboard (Ctrl+V).
    Paste,
    /// Move the cursor left, or select the previous result when at position 0.
    PrevCursorOrResult,
    /// Move the cursor right, or select the next result when at the end.
    NextCursorOrResult,
    /// Select the previous result in the list.
    ///
    /// Bound to: Up, Shift+Tab, Alt+H, Ctrl/Alt+K, Ctrl/Alt+P, Ctrl/Alt+B.
    PrevResult,
    /// Select the next result in the list.
    ///
    /// Bound to: Down, Tab, Alt+L, Ctrl/Alt+J, Ctrl/Alt+N, Ctrl/Alt+F.
    NextResult,
    /// Jump back one page of results (Page Up).
    PrevPage,
    /// Jump forward one page of results (Page Down).
    NextPage,
    /// Reset the selection to the first result (Home).
    ResetSelection,
    /// Close the launcher (Escape / Ctrl+C / Ctrl+G / Ctrl+\[).
    Close,
    /// Accept the current selection (Enter / KP-Enter / Ctrl+M).
    Submit,
    /// Key is not bound — take no action.
    Unknown,
}

/// Classify a key event into a [`KeyAction`] without any Wayland or renderer
/// dependency.
///
/// `key` is a Linux evdev key code (physical or keysym-mapped, depending on
/// `physical_keybindings` — the caller resolves that before calling here).
/// `ch` is the UTF-32 codepoint produced by the key in the current XKB state
/// (`0` when no character is produced).
pub fn classify_keypress(ctrl: bool, alt: bool, shift: bool, key: u32, ch: u32) -> KeyAction {
    // Printable character — insert at cursor.
    if let Some(c) = char::from_u32(ch)
        && crate::unicode::utf32_isprint(c)
        && !ctrl
        && !alt
    {
        return KeyAction::InsertChar(c);
    }

    // Ctrl+W / Ctrl+Backspace — delete word.
    if (key == KEY_BACKSPACE || key == KEY_W) && ctrl {
        return KeyAction::DeleteWord;
    }

    // Backspace / Ctrl+H — delete character.
    if key == KEY_BACKSPACE || (key == KEY_H && ctrl) {
        return KeyAction::DeleteChar;
    }

    // Ctrl+U — clear input.
    if key == KEY_U && ctrl {
        return KeyAction::ClearInput;
    }

    // Ctrl+V — paste.
    if key == KEY_V && ctrl {
        return KeyAction::Paste;
    }

    // Left — previous cursor position or result.
    // Handled before the `PrevResult` block because `KEY_LEFT` also appears
    // there in the C source (dead code in C; we make the intent explicit here).
    if key == KEY_LEFT {
        return KeyAction::PrevCursorOrResult;
    }

    // Right — next cursor position or result.
    if key == KEY_RIGHT {
        return KeyAction::NextCursorOrResult;
    }

    // Previous result.
    if key == KEY_UP
        || (key == KEY_TAB && shift)
        || (key == KEY_H && alt)
        || ((key == KEY_K || key == KEY_P || key == KEY_B) && (ctrl || alt))
    {
        return KeyAction::PrevResult;
    }

    // Next result.
    if key == KEY_DOWN
        || key == KEY_TAB
        || (key == KEY_L && alt)
        || ((key == KEY_J || key == KEY_N || key == KEY_F) && (ctrl || alt))
    {
        return KeyAction::NextResult;
    }

    // Home — reset to first result.
    if key == KEY_HOME {
        return KeyAction::ResetSelection;
    }

    // Page Up — previous page.
    if key == KEY_PAGEUP {
        return KeyAction::PrevPage;
    }

    // Page Down — next page.
    if key == KEY_PAGEDOWN {
        return KeyAction::NextPage;
    }

    // Escape / Ctrl+C / Ctrl+[ / Ctrl+G — close.
    if key == KEY_ESC || ((key == KEY_C || key == KEY_LEFTBRACE || key == KEY_G) && ctrl) {
        return KeyAction::Close;
    }

    // Enter / KP-Enter / Ctrl+M — submit.
    if key == KEY_ENTER || key == KEY_KPENTER || (key == KEY_M && ctrl) {
        return KeyAction::Submit;
    }

    KeyAction::Unknown
}

// ── Text editing helpers ──────────────────────────────────────────────────────
// Pure functions that operate on (input: &mut String, cursor: &mut usize) so
// they can be tested without any Wayland or renderer dependency.

/// Insert a printable `ch` at `cursor` and advance the cursor by one.
///
/// Does nothing if `input` already contains
/// [`crate::entry::MAX_INPUT_LENGTH`] codepoints.
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
/// Skips trailing spaces, then skips non-spaces.
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
pub fn clear_input(input: &mut String, cursor: &mut usize) {
    input.clear();
    *cursor = 0;
}
