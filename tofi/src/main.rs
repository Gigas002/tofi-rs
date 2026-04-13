//! `tofi` binary — wires [`cli::Cli`] to the rest of the program.

mod cli;
#[allow(dead_code)]
mod config;
#[cfg(feature = "history")]
#[allow(dead_code)]
mod history;
#[cfg(feature = "run-commands")]
#[allow(dead_code)]
mod run_commands;

use clap::Parser as _;

fn main() {
    // Initialise tracing; verbosity controlled by RUST_LOG (e.g. RUST_LOG=debug).
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();
    #[allow(unused_variables)]
    let (config, _errors) = cli.into_config().expect("Failed to load config");

    #[cfg(feature = "wayland")]
    {
        use libtofi_rs::wayland::{Anchor, surface::SurfaceConfig};

        let (mut state, mut event_queue) =
            libtofi_rs::wayland::connect().expect("Failed to initialize Wayland");

        // Wire config options that the keyboard / pointer handlers need.
        state.physical_keybindings = config.physical_keybindings;
        state.auto_accept_single = config.auto_accept_single;
        state.hide_cursor = config.hide_cursor;
        // Propagate physical_keybindings into the already-constructed KeyboardState.
        state.keyboard_state.physical_keybindings = config.physical_keybindings;

        // Resolve UnitValues to pixels using the first output's dimensions.
        // Swap width/height for rotated outputs (90°/270°).
        // C reference: transform handling in src/main.c ~1427–1438.
        let (out_w, out_h) = state
            .outputs
            .first()
            .map(|o| {
                use libtofi_rs::wayland::OutputTransform;
                match o.transform {
                    OutputTransform::_90
                    | OutputTransform::_270
                    | OutputTransform::Flipped90
                    | OutputTransform::Flipped270 => (o.height as u32, o.width as u32),
                    _ => (o.width as u32, o.height as u32),
                }
            })
            .unwrap_or((1920, 1080));

        let resolve_px = |uv: &config::UnitValue, dim: u32| -> u32 {
            if uv.is_percent {
                uv.value * dim / 100
            } else {
                uv.value
            }
        };

        let width = resolve_px(&config.width, out_w);
        let height = resolve_px(&config.height, out_h);

        let surface_cfg = SurfaceConfig {
            width,
            height,
            anchor: config_anchor_to_layer(config.anchor),
            exclusive_zone: config.exclusive_zone,
            margin_top: resolve_px(&config.margin_top, out_h) as i32,
            margin_right: resolve_px(&config.margin_right, out_w) as i32,
            margin_bottom: resolve_px(&config.margin_bottom, out_h) as i32,
            margin_left: resolve_px(&config.margin_left, out_w) as i32,
            output: None, // Step 6.4: target_output_name selection
        };

        libtofi_rs::wayland::surface::create_surface(
            &mut state,
            &mut event_queue,
            &surface_cfg,
            config.background_color,
        )
        .expect("Failed to create layer surface");

        // Step 5.2 / 6.1 — initialise the Entry layout engine and store it in
        // `state.entry` so keyboard event handlers can update it.
        //
        // Declared here (after create_surface) so the SHM pool is already
        // allocated.  The entry is placed in `state.entry` rather than dropped,
        // keeping it alive for the duration of the event loop.
        #[cfg(feature = "renderer")]
        {
            use libtofi_rs::entry::{Entry, EntryConfig};

            let entry_config = EntryConfig {
                font_name: config.font.clone(),
                font_size: config.font_size,
                font_features: config.font_features.clone(),
                font_variations: config.font_variations.clone(),
                foreground_color: config.foreground_color,
                background_color: config.background_color,
                border_color: config.border_color,
                outline_color: config.outline_color,
                selection_highlight_color: config.selection_highlight_color,
                corner_radius: config.corner_radius,
                border_width: config.border_width,
                outline_width: config.outline_width,
                padding_top: resolve_px(&config.padding_top, height),
                padding_bottom: resolve_px(&config.padding_bottom, height),
                padding_left: resolve_px(&config.padding_left, width),
                padding_right: resolve_px(&config.padding_right, width),
                clip_to_padding: config.clip_to_padding,
                prompt_text: config.prompt_text.clone(),
                prompt_padding: config.prompt_padding,
                placeholder_text: config.placeholder_text.clone(),
                num_results: config.num_results,
                result_spacing: config.result_spacing,
                horizontal: config.horizontal,
                input_width: config.min_input_width,
                hide_input: config.hide_input,
                hidden_character: config
                    .hidden_character
                    .0
                    .map(|c| c.to_string())
                    .unwrap_or_default(),
                prompt_theme: config_theme_to_entry(&config.prompt_theme),
                input_theme: config_theme_to_entry(&config.input_theme),
                placeholder_theme: config_theme_to_entry(&config.placeholder_theme),
                default_result_theme: config_theme_to_entry(&config.default_result_theme),
                alternate_result_theme: config_theme_to_entry(&config.alternate_result_theme),
                selection_theme: config_theme_to_entry(&config.selection_theme),
                cursor_theme: config_cursor_to_entry(&config.cursor_theme),
            };

            // Resolve effective scale.
            let scale_num = if state.fractional_scale != 0 {
                state.fractional_scale
            } else {
                let int_scale = state.outputs.first().map(|o| o.scale).unwrap_or(1);
                (int_scale as u32) * 120
            };

            // Obtain a pointer to the full double-buffered SHM region.
            // SAFETY: The ShmPool lives inside state.surface for the lifetime of
            // the event loop.  Entry is in state.entry (declared before surface in
            // WaylandState), so entry is dropped before the SHM pool — safe.
            let (data_ptr, phys_w, phys_h) = {
                let surf = state.surface.as_mut().expect("surface must exist");
                let shm = surf.shm.as_mut().expect("SHM pool must exist");
                let ptr = shm.data_both_frames_ptr();
                (ptr, surf.phys_width, surf.phys_height)
            };

            let mut entry =
                unsafe { Entry::new(data_ptr, phys_w, phys_h, scale_num, entry_config) }
                    .expect("Failed to create entry");

            // Populate results (stdin / run / drun — Phase 6.4).
            entry.results = vec![];

            entry.flush();

            // Commit the initial rendered frame.
            libtofi_rs::wayland::surface::draw(&mut state).expect("Failed to commit entry frame");
            event_queue.flush().expect("Wayland flush failed");
            tracing::debug!("Step 6.1: entry initial frame committed");

            // Keep the entry alive in state so keyboard handlers can update it.
            state.entry = Some(entry);
        }

        // ── Event loop ────────────────────────────────────────────────────────
        // Non-blocking dispatch with key-repeat timeout so held keys fire
        // repeated input events between Wayland events.
        //
        // C reference: poll loop (wl_display fd + timerfd) in `src/main.c`.
        tracing::debug!("Entering event loop");

        'event_loop: loop {
            // Flush outgoing Wayland messages.
            event_queue.flush().expect("Wayland flush failed");

            // Compute the poll timeout based on pending key repeat.
            let timeout_ms: i32 = match state.keyboard_state.repeat.timeout() {
                None => -1, // block indefinitely (no repeat pending)
                Some(d) => d.as_millis().min(i32::MAX as u128) as i32,
            };

            // Poll the Wayland fd with timeout so key repeat can fire.
            let guard = event_queue.prepare_read();
            if let Some(ref g) = guard {
                use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
                use std::os::fd::AsFd as _;
                let fd = g.connection_fd();
                let mut pfds = [PollFd::new(fd.as_fd(), PollFlags::POLLIN)];
                let pt = if timeout_ms < 0 {
                    PollTimeout::NONE
                } else {
                    PollTimeout::try_from(timeout_ms).unwrap_or(PollTimeout::NONE)
                };
                let _ = poll(&mut pfds, pt);
            }
            if let Some(g) = guard {
                let _ = g.read();
            }
            event_queue
                .dispatch_pending(&mut state)
                .expect("Wayland dispatch error");

            // ── Key repeat ────────────────────────────────────────────────────
            // C: poll timerfd / gettime_ms check in the main loop.
            if state.keyboard_state.repeat.active && state.keyboard_state.repeat.rate > 0 {
                use std::time::Instant;
                if Instant::now() >= state.keyboard_state.repeat.next {
                    let keycode = state.keyboard_state.repeat.keycode;
                    state.keyboard_state.advance_repeat();
                    libtofi_rs::wayland::handle_keypress(&mut state, keycode);
                }
            }

            // ── Exit conditions ───────────────────────────────────────────────
            if state.closed {
                tracing::debug!("Surface closed — exiting");
                break 'event_loop;
            }

            if state.submit {
                // Step 6.5 (do_submit): print the selected result to stdout.
                // Full history-append and drun-launch logic lands in Step 6.5.
                #[cfg(feature = "renderer")]
                if let Some(entry) = state.entry.as_ref() {
                    let abs_idx = entry.first_result + entry.selection;
                    if let Some(result) = entry.results.get(abs_idx) {
                        println!("{result}");
                    }
                }
                tracing::debug!("Submit — exiting");
                break 'event_loop;
            }

            // ── Redraw ────────────────────────────────────────────────────────
            // C: `if (tofi->window.surface.redraw) { entry_update; surface_draw; }`
            if state.redraw {
                state.redraw = false;

                #[cfg(feature = "renderer")]
                if let Some(entry) = state.entry.as_mut() {
                    entry.update();
                    entry.flush();
                    libtofi_rs::wayland::surface::draw(&mut state).expect("Failed to redraw entry");
                    event_queue.flush().expect("Wayland flush after redraw");
                }
            }
        }

        tracing::debug!("Event loop exited");

        let _ = Anchor::Top; // ensure re-export is used / accessible
    }

    libtofi_rs::noop();
}

/// Convert a [`config::TextTheme`] to [`libtofi_rs::entry::TextTheme`].
#[cfg(all(feature = "wayland", feature = "renderer"))]
fn config_theme_to_entry(t: &config::TextTheme) -> libtofi_rs::entry::TextTheme {
    libtofi_rs::entry::TextTheme {
        foreground_color: t.foreground_color,
        background_color: t.background_color,
        padding: t.padding.map(|p| libtofi_rs::entry::Directional {
            top: p.top,
            right: p.right,
            bottom: p.bottom,
            left: p.left,
        }),
        background_corner_radius: t.background_corner_radius,
    }
}

/// Convert a [`config::CursorTheme`] to [`libtofi_rs::entry::CursorTheme`].
#[cfg(all(feature = "wayland", feature = "renderer"))]
fn config_cursor_to_entry(c: &config::CursorTheme) -> libtofi_rs::entry::CursorTheme {
    use libtofi_rs::entry::CursorStyle;
    libtofi_rs::entry::CursorTheme {
        color: c.color,
        text_color: c.text_color,
        style: match c.style {
            config::CursorStyle::Bar => CursorStyle::Bar,
            config::CursorStyle::Block => CursorStyle::Block,
            config::CursorStyle::Underscore => CursorStyle::Underscore,
        },
        corner_radius: c.corner_radius,
        thickness: c.thickness,
        show: c.show,
    }
}

/// Convert [`config::Anchor`] to `zwlr_layer_surface_v1` anchor bit-flags.
///
/// Mirrors the `ANCHOR_*` macros in `src/config.c`.
#[cfg(feature = "wayland")]
fn config_anchor_to_layer(anchor: config::Anchor) -> libtofi_rs::wayland::Anchor {
    use config::Anchor as A;
    use libtofi_rs::wayland::Anchor;
    match anchor {
        A::TopLeft => Anchor::Top | Anchor::Left,
        A::Top => Anchor::Top | Anchor::Left | Anchor::Right,
        A::TopRight => Anchor::Top | Anchor::Right,
        A::Right => Anchor::Right | Anchor::Top | Anchor::Bottom,
        A::BottomRight => Anchor::Bottom | Anchor::Right,
        A::Bottom => Anchor::Bottom | Anchor::Left | Anchor::Right,
        A::BottomLeft => Anchor::Bottom | Anchor::Left,
        A::Left => Anchor::Left | Anchor::Top | Anchor::Bottom,
        A::Center => Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
    }
}
