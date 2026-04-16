//! Drawing / text layout — Cairo + Pango backend (feature **`renderer`**).
//!
//! [`Renderer`] wraps a Cairo [`ImageSurface`] backed by a raw SHM buffer
//! slice and exposes a minimal Pango text-drawing API.
//!
//! # Safety note
//!
//! [`Renderer::create_for_data`] is `unsafe` because the raw pointer must
//! remain valid for the lifetime of the `Renderer`.  The SHM buffer from
//! [`crate::shm::ShmPool`] satisfies this requirement as long as the
//! `ShmPool` is not dropped or resized while the `Renderer` is alive.

#[cfg(feature = "renderer")]
use cairo::{Context, Format, ImageSurface};
#[cfg(feature = "renderer")]
use pango::FontDescription;

#[cfg(feature = "renderer")]
use crate::{Error, Result};

#[cfg(test)]
mod tests;

// ── Renderer ──────────────────────────────────────────────────────────────────

/// Cairo + Pango renderer backed by a single SHM frame.
///
/// Create via [`Renderer::create_for_data`], draw into it, call [`Renderer::flush`], then
/// detach (drop) before handing the frame to the compositor via
/// [`crate::wayland::surface::draw`].
#[cfg(feature = "renderer")]
pub struct Renderer {
    surface: ImageSurface,
    cr: Context,
    /// Width in **logical** pixels (physical ÷ scale).
    pub logical_width: u32,
    /// Height in **logical** pixels (physical ÷ scale).
    pub logical_height: u32,
}

#[cfg(feature = "renderer")]
impl Renderer {
    /// Create a Cairo `ImageSurface` (ARGB32) backed by a raw SHM buffer and
    /// attach a device scale so all subsequent drawing operations use **logical**
    /// pixel coordinates.
    ///
    /// # Arguments
    ///
    /// * `data` — pointer to the start of one SHM frame; must be at least
    ///   `stride × height` bytes writable and remain valid for `'self`.
    /// * `width` / `height` — physical pixel dimensions of the frame.
    /// * `scale_numerator` — fractional scale (denominator 120) as reported by
    ///   `wp_fractional_scale_v1` or `wl_output::scale × 120`.
    ///
    /// # Safety
    ///
    /// `data` must point to valid, writable memory of at least `width * 4 *
    /// height` bytes that outlives the returned `Renderer`.
    pub unsafe fn create_for_data(
        data: *mut u8,
        width: u32,
        height: u32,
        scale_numerator: u32,
    ) -> Result<Self> {
        let scale = scale_numerator as f64 / 120.0;
        let stride = width as i32 * 4; // ARGB8888: 4 bytes per pixel

        // SAFETY: caller guarantees data is valid for stride * height bytes.
        let surface = unsafe {
            ImageSurface::create_for_data_unsafe(
                data,
                Format::ARgb32,
                width as i32,
                height as i32,
                stride,
            )
        }
        .map_err(|e| Error::Renderer(format!("cairo ImageSurface::create_for_data: {e}")))?;

        surface.set_device_scale(scale, scale);

        let cr = Context::new(&surface)
            .map_err(|e| Error::Renderer(format!("cairo Context::new: {e}")))?;

        // Logical dimensions: physical ÷ scale (rounded to nearest pixel).
        let logical_width = (width as f64 / scale).round() as u32;
        let logical_height = (height as f64 / scale).round() as u32;

        tracing::debug!(
            "Renderer: {width}×{height} physical, {logical_width}×{logical_height} logical, scale={scale:.4}"
        );

        Ok(Self {
            surface,
            cr,
            logical_width,
            logical_height,
        })
    }

    /// Draw the text `"hello"` centered on the surface.
    ///
    /// Fills the surface with a solid black background then renders white
    /// sans-serif text.
    pub fn draw_hello(&self) {
        // Background: solid black.
        self.cr.set_source_rgb(0.0, 0.0, 0.0);
        self.cr.paint().unwrap_or_default();

        // Foreground: white text.
        self.cr.set_source_rgb(1.0, 1.0, 1.0);

        // Build Pango layout.
        let layout = pangocairo::functions::create_layout(&self.cr);
        let mut font_desc = FontDescription::new();
        font_desc.set_family("Sans");
        font_desc.set_size(24 * pango::SCALE);
        layout.set_font_description(Some(&font_desc));
        layout.set_text("hello");

        // Center text on the logical canvas.
        let (text_w, text_h) = layout.pixel_size();
        let x = (self.logical_width as f64 - text_w as f64) / 2.0;
        let y = (self.logical_height as f64 - text_h as f64) / 2.0;

        self.cr.move_to(x.max(0.0), y.max(0.0));

        pangocairo::functions::show_layout(&self.cr, &layout);

        tracing::debug!("Renderer::draw_hello: text {text_w}×{text_h} at ({x:.1},{y:.1})");
    }

    /// Flush the Cairo surface so all pixel writes are visible in the SHM buffer.
    ///
    /// Must be called before detaching the `Renderer` (dropping it) and
    /// attaching the underlying buffer to the compositor via
    /// [`crate::wayland::surface::draw`].
    pub fn flush(&self) {
        self.surface.flush();
    }
}
