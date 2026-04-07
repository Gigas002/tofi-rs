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

        // Step 5.1 — draw "hello" into the back buffer using Cairo + Pango.
        #[cfg(feature = "renderer")]
        {
            use libtofi_rs::renderer::Renderer;

            // After create_surface the front buffer (index 0) is on screen and
            // `surface.index` points to the back buffer (1).  We draw "hello"
            // into the back buffer, flush it, then commit it to the compositor.
            let (data_ptr, phys_w, phys_h) = {
                let surf = state.surface.as_mut().expect("surface must exist");
                let shm = surf.shm.as_mut().expect("SHM pool must exist");
                let idx = surf.index;
                let ptr = shm.data_mut(idx).as_mut_ptr();
                (ptr, surf.phys_width, surf.phys_height)
            };

            // Effective scale: prefer fractional, fall back to integer × 120.
            let scale_num = if state.fractional_scale != 0 {
                state.fractional_scale
            } else {
                let int_scale = state.outputs.first().map(|o| o.scale).unwrap_or(1);
                (int_scale as u32) * 120
            };

            // SAFETY: data_ptr points into the ShmPool mapping which is alive
            // for the duration of this block; we drop the Renderer before
            // calling draw() so Cairo has no reference to the buffer at commit.
            let renderer =
                unsafe { Renderer::create_for_data(data_ptr, phys_w, phys_h, scale_num) }
                    .expect("Failed to create renderer");
            renderer.draw_hello();
            renderer.flush();
            drop(renderer);

            // Commit the drawn frame.
            libtofi_rs::wayland::surface::draw(&mut state)
                .expect("Failed to commit rendered frame");
            event_queue.flush().expect("Wayland flush failed");
            tracing::debug!("Step 5.1: 'hello' frame committed");
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
