//! Wayland client, SHM, surfaces (feature **`wayland`**).
//!
//! # Steps
//!
//! * **Step 4.1** — [`connect`] establishes a connection.
//! * **Step 4.2** — [`connect`] binds all registry globals and performs two
//!   roundtrips.
//! * **Step 4.3** — [`surface::create_surface`] creates the layer surface,
//!   fills a solid-color SHM buffer, and commits it to prove placement.
//! * **Step 4.5** — [`surface::create_surface`] uses output integer scale or
//!   `wp_fractional_scale_v1` to compute physical buffer dimensions; sets up
//!   `wp_viewport` for correct fractional HiDPI presentation.
//! * **Step 6.1** — [`WaylandState`] receives `wl_keyboard` events; XKB
//!   keymap and modifier state are maintained in
//!   [`crate::input::keyboard::KeyboardState`]; key events update the entry
//!   and set [`WaylandState::redraw`].
//!
//! # C reference
//!
//! `src/main.c`, `src/surface.c`, `src/shm.c`, `src/scale.c`, `src/tofi.h`.

pub mod surface;

use std::io::Read as _;

use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat, wl_shm,
        wl_shm_pool, wl_surface,
    },
};

#[cfg(feature = "clipboard")]
use wayland_client::protocol::{wl_data_device, wl_data_device_manager, wl_data_offer};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{wp_fractional_scale_manager_v1, wp_fractional_scale_v1},
    viewporter::client::{wp_viewport, wp_viewporter},
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::{Error, Result};

/// Re-export of the layer-surface anchor bitflag type so callers do not need a
/// direct `wayland-protocols-wlr` dependency.
pub use zwlr_layer_surface_v1::Anchor;

/// Re-export of the output transform enum so callers do not need a direct
/// `wayland-client` dependency.
pub use wl_output::Transform as OutputTransform;

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
    ///
    /// C reference: `output_scale` listener in `src/main.c`.
    pub scale: i32,
    /// Pixel width of the current mode.
    pub width: i32,
    /// Pixel height of the current mode.
    pub height: i32,
    /// Output transform (rotation/flip).
    ///
    /// When this is `_90`, `_270`, `Flipped90`, or `Flipped270` the physical
    /// width and height are swapped relative to the logical output rectangle.
    ///
    /// C reference: `output_done` / transform fields in `src/main.c`.
    pub transform: wl_output::Transform,
}

impl OutputInfo {
    fn new(output: wl_output::WlOutput) -> Self {
        Self {
            output,
            name: String::new(),
            scale: 1,
            width: 0,
            height: 0,
            transform: wl_output::Transform::Normal,
        }
    }
}

// ── WaylandState ─────────────────────────────────────────────────────────────

/// Bound Wayland globals, live connection, entry widget, and event flags.
///
/// Created by [`connect`]; drop to disconnect cleanly.  The [`EventQueue`]
/// returned alongside must be dispatched to keep the connection alive.
///
/// C reference: `struct tofi` in `src/tofi.h`.
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

    // ── Step 6.1: keyboard ───────────────────────────────────────────────────
    /// `wl_keyboard` obtained from `wl_seat` capabilities.
    pub keyboard: Option<wl_keyboard::WlKeyboard>,

    // ── Step 7.1: clipboard ──────────────────────────────────────────────────
    /// `wl_data_device_manager` — bound from registry; required for clipboard.
    ///
    /// C reference: `tofi->wl_data_device_manager` in `src/tofi.h`.
    #[cfg(feature = "clipboard")]
    pub data_device_manager: Option<wl_data_device_manager::WlDataDeviceManager>,
    /// `wl_data_device` — created from the manager; delivers selection events.
    ///
    /// C reference: `tofi->wl_data_device` in `src/tofi.h`.
    #[cfg(feature = "clipboard")]
    pub data_device: Option<wl_data_device::WlDataDevice>,
    /// Clipboard paste state — current offer and active paste pipe.
    ///
    /// C reference: `tofi->clipboard` in `src/tofi.h`.
    #[cfg(feature = "clipboard")]
    pub clipboard: clipboard::ClipboardState,

    // ── Step 6.3: pointer ────────────────────────────────────────────────────
    /// `wl_pointer` obtained from `wl_seat` capabilities.
    pub pointer: Option<wl_pointer::WlPointer>,
    /// When `true`, the pointer cursor is hidden on `wl_pointer::enter`.
    ///
    /// C reference: `tofi->hide_cursor` in `src/tofi.h`.
    pub hide_cursor: bool,
    /// XKB keymap, modifier state, and key-repeat tracking.
    ///
    /// C reference: `tofi->xkb_*` and `tofi->repeat` in `src/tofi.h`.
    pub keyboard_state: crate::input::keyboard::KeyboardState,

    // ── Step 6.1: entry widget ───────────────────────────────────────────────
    // Declared before `surface` so it is dropped before the SHM pool is freed.
    /// The entry layout widget.  Populated after `create_surface` and kept
    /// alive for the duration of the session.  `None` before initialisation or
    /// when the `renderer` feature is disabled.
    ///
    /// C reference: `tofi->window.entry` in `src/tofi.h`.
    #[cfg(feature = "renderer")]
    pub entry: Option<crate::entry::Entry>,

    // ── Step 6.4: drun desktop entries ───────────────────────────────────────
    /// Desktop entries loaded in drun mode; kept for submit (Step 6.5).
    ///
    /// C reference: `tofi->window.entry.apps` in `src/tofi.h`.
    #[cfg(feature = "drun")]
    pub drun_entries: Vec<crate::drun::DesktopEntry>,

    // ── Step 6.1: event flags ────────────────────────────────────────────────
    /// Set by keyboard / repeat handlers when the entry needs to be redrawn.
    ///
    /// C reference: `tofi->window.surface.redraw` in `src/tofi.h`.
    pub redraw: bool,
    /// Set when the user presses Enter — the main loop prints the result and
    /// exits.
    ///
    /// C reference: `tofi->submit` in `src/tofi.h`.
    pub submit: bool,

    // ── Step 6.1: config options forwarded from `TofiConfig` ─────────────────
    /// See `TofiConfig::physical_keybindings`.  Default: `true`.
    pub physical_keybindings: bool,
    /// See `TofiConfig::auto_accept_single`.  Default: `false`.
    pub auto_accept_single: bool,

    /// The launcher's layer surface; populated by [`surface::create_surface`].
    pub surface: Option<surface::SurfaceState>,
    /// Set to `true` when the compositor sends `zwlr_layer_surface_v1::closed`.
    pub closed: bool,
    /// Preferred fractional scale from `wp_fractional_scale_v1` (denominator 120).
    ///
    /// `0` means not yet received; the caller should fall back to
    /// `integer_scale * 120` in that case.
    ///
    /// C reference: `tofi.window.fractional_scale` in `src/main.c`.
    pub fractional_scale: u32,
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
            #[cfg(feature = "clipboard")]
            data_device_manager: None,
            #[cfg(feature = "clipboard")]
            data_device: None,
            #[cfg(feature = "clipboard")]
            clipboard: clipboard::ClipboardState::default(),
            keyboard: None,
            keyboard_state: crate::input::keyboard::KeyboardState::new(true),
            pointer: None,
            hide_cursor: false,
            #[cfg(feature = "drun")]
            drun_entries: Vec::new(),
            #[cfg(feature = "renderer")]
            entry: None,
            redraw: false,
            submit: false,
            physical_keybindings: true,
            auto_accept_single: false,
            surface: None,
            closed: false,
            fractional_scale: 0,
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
            #[cfg(feature = "clipboard")]
            "wl_data_device_manager" => {
                let v = version.min(3);
                state.data_device_manager = Some(registry.bind(name, v, qh, ()));
                tracing::debug!("Bound wl_data_device_manager v{v}");
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

/// Seat capabilities — obtain `wl_keyboard` and/or `wl_pointer` as advertised.
///
/// C reference: `wl_seat_capabilities` in `src/main.c`.
impl Dispatch<wl_seat::WlSeat, ()> for WaylandState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities {
            capabilities: wayland_client::WEnum::Value(caps),
        } = event
        else {
            return;
        };

        // ── Keyboard ──────────────────────────────────────────────────────────
        let have_keyboard = caps.contains(wl_seat::Capability::Keyboard);
        if have_keyboard && state.keyboard.is_none() {
            let kb = seat.get_keyboard(qh, ());
            tracing::debug!("Got keyboard from seat");
            state.keyboard = Some(kb);
        } else if !have_keyboard && let Some(kb) = state.keyboard.take() {
            kb.release();
            tracing::debug!("Released keyboard");
        }

        // ── Pointer ───────────────────────────────────────────────────────────
        // C: `wl_seat_get_pointer` + `wl_pointer_add_listener` in
        // `wl_seat_capabilities` in `src/main.c`.
        let have_pointer = caps.contains(wl_seat::Capability::Pointer);
        if have_pointer && state.pointer.is_none() {
            let ptr = seat.get_pointer(qh, ());
            tracing::debug!("Got pointer from seat");
            state.pointer = Some(ptr);
        } else if !have_pointer && let Some(ptr) = state.pointer.take() {
            ptr.release();
            tracing::debug!("Released pointer");
        }
    }
}

/// `wl_keyboard` events — keymap, modifier state, and key presses.
///
/// C reference: `wl_keyboard_listener` in `src/main.c`.
impl Dispatch<wl_keyboard::WlKeyboard, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            // ── Keymap ────────────────────────────────────────────────────────
            // C: wl_keyboard_keymap — mmap the fd, parse XKB string, build
            // xkb_keymap + xkb_state.
            wl_keyboard::Event::Keymap {
                format,
                fd,
                size: _,
            } => {
                if !matches!(
                    format,
                    wayland_client::WEnum::Value(wl_keyboard::KeymapFormat::XkbV1)
                ) {
                    tracing::warn!("Unsupported keymap format; ignoring");
                    return;
                }
                // Read the keymap string from the fd (file ends at EOF).
                let mut file = std::fs::File::from(fd);
                let mut keymap_str = String::new();
                if file.read_to_string(&mut keymap_str).is_err() {
                    tracing::error!("Failed to read keymap from fd");
                    return;
                }
                state.keyboard_state.load_keymap(&keymap_str);
            }

            // ── Enter / Leave ─────────────────────────────────────────────────
            // C: deliberately blank.
            wl_keyboard::Event::Enter { .. } | wl_keyboard::Event::Leave { .. } => {}

            // ── Key ───────────────────────────────────────────────────────────
            // C: wl_keyboard_key — start/stop repeat, call input_handle_keypress.
            wl_keyboard::Event::Key {
                key,
                state: wayland_client::WEnum::Value(key_state),
                ..
            } => {
                // XKB keycode = evdev key + 8.
                let keycode = key + 8;

                match key_state {
                    wl_keyboard::KeyState::Released => {
                        state.keyboard_state.disarm_repeat(keycode);
                    }
                    wl_keyboard::KeyState::Pressed => {
                        if state.keyboard_state.key_repeats(keycode)
                            && state.keyboard_state.repeat.rate != 0
                        {
                            state.keyboard_state.arm_repeat(keycode);
                        }
                        handle_keypress(state, keycode);
                    }
                    _ => {}
                }
            }

            // ── Modifiers ─────────────────────────────────────────────────────
            // C: wl_keyboard_modifiers — xkb_state_update_mask.
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                state.keyboard_state.update_modifiers(
                    mods_depressed,
                    mods_latched,
                    mods_locked,
                    group,
                );
            }

            // ── Repeat info ───────────────────────────────────────────────────
            // C: wl_keyboard_repeat_info — store rate + delay.
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                state.keyboard_state.set_repeat_info(rate, delay);
            }

            _ => {}
        }
    }
}

/// Pointer events — only `enter` is meaningful (cursor hiding).
///
/// All other events (`leave`, `motion`, `button`, `axis`, `frame`, …) are
/// intentionally left blank, matching the C implementation.
///
/// C reference: `wl_pointer_listener` in `src/main.c`.
impl Dispatch<wl_pointer::WlPointer, ()> for WaylandState {
    fn event(
        state: &mut Self,
        pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // C: `wl_pointer_enter` — hide by setting cursor surface to NULL.
        // All other pointer events (leave, motion, button, axis, frame, …)
        // are deliberately left blank — same as C.
        if let wl_pointer::Event::Enter { serial, .. } = event
            && state.hide_cursor
        {
            pointer.set_cursor(serial, None, 0, 0);
        }
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
            wl_output::Event::Mode {
                flags: wayland_client::WEnum::Value(m),
                width,
                height,
                ..
            } if m.contains(wl_output::Mode::Current) => {
                // Only record the mode that is currently active.
                // C reference: `output_mode` in `src/main.c` (implicitly uses current mode).
                info.width = width;
                info.height = height;
            }
            wl_output::Event::Geometry {
                transform: wayland_client::WEnum::Value(t),
                ..
            } => {
                info.transform = t;
            }
            wl_output::Event::Done => {
                tracing::debug!(
                    "Output {}: {:?} {}x{} scale={} transform={:?}",
                    data,
                    info.name,
                    info.width,
                    info.height,
                    info.scale,
                    info.transform,
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

/// `wp_fractional_scale_v1::preferred_scale` — store the compositor's preferred
/// scale factor (numerator / 120) so [`surface::create_surface`] can compute
/// correct physical buffer dimensions.
///
/// C reference: `dummy_fractional_scale_preferred_scale` in `src/main.c`.
impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _: &wp_fractional_scale_v1::WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            tracing::debug!(
                "wp_fractional_scale_v1 preferred_scale = {scale} (×{:.4})",
                scale as f64 / 120.0
            );
            state.fractional_scale = scale;
        }
    }
}

/// `wp_viewport` has no client events; impl required by the dispatch machinery.
impl Dispatch<wp_viewport::WpViewport, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wp_viewport::WpViewport,
        _event: wp_viewport::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

// ── Step 7.1: clipboard dispatch ─────────────────────────────────────────────

/// `wl_data_device_manager` has no events; impl required by dispatch machinery.
///
/// C reference: `wl_data_device_manager_get_data_device` in `src/main.c`.
#[cfg(feature = "clipboard")]
impl Dispatch<wl_data_device_manager::WlDataDeviceManager, ()> for WaylandState {
    fn event(
        _: &mut Self,
        _: &wl_data_device_manager::WlDataDeviceManager,
        _event: wl_data_device_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

/// `wl_data_device` events — track the current clipboard selection offer.
///
/// `DataOffer` creates a new offer object (opcode 0) and replaces the
/// previous one.  `Selection` with `None` clears the current selection.
/// `Enter` (DnD) is rejected by accepting with `None` mime type.
/// `Leave`, `Motion`, `Drop` are blank (DnD not supported).
///
/// C reference: `wl_data_device_listener` in `src/main.c`.
#[cfg(feature = "clipboard")]
impl Dispatch<wl_data_device::WlDataDevice, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &wl_data_device::WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            // A new offer object was created; replace the old one.
            // C: `wl_data_device_data_offer` — reset clipboard, store new offer,
            // add listener (here: Dispatch handles events automatically).
            wl_data_device::Event::DataOffer { id } => {
                state.clipboard.reset();
                state.clipboard.offer = Some(id);
                tracing::debug!("wl_data_device: new data_offer");
            }

            // The compositor announces the current clipboard selection.
            // C: `wl_data_device_selection` — if NULL, reset clipboard.
            wl_data_device::Event::Selection { id } => {
                if id.is_none() {
                    state.clipboard.reset();
                    tracing::debug!("wl_data_device: selection cleared");
                }
                // If Some: the offer was already stored by DataOffer; no action.
            }

            // DnD drag enters our surface — reject it (we don't support DnD).
            // C: `wl_data_device_enter` — accept with NULL + set_actions(none).
            wl_data_device::Event::Enter {
                serial,
                id: Some(offer),
                ..
            } => {
                offer.accept(serial, None);
            }

            // Leave / Motion / Drop: deliberately blank.
            // C: `wl_data_device_leave`, `motion`, `drop` — blank.
            _ => {}
        }
    }

    // The `DataOffer` event (opcode 0) creates a new `wl_data_offer` object.
    // Provide user-data type `()` for its dispatch.
    wayland_client::event_created_child!(WaylandState, wl_data_device::WlDataDevice, [
        0 => (wl_data_offer::WlDataOffer, ())
    ]);
}

/// `wl_data_offer` events — negotiate MIME type for the current selection.
///
/// `Offer` events arrive immediately after the offer object is created and
/// announce which MIME types the clipboard source supports.  We prefer
/// `text/plain;charset=utf-8` and fall back to `text/plain`.
/// `SourceActions` and `Action` are DnD-only; ignored.
///
/// C reference: `wl_data_offer_listener` in `src/main.c`.
#[cfg(feature = "clipboard")]
impl Dispatch<wl_data_offer::WlDataOffer, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &wl_data_offer::WlDataOffer,
        event: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // C: `wl_data_offer_offer` — prefer UTF-8, accept plain as fallback.
        if let wl_data_offer::Event::Offer { mime_type } = event {
            let current = state.clipboard.mime_type.as_deref();
            if mime_type == clipboard::MIME_TEXT_UTF8 {
                // UTF-8 is the best option; always upgrade to it.
                state.clipboard.mime_type = Some(mime_type);
            } else if mime_type == clipboard::MIME_TEXT_PLAIN
                && current != Some(clipboard::MIME_TEXT_UTF8)
            {
                // Accept plain text only if we haven't seen UTF-8 yet.
                state.clipboard.mime_type = Some(mime_type);
            }
        }
        // SourceActions and Action events are DnD-only; intentionally blank.
        // C: `wl_data_offer_source_actions`, `wl_data_offer_action` — blank.
    }
}

// ── handle_keypress ───────────────────────────────────────────────────────────

/// Process one XKB keycode — classify the key into a [`crate::input::KeyAction`]
/// and apply it to the entry widget and event flags.
///
/// Called from the `wl_keyboard::key` event handler and from the key-repeat
/// ticker in the main event loop.
///
/// `Close` and `Submit` actions return early without setting `redraw`.
/// `Unknown` (unbound key) returns early without triggering `auto_accept_single`
/// or `redraw`.  All other actions set `redraw = true` and, when
/// `auto_accept_single` is enabled and exactly one result remains, set
/// `submit = true` — matching the C behaviour where the check follows the
/// entire if-else chain.
///
/// C reference: `input_handle_keypress` in `src/input.c`.
pub fn handle_keypress(state: &mut WaylandState, keycode: u32) {
    if !state.keyboard_state.is_ready() {
        return;
    }

    let ctrl = state.keyboard_state.mod_ctrl();
    let alt = state.keyboard_state.mod_alt();
    let shift = state.keyboard_state.mod_shift();
    let ch = state.keyboard_state.key_get_utf32(keycode);
    let key = state.keyboard_state.keycode_to_linux_key(keycode);

    let action = crate::input::classify_keypress(ctrl, alt, shift, key, ch);

    match action {
        // ── Terminal actions (return immediately, no redraw) ───────────────────
        crate::input::KeyAction::Close => {
            state.closed = true;
            return;
        }
        crate::input::KeyAction::Submit => {
            state.submit = true;
            return;
        }
        // ── Unbound key ───────────────────────────────────────────────────────
        crate::input::KeyAction::Unknown => {
            return;
        }

        // ── Text editing ──────────────────────────────────────────────────────
        crate::input::KeyAction::InsertChar(c) =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                crate::input::add_char(
                    &mut entry.input,
                    &mut entry.cursor_position,
                    c,
                    crate::entry::MAX_INPUT_LENGTH,
                );
                entry.reset_selection();
            }
        }
        crate::input::KeyAction::DeleteChar =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                crate::input::delete_char(&mut entry.input, &mut entry.cursor_position);
                entry.reset_selection();
            }
        }
        crate::input::KeyAction::DeleteWord =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                crate::input::delete_word(&mut entry.input, &mut entry.cursor_position);
                entry.reset_selection();
            }
        }
        crate::input::KeyAction::ClearInput =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                crate::input::clear_input(&mut entry.input, &mut entry.cursor_position);
                entry.reset_selection();
            }
        }

        // ── Clipboard ─────────────────────────────────────────────────────────
        // C: `paste` in `src/input.c` — open pipe, call wl_data_offer_receive.
        crate::input::KeyAction::Paste => {
            #[cfg(feature = "clipboard")]
            {
                state.clipboard.begin_paste();
            }
            #[cfg(not(feature = "clipboard"))]
            tracing::debug!("Paste: clipboard feature disabled");
        }

        // ── Navigation ────────────────────────────────────────────────────────
        // C: `previous_cursor_or_result`.
        crate::input::KeyAction::PrevCursorOrResult =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                if entry.config.cursor_theme.show
                    && entry.selection == 0
                    && entry.cursor_position > 0
                {
                    entry.cursor_position -= 1;
                } else {
                    entry.select_prev();
                }
            }
        }
        // C: `next_cursor_or_result`.
        crate::input::KeyAction::NextCursorOrResult =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                let n_chars = entry.input.chars().count();
                if entry.config.cursor_theme.show && entry.cursor_position < n_chars {
                    entry.cursor_position += 1;
                } else {
                    entry.select_next();
                }
            }
        }
        crate::input::KeyAction::PrevResult =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                entry.select_prev();
            }
        }
        crate::input::KeyAction::NextResult =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                entry.select_next();
            }
        }
        crate::input::KeyAction::ResetSelection =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                entry.reset_selection();
            }
        }
        crate::input::KeyAction::PrevPage =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                entry.select_prev_page();
            }
        }
        crate::input::KeyAction::NextPage =>
        {
            #[cfg(feature = "renderer")]
            if let Some(entry) = state.entry.as_mut() {
                entry.select_next_page();
            }
        }
    }

    // auto_accept_single: mirrors C — fires after any bound key that is not
    // Close, Submit, or Unknown.
    #[cfg(feature = "renderer")]
    if state.auto_accept_single
        && let Some(entry) = state.entry.as_ref()
        && entry.results.len() == 1
    {
        state.submit = true;
    }

    state.redraw = true;
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

    // Create a data device so the compositor delivers clipboard selection
    // events.  This mirrors the C setup in `src/main.c` after the roundtrips.
    //
    // C reference: `wl_data_device_manager_get_data_device` + listener in
    // `src/main.c`.
    #[cfg(feature = "clipboard")]
    if let (Some(manager), Some(seat)) = (state.data_device_manager.as_ref(), state.seat.as_ref()) {
        let device = manager.get_data_device(seat, &qh, ());
        state.data_device = Some(device);
        tracing::debug!("Created wl_data_device for clipboard");
    } else {
        tracing::warn!(
            "wl_data_device_manager not advertised by compositor; \
             clipboard (paste) will be unavailable"
        );
    }

    Ok((state, event_queue))
}

/// Read available clipboard data from the active paste pipe and insert the
/// decoded UTF-8 characters into the entry widget at the current cursor.
///
/// Should be called from the main event loop whenever the clipboard fd may
/// be readable (after a blocking poll that includes the fd, or after any
/// Wayland dispatch when a paste is in progress).  The fd is opened with
/// `O_NONBLOCK`, so this function returns immediately when no data is
/// available (`EAGAIN`).
///
/// On EOF the pipe is closed and a redraw is triggered.  On a read error
/// the paste is cancelled.
///
/// C reference: `read_clipboard` in `src/main.c`.
#[cfg(all(feature = "clipboard", feature = "renderer"))]
pub fn read_clipboard(state: &mut WaylandState) {
    use std::os::fd::AsRawFd as _;

    let Some(rawfd) = state.clipboard.read_fd.as_ref().map(|f| f.as_raw_fd()) else {
        return;
    };

    let mut buf = [0u8; 4096];
    match nix::unistd::read(rawfd, &mut buf) {
        Ok(0) => {
            // EOF — compositor finished writing; paste is complete.
            state.clipboard.finish_paste();
            state.redraw = true;
            tracing::debug!("Clipboard paste complete (EOF)");
        }
        Ok(n) => {
            // Insert received bytes as UTF-8 characters into the entry input.
            // C: `entry->input_utf32[cursor_position] = unichar; cursor_position++`
            match std::str::from_utf8(&buf[..n]) {
                Ok(text) => {
                    if let Some(entry) = state.entry.as_mut() {
                        for c in text.chars() {
                            crate::input::add_char(
                                &mut entry.input,
                                &mut entry.cursor_position,
                                c,
                                crate::entry::MAX_INPUT_LENGTH,
                            );
                        }
                        entry.reset_selection();
                    }
                    state.redraw = true;
                }
                Err(_) => {
                    tracing::warn!("Clipboard data is not valid UTF-8; discarding chunk");
                    state.clipboard.finish_paste();
                }
            }
        }
        Err(nix::errno::Errno::EAGAIN) => {
            // No data available right now — will retry on next poll wakeup.
            // (On Linux EAGAIN == EWOULDBLOCK.)
        }
        Err(e) => {
            tracing::error!("Failed to read clipboard: {e}");
            state.clipboard.finish_paste();
        }
    }
}
