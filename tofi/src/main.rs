//! `tofi` binary — wires [`cli::Cli`] to the rest of the program.
// Deny unsafe across the whole CLI crate.  The one call-site that wraps a raw
// SHM pointer (Entry::new) is explicitly opted out via #[allow(unsafe_code)]
// on `main` — all other unsafe in the system lives in `libtofi-rs`.
#![deny(unsafe_code)]

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

/// Launch mode — determined from `argv[0]` before Wayland is initialised.
#[cfg(feature = "wayland")]
///
/// C reference: `strstr(argv[0], "-run")` / `-drun` in `src/main.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchMode {
    /// Default: read entries from stdin (one per line).
    Stdin,
    /// Invoked as `tofi-run`: list executables from `$PATH`.
    #[cfg(feature = "run-commands")]
    Run,
    /// Invoked as `tofi-drun`: list desktop applications.
    #[cfg(feature = "drun")]
    Drun,
}

/// Detect launch mode from `argv[0]`.
#[cfg(feature = "wayland")]
///
/// Checks for `-drun` before `-run` so that a hypothetical binary named
/// `tofi-drun` is not misdetected; functionally equivalent to the C order
/// because `"tofi-drun"` does not contain the substring `"-run"`.
///
/// C reference: the `strstr` checks in `src/main.c`.
fn detect_mode() -> LaunchMode {
    let argv0 = std::env::args().next().unwrap_or_default();
    #[cfg(feature = "drun")]
    if argv0.contains("-drun") {
        return LaunchMode::Drun;
    }
    #[cfg(feature = "run-commands")]
    if argv0.contains("-run") {
        return LaunchMode::Run;
    }
    let _ = argv0;
    LaunchMode::Stdin
}

/// Read stdin line-by-line into an owned `Vec<String>`.
#[cfg(feature = "wayland")]
///
/// Empty lines are skipped.  When `normalize` is `true` each line is
/// NFC-normalised (mirrors C's `utf8_normalize` on the raw buffer).
///
/// C reference: `read_stdin` + `string_ref_vec_from_buffer` in `src/main.c`.
fn read_stdin(normalize: bool) -> Vec<String> {
    use std::io::BufRead as _;
    std::io::stdin()
        .lock()
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.is_empty())
        .map(|l| {
            if normalize {
                libtofi_rs::unicode::utf8_normalize(&l)
            } else {
                l
            }
        })
        .collect()
}

// Entry::new wraps a raw pointer into the SHM double-buffer.  The SAFETY
// comment at the call-site documents the lifetime invariant; no other unsafe
// is present in this crate.
#[allow(unsafe_code)]
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

        let mode = detect_mode();

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

        // Snapshot of the full unfiltered result list, used by `do_submit` for
        // `print_index` and history lookup.  Declared outside the renderer block
        // but only compiled in when the renderer feature is active — the event
        // loop's submit handler is gated on the same feature.
        //
        // C reference: `entry->commands` in `do_submit` / `src/main.c`.
        #[cfg(feature = "renderer")]
        let all_commands: Vec<String>;

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

            // ── Populate results (Phase 6.4) ──────────────────────────────
            // C reference: the argv[0] mode detection block in `src/main.c`.
            entry.results = match mode {
                // ── stdin ─────────────────────────────────────────────────────
                LaunchMode::Stdin => {
                    tracing::debug!("Mode: stdin");
                    let mut items = read_stdin(!config.ascii_input);
                    #[cfg(feature = "history")]
                    if config.use_history {
                        // C: history only applies in stdin mode when an explicit
                        // history file is configured (no default path for stdin).
                        if let Some(ref hf) = config.history_file
                            && let Ok(hist) = history::load(std::path::Path::new(hf))
                        {
                            sort_by_history(&mut items, &hist);
                        }
                    }
                    items
                }

                // ── run ───────────────────────────────────────────────────────
                #[cfg(feature = "run-commands")]
                LaunchMode::Run => {
                    tracing::debug!("Mode: run");
                    let path_var = std::env::var("PATH").unwrap_or_default();
                    let cache_path = run_commands::default_cache_path()
                        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/tofi-compgen"));
                    let mut commands =
                        run_commands::commands_cached(&path_var, &cache_path).unwrap_or_default();
                    #[cfg(feature = "history")]
                    if config.use_history {
                        let hist_path = config
                            .history_file
                            .as_deref()
                            .map(std::path::PathBuf::from)
                            .or_else(|| history::default_history_path(false));
                        if let Some(hp) = hist_path
                            && let Ok(hist) = history::load(&hp)
                        {
                            sort_by_history(&mut commands, &hist);
                        }
                    }
                    commands
                }

                // ── drun ──────────────────────────────────────────────────────
                #[cfg(feature = "drun")]
                LaunchMode::Drun => {
                    tracing::debug!("Mode: drun");
                    let dirs = libtofi_rs::drun::application_dirs();
                    let cache_path = libtofi_rs::drun::default_cache_path()
                        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/tofi-drun"));
                    let mut entries =
                        libtofi_rs::drun::entries_cached(&dirs, &cache_path).unwrap_or_default();
                    #[cfg(feature = "history")]
                    if config.use_history {
                        let hist_path = config
                            .history_file
                            .as_deref()
                            .map(std::path::PathBuf::from)
                            .or_else(|| history::default_history_path(true));
                        if let Some(hp) = hist_path
                            && let Ok(hist) = history::load(&hp)
                        {
                            sort_drun_by_history(&mut entries, &hist);
                        }
                    }
                    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
                    state.drun_entries = entries;
                    names
                }
            };

            // Snapshot the full unfiltered list for `print_index` (Step 6.5).
            // C reference: `entry->commands` in `do_submit`.
            all_commands = entry.results.clone();

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

            // Poll the Wayland fd (and clipboard fd when a paste is active)
            // with timeout so key repeat can fire between events.
            //
            // C reference: `poll` loop with `pollfds[0]` (Wayland) and
            // `pollfds[1]` (clipboard pipe) in `src/main.c`.
            let guard = event_queue.prepare_read();
            if let Some(ref g) = guard {
                use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
                use std::os::fd::AsFd as _;
                let wayland_fd = g.connection_fd();
                let pt = if timeout_ms < 0 {
                    PollTimeout::NONE
                } else {
                    PollTimeout::try_from(timeout_ms).unwrap_or(PollTimeout::NONE)
                };

                // When a clipboard paste is in progress, add its fd so we
                // wake up as soon as data arrives instead of spinning.
                #[cfg(feature = "clipboard")]
                if let Some(cfd) = state.clipboard.read_fd.as_ref() {
                    let mut pfds = [
                        PollFd::new(wayland_fd.as_fd(), PollFlags::POLLIN),
                        PollFd::new(cfd.as_fd(), PollFlags::POLLIN),
                    ];
                    let _ = poll(&mut pfds, pt);
                } else {
                    let mut pfds = [PollFd::new(wayland_fd.as_fd(), PollFlags::POLLIN)];
                    let _ = poll(&mut pfds, pt);
                }

                #[cfg(not(feature = "clipboard"))]
                {
                    let mut pfds = [PollFd::new(wayland_fd.as_fd(), PollFlags::POLLIN)];
                    let _ = poll(&mut pfds, pt);
                }
            }
            if let Some(g) = guard {
                let _ = g.read();
            }
            event_queue
                .dispatch_pending(&mut state)
                .expect("Wayland dispatch error");

            // Drain any available clipboard data from the paste pipe.
            // C reference: `read_clipboard` called when `pollfds[1]` is ready
            // in `src/main.c`.
            #[cfg(all(feature = "clipboard", feature = "renderer"))]
            if state.clipboard.read_fd.is_some() {
                libtofi_rs::wayland::read_clipboard(&mut state);
            }

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
                state.submit = false;
                // C reference: `do_submit` in `src/main.c`.
                #[cfg(feature = "renderer")]
                {
                    let submitted = do_submit(&state, &config, mode, &all_commands);
                    if submitted {
                        tracing::debug!("Submit — exiting");
                        break 'event_loop;
                    }
                    // No result to accept (require_match / empty) — continue.
                }
                #[cfg(not(feature = "renderer"))]
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

// ── do_submit ─────────────────────────────────────────────────────────────────

/// Handle selection acceptance — print to stdout, update history, optionally
/// launch the app.  Returns `true` when the selection was accepted and the
/// launcher should exit; `false` when no match is available and the loop
/// should continue.
///
/// C reference: `do_submit` in `src/main.c`.
#[cfg(all(feature = "wayland", feature = "renderer"))]
fn do_submit(
    state: &libtofi_rs::wayland::WaylandState,
    config: &config::TofiConfig,
    mode: LaunchMode,
    all_commands: &[String],
) -> bool {
    let Some(entry) = state.entry.as_ref() else {
        return false;
    };

    // ── No results ────────────────────────────────────────────────────────────
    if entry.results.is_empty() {
        // In drun mode (or when require_match is set) reject silently.
        // C: `if (tofi->require_match || entry->mode == TOFI_MODE_DRUN) return false`.
        #[cfg(feature = "drun")]
        if matches!(mode, LaunchMode::Drun) {
            return false;
        }
        if config.require_match {
            return false;
        }
        // Stdin/run mode without require_match: echo back raw input.
        println!("{}", entry.input);
        return true;
    }

    let abs_idx = (entry.first_result + entry.selection).min(entry.results.len() - 1);
    let result = &entry.results[abs_idx];

    // ── Dispatch by mode ──────────────────────────────────────────────────────
    #[cfg(feature = "drun")]
    if matches!(mode, LaunchMode::Drun) {
        // Find the desktop entry by display name.
        // C: linear scan because the list may be history-sorted.
        let app = state.drun_entries.iter().find(|e| e.name == *result);
        let Some(app) = app else {
            tracing::error!("Couldn't find application '{result}' in drun_entries");
            return false;
        };
        if config.drun_launch {
            drun_launch(app, config.default_terminal.as_deref());
        } else {
            drun_print(app, config.default_terminal.as_deref());
        }
    } else {
        // stdin / run modes.
        // C: `if (entry->mode == TOFI_MODE_PLAIN && tofi->print_index)`.
        if matches!(mode, LaunchMode::Stdin) && config.print_index {
            if let Some(idx) = all_commands.iter().position(|s| s == result) {
                println!("{}", idx + 1);
            }
        } else {
            println!("{result}");
        }
    }
    #[cfg(not(feature = "drun"))]
    {
        // Without drun support only stdin/run exist.
        if matches!(mode, LaunchMode::Stdin) && config.print_index {
            if let Some(idx) = all_commands.iter().position(|s| s == result) {
                println!("{}", idx + 1);
            }
        } else {
            println!("{result}");
        }
    }

    // ── History ───────────────────────────────────────────────────────────────
    // C: `if (tofi->use_history) { history_add; history_save_default_file; }`.
    #[cfg(feature = "history")]
    if config.use_history {
        let is_drun = cfg!(feature = "drun") && matches!(mode, LaunchMode::Drun);
        let hist_path = config
            .history_file
            .as_deref()
            .map(std::path::PathBuf::from)
            .or_else(|| history::default_history_path(is_drun));
        if let Some(hp) = hist_path {
            let mut hist = history::load(&hp).unwrap_or_default();
            hist.add(result);
            if let Err(e) = history::save(&hist, &hp) {
                tracing::warn!("Failed to save history: {e}");
            }
        }
    }

    true
}

/// Print the expanded exec command for a desktop entry to stdout.
///
/// Prepends the terminal command when `entry.terminal` is `true`.
///
/// C reference: `drun_print` in `src/drun.c`.
#[cfg(all(feature = "wayland", feature = "drun"))]
fn drun_print(entry: &libtofi_rs::drun::DesktopEntry, terminal: Option<&str>) {
    let cmd = libtofi_rs::drun::exec_command(entry);
    if entry.terminal {
        match terminal {
            Some(t) if !t.is_empty() => print!("{t} "),
            _ => tracing::warn!(
                "Terminal application '{}' launched but no terminal is configured \
                 (set --terminal or $TERMINAL).",
                entry.name
            ),
        }
    }
    println!("{cmd}");
}

/// Launch a desktop application directly via `std::process::Command`.
///
/// The exec string is split on whitespace for argument handling.  A proper
/// shell-quoting parser (e.g. `shlex`) would be more robust, but matches the
/// typical exec strings found in `.desktop` files.
///
/// C reference: `drun_launch` in `src/drun.c` (uses `g_desktop_app_info_new_from_filename`).
#[cfg(all(feature = "wayland", feature = "drun"))]
fn drun_launch(entry: &libtofi_rs::drun::DesktopEntry, terminal: Option<&str>) {
    let cmd = libtofi_rs::drun::exec_command(entry);
    let full_cmd = if entry.terminal {
        let term = terminal.filter(|t| !t.is_empty()).unwrap_or("xterm");
        format!("{term} {cmd}")
    } else {
        cmd
    };

    let mut parts = full_cmd.split_whitespace();
    let Some(program) = parts.next() else {
        tracing::error!("Empty exec command for '{}'", entry.name);
        return;
    };
    let args: Vec<&str> = parts.collect();

    match std::process::Command::new(program).args(&args).spawn() {
        Ok(_) => tracing::debug!("Launched '{}': {full_cmd}", entry.name),
        Err(e) => tracing::error!("Failed to launch '{}': {e}", entry.name),
    }
}

// ── History-sort helpers ──────────────────────────────────────────────────────

/// Sort a list of command strings by descending history run-count.
///
/// Commands not present in `hist` stay at their original relative order
/// (stable sort with a score of 0).
///
/// C reference: `compgen_history_sort` + `string_ref_vec_history_sort` in
/// `src/compgen.c` / `src/string_vec.c`.
#[cfg(feature = "history")]
fn sort_by_history(items: &mut [String], hist: &history::History) {
    use std::collections::HashMap;
    let scores: HashMap<&str, i32> = hist
        .entries()
        .iter()
        .map(|p| (p.name.as_str(), p.run_count as i32))
        .collect();
    items.sort_by(|a, b| {
        let sa = scores.get(a.as_str()).copied().unwrap_or(0);
        let sb = scores.get(b.as_str()).copied().unwrap_or(0);
        sb.cmp(&sa)
    });
}

/// Sort desktop entries by descending history run-count.
///
/// C reference: `drun_history_sort` in `src/drun.c`.
#[cfg(all(feature = "history", feature = "drun"))]
fn sort_drun_by_history(entries: &mut [libtofi_rs::drun::DesktopEntry], hist: &history::History) {
    use std::collections::HashMap;
    let scores: HashMap<&str, i32> = hist
        .entries()
        .iter()
        .map(|p| (p.name.as_str(), p.run_count as i32))
        .collect();
    entries.sort_by(|a, b| {
        let sa = scores.get(a.name.as_str()).copied().unwrap_or(0);
        let sb = scores.get(b.name.as_str()).copied().unwrap_or(0);
        sb.cmp(&sa)
    });
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
