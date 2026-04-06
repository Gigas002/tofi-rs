//! Layer surface creation and double-buffered SHM presentation.
//!
//! # C reference
//!
//! `src/main.c` layer setup (~line 1524), `src/surface.c` `surface_init` /
//! `surface_draw`, `src/shm.c` `shm_allocate_file`.

use wayland_client::{EventQueue, QueueHandle, protocol::wl_surface};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::color::Color;
use crate::shm::ShmPool;
use crate::{Error, Result};

use super::WaylandState;

// ── SurfaceConfig ─────────────────────────────────────────────────────────────

/// Parameters for creating the launcher layer surface.
///
/// All dimensions are resolved logical pixels; percent resolution happens in
/// the CLI crate before this struct is constructed (Step 4.5 provides helpers).
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

/// Create the launcher layer surface, allocate a double-buffered SHM pool, and
/// commit the first frame so the window becomes visible.
///
/// After this call [`WaylandState::surface`] is populated and the window is on
/// screen.  Run [`EventQueue::blocking_dispatch`] until [`WaylandState::closed`].
///
/// # C reference
///
/// `src/main.c` lines ~1524–1701: surface creation, layer surface setup,
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
    layer_surface.set_size(cfg.width, cfg.height);
    tracing::debug!(
        "Layer surface: {}×{}  anchor={:?}  exclusive_zone={}",
        cfg.width,
        cfg.height,
        cfg.anchor,
        cfg.exclusive_zone,
    );

    // ── 2. Store surface state before committing ──────────────────────────────
    state.surface = Some(SurfaceState {
        wl_surface: wl_surface.clone(),
        layer_surface,
        width: cfg.width,
        height: cfg.height,
        configured: false,
        shm: None,
        index: 0,
    });

    // Commit without a buffer to trigger the configure event.
    wl_surface.commit();

    // ── 3. Third roundtrip — configure must fire ──────────────────────────────
    tracing::debug!("Third roundtrip: waiting for configure");
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

    let (width, height) = state.surface.as_ref().map(|s| (s.width, s.height)).unwrap();
    tracing::debug!("Configured at {width}×{height}");

    // ── 4. Allocate double-buffered SHM pool ──────────────────────────────────
    // C: surface_init in src/surface.c
    let wl_shm = state
        .shm
        .as_ref()
        .ok_or_else(|| Error::Wayland("wl_shm missing".into()))?;
    let mut pool = ShmPool::new(wl_shm, &qh, width, height)?;

    // Pre-fill both buffers with the background colour.
    fill_argb8888(pool.data_mut(0), bg);
    fill_argb8888(pool.data_mut(1), bg);

    state.surface.as_mut().unwrap().shm = Some(pool);

    // ── 5. First draw ─────────────────────────────────────────────────────────
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
