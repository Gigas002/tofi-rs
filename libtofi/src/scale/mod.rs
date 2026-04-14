//! Scale factor helpers.
#![deny(unsafe_code)]
//!
//! Ports `src/scale.c`: `scale_apply` and `scale_apply_inverse`.
//!
//! # Scale encoding
//!
//! The Wayland `wp_fractional_scale_v1` protocol encodes a scale factor as a
//! numerator over a fixed denominator of **120**, enabling fractions of 120.
//! An integer scale of 2× is encoded as 240; 1.5× is 180; 1× is 120.
//!
//! Integer scales (from `wl_output.scale`) are converted to the same encoding
//! by multiplying by 120 before calling these helpers.
//!
//! # C reference
//!
//! `src/scale.c`, `src/scale.h`.

/// Convert a logical pixel dimension to physical pixels at the given `scale`.
///
/// `scale` is the numerator of a fraction with denominator 120, as used by
/// `wp_fractional_scale_v1`.  For an integer scale `n`, pass `n * 120`.
///
/// A tiny bias (`1e-6`) is added before rounding so exact multiples are not
/// penalised by floating-point representation noise; this matches the C source.
///
/// # C reference
///
/// `scale_apply` in `src/scale.c`.
pub fn scale_apply(base: u32, scale: u32) -> u32 {
    // C: round(base * (scale / 120.) + 1e-6)
    ((base as f64) * (scale as f64 / 120.0) + 1e-6).round() as u32
}

/// Convert a physical pixel dimension back to logical pixels at the given `scale`.
///
/// Inverse of [`scale_apply`].
///
/// # C reference
///
/// `scale_apply_inverse` in `src/scale.c`.
pub fn scale_apply_inverse(base: u32, scale: u32) -> u32 {
    // C: round(base * (120. / scale) + 1e-6)
    ((base as f64) * (120.0 / scale as f64) + 1e-6).round() as u32
}

#[cfg(test)]
mod tests;
