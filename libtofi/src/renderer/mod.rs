//! Drawing / text layout — tiny-skia + cosmic-text backend (feature **`renderer`**).
//!
//! [`Renderer`] wraps a tiny-skia [`PixmapMut`] backed by a raw SHM buffer
//! slice and exposes a minimal text-drawing API.
//!
//! # Safety note
//!
//! [`Renderer::create_for_data`] is `unsafe` because the raw pointer must
//! remain valid for the lifetime of the `Renderer`.  The SHM buffer from
//! [`crate::shm::ShmPool`] satisfies this requirement as long as the
//! `ShmPool` is not dropped or resized while the `Renderer` is alive.

#[cfg(feature = "renderer")]
use cosmic_text::{
    Attrs, Buffer, Color as CTColor, Family, FontSystem, Metrics, Shaping, SwashCache, Wrap,
};
#[cfg(feature = "renderer")]
use tiny_skia::Pixmap;

#[cfg(feature = "renderer")]
use crate::entry::pixel_format::{fill_rgba_premul, rgba_premul_to_argb8888};
#[cfg(feature = "renderer")]
use crate::{Error, Result};

#[cfg(test)]
mod tests;

// ── Renderer ──────────────────────────────────────────────────────────────────

/// tiny-skia + cosmic-text renderer backed by a single SHM frame.
#[cfg(feature = "renderer")]
pub struct Renderer {
    pixmap: Pixmap,
    shm_data: *mut u8,
    frame_bytes: usize,
    scale_numerator: u32,
    /// Width in **logical** pixels (physical ÷ scale).
    pub logical_width: u32,
    /// Height in **logical** pixels (physical ÷ scale).
    pub logical_height: u32,
}

#[cfg(feature = "renderer")]
impl Renderer {
    /// Create a tiny-skia pixmap backed by a raw SHM buffer.
    ///
    /// Drawing uses **logical** pixel coordinates; the pixmap is physical size.
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
        let frame_bytes = (width * height * 4) as usize;

        let pixmap = Pixmap::new(width, height)
            .ok_or_else(|| Error::Renderer("Renderer pixmap allocation failed".into()))?;

        let logical_width = (width as f64 / scale).round() as u32;
        let logical_height = (height as f64 / scale).round() as u32;

        tracing::debug!(
            "Renderer: {width}×{height} physical, {logical_width}×{logical_height} logical, scale={scale:.4}"
        );

        Ok(Self {
            pixmap,
            shm_data: data,
            frame_bytes,
            scale_numerator,
            logical_width,
            logical_height,
        })
    }

    /// Draw the text `"hello"` centered on the surface.
    pub fn draw_hello(&mut self) {
        fill_rgba_premul(self.pixmap.data_mut(), 0, 0, 0, 255);

        let scale = self.scale_numerator as f64 / 120.0;
        let mut font_system = FontSystem::new();
        let mut swash_cache = SwashCache::new();
        let metrics = Metrics::new(24.0, 28.8);
        let mut buffer = Buffer::new(&mut font_system, metrics);
        buffer.set_size(Some(10_000.0), Some(28.8));
        buffer.set_wrap(Wrap::None);
        let attrs = Attrs::new().family(Family::SansSerif);
        buffer.set_text("hello", &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut font_system, false);

        let (text_w, text_h) = buffer
            .layout_runs()
            .next()
            .map(|run| (run.line_w, run.line_height))
            .unwrap_or((0.0, 28.8));

        let x = ((self.logical_width as f64 - text_w as f64) / 2.0).max(0.0);
        let y = ((self.logical_height as f64 - text_h as f64) / 2.0).max(0.0);
        let color = CTColor::rgb(255, 255, 255);

        let width = self.pixmap.width();
        let height = self.pixmap.height();
        buffer.draw(
            &mut font_system,
            &mut swash_cache,
            color,
            |gx, gy, w, h, c| {
                blit_glyph(
                    self.pixmap.data_mut(),
                    width,
                    height,
                    scale,
                    x,
                    y,
                    gx,
                    gy,
                    w,
                    h,
                    c,
                );
            },
        );

        tracing::debug!("Renderer::draw_hello: text {text_w}×{text_h} at ({x:.1},{y:.1})");
    }

    /// Convert the pixmap into the ARGB8888 SHM buffer.
    pub fn flush(&self) {
        // SAFETY: caller guarantees shm_data validity.
        unsafe {
            rgba_premul_to_argb8888(
                self.pixmap.data(),
                std::slice::from_raw_parts_mut(self.shm_data, self.frame_bytes),
            );
        }
    }
}

#[cfg(feature = "renderer")]
fn blit_glyph(
    data: &mut [u8],
    width: u32,
    height: u32,
    scale: f64,
    origin_x: f64,
    origin_y: f64,
    gx: i32,
    gy: i32,
    gw: u32,
    gh: u32,
    color: CTColor,
) {
    let base_x = (origin_x + gx as f64) * scale;
    let base_y = (origin_y + gy as f64) * scale;
    let pw = gw as f64 * scale;
    let ph = gh as f64 * scale;

    let start_x = base_x.floor().max(0.0) as u32;
    let start_y = base_y.floor().max(0.0) as u32;
    let end_x = (base_x + pw).ceil().min(width as f64) as u32;
    let end_y = (base_y + ph).ceil().min(height as f64) as u32;

    let [r, g, b, a] = color.as_rgba();
    for py in start_y..end_y {
        for px in start_x..end_x {
            let lx = ((px as f64 + 0.5 - base_x) / scale) as u32;
            let ly = ((py as f64 + 0.5 - base_y) / scale) as u32;
            if lx >= gw || ly >= gh {
                continue;
            }
            if a == 0 {
                continue;
            }
            let offset = (py as usize * width as usize + px as usize) * 4;
            if offset + 3 >= data.len() {
                continue;
            }
            if a == 255 {
                let pr = ((r as u16 * a as u16 + 127) / 255) as u8;
                let pg = ((g as u16 * a as u16 + 127) / 255) as u8;
                let pb = ((b as u16 * a as u16 + 127) / 255) as u8;
                data[offset] = pr;
                data[offset + 1] = pg;
                data[offset + 2] = pb;
                data[offset + 3] = a;
            } else {
                blend_pixel(&mut data[offset..offset + 4], r, g, b, a);
            }
        }
    }
}

#[cfg(feature = "renderer")]
fn blend_pixel(dst: &mut [u8], r: u8, g: u8, b: u8, a: u8) {
    let pr = ((r as u16 * a as u16 + 127) / 255) as u8;
    let pg = ((g as u16 * a as u16 + 127) / 255) as u8;
    let pb = ((b as u16 * a as u16 + 127) / 255) as u8;
    let inv = 255 - a;
    dst[0] = ((pr as u16 * a as u16 + dst[0] as u16 * inv as u16 + 127) / 255) as u8;
    dst[1] = ((pg as u16 * a as u16 + dst[1] as u16 * inv as u16 + 127) / 255) as u8;
    dst[2] = ((pb as u16 * a as u16 + dst[2] as u16 * inv as u16 + 127) / 255) as u8;
    dst[3] = (a as u16 + (dst[3] as u16 * inv as u16 + 127) / 255) as u8;
}
