//! Double-buffered SHM pool for a Wayland surface.
//!
//! Ports `src/shm.c` (`shm_allocate_file`) and the SHM portion of
//! `src/surface.c` (`surface_init` / `surface_destroy`).
//!
//! # Design
//!
//! A single `memfd`-backed pool of `2 × stride × height` bytes backs two
//! [`wl_buffer`] proxies at offsets 0 and `stride × height`.  The caller
//! writes pixels into [`ShmPool::data_mut`], then passes the matching
//! [`ShmPool::buffer`] to `wl_surface_attach`.
//!
//! # C reference
//!
//! `src/shm.c` `shm_allocate_file`, `src/surface.c` `surface_init` /
//! `surface_destroy`.

use std::ffi::c_void;
use std::os::unix::io::{AsFd, OwnedFd};
use std::ptr::{self, NonNull};

use rustix::fs::{MemfdFlags, ftruncate, memfd_create};
#[cfg(target_os = "linux")]
use rustix::mm::{Advice, madvise};
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use wayland_client::{
    QueueHandle,
    protocol::{wl_buffer, wl_shm, wl_shm_pool},
};

use crate::{Error, Result};

// ── ShmPool ───────────────────────────────────────────────────────────────────

/// Double-buffered SHM pool.
///
/// Allocates a `memfd` of `2 × stride × height` bytes and exposes two
/// [`wl_buffer`] proxies at offsets 0 and `stride × height`.
///
/// Generic over the Wayland event-dispatch state `D` so it can be created
/// with any [`QueueHandle<D>`] without depending on [`crate::wayland::WaylandState`]
/// directly.
///
/// C reference: `struct surface` (pool/buffer fields) in `src/surface.h`.
pub struct ShmPool<D: 'static> {
    /// The underlying shared memory file; kept alive so the compositor can
    /// continue reading it until `wl_buffer::release` fires.
    _fd: OwnedFd,
    /// CPU-side mapping of the entire pool (both frames).
    ptr: NonNull<c_void>,
    /// Total size of the mapping in bytes (`frame_size * 2`).
    pool_size: usize,
    /// Bytes per row (`width * 4`).
    stride: usize,
    /// Bytes for one frame (`stride * height`).
    frame_size: usize,
    /// Wayland pool proxy; kept alive so `buffers` remain valid.
    _pool: wl_shm_pool::WlShmPool,
    /// Two buffers at offsets 0 and `frame_size`.
    buffers: [wl_buffer::WlBuffer; 2],
    /// Carries the `D` type parameter without storing a `D`.
    _phantom: std::marker::PhantomData<D>,
}

// SAFETY: `ptr` is only accessed from a single thread and is freed in `Drop`.
unsafe impl<D> Send for ShmPool<D> {}
unsafe impl<D> Sync for ShmPool<D> {}

impl<D: 'static> ShmPool<D>
where
    D: wayland_client::Dispatch<wl_shm_pool::WlShmPool, ()>,
    D: wayland_client::Dispatch<wl_buffer::WlBuffer, ()>,
{
    /// Allocate a new double-buffered SHM pool for a surface of `width × height`.
    ///
    /// # C reference
    ///
    /// `shm_allocate_file` in `src/shm.c` + `surface_init` in `src/surface.c`.
    pub fn new(
        wl_shm: &wl_shm::WlShm,
        qh: &QueueHandle<D>,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let stride = width as usize * 4; // ARGB8888: 4 bytes/pixel
        let frame_size = stride * height as usize;
        let pool_size = frame_size * 2; // double-buffered

        // ── Allocate shared memory file ────────────────────────────────────────
        // C: shm_allocate_file (memfd_create branch)
        let fd: OwnedFd = memfd_create("wl_shm", MemfdFlags::CLOEXEC)
            .map_err(|e| Error::Wayland(format!("memfd_create: {e}")))?;
        ftruncate(&fd, pool_size as u64).map_err(|e| Error::Wayland(format!("ftruncate: {e}")))?;

        // ── Map the pool into process address space ────────────────────────────
        // SAFETY: fd is valid, size matches ftruncate, MAP_SHARED is correct for shm.
        let raw: *mut c_void = unsafe {
            mmap(
                ptr::null_mut(),
                pool_size,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED,
                &fd,
                0,
            )
            .map_err(|e| Error::Wayland(format!("mmap: {e}")))?
        };
        // SAFETY: rustix converts MAP_FAILED to Err; a successful mmap is never null.
        let ptr: NonNull<c_void> = unsafe { NonNull::new_unchecked(raw) };

        // ── MADV_HUGEPAGE (Linux only) ────────────────────────────────────────
        // C: src/surface.c — madvise(MADV_HUGEPAGE) when pool >= 2 MiB.
        // Transparent HugePages can reduce page-fault overhead on the first
        // cairo_paint().  Disabled for shared memory in many kernels, but the
        // hint is harmless and costs nothing to request.
        #[cfg(target_os = "linux")]
        if pool_size >= 2 * 1024 * 1024 {
            // SAFETY: ptr is a valid mmap of `pool_size` bytes.
            if let Err(e) = unsafe { madvise(ptr.as_ptr(), pool_size, Advice::LinuxHugepage) } {
                tracing::debug!("MADV_HUGEPAGE unavailable (ignored): {e}");
            }
        }

        // ── Create Wayland pool + two buffers ──────────────────────────────────
        // C: wl_shm_create_pool + wl_shm_pool_create_buffer ×2
        let pool = wl_shm.create_pool(fd.as_fd(), pool_size as i32, qh, ());
        let buf0 = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );
        let buf1 = pool.create_buffer(
            frame_size as i32,
            width as i32,
            height as i32,
            stride as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );

        tracing::debug!(
            "SHM pool: {} KiB ({width}×{height} ×2 frames)",
            pool_size / 1024
        );

        Ok(Self {
            _fd: fd,
            ptr,
            pool_size,
            stride,
            frame_size,
            _pool: pool,
            buffers: [buf0, buf1],
            _phantom: std::marker::PhantomData,
        })
    }

    /// Mutable byte slice for frame `index` (0 or 1).
    ///
    /// The slice covers exactly one frame (`stride × height` bytes, ARGB8888).
    pub fn data_mut(&mut self, index: usize) -> &mut [u8] {
        debug_assert!(index < 2, "buffer index must be 0 or 1");
        let offset = self.frame_size * index;
        // SAFETY: ptr is a valid mmap of `pool_size` bytes; offset + frame_size
        // ≤ pool_size; lifetime tied to &mut self.
        unsafe {
            std::slice::from_raw_parts_mut(
                (self.ptr.as_ptr() as *mut u8).add(offset),
                self.frame_size,
            )
        }
    }

    /// Reference to the [`wl_buffer`] for frame `index` (0 or 1).
    pub fn buffer(&self, index: usize) -> &wl_buffer::WlBuffer {
        &self.buffers[index]
    }

    /// Bytes per row.
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Raw pointer to the start of the entire pool (both frames contiguous).
    ///
    /// Used by [`crate::entry::Entry::new`] which creates two Cairo surfaces
    /// from consecutive halves of this mapping — matching the C `entry_init`
    /// double-buffer layout.
    ///
    /// # Safety
    ///
    /// The returned pointer is valid for `pool_size` bytes and remains so as
    /// long as this `ShmPool` is alive.  The caller must not outlive `self`.
    pub fn data_both_frames_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr() as *mut u8
    }
}

impl<D> Drop for ShmPool<D> {
    fn drop(&mut self) {
        // SAFETY: ptr/pool_size came from a successful mmap and are only freed here.
        unsafe { munmap(self.ptr.as_ptr(), self.pool_size).ok() };
        // _fd, _pool, and buffers drop automatically; compositor releases them
        // after the next wl_buffer::release event.
    }
}
