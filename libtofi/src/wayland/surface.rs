//! Layer surface creation and the inline SHM solid-color buffer for Step 4.3.
//!
//! The SHM allocation here (one buffer, `memfd_create` + `mmap`) will be
//! extracted into [`crate::shm`] in Step 4.4.
//!
//! # C reference
//!
//! `src/main.c` layer setup (~line 1524), `src/surface.c` `surface_init` /
//! `surface_draw`, `src/shm.c` `shm_allocate_file`.

use std::ffi::{CString, c_void};
use std::num::NonZeroUsize;
use std::os::unix::io::{AsFd, OwnedFd};
use std::ptr::NonNull;

use nix::sys::memfd::{MemFdCreateFlag, memfd_create};
use nix::sys::mman::{MapFlags, ProtFlags, mmap, munmap};
use nix::unistd::ftruncate;
use wayland_client::{
    EventQueue, QueueHandle,
    protocol::{wl_buffer, wl_shm, wl_shm_pool, wl_surface},
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::color::Color;
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

// ── ShmMmap ───────────────────────────────────────────────────────────────────

/// RAII wrapper for a `mmap`-ed region; calls `munmap` on drop.
///
/// Will be replaced by a proper `libtofi::shm::ShmPool` in Step 4.4.
struct ShmMmap {
    ptr: NonNull<c_void>,
    len: usize,
}

impl Drop for ShmMmap {
    fn drop(&mut self) {
        // SAFETY: ptr/len came from a successful `mmap` call and are only
        // freed here.
        unsafe { munmap(self.ptr, self.len).ok() };
    }
}

// SAFETY: the mapped memory is only used from a single thread and freed in Drop.
unsafe impl Send for ShmMmap {}
unsafe impl Sync for ShmMmap {}

// ── SurfaceShm ────────────────────────────────────────────────────────────────

/// SHM resources backing the visible surface buffer.
///
/// Holds the wayland pool+buffer proxies and the CPU-side mapped memory.
/// Step 4.4 will extract this into `libtofi::shm`.
struct SurfaceShm {
    _pool: wl_shm_pool::WlShmPool,
    #[allow(dead_code)]
    buffer: wl_buffer::WlBuffer,
    #[allow(dead_code)]
    mmap: ShmMmap,
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
    /// SHM buffer; `None` until after the configure roundtrip.
    shm: Option<SurfaceShm>,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Allocate a `memfd_create`-based shared memory file of `size` bytes.
///
/// C reference: `shm_allocate_file` in `src/shm.c` (Linux branch).
fn alloc_shm_fd(size: usize) -> Result<OwnedFd> {
    let name = CString::new("wl_shm").expect("static CStr");
    let fd = memfd_create(&name, MemFdCreateFlag::MFD_CLOEXEC)
        .map_err(|e| Error::Wayland(format!("memfd_create: {e}")))?;
    ftruncate(&fd, size as i64).map_err(|e| Error::Wayland(format!("ftruncate: {e}")))?;
    Ok(fd)
}

/// Map `fd` into process address space for read+write.
fn mmap_fd(fd: &OwnedFd, size: usize) -> Result<ShmMmap> {
    let len = NonZeroUsize::new(size).expect("non-zero size");
    // SAFETY: fd is valid, size matches ftruncate, MAP_SHARED is correct for shm.
    let ptr: NonNull<c_void> = unsafe {
        mmap(
            None,
            len,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
            MapFlags::MAP_SHARED,
            fd.as_fd(),
            0,
        )
        .map_err(|e| Error::Wayland(format!("mmap: {e}")))?
    };
    Ok(ShmMmap { ptr, len: size })
}

/// Fill an ARGB8888 buffer with a solid colour.
fn fill_argb8888(mmap: &ShmMmap, pixels: usize, color: Color) {
    let a = (color.a * 255.0) as u8;
    let r = (color.r * 255.0) as u8;
    let g = (color.g * 255.0) as u8;
    let b = (color.b * 255.0) as u8;
    let pixel = (a as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32;
    // SAFETY: ptr is a valid mmap of at least `pixels * 4` bytes.
    let slice = unsafe { std::slice::from_raw_parts_mut(mmap.ptr.as_ptr() as *mut u32, pixels) };
    slice.fill(pixel);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Create the launcher layer surface, allocate a solid-color SHM buffer, and
/// commit it so the window becomes visible.
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

    // ── 4. Allocate SHM buffer with the confirmed dimensions ──────────────────
    let stride = width as usize * 4; // ARGB8888: 4 bytes/pixel
    let pool_size = stride * height as usize;

    let fd = alloc_shm_fd(pool_size)?;
    let mmap = mmap_fd(&fd, pool_size)?;
    fill_argb8888(&mmap, (width * height) as usize, bg);
    tracing::debug!("SHM pool: {} KiB ({width}×{height})", pool_size / 1024);

    let shm = state
        .shm
        .as_ref()
        .ok_or_else(|| Error::Wayland("wl_shm missing".into()))?;
    let pool: wl_shm_pool::WlShmPool = shm.create_pool(fd.as_fd(), pool_size as i32, &qh, ());
    let buffer: wl_buffer::WlBuffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        stride as i32,
        wl_shm::Format::Argb8888,
        &qh,
        (),
    );

    // ── 5. Attach, damage, commit ─────────────────────────────────────────────
    wl_surface.attach(Some(&buffer), 0, 0);
    wl_surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
    wl_surface.commit();

    // Store the SHM resources so they live as long as the surface.
    state.surface.as_mut().unwrap().shm = Some(SurfaceShm {
        _pool: pool,
        buffer,
        mmap,
    });

    event_queue
        .flush()
        .map_err(|e| Error::Wayland(e.to_string()))?;
    tracing::debug!("Surface committed — window should now be visible");

    Ok(())
}
