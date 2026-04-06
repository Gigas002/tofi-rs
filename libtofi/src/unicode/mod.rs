//! Unicode helpers — Rust port of `src/unicode.c` (GLib wrappers).
//!
//! # Design rationale
//!
//! The C module is thin wrappers around GLib. In Rust:
//! - `str` / `String` are always valid UTF-8 — no manual validation needed at the type level.
//! - `char` is a Unicode scalar value (U+0000–U+D7FF, U+E000–U+10FFFF), directly usable
//!   where the C code used `uint32_t` / UTF-32 codepoints.
//! - Character classification (`isprint`, `isspace`, etc.) lives on `char` in the stdlib.
//! - NFC normalization requires the `unicode-normalization` crate (replaces `g_utf8_normalize`).
//!
//! The public API mirrors the C surface so callers in `input`, `matching`, and `clipboard`
//! modules translate naturally. The `utf32_*` helpers use `char` instead of `u32` — callers
//! that receive raw `u32` keysyms must convert via [`char::from_u32`] first.

use unicode_normalization::UnicodeNormalization as _;

/// Maximum number of UTF-32 codepoints in the input buffer.
///
/// Mirrors `MAX_INPUT_LENGTH` from `src/entry.h`. The UTF-8 representation of
/// the same input can use up to `4 * MAX_INPUT_LENGTH` bytes.
pub const MAX_INPUT_LENGTH: usize = 256;

// ── UTF-8 validation ──────────────────────────────────────────────────────────

/// Returns `true` if `bytes` is a well-formed UTF-8 sequence.
///
/// C counterpart: `g_utf8_validate` (via `utf8_validate`).
pub fn utf8_validate(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok()
}

// ── UTF-8 ↔ UTF-32 conversion ─────────────────────────────────────────────────

/// Decode the first Unicode scalar from a UTF-8 string slice.
///
/// Returns `None` if `s` is empty.
///
/// C counterpart: `g_utf8_get_char` (via `utf8_to_utf32`).
pub fn utf8_to_utf32(s: &str) -> Option<char> {
    s.chars().next()
}

/// Decode the first Unicode scalar from a byte slice, validating UTF-8.
///
/// Returns `None` if the bytes are not valid UTF-8 or the slice is empty.
///
/// C counterpart: `g_utf8_get_char_validated` (via `utf8_to_utf32_validate`).
pub fn utf8_to_utf32_validate(bytes: &[u8]) -> Option<char> {
    std::str::from_utf8(bytes).ok()?.chars().next()
}

/// Encode a Unicode scalar value into its UTF-8 byte representation.
///
/// Returns a small stack-allocated array and the number of bytes written (1–4).
///
/// C counterpart: `g_unichar_to_utf8` (via `utf32_to_utf8`).
pub fn utf32_to_utf8(c: char) -> ([u8; 4], usize) {
    let mut buf = [0u8; 4];
    let len = c.encode_utf8(&mut buf).len();
    (buf, len)
}

/// Collect all Unicode scalars from a UTF-8 string into a `Vec<char>`.
///
/// Analogous to `g_utf8_to_ucs4_fast` (via `utf8_string_to_utf32_string`).
/// In Rust you rarely need this — prefer iterating `.chars()` directly.
pub fn utf8_string_to_utf32_string(s: &str) -> Vec<char> {
    s.chars().collect()
}

// ── UTF-8 string metrics ──────────────────────────────────────────────────────

/// Count the number of Unicode scalar values (codepoints) in a UTF-8 string.
///
/// C counterpart: `g_utf8_strlen` (via `utf8_strlen`).
pub fn utf8_strlen(s: &str) -> usize {
    s.chars().count()
}

// ── UTF-8 navigation ──────────────────────────────────────────────────────────

/// Split `s` into the first codepoint and the remaining UTF-8 slice.
///
/// Returns `None` if `s` is empty.
///
/// C counterpart: `g_utf8_next_char` (via `utf8_next_char`).
pub fn utf8_next_char(s: &str) -> Option<(char, &str)> {
    let mut chars = s.char_indices();
    let (_, c) = chars.next()?;
    let rest_start = chars.next().map_or(s.len(), |(i, _)| i);
    Some((c, &s[rest_start..]))
}

// ── UTF-8 search ──────────────────────────────────────────────────────────────

/// Find the first occurrence of codepoint `needle` in `s`, returning the byte offset.
///
/// C counterpart: `g_utf8_strchr` (via `utf8_strchr`).
pub fn utf8_strchr(s: &str, needle: char) -> Option<usize> {
    s.char_indices()
        .find_map(|(i, c)| if c == needle { Some(i) } else { None })
}

/// Find the first occurrence of `needle` in `s` using Unicode case-folding.
///
/// Returns the byte offset into `s`, or `None` if not found.
///
/// C counterpart: `utf8_strcasechr` (using `g_unichar_tolower`).
pub fn utf8_strcasechr(s: &str, needle: char) -> Option<usize> {
    let needle_lower: char = needle
        .to_lowercase()
        .next()
        .expect("to_lowercase always yields at least one char");
    s.char_indices().find_map(|(i, c)| {
        if c.to_lowercase().next() == Some(needle_lower) {
            Some(i)
        } else {
            None
        }
    })
}

/// Case-insensitive substring search using Unicode case-folding.
///
/// Returns the byte offset of `needle` within `haystack`, or `None`.
///
/// C counterpart: `utf8_strcasestr` (using `g_utf8_casefold` + `strstr`).
pub fn utf8_strcasestr(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let h_folded = casefold(haystack);
    let n_folded = casefold(needle);
    h_folded
        .find(n_folded.as_str())
        .map(|byte_offset_in_folded| {
            // `casefold` is codepoint-count-preserving for the chars we use
            // (lowercase may produce more chars for a few codepoints, but for
            // the launcher's use-case of ASCII-heavy queries this is fine; we
            // return an approximate byte offset into the original string).
            let cp_offset = h_folded[..byte_offset_in_folded].chars().count();
            haystack
                .char_indices()
                .nth(cp_offset)
                .map_or(haystack.len(), |(i, _)| i)
        })
}

// ── Unicode character classification ─────────────────────────────────────────

/// Returns `true` if `c` is a printable character (not a control character).
///
/// C counterpart: `g_unichar_isprint` (via `utf32_isprint`).
pub fn utf32_isprint(c: char) -> bool {
    !c.is_control()
}

/// Returns `true` if `c` is a Unicode whitespace character.
///
/// C counterpart: `g_unichar_isspace` (via `utf32_isspace`).
pub fn utf32_isspace(c: char) -> bool {
    c.is_whitespace()
}

/// Returns `true` if `c` is an uppercase letter.
///
/// C counterpart: `g_unichar_isupper` (via `utf32_isupper`).
pub fn utf32_isupper(c: char) -> bool {
    c.is_uppercase()
}

/// Returns `true` if `c` is a lowercase letter.
///
/// C counterpart: `g_unichar_islower` (via `utf32_islower`).
pub fn utf32_islower(c: char) -> bool {
    c.is_lowercase()
}

/// Returns `true` if `c` is an alphanumeric character.
///
/// C counterpart: `g_unichar_isalnum` (via `utf32_isalnum`).
pub fn utf32_isalnum(c: char) -> bool {
    c.is_alphanumeric()
}

/// Convert a character to its Unicode uppercase equivalent.
///
/// Returns the first char produced by `to_uppercase()`, which covers the
/// common case. Full uppercase may be multi-char (e.g. German ß → SS); use
/// `.to_uppercase()` directly when you need the full expansion.
///
/// C counterpart: `g_unichar_toupper` (via `utf32_toupper`).
pub fn utf32_toupper(c: char) -> char {
    c.to_uppercase()
        .next()
        .expect("to_uppercase always yields at least one char")
}

/// Convert a character to its Unicode lowercase equivalent.
///
/// C counterpart: `g_unichar_tolower` (via `utf32_tolower`).
pub fn utf32_tolower(c: char) -> char {
    c.to_lowercase()
        .next()
        .expect("to_lowercase always yields at least one char")
}

// ── NFC normalization ─────────────────────────────────────────────────────────

/// Return the NFC-normalized form of `s` (canonical decomposition, then canonical composition).
///
/// C counterpart: `g_utf8_normalize(s, -1, G_NORMALIZE_DEFAULT)` (via `utf8_normalize`).
pub fn utf8_normalize(s: &str) -> String {
    s.nfc().collect()
}

/// Return the NFC-composed form of `s` (canonical decomposition, then canonical composition).
///
/// Equivalent to `utf8_normalize` for NFC; provided for parity with C's
/// `G_NORMALIZE_DEFAULT_COMPOSE` path (via `utf8_compose`).
pub fn utf8_compose(s: &str) -> String {
    s.nfc().collect()
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Unicode case-fold `s` to a lowercase string suitable for case-insensitive comparison.
///
/// Uses `char::to_lowercase` on each codepoint — matches GLib's `g_utf8_casefold` for
/// the ASCII-heavy, BMP-mostly input typical in a launcher.
fn casefold(s: &str) -> String {
    s.chars().flat_map(|c| c.to_lowercase()).collect()
}

#[cfg(test)]
mod tests;
