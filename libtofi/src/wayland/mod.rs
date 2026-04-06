//! Wayland client, SHM, surfaces (feature **`wayland`**).
//!
//! # Steps
//!
//! * **Step 4.1** — [`connect`] establishes a connection.
//! * **Step 4.2** — [`connect`] binds all registry globals and performs two
//!   roundtrips.
//! * **Step 4.3** — [`surface::create_surface`] creates the layer surface,
//!   fills a solid-color SHM buffer, and commits it to prove placement.
//!
//! # C reference
//!
//! `src/main.c`, `src/surface.c`, `src/shm.c`, `src/tofi.h`.

pub mod surface;

use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle,
    protocol::{
        wl_buffer, wl_compositor, wl_output, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    },
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::wp_fractional_scale_manager_v1, viewporter::client::wp_viewporter,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::{Error, Result};

/// Re-export of the layer-surface anchor bitflag type so callers do not need a
/// direct `wayland-protocols-wlr` dependency.
pub use zwlr_layer_surface_v1::Anchor;

#[cfg(feature = "clipboard")]
pub mod clipboard;

#[cfg(test)]
mod tests;

// ── OutputInfo ───────────────────────────────────────────────────────────────

/// Information about a bound `wl_output`.
///
/// Populated during the second roundtrip.  `name` is empty for compositors
/// that advertise `wl_output` < version 4.
///
/// C reference: `struct output_list_element` in `src/tofi.h`.
#[derive(Debug)]
pub struct OutputInfo {
    /// The underlying output proxy; kept for surface binding (Step 4.3+).
    pub output: wl_output::WlOutput,
    /// Human-readable output name (e.g. `"HDMI-A-1"`); empty pre-v4.
    pub name: String,
    /// Integer scale factor reported by the compositor.
    pub scale: i32,
    /// Pixel width of the current mode.
    pub width: i32,
    /// Pixel height of the current mode.
    pub height: i32,
}

impl OutputInfo {
    fn new(output: wl_output::WlOutput) -> Self {
        Self {
            output,
            name: String::new(),
            scale: 1,
            width: 0,
            height: 0,
        }
    }
}

// ── WaylandState ─────────────────────────────────────────────────────────────

/// Bound Wayland globals, live connection, and (after Step 4.3) the launcher
/// surface.
///
/// Created by [`connect`]; drop to disconnect cleanly.  The [`EventQueue`]
/// returned alongside must be dispatched to keep the connection alive.
///
/// C reference: `struct tofi` (Wayland globals section) in `src/tofi.h`.
pub struct WaylandState {
    /// Raw connection; held to keep the backend alive for the session.
    pub connection: Connection,
    /// `wl_compositor` — required for surface creation (Step 4.3).
    pub compositor: Option<wl_compositor::WlCompositor>,
    /// `wl_shm` — required for SHM buffer allocation (Step 4.4).
    pub shm: Option<wl_shm::WlShm>,
    /// `wl_seat` — required for keyboard/pointer input (Step 6.1).
    pub seat: Option<wl_seat::WlSeat>,
    /// `zwlr_layer_shell_v1` — required for the launcher layer surface.
    pub layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    /// `wp_viewporter` — optional; used for scaled surfaces (Step 4.5).
    pub viewporter: Option<wp_viewporter::WpViewporter>,
    /// `wp_fractional_scale_manager_v1` — optional; HiDPI support (Step 4.5).
    pub fractional_scale_manager:
        Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    /// All advertised outputs; names/modes populated after the second roundtrip.
    pub outputs: Vec<OutputInfo>,
    /// The launcher's layer surface; populated by [`surface::create_surface`].
    pub surface: Option<surface::SurfaceState>,
    /// Set to `true` when the compositor sends `zwlr_layer_surface_v1::closed`.
    pub closed: bool,
}

impl WaylandState {
    fn new(connection: Connection) -> Self {
        Self {
            connection,
            compositor: None,
            shm: None,
            seat: None,
            layer_shell: None,
            viewporter: None,
            fractional_scale_manager: None,
            outputs: Vec::new(),
            surface: None,
            closed: false,
        }
    }
}

// ── Dispatch implementations ──────────────────────────────────────────────────

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_compositor" => {
                let v = version.min(4);
                state.compositor = Some(registry.bind(name, v, qh, ()));
                tracing::debug!("Bound wl_compositor v{v}");
            }
            "wl_shm" => {
                state.shm = Some(registry.bind(name, 1, qh, ()));
                tracing::debug!("Bound wl_shm v1");
            }
            "wl_seat" => {
                let v = version.min(7);
                state.seat = Some(registry.bind(name, v, qh, ()));
                tracing::debug!("Bound wl_seat v{v}");
            }
            "wl_output" => {
                if version < 4 {
                    tracing::warn!(
                        "Compositor advertises wl_output v{version} < 4; \
                         output name selection will not work"
                    );
                }
                let v = version.min(4);
                let idx = state.outputs.len();
                let output: wl_output::WlOutput = registry.bind(name, v, qh, idx);
                state.outputs.push(OutputInfo::new(output));
                tracing::debug!("Bound wl_output {name} v{v}");
            }
            "zwlr_layer_shell_v1" => {
                if version < 3 {
                    tracing::warn!(
                        "Compositor advertises zwlr_layer_shell_v1 v{version} < 3; \
                         screen anchoring may not work"
                    );
                }
                let v = version.min(3);
                state.layer_shell = Some(registry.bind(name, v, qh, ()));
                tracing::debug!("Bound zwlr_layer_shell_v1 v{v}");
            }
            "wp_viewporter" => {
                state.viewporter = Some(registry.bind(name, 1, qh, ()));
                tracing::debug!("Bound wp_viewporter v1");
            }
            "wp_fractional_scale_manager_v1" => {
                state.fractional_scale_manager = Some(registry.bind(name, 1, qh, ()));
                tracing::debug!("Bound wp_fractional_scale_manager_v1 v1");
            }
            _ => {}
        }
    }
}

// `wl_compositor` has no events; impl required but never called.
impl Dispatch<wl_compositor::WlCompositor, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_compositor::WlCompositor,
        _event: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm::WlShm, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_shm::WlShm,
        _event: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Pixel formats collected in Step 4.4 (SHM buffer allocation).
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Capabilities and name handled in Step 6.1 (keyboard/pointer setup).
    }
}

/// Output events populate [`OutputInfo`] entries; index is passed as user data.
///
/// C reference: `output_mode`, `output_scale`, `output_name`, `output_done`
/// listeners in `src/main.c`.
impl Dispatch<wl_output::WlOutput, usize> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &wl_output::WlOutput,
        event: wl_output::Event,
        data: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(info) = state.outputs.get_mut(*data) else {
            return;
        };
        match event {
            wl_output::Event::Name { name } => {
                info.name = name;
            }
            wl_output::Event::Scale { factor } => {
                info.scale = factor;
            }
            wl_output::Event::Mode { width, height, .. } => {
                // Current-flag filtering deferred to Step 4.5.
                info.width = width;
                info.height = height;
            }
            wl_output::Event::Done => {
                tracing::debug!(
                    "Output {}: {:?} {}x{} scale={}",
                    data,
                    info.name,
                    info.width,
                    info.height,
                    info.scale
                );
            }
            _ => {}
        }
    }
}

// `zwlr_layer_shell_v1` has no events; impl required but never called.
impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &zwlr_layer_shell_v1::ZwlrLayerShellV1,
        _event: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// Layer surface configure / close — core of the launcher window lifecycle.
///
/// C reference: `zwlr_layer_surface_configure` / `zwlr_layer_surface_close`
/// in `src/main.c`.
impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        proxy: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                if width == 0 || height == 0 {
                    // Compositor is deferring to us; mirrors C early-return.
                    tracing::debug!("Layer surface configure: deferred (0×0)");
                    return;
                }
                tracing::debug!("Layer surface configure: {width}×{height} serial={serial}");
                proxy.ack_configure(serial);
                if let Some(s) = state.surface.as_mut() {
                    s.width = width;
                    s.height = height;
                    s.configured = true;
                }
            }
            zwlr_layer_surface_v1::Event::Closed => {
                tracing::debug!("Layer surface closed");
                state.closed = true;
            }
            _ => {}
        }
    }
}

/// `wl_surface` enter/leave events (output changes) — ignored until Step 4.5.
impl Dispatch<wl_surface::WlSurface, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Output enter/leave handled in Step 4.5.
    }
}

/// `wl_shm_pool` has no events; impl required but never called.
impl Dispatch<wl_shm_pool::WlShmPool, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_shm_pool::WlShmPool,
        _event: wl_shm_pool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// `wl_buffer` release event — compositor is done with the buffer.
/// Double-buffer swap handled in Step 4.4.
impl Dispatch<wl_buffer::WlBuffer, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        _event: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Buffer reuse / swapping handled in Step 4.4.
    }
}

// `wp_viewporter` has no events; impl required but never called.
impl Dispatch<wp_viewporter::WpViewporter, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wp_viewporter::WpViewporter,
        _event: wp_viewporter::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

// `wp_fractional_scale_manager_v1` has no events; impl required but never called.
impl Dispatch<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        _event: wp_fractional_scale_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Connect to the Wayland compositor, bind all required globals, and return
/// the populated [`WaylandState`] with the live [`EventQueue`].
///
/// Mirrors the C startup sequence in `src/main.c`:
/// 1. `wl_display_connect(NULL)` — open compositor socket.
/// 2. Attach registry listener.
/// 3. **First roundtrip** — `registry_global` fires; globals are bound.
/// 4. **Second roundtrip** — output listeners fire; scale/mode/name available.
///
/// # Errors
///
/// Returns [`Error::Wayland`] if the compositor cannot be reached or a
/// required global (`wl_compositor`, `wl_shm`, `wl_seat`,
/// `zwlr_layer_shell_v1`) is absent.
pub fn connect() -> Result<(WaylandState, EventQueue<WaylandState>)> {
    let connection = Connection::connect_to_env().map_err(|e| Error::Wayland(e.to_string()))?;
    tracing::debug!("Connected to Wayland display");

    let mut event_queue: EventQueue<WaylandState> = connection.new_event_queue();
    let qh = event_queue.handle();
    connection.display().get_registry(&qh, ());

    let mut state = WaylandState::new(connection);

    tracing::debug!("First roundtrip: binding globals");
    event_queue
        .roundtrip(&mut state)
        .map_err(|e| Error::Wayland(e.to_string()))?;
    tracing::debug!("First roundtrip done");

    tracing::debug!("Second roundtrip: receiving output info");
    event_queue
        .roundtrip(&mut state)
        .map_err(|e| Error::Wayland(e.to_string()))?;
    tracing::debug!("Second roundtrip done");

    // Validate required globals — mirrors C's abort-on-missing behaviour.
    if state.compositor.is_none() {
        return Err(Error::Wayland("wl_compositor not advertised".into()));
    }
    if state.shm.is_none() {
        return Err(Error::Wayland("wl_shm not advertised".into()));
    }
    if state.seat.is_none() {
        return Err(Error::Wayland("wl_seat not advertised".into()));
    }
    if state.layer_shell.is_none() {
        return Err(Error::Wayland(
            "zwlr_layer_shell_v1 not advertised (is this a wlroots compositor?)".into(),
        ));
    }

    tracing::debug!(
        "Globals ready — compositor: ✓  shm: ✓  seat: ✓  layer_shell: ✓  \
         viewporter: {}  fractional_scale: {}  outputs: {}",
        state.viewporter.is_some(),
        state.fractional_scale_manager.is_some(),
        state.outputs.len(),
    );

    Ok((state, event_queue))
}
