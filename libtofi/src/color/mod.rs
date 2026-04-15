//! Color type and hex-string parsing.
#![deny(unsafe_code)]
//!
//! # Format
//!
//! Accepted inputs (leading `#` optional):
//!
//! | Input length | Meaning        | Alpha |
//! |---|---|---|
//! | 3 (`RGB`)      | Expand to RRGGBB | `0xFF` |
//! | 4 (`RGBA`)     | Expand to RRGGBBAA | from input |
//! | 6 (`RRGGBB`)   | Literal          | `0xFF` |
//! | 8 (`RRGGBBAA`) | Literal          | from input |
//!
//! All other lengths are invalid. Channel values are stored as `f32` in `0.0..=1.0`.

use std::str::FromStr;

use crate::error::{Error, Result};

/// RGBA color with linear `f32` channels in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// Parse a hex color string (with or without a leading `#`).
    ///
    /// Accepted formats: `RGB`, `RGBA`, `RRGGBB`, `RRGGBBAA`.
    /// Returns `Err` for any other input, mirroring the `{ -1, -1, -1, -1 }` sentinel
    /// that `hex_to_color` returns in C.
    pub fn from_hex(s: &str) -> Result<Self> {
        let hex = s.strip_prefix('#').unwrap_or(s);

        let val: u32 = match hex.len() {
            3 => {
                // Expand RGB → RRGGBB, then append opaque alpha.
                let expanded = expand_nibbles(hex, 3)?;
                (expanded << 8) | 0xFF
            }
            4 => {
                // Expand RGBA → RRGGBBAA.
                expand_nibbles(hex, 4)?
            }
            6 => {
                let v = parse_hex(hex)?;
                (v << 8) | 0xFF
            }
            8 => parse_hex(hex)?,
            _ => {
                return Err(Error::InvalidValue(format!(
                    "invalid color length {} in {:?}",
                    hex.len(),
                    s
                )));
            }
        };

        Ok(Self {
            r: channel(val, 24),
            g: channel(val, 16),
            b: channel(val, 8),
            a: channel(val, 0),
        })
    }
}

impl FromStr for Color {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_hex(s)
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Parse a hex string into a `u32`, returning `InvalidValue` on failure.
fn parse_hex(s: &str) -> Result<u32> {
    u32::from_str_radix(s, 16)
        .map_err(|_| Error::InvalidValue(format!("invalid hex digits in color {:?}", s)))
}

/// Expand `n` nibbles (each doubled) into a `u32`.
///
/// e.g. `"RGB"` → `0xRRGGBB`, `"RGBA"` → `0xRRGGBBAA`.
fn expand_nibbles(s: &str, n: usize) -> Result<u32> {
    let mut val: u32 = 0;
    for (i, ch) in s.chars().enumerate() {
        if i >= n {
            break;
        }
        let nibble = ch
            .to_digit(16)
            .ok_or_else(|| Error::InvalidValue(format!("invalid hex digit {:?} in color", ch)))?;
        val = (val << 8) | (nibble << 4) | nibble;
    }
    Ok(val)
}

/// Extract an 8-bit channel from `val` at `shift`, returning it as `f32` in `0.0..=1.0`.
fn channel(val: u32, shift: u32) -> f32 {
    ((val >> shift) & 0xFF) as f32 / 255.0
}

#[cfg(test)]
mod tests;
