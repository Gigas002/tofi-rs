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
            output: None, // Step 6: target_output_name selection
        };

        libtofi_rs::wayland::surface::create_surface(
            &mut state,
            &mut event_queue,
            &surface_cfg,
            config.background_color,
        )
        .expect("Failed to create layer surface");

        // Step 5.2 — initialise the Entry layout engine (prompt, input, results).
        //
        // The Entry is backed by the double-buffered SHM pool.  It draws the
        // static border/background on construction and the text layer on every
        // `entry.update()` call (triggered by input or selection changes).
        //
        // Full wiring (event-driven updates) is completed in Phase 6 once
        // keyboard input is wired up.  Here we initialise the entry and commit
        // the initial frame so the themed window appears on screen.
        #[cfg(feature = "renderer")]
        {
            use libtofi_rs::entry::{Entry, EntryConfig};

            // Resolve padding UnitValues to logical pixels.
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
                // Per-element themes — convert from tofi::config::TextTheme to
                // libtofi_rs::entry::TextTheme.
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
            // SAFETY: The ShmPool lives for the duration of this block; the
            // Entry is dropped before `draw()` commits the frame.
            let (data_ptr, phys_w, phys_h) = {
                let surf = state.surface.as_mut().expect("surface must exist");
                let shm = surf.shm.as_mut().expect("SHM pool must exist");
                // data_for_entry gives a pointer to the start of both frames.
                let ptr = shm.data_both_frames_ptr();
                (ptr, surf.phys_width, surf.phys_height)
            };

            let mut entry =
                unsafe { Entry::new(data_ptr, phys_w, phys_h, scale_num, entry_config) }
                    .expect("Failed to create entry");

            // Populate results (from stdin / run / drun — placeholder for Phase 6.4).
            entry.results = vec![];

            entry.flush();
            drop(entry);

            // Commit the initial rendered frame.
            libtofi_rs::wayland::surface::draw(&mut state).expect("Failed to commit entry frame");
            event_queue.flush().expect("Wayland flush failed");
            tracing::debug!("Step 5.2: entry initial frame committed");
        }

        // Step 4.3 event loop: dispatch until the compositor closes the surface.
        // ESC / keyboard input wired in Step 6.1.
        tracing::debug!("Entering event loop");
        while !state.closed {
            event_queue
                .blocking_dispatch(&mut state)
                .expect("Wayland event loop error");
        }
        tracing::debug!("Surface closed — exiting");

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

/// Convert [`config::Anchor`] to [`zwlr_layer_surface_v1`] anchor bit-flags.
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
