//! Layer surface creation and double-buffered SHM presentation.
//!
//! # Scale handling (Step 4.5)
//!
//! The layer surface size is always expressed in logical pixels.  The SHM
//! buffer (and Cairo canvas from Step 5) uses **physical** pixels:
//!
//! ```text
//! physical = scale_apply(logical, effective_scale)
//! ```
//!
//! where `effective_scale` is the `wp_fractional_scale_v1` preferred scale
//! (denominator 120) if available, or `integer_scale × 120` from `wl_output`.
//!
//! A `wp_viewport` is set up when `wp_viewporter` is present so the compositor
//! maps the physical buffer back to the logical size, enabling fractional HiDPI.
//!
//! # C reference
//!
//! `src/main.c` layer setup (~line 1524), `src/surface.c` `surface_init` /
//! `surface_draw`, `src/shm.c` `shm_allocate_file`, `src/scale.c`.

use wayland_client::{EventQueue, QueueHandle, protocol::wl_surface};
use wayland_protocols::wp::{
    fractional_scale::v1::client::wp_fractional_scale_v1,
    viewporter::client::{wp_viewport, wp_viewporter},
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::color::Color;
use crate::scale::scale_apply;
use crate::shm::ShmPool;
use crate::{Error, Result};

use super::WaylandState;

// ── SurfaceConfig ─────────────────────────────────────────────────────────────

/// Parameters for creating the launcher layer surface.
///
/// All dimensions are **logical pixels**; the scale-to-physical conversion
/// happens inside [`create_surface`] using the compositor-reported scale factor.
///
/// C reference: the fields of `struct tofi` that feed `zwlr_layer_surface_v1`
/// requests in `src/main.c`.
pub struct SurfaceConfig {
    /// Logical width in pixels (0 = let compositor decide).
    pub width: u32,
    /// Logical height in pixels (0 = let compositor decide).
    pub height: u32,
    /// Edges the surface is anchored to.
    pub anchor: zwlr_layer_surface_v1::Anchor,
    /// Exclusive zone: -1 = ignore others, 0 = avoid, >0 = claim space.
    pub exclusive_zone: i32,
    pub margin_top: i32,
    pub margin_right: i32,
    pub margin_bottom: i32,
    pub margin_left: i32,
    /// Output to appear on; `None` means let the compositor choose.
    pub output: Option<super::OutputInfo>,
}

// ── SurfaceState ──────────────────────────────────────────────────────────────

/// Live Wayland surface state for the launcher window.
///
/// C reference: `struct surface` in `src/surface.h` + layer surface fields of
/// `struct tofi` in `src/tofi.h`.
pub struct SurfaceState {
    pub wl_surface: wl_surface::WlSurface,
    pub layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    /// Logical width as reported by the `configure` event.
    pub width: u32,
    /// Logical height as reported by the `configure` event.
    pub height: u32,
    /// Physical width in pixels (logical × scale).
    pub phys_width: u32,
    /// Physical height in pixels (logical × scale).
    pub phys_height: u32,
    /// `true` once `ack_configure` has been sent.
    pub configured: bool,
    /// Double-buffered SHM pool; `None` until after the configure roundtrip.
    ///
    /// C reference: `shm_pool_fd`, `shm_pool_data`, `buffers[2]` in `struct surface`.
    pub(super) shm: Option<ShmPool<WaylandState>>,
    /// Index of the buffer currently being written (0 or 1).
    ///
    /// C reference: `surface->index` in `src/surface.c`.
    pub(super) index: usize,
    /// Viewport for fractional-scale presentation; `None` if not supported.
    ///
    /// C reference: `tofi.window.wp_viewport` in `src/main.c`.
    pub(super) viewport: Option<wp_viewport::WpViewport>,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Fill an ARGB8888 byte buffer with a solid colour.
///
/// `buf` must be a multiple of 4 bytes (one u32 per pixel).
fn fill_argb8888(buf: &mut [u8], color: Color) {
    let a = (color.a * 255.0) as u8;
    let r = (color.r * 255.0) as u8;
    let g = (color.g * 255.0) as u8;
    let b = (color.b * 255.0) as u8;
    let pixel = (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | (b as u32);
    let bytes = pixel.to_ne_bytes();
    for chunk in buf.chunks_exact_mut(4) {
        chunk.copy_from_slice(&bytes);
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Create the launcher layer surface, allocate a double-buffered SHM pool
/// sized to **physical** pixels, and commit the first frame.
///
/// Scale handling mirrors `src/main.c` (Steps 4.5 section):
/// - If `wp_fractional_scale_v1` is supported, its `preferred_scale` event
///   fires during the configure roundtrip and is used for physical dimensions.
/// - Otherwise, the integer scale from `wl_output` is used (`scale × 120`).
/// - If `wp_viewporter` is available, a viewport is set up so the compositor
///   maps the physical buffer back to logical dimensions (fractional HiDPI).
/// - If width or height is 0, fractional scaling is skipped and
///   `wl_surface_set_buffer_scale` is used (legacy behaviour).
///
/// After this call [`WaylandState::surface`] is populated and the window is on
/// screen.  Run [`EventQueue::blocking_dispatch`] until [`WaylandState::closed`].
///
/// # C reference
///
/// `src/main.c` lines ~1527–1701: surface creation, layer surface setup,
/// third roundtrip, `surface_init`, `surface_draw`.
pub fn create_surface(
    state: &mut WaylandState,
    event_queue: &mut EventQueue<WaylandState>,
    cfg: &SurfaceConfig,
    bg: Color,
) -> Result<()> {
    let qh: QueueHandle<WaylandState> = event_queue.handle();

    let compositor = state
        .compositor
        .as_ref()
        .ok_or_else(|| Error::Wayland("wl_compositor missing".into()))?;
    let layer_shell = state
        .layer_shell
        .as_ref()
        .ok_or_else(|| Error::Wayland("zwlr_layer_shell_v1 missing".into()))?;

    // ── 1. Create wl_surface + layer surface ─────────────────────────────────
    let wl_surface: wl_surface::WlSurface = compositor.create_surface(&qh, ());
    tracing::debug!("Created wl_surface");

    let wl_output = cfg.output.as_ref().map(|o| &o.output);
    let layer_surface: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1 = layer_shell.get_layer_surface(
        &wl_surface,
        wl_output,
        zwlr_layer_shell_v1::Layer::Overlay,
        "launcher".to_owned(),
        &qh,
        (),
    );

    layer_surface
        .set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive);
    layer_surface.set_anchor(cfg.anchor);
    layer_surface.set_exclusive_zone(cfg.exclusive_zone);
    layer_surface.set_margin(
        cfg.margin_top,
        cfg.margin_right,
        cfg.margin_bottom,
        cfg.margin_left,
    );
    // Layer surface size is always in logical pixels.
    layer_surface.set_size(cfg.width, cfg.height);
    tracing::debug!(
        "Layer surface: {}×{} logical  anchor={:?}  exclusive_zone={}",
        cfg.width,
        cfg.height,
        cfg.anchor,
        cfg.exclusive_zone,
    );

    // ── 2. Attach fractional scale listener (before commit) ───────────────────
    // C: wp_fractional_scale_manager_v1_get_fractional_scale + listener setup
    // Reset any previously stored scale so we detect a fresh value.
    state.fractional_scale = 0;
    let _frac_scale: Option<wp_fractional_scale_v1::WpFractionalScaleV1> =
        state.fractional_scale_manager.as_ref().map(|mgr| {
            let fs = mgr.get_fractional_scale(&wl_surface, &qh, ());
            tracing::debug!("Attached wp_fractional_scale_v1");
            fs
        });

    // ── 3. Create viewport if supported ───────────────────────────────────────
    // C: wp_viewporter_get_viewport
    let viewport: Option<wp_viewport::WpViewport> =
        state
            .viewporter
            .as_ref()
            .map(|vp: &wp_viewporter::WpViewporter| {
                let v = vp.get_viewport(&wl_surface, &qh, ());
                tracing::debug!("Created wp_viewport");
                v
            });

    // ── 4. Store surface state before committing ──────────────────────────────
    let has_viewport = viewport.is_some();
    state.surface = Some(SurfaceState {
        wl_surface: wl_surface.clone(),
        layer_surface,
        width: cfg.width,
        height: cfg.height,
        phys_width: cfg.width, // updated after configure roundtrip
        phys_height: cfg.height,
        configured: false,
        shm: None,
        index: 0,
        viewport,
    });

    // Commit without a buffer to trigger the configure (and preferred_scale) event.
    wl_surface.commit();

    // ── 5. Third roundtrip — configure + preferred_scale must fire ────────────
    tracing::debug!("Third roundtrip: waiting for configure + scale");
    event_queue
        .roundtrip(state)
        .map_err(|e| Error::Wayland(e.to_string()))?;
    tracing::debug!("Third roundtrip done");

    if !state
        .surface
        .as_ref()
        .map(|s| s.configured)
        .unwrap_or(false)
    {
        return Err(Error::Wayland(
            "layer surface configure event did not arrive".into(),
        ));
    }

    let (logical_w, logical_h) = state.surface.as_ref().map(|s| (s.width, s.height)).unwrap();
    tracing::debug!("Configured at {logical_w}×{logical_h} (logical)");

    // ── 6. Determine effective scale ──────────────────────────────────────────
    // C: tofi.window.fractional_scale / tofi.window.scale logic in src/main.c
    //
    // If fractional scale arrived, use it. Otherwise fall back to integer scale
    // from the target output (or 1× if unknown).
    let int_scale = cfg
        .output
        .as_ref()
        .map(|o| o.scale)
        .or_else(|| state.outputs.first().map(|o| o.scale))
        .unwrap_or(1);

    let effective_scale = if state.fractional_scale != 0 {
        tracing::debug!(
            "Using fractional scale {}/{} (×{:.4})",
            state.fractional_scale,
            120,
            state.fractional_scale as f64 / 120.0
        );
        state.fractional_scale
    } else {
        let s = (int_scale as u32) * 120;
        tracing::debug!("Using integer scale {int_scale} → {s}/120");
        s
    };

    // ── 7. Handle legacy zero-size / no-viewporter cases ──────────────────────
    // C: src/main.c ~1530–1565
    if logical_w == 0 || logical_h == 0 {
        tracing::warn!(
            "Width or height is 0 — fractional scaling disabled; \
             using wl_surface_set_buffer_scale({int_scale})"
        );
        wl_surface.set_buffer_scale(int_scale);
    } else if !has_viewport && effective_scale != 120 {
        tracing::warn!(
            "wp_viewporter not available — fractional scaling will not work; \
             falling back to wl_surface_set_buffer_scale({int_scale})"
        );
        wl_surface.set_buffer_scale(int_scale);
    }

    // ── 8. Compute physical dimensions ───────────────────────────────────────
    // C: scale_apply(width, fractional_scale) / width * scale
    let phys_w = scale_apply(logical_w, effective_scale);
    let phys_h = scale_apply(logical_h, effective_scale);
    tracing::debug!("Physical buffer: {phys_w}×{phys_h} px (scale={effective_scale}/120)");

    // Update stored physical dimensions.
    if let Some(s) = state.surface.as_mut() {
        s.phys_width = phys_w;
        s.phys_height = phys_h;
    }

    // ── 9. Set viewport destination to logical size ───────────────────────────
    // C: wp_viewport_set_destination(viewport, width, height)
    if let Some(vp) = state.surface.as_ref().and_then(|s| s.viewport.as_ref())
        && logical_w > 0
        && logical_h > 0
    {
        vp.set_destination(logical_w as i32, logical_h as i32);
        tracing::debug!("Viewport destination: {logical_w}×{logical_h} (logical)");
    }

    // ── 10. Allocate double-buffered SHM pool at physical dimensions ──────────
    // C: surface_init in src/surface.c
    let wl_shm = state
        .shm
        .as_ref()
        .ok_or_else(|| Error::Wayland("wl_shm missing".into()))?;
    let mut pool = ShmPool::new(wl_shm, &qh, phys_w, phys_h)?;

    // Pre-fill both frames with the background colour.
    fill_argb8888(pool.data_mut(0), bg);
    fill_argb8888(pool.data_mut(1), bg);

    state.surface.as_mut().unwrap().shm = Some(pool);

    // ── 11. First draw + flush ────────────────────────────────────────────────
    // C: surface_draw in src/surface.c
    draw(state)?;

    event_queue
        .flush()
        .map_err(|e| Error::Wayland(e.to_string()))?;
    tracing::debug!("Surface committed — window should now be visible");

    Ok(())
}

/// Present the current buffer and flip to the next one.
///
/// Ports `surface_draw` from `src/surface.c`:
/// - attach the current buffer
/// - damage the entire surface
/// - commit
/// - flip `index`
///
/// The caller is responsible for filling [`SurfaceState::shm`]`::data_mut(index)`
/// with rendered pixel data before calling `draw`.
///
/// # C reference
///
/// `surface_draw` in `src/surface.c`.
pub fn draw(state: &mut WaylandState) -> Result<()> {
    let surface = state
        .surface
        .as_mut()
        .ok_or_else(|| Error::Wayland("no surface".into()))?;
    let shm = surface
        .shm
        .as_ref()
        .ok_or_else(|| Error::Wayland("SHM pool not initialised".into()))?;

    // C: wl_surface_attach(surface->wl_surface, surface->buffers[surface->index], 0, 0)
    surface
        .wl_surface
        .attach(Some(shm.buffer(surface.index)), 0, 0);
    // C: wl_surface_damage_buffer(surface->wl_surface, 0, 0, INT32_MAX, INT32_MAX)
    surface.wl_surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
    // C: wl_surface_commit(surface->wl_surface)
    surface.wl_surface.commit();
    // C: surface->index = !surface->index
    surface.index ^= 1;

    Ok(())
}
