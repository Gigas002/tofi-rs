//! Drawing / text layout — vello_cpu + parley backend (feature **`renderer`**).
//!
//! [`Renderer`] draws into a raw SHM buffer slice with the pure-Rust
//! [`vello_cpu`] rasterizer and [`parley`] text layout.  This module also
//! holds the drawing primitives shared with [`crate::entry`].
//!
//! # Safety note
//!
//! [`Renderer::create_for_data`] is `unsafe` because the raw pointer must
//! remain valid for the lifetime of the `Renderer`.  The SHM buffer from
//! [`crate::shm::ShmPool`] satisfies this requirement as long as the
//! `ShmPool` is not dropped or resized while the `Renderer` is alive.

#[cfg(feature = "renderer")]
use parley::{
    FontContext, FontFamily, FontFamilyName, GenericFamily, Layout, LayoutContext,
    PositionedLayoutItem, StyleProperty,
};
#[cfg(feature = "renderer")]
use vello_cpu::kurbo::{Affine, Rect};
#[cfg(feature = "renderer")]
use vello_cpu::{Glyph, PixmapMut, RasterizerSettings, RenderContext, Resources};

#[cfg(feature = "renderer")]
use crate::color::Color;
#[cfg(feature = "renderer")]
use crate::{Error, Result};

#[cfg(test)]
mod tests;

// ── Shared primitives ─────────────────────────────────────────────────────────

/// Points → pixels at 96 dpi, the resolution Pango assumed for font sizes.
#[cfg(feature = "renderer")]
pub(crate) const PT_TO_PX: f32 = 96.0 / 72.0;

/// Convert a tofi [`Color`] into a vello paint color.
#[cfg(feature = "renderer")]
pub(crate) fn paint(c: Color) -> vello_cpu::peniko::Color {
    vello_cpu::peniko::Color::new([c.r, c.g, c.b, c.a])
}

/// Check that physical frame dimensions fit vello's `u16` canvas size.
#[cfg(feature = "renderer")]
pub(crate) fn canvas_size(width: u32, height: u32) -> Result<(u16, u16)> {
    match (u16::try_from(width), u16::try_from(height)) {
        (Ok(w), Ok(h)) => Ok((w, h)),
        _ => Err(Error::Renderer(format!(
            "frame {width}×{height} exceeds the maximum canvas size"
        ))),
    }
}

/// Draw every glyph run of `layout` with its top-left corner at `(x, y)`.
///
/// Each run is painted with its brush color unless `color_override` is set.
#[cfg(feature = "renderer")]
pub(crate) fn draw_layout(
    ctx: &mut RenderContext,
    resources: &mut Resources,
    layout: &Layout<Color>,
    (x, y): (f64, f64),
    color_override: Option<Color>,
) {
    let base = *ctx.transform();
    ctx.set_transform(base * Affine::translate((x, y)));

    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let run = glyph_run.run();
            ctx.set_paint(paint(color_override.unwrap_or(glyph_run.style().brush)));

            let coords: Vec<i16> = run
                .normalized_coords()
                .iter()
                .map(|c| c.to_bits())
                .collect();
            let mut builder = ctx
                .glyph_run(resources, run.font())
                .font_size(run.font_size())
                .normalized_coords(&coords)
                .hint(true);
            // Synthetic italic when the matched face has no real italic.
            if let Some(angle) = run.synthesis().skew() {
                builder =
                    builder.glyph_transform(Affine::skew(f64::from(angle.to_radians().tan()), 0.0));
            }
            let glyphs = glyph_run.positioned_glyphs().map(|g| Glyph {
                id: g.id,
                x: g.x,
                y: g.y,
            });
            if let Err(e) = builder.fill_glyphs(glyphs) {
                tracing::debug!("draw_layout: {e}");
            }
        }
    }

    ctx.set_transform(base);
}

/// Rasterize `ctx` into `frame` and convert it to Wayland `ARGB8888`.
///
/// vello_cpu writes premultiplied RGBA8; `wl_shm` `ARGB8888` is the same
/// premultiplied data stored as native-endian 32-bit `0xAARRGGBB` words.
#[cfg(feature = "renderer")]
pub(crate) fn rasterize(
    ctx: &mut RenderContext,
    resources: &mut Resources,
    frame: &mut [u8],
    width: u16,
    height: u16,
) {
    ctx.flush();
    let Some(pixmap) = PixmapMut::new(width, height, frame) else {
        tracing::error!("rasterize: frame is not {width}×{height}×4 bytes");
        return;
    };
    ctx.render_with(pixmap, resources, RasterizerSettings::default());

    for px in frame.as_chunks_mut::<4>().0 {
        if cfg!(target_endian = "little") {
            // RGBA → BGRA
            px.swap(0, 2);
        } else {
            // RGBA → ARGB
            px.rotate_right(1);
        }
    }
}

// ── Renderer ──────────────────────────────────────────────────────────────────

/// vello_cpu + parley renderer backed by a single SHM frame.
///
/// Create via [`Renderer::create_for_data`] and draw into it; each draw call
/// writes finished pixels, so the frame can be handed to the compositor via
/// [`crate::wayland::surface::draw`] right after.
#[cfg(feature = "renderer")]
pub struct Renderer {
    data: *mut u8,
    width: u16,
    height: u16,
    scale: f64,
    /// Width in **logical** pixels (physical ÷ scale).
    pub logical_width: u32,
    /// Height in **logical** pixels (physical ÷ scale).
    pub logical_height: u32,
}

#[cfg(feature = "renderer")]
impl Renderer {
    /// Wrap a raw SHM frame (premultiplied `ARGB8888`) and record the device
    /// scale so all subsequent drawing operations use **logical** pixel
    /// coordinates.
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
        let (w16, h16) = canvas_size(width, height)?;

        // Logical dimensions: physical ÷ scale (rounded to nearest pixel).
        let logical_width = (width as f64 / scale).round() as u32;
        let logical_height = (height as f64 / scale).round() as u32;

        tracing::debug!(
            "Renderer: {width}×{height} physical, {logical_width}×{logical_height} logical, scale={scale:.4}"
        );

        Ok(Self {
            data,
            width: w16,
            height: h16,
            scale,
            logical_width,
            logical_height,
        })
    }

    /// Draw the text `"hello"` centered on the surface.
    ///
    /// Fills the surface with a solid black background then renders white
    /// sans-serif text.
    pub fn draw_hello(&self) {
        let mut ctx = RenderContext::new(self.width, self.height);
        let mut resources = Resources::new();
        ctx.set_transform(Affine::scale(self.scale));

        // Background: solid black.
        let (lw, lh) = (self.logical_width as f64, self.logical_height as f64);
        ctx.set_paint(paint(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }));
        ctx.fill_rect(&Rect::new(0.0, 0.0, lw, lh));

        // Foreground: white text.
        let white = Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let mut font_cx = FontContext::new();
        let mut layout_cx = LayoutContext::new();
        let mut builder = layout_cx.ranged_builder(&mut font_cx, "hello", 1.0, true);
        builder.push_default(StyleProperty::FontFamily(FontFamily::Single(
            FontFamilyName::Generic(GenericFamily::SansSerif),
        )));
        builder.push_default(StyleProperty::FontSize(24.0 * PT_TO_PX));
        builder.push_default(StyleProperty::Brush(white));
        let mut layout: Layout<Color> = builder.build("hello");
        layout.break_all_lines(None);

        // Center text on the logical canvas.
        let (text_w, text_h) = (layout.width() as f64, layout.height() as f64);
        let x = ((lw - text_w) / 2.0).max(0.0);
        let y = ((lh - text_h) / 2.0).max(0.0);
        draw_layout(&mut ctx, &mut resources, &layout, (x, y), None);

        // SAFETY: `create_for_data`'s contract guarantees `data` is valid for
        // `width × height × 4` writable bytes for the lifetime of `self`.
        let frame = unsafe {
            std::slice::from_raw_parts_mut(
                self.data,
                self.width as usize * self.height as usize * 4,
            )
        };
        rasterize(&mut ctx, &mut resources, frame, self.width, self.height);

        tracing::debug!("Renderer::draw_hello: text {text_w:.1}×{text_h:.1} at ({x:.1},{y:.1})");
    }

    /// No-op kept for API compatibility.
    ///
    /// Draw calls now write finished pixels directly into the SHM buffer.
    #[deprecated(note = "draw calls write finished pixels; flushing is no longer needed")]
    pub fn flush(&self) {}
}
