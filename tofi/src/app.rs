//! Application entry point — Wayland wiring and the main event loop.
#![deny(unsafe_code)]

use crate::config::TofiConfig;

// ── Launch mode ───────────────────────────────────────────────────────────────

/// Launch mode — determined from `argv[0]` before Wayland is initialised.
#[cfg(feature = "wayland")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// Default: read entries from stdin (one per line).
    Stdin,
    /// Invoked as `tofi-run`: list executables from `$PATH`.
    #[cfg(feature = "run-commands")]
    Run,
    /// Invoked as `tofi-drun`: list desktop applications.
    #[cfg(feature = "drun")]
    Drun,
}

/// Detect launch mode.
///
/// `flag_drun` / `flag_run` are set when `--drun` / `--run` was passed on the
/// command line.  If neither flag is set, `argv[0]` is checked for `-drun` /
/// `-run` (symlink convention).  Falls back to `Stdin`.
#[cfg(feature = "wayland")]
pub fn detect_mode(
    #[cfg(feature = "drun")] flag_drun: bool,
    #[cfg(feature = "run-commands")] flag_run: bool,
) -> LaunchMode {
    // Explicit CLI flags take priority over the argv[0] symlink convention.
    #[cfg(feature = "drun")]
    if flag_drun {
        tracing::debug!("detect_mode: --drun flag → Drun");
        return LaunchMode::Drun;
    }
    #[cfg(feature = "run-commands")]
    if flag_run {
        tracing::debug!("detect_mode: --run flag → Run");
        return LaunchMode::Run;
    }

    // Fall back to argv[0] symlink convention.
    let argv0 = std::env::args().next().unwrap_or_default();
    tracing::debug!("detect_mode: argv[0]={argv0:?}");
    #[cfg(feature = "drun")]
    if argv0.contains("-drun") {
        tracing::debug!("detect_mode: argv[0] contains '-drun' → Drun");
        return LaunchMode::Drun;
    }
    #[cfg(feature = "run-commands")]
    if argv0.contains("-run") {
        tracing::debug!("detect_mode: argv[0] contains '-run' → Run");
        return LaunchMode::Run;
    }
    let _ = argv0;
    tracing::debug!("detect_mode: no match → Stdin");
    LaunchMode::Stdin
}

// ── stdin reader ──────────────────────────────────────────────────────────────

/// Read stdin line-by-line into an owned `Vec<String>`.
///
/// Empty lines are skipped. When `normalize` is `true` each line is
/// NFC-normalised.
#[cfg(feature = "wayland")]
pub fn read_stdin(normalize: bool) -> Vec<String> {
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

// ── run ───────────────────────────────────────────────────────────────────────

/// Wire configuration into the Wayland event loop and run until the user
/// accepts a result or closes the window.
///
/// `Entry::new` wraps a raw SHM pointer; the `allow(unsafe_code)` below is
/// the only unsafe site in this crate.
#[allow(unsafe_code)]
pub fn run(
    config: TofiConfig,
    #[cfg(feature = "drun")] flag_drun: bool,
    #[cfg(feature = "run-commands")] flag_run: bool,
) {
    #[cfg(feature = "wayland")]
    {
        use libtofi_rs::wayland::{Anchor, surface::SurfaceConfig};

        let (mut state, mut event_queue) =
            libtofi_rs::wayland::connect().expect("Failed to initialize Wayland");

        let mode = detect_mode(
            #[cfg(feature = "drun")]
            flag_drun,
            #[cfg(feature = "run-commands")]
            flag_run,
        );

        // Wire config options that the keyboard / pointer handlers need.
        state.physical_keybindings = config.physical_keybindings;
        state.auto_accept_single = config.auto_accept_single;
        state.hide_cursor = config.hide_cursor;
        state.keyboard_state.physical_keybindings = config.physical_keybindings;

        // Resolve UnitValues to pixels using the first output's dimensions.
        // Swap width/height for rotated outputs (90°/270°).
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

        let resolve_px = |uv: &crate::config::UnitValue, dim: u32| -> u32 {
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
            anchor: crate::submit::config_anchor_to_layer(config.anchor),
            exclusive_zone: config.exclusive_zone,
            margin_top: resolve_px(&config.margin_top, out_h) as i32,
            margin_right: resolve_px(&config.margin_right, out_w) as i32,
            margin_bottom: resolve_px(&config.margin_bottom, out_h) as i32,
            margin_left: resolve_px(&config.margin_left, out_w) as i32,
            output: None,
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
        #[cfg(feature = "renderer")]
        let all_commands: Vec<String>;

        // Initialise the Entry layout engine and store it in `state.entry` so
        // keyboard event handlers can update it.  Declared here (after
        // create_surface) so the SHM pool is already allocated.
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
                prompt_theme: crate::submit::config_theme_to_entry(&config.prompt_theme),
                input_theme: crate::submit::config_theme_to_entry(&config.input_theme),
                placeholder_theme: crate::submit::config_theme_to_entry(&config.placeholder_theme),
                default_result_theme: crate::submit::config_theme_to_entry(
                    &config.default_result_theme,
                ),
                alternate_result_theme: crate::submit::config_theme_to_entry(
                    &config.alternate_result_theme,
                ),
                selection_theme: crate::submit::config_theme_to_entry(&config.selection_theme),
                cursor_theme: crate::submit::config_cursor_to_entry(&config.cursor_theme),
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

            entry.results = match mode {
                // ── stdin ─────────────────────────────────────────────────────
                LaunchMode::Stdin => {
                    tracing::debug!("Mode: stdin");
                    let mut items = read_stdin(!config.ascii_input);
                    #[cfg(feature = "history")]
                    if config.use_history {
                        // History for stdin mode requires an explicit history file path.
                        if let Some(ref hf) = config.history_file
                            && let Ok(hist) = crate::history::load(std::path::Path::new(hf))
                        {
                            crate::submit::sort_by_history(&mut items, &hist);
                        }
                    }
                    items
                }

                // ── run ───────────────────────────────────────────────────────
                #[cfg(feature = "run-commands")]
                LaunchMode::Run => {
                    tracing::debug!("Mode: run");
                    let path_var = std::env::var("PATH").unwrap_or_default();
                    let cache_path = crate::run_commands::default_cache_path()
                        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/tofi-compgen"));
                    let mut commands = crate::run_commands::commands_cached(&path_var, &cache_path)
                        .unwrap_or_default();
                    #[cfg(feature = "history")]
                    if config.use_history {
                        let hist_path = config
                            .history_file
                            .as_deref()
                            .map(std::path::PathBuf::from)
                            .or_else(|| crate::history::default_history_path(false));
                        if let Some(hp) = hist_path
                            && let Ok(hist) = crate::history::load(&hp)
                        {
                            crate::submit::sort_by_history(&mut commands, &hist);
                        }
                    }
                    commands
                }

                // ── drun ──────────────────────────────────────────────────────
                #[cfg(feature = "drun")]
                LaunchMode::Drun => {
                    tracing::debug!("Mode: drun");
                    let dirs = libtofi_rs::drun::application_dirs();
                    tracing::debug!("drun: searching {} XDG dirs: {dirs:?}", dirs.len());
                    let cache_path = libtofi_rs::drun::default_cache_path()
                        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/tofi-drun"));
                    tracing::debug!("drun: cache path = {cache_path:?}");
                    let mut entries =
                        libtofi_rs::drun::entries_cached(&dirs, &cache_path).unwrap_or_default();
                    tracing::debug!("drun: loaded {} desktop entries", entries.len());
                    if entries.is_empty() {
                        tracing::warn!(
                            "drun: no desktop entries found — check XDG_DATA_DIRS and \
                             that .desktop files exist in <dir>/applications/"
                        );
                    }
                    #[cfg(feature = "history")]
                    if config.use_history {
                        let hist_path = config
                            .history_file
                            .as_deref()
                            .map(std::path::PathBuf::from)
                            .or_else(|| crate::history::default_history_path(true));
                        if let Some(hp) = hist_path
                            && let Ok(hist) = crate::history::load(&hp)
                        {
                            crate::submit::sort_drun_by_history(&mut entries, &hist);
                        }
                    }
                    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
                    tracing::debug!("drun: entry.results will have {} names", names.len());
                    state.drun_entries = entries;
                    names
                }
            };

            // Snapshot the full unfiltered list for `print_index`.
            all_commands = entry.results.clone();

            entry.flush();

            libtofi_rs::wayland::surface::draw(&mut state).expect("Failed to commit entry frame");
            event_queue.flush().expect("Wayland flush failed");
            tracing::debug!("Entry initial frame committed");

            state.entry = Some(entry);
        }

        // ── Event loop ────────────────────────────────────────────────────────
        // Non-blocking dispatch with key-repeat timeout so held keys fire
        // repeated input events between Wayland events.
        tracing::debug!(
            "Entering event loop — keyboard_ready={}",
            state.keyboard_state.is_ready()
        );

        'event_loop: loop {
            event_queue.flush().expect("Wayland flush failed");

            let timeout_ms: i32 = match state.keyboard_state.repeat.timeout() {
                None => -1,
                Some(d) => d.as_millis().min(i32::MAX as u128) as i32,
            };

            // Poll the Wayland fd (and clipboard fd when a paste is active)
            // with timeout so key repeat can fire between events.
            let guard = event_queue.prepare_read();
            if let Some(ref g) = guard {
                use rustix::event::{PollFd, PollFlags, Timespec, poll};
                let wayland_fd = g.connection_fd();

                let ts;
                let timeout: Option<&Timespec> = if timeout_ms < 0 {
                    None
                } else {
                    let ms = timeout_ms as i64;
                    ts = Timespec {
                        tv_sec: ms / 1000,
                        tv_nsec: (ms % 1000) * 1_000_000,
                    };
                    Some(&ts)
                };

                #[cfg(feature = "clipboard")]
                if let Some(cfd) = state.clipboard.read_fd.as_ref() {
                    let mut pfds = [
                        PollFd::new(&wayland_fd, PollFlags::IN),
                        PollFd::new(cfd, PollFlags::IN),
                    ];
                    let _ = poll(&mut pfds, timeout);
                } else {
                    let mut pfds = [PollFd::new(&wayland_fd, PollFlags::IN)];
                    let _ = poll(&mut pfds, timeout);
                }

                #[cfg(not(feature = "clipboard"))]
                {
                    let mut pfds = [PollFd::new(&wayland_fd, PollFlags::IN)];
                    let _ = poll(&mut pfds, timeout);
                }
            }
            if let Some(g) = guard {
                let _ = g.read();
            }
            event_queue
                .dispatch_pending(&mut state)
                .expect("Wayland dispatch error");

            // Drain any available clipboard data from the paste pipe.
            #[cfg(all(feature = "clipboard", feature = "renderer"))]
            if state.clipboard.read_fd.is_some() {
                libtofi_rs::wayland::read_clipboard(&mut state);
            }

            // ── Key repeat ────────────────────────────────────────────────────
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
                tracing::debug!("Event loop: state.closed=true → breaking");
                break 'event_loop;
            }

            if state.submit {
                tracing::debug!("Event loop: state.submit=true → handling submission");
                state.submit = false;
                #[cfg(feature = "renderer")]
                {
                    let submitted = crate::submit::do_submit(&state, &config, mode, &all_commands);
                    tracing::debug!("Event loop: do_submit returned {submitted}");
                    if submitted {
                        tracing::debug!("Submit accepted — breaking event loop");
                        break 'event_loop;
                    }
                }
                #[cfg(not(feature = "renderer"))]
                break 'event_loop;
            }

            // ── Redraw ────────────────────────────────────────────────────────
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

    #[cfg(not(feature = "wayland"))]
    let _ = config;
    libtofi_rs::noop();
}
