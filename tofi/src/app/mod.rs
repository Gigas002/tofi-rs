//! Application entry point — Wayland wiring, event loop, and submission logic.
#![deny(unsafe_code)]

mod history_utils;
mod run_utils;
mod submit_utils;

#[cfg(feature = "wayland")]
use crate::settings::LaunchMode;
use crate::settings::Settings;

// ── Public entry point ────────────────────────────────────────────────────────

/// Run the Wayland event loop. Returns `true` on selection, `false` on cancel.
pub fn run(settings: Settings) -> bool {
    let result = run_inner(settings);
    libtofi_rs::noop();
    result
}

// ── Feature-gated top-level implementations ───────────────────────────────────

#[cfg(feature = "wayland")]
fn run_inner(settings: Settings) -> bool {
    tracing::debug!("connecting to Wayland");
    let (mut state, mut event_queue) =
        libtofi_rs::wayland::connect().expect("Failed to initialize Wayland");
    tracing::debug!("Wayland connected");

    let mode = settings.mode;
    state.physical_keybindings = true;
    state.auto_accept_single = false;
    state.hide_cursor = settings.hide_cursor;
    state.keyboard_state.physical_keybindings = true;

    let (out_w, out_h) = output_size(&state.outputs);
    tracing::debug!(width = out_w, height = out_h, "output size");
    let surface_cfg = make_surface_config(&settings, out_w, out_h);
    tracing::debug!(
        width = surface_cfg.width,
        height = surface_cfg.height,
        "creating layer surface",
    );

    libtofi_rs::wayland::surface::create_surface(
        &mut state,
        &mut event_queue,
        &surface_cfg,
        settings.background_color,
    )
    .expect("Failed to create layer surface");

    #[cfg(feature = "renderer")]
    let all_entries = {
        let (width, height) = (surface_cfg.width, surface_cfg.height);
        init_entry(&mut state, &mut event_queue, &settings, width, height, mode)
    };
    #[cfg(not(feature = "renderer"))]
    let all_entries: Vec<String> = Vec::new();

    let _ = libtofi_rs::wayland::Anchor::Top;
    event_loop(&mut state, &mut event_queue, &settings, mode, all_entries)
}

#[cfg(not(feature = "wayland"))]
fn run_inner(settings: Settings) -> bool {
    let _ = settings;
    false
}

// ── Wayland event loop ────────────────────────────────────────────────────────

#[cfg(feature = "wayland")]
fn event_loop(
    state: &mut libtofi_rs::wayland::WaylandState,
    event_queue: &mut libtofi_rs::wayland::EventQueue<libtofi_rs::wayland::WaylandState>,
    settings: &Settings,
    mode: LaunchMode,
    all_entries: Vec<String>,
) -> bool {
    #[cfg(not(feature = "renderer"))]
    let _ = all_entries;

    loop {
        event_queue.flush().expect("Wayland flush failed");

        let timeout_ms = compute_timeout(state);
        poll_events(state, event_queue, timeout_ms);

        #[cfg(all(feature = "clipboard", feature = "renderer"))]
        if state.clipboard.read_fd.is_some() {
            libtofi_rs::wayland::read_clipboard(state);
        }

        handle_key_repeat(state);

        if state.closed {
            tracing::debug!("user cancelled");
            return false;
        }

        if state.submit {
            state.submit = false;
            tracing::debug!("user submitted");
            if let Some(result) = on_submit(state, settings, mode) {
                return result;
            }
        }

        if state.redraw {
            state.redraw = false;
            #[cfg(feature = "renderer")]
            handle_redraw(state, event_queue, &all_entries, settings);
        }
    }
}

// ── Event loop helpers ────────────────────────────────────────────────────────

#[cfg(feature = "wayland")]
fn compute_timeout(state: &libtofi_rs::wayland::WaylandState) -> i32 {
    match state.keyboard_state.repeat.timeout() {
        None => -1,
        Some(d) => d.as_millis().min(i32::MAX as u128) as i32,
    }
}

#[cfg(feature = "wayland")]
fn poll_events(
    state: &mut libtofi_rs::wayland::WaylandState,
    event_queue: &mut libtofi_rs::wayland::EventQueue<libtofi_rs::wayland::WaylandState>,
    timeout_ms: i32,
) {
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
        .dispatch_pending(state)
        .expect("Wayland dispatch error");
}

#[cfg(feature = "wayland")]
fn handle_key_repeat(state: &mut libtofi_rs::wayland::WaylandState) {
    if state.keyboard_state.repeat.active && state.keyboard_state.repeat.rate > 0 {
        use std::time::Instant;
        if Instant::now() >= state.keyboard_state.repeat.next {
            let keycode = state.keyboard_state.repeat.keycode;
            state.keyboard_state.advance_repeat();
            libtofi_rs::wayland::handle_keypress(state, keycode);
        }
    }
}

#[cfg(all(feature = "wayland", feature = "renderer"))]
fn on_submit(
    state: &libtofi_rs::wayland::WaylandState,
    settings: &Settings,
    mode: LaunchMode,
) -> Option<bool> {
    if submit_utils::do_submit(state, settings, mode) {
        Some(true)
    } else {
        None
    }
}

#[cfg(all(feature = "wayland", not(feature = "renderer")))]
fn on_submit(
    _state: &libtofi_rs::wayland::WaylandState,
    _settings: &Settings,
    _mode: LaunchMode,
) -> Option<bool> {
    Some(true)
}

#[cfg(all(feature = "wayland", feature = "renderer"))]
fn handle_redraw(
    state: &mut libtofi_rs::wayland::WaylandState,
    event_queue: &mut libtofi_rs::wayland::EventQueue<libtofi_rs::wayland::WaylandState>,
    all_entries: &[String],
    settings: &Settings,
) {
    if let Some(entry) = state.entry.as_mut() {
        if entry.input.is_empty() {
            entry.results = all_entries.to_vec();
        } else {
            let query = entry.input.clone();
            let algorithm = settings.algorithm;
            let mut scored: Vec<(i32, &String)> = all_entries
                .iter()
                .filter_map(|s| {
                    let score = libtofi_rs::matching::match_words(algorithm, &query, s);
                    if score > i32::MIN {
                        Some((score, s))
                    } else {
                        None
                    }
                })
                .collect();
            scored.sort_by_key(|b| std::cmp::Reverse(b.0));
            entry.results = scored.into_iter().map(|(_, s)| s.clone()).collect();
        }
        entry.update();
        entry.flush();
        libtofi_rs::wayland::surface::draw(state).expect("Failed to redraw entry");
        event_queue.flush().expect("Wayland flush after redraw");
    }
}

// ── Renderer initialisation ───────────────────────────────────────────────────

#[cfg(all(feature = "wayland", feature = "renderer"))]
#[allow(unsafe_code)]
fn init_entry(
    state: &mut libtofi_rs::wayland::WaylandState,
    event_queue: &mut libtofi_rs::wayland::EventQueue<libtofi_rs::wayland::WaylandState>,
    settings: &Settings,
    width: u32,
    height: u32,
    mode: LaunchMode,
) -> Vec<String> {
    use libtofi_rs::entry::Entry;

    let entry_config = make_entry_config(settings, width, height);

    let scale_num = if state.fractional_scale != 0 {
        state.fractional_scale
    } else {
        (state.outputs.first().map(|o| o.scale).unwrap_or(1) as u32) * 120
    };

    let (data_ptr, phys_w, phys_h) = {
        let surf = state.surface.as_mut().expect("surface must exist");
        let shm = surf.shm.as_mut().expect("SHM pool must exist");
        let ptr = shm.data_both_frames_ptr();
        (ptr, surf.phys_width, surf.phys_height)
    };

    let mut entry = unsafe { Entry::new(data_ptr, phys_w, phys_h, scale_num, entry_config) }
        .expect("Failed to create entry");

    entry.results = load_mode_entries(mode, settings, state);
    let all_entries = entry.results.clone();
    tracing::debug!(count = all_entries.len(), "entries loaded");
    entry.update();
    entry.flush();

    if let Some(surf) = state.surface.as_mut() {
        surf.index = entry.ready_index();
    }
    libtofi_rs::wayland::surface::draw(state).expect("Failed to commit entry frame");
    event_queue.flush().expect("Wayland flush failed");
    state.entry = Some(entry);
    tracing::debug!("entry widget initialized");

    all_entries
}

// ── Private helpers ───────────────────────────────────────────────────────────

#[cfg(feature = "wayland")]
fn output_size(outputs: &[libtofi_rs::wayland::OutputInfo]) -> (u32, u32) {
    outputs
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
        .unwrap_or((1920, 1080))
}

#[cfg(feature = "wayland")]
fn px(uv: &crate::theme::UnitValue, dim: u32) -> u32 {
    if uv.is_percent {
        uv.value * dim / 100
    } else {
        uv.value
    }
}

#[cfg(feature = "wayland")]
fn make_surface_config(
    settings: &Settings,
    out_w: u32,
    out_h: u32,
) -> libtofi_rs::wayland::surface::SurfaceConfig {
    use submit_utils::anchor_to_layer;
    let width = px(&settings.width, out_w);
    let height = px(&settings.height, out_h);
    libtofi_rs::wayland::surface::SurfaceConfig {
        width,
        height,
        anchor: anchor_to_layer(settings.anchor),
        exclusive_zone: -1,
        margin_top: px(&settings.margin_top, out_h) as i32,
        margin_right: px(&settings.margin_right, out_w) as i32,
        margin_bottom: px(&settings.margin_bottom, out_h) as i32,
        margin_left: px(&settings.margin_left, out_w) as i32,
        output: None,
    }
}

#[cfg(all(feature = "wayland", feature = "renderer"))]
fn make_entry_config(
    settings: &Settings,
    width: u32,
    height: u32,
) -> libtofi_rs::entry::EntryConfig {
    use submit_utils::{cursor_to_entry, text_style_to_entry};
    libtofi_rs::entry::EntryConfig {
        font_name: settings.font.clone(),
        font_size: settings.font_size,
        font_features: settings.font_features.clone(),
        font_variations: settings.font_variations.clone(),
        foreground_color: settings.foreground_color,
        background_color: settings.background_color,
        border_color: settings.border_color,
        outline_color: settings.outline_color,
        selection_highlight_color: settings.selection_highlight_color,
        corner_radius: settings.corner_radius,
        border_width: settings.border_width,
        outline_width: settings.outline_width,
        padding_top: px(&settings.padding_top, height),
        padding_bottom: px(&settings.padding_bottom, height),
        padding_left: px(&settings.padding_left, width),
        padding_right: px(&settings.padding_right, width),
        clip_to_padding: settings.clip_to_padding,
        prompt_text: settings.prompt_text.clone(),
        prompt_padding: settings.prompt_padding,
        placeholder_text: String::new(),
        num_results: settings.num_results,
        result_spacing: settings.result_spacing as i32,
        horizontal: settings.horizontal,
        input_width: 0,
        hide_input: settings.hide_input,
        hidden_character: settings.hidden_character.to_string(),
        prompt_theme: text_style_to_entry(&settings.prompt_style),
        input_theme: text_style_to_entry(&settings.input_style),
        placeholder_theme: text_style_to_entry(&crate::theme::TextStyle::default()),
        default_result_theme: text_style_to_entry(&settings.default_result_style),
        alternate_result_theme: text_style_to_entry(&settings.alternate_result_style),
        selection_theme: text_style_to_entry(&settings.selection_style),
        cursor_theme: cursor_to_entry(&settings.cursor),
    }
}

#[cfg(all(feature = "wayland", feature = "renderer"))]
fn load_mode_entries(
    mode: LaunchMode,
    settings: &Settings,
    state: &mut libtofi_rs::wayland::WaylandState,
) -> Vec<String> {
    match mode {
        LaunchMode::Dmenu => {
            let mut items = libtofi_rs::dmenu::read_lines(!settings.ascii_input);
            #[cfg(feature = "history")]
            if settings.use_history
                && let Some(hp) =
                    history_utils::history_path(mode, settings.history_file.as_deref())
                && let Ok(hist) = crate::history::load(&hp)
            {
                history_utils::sort_by_history(&mut items, &hist);
            }
            items
        }
        LaunchMode::Run => {
            let mut commands = run_utils::run_commands_cached();
            #[cfg(feature = "history")]
            if settings.use_history
                && let Some(hp) =
                    history_utils::history_path(mode, settings.history_file.as_deref())
                && let Ok(hist) = crate::history::load(&hp)
            {
                history_utils::sort_by_history(&mut commands, &hist);
            }
            commands
        }
        LaunchMode::Drun => {
            let dirs = libtofi_rs::drun::application_dirs();
            let cache_path = libtofi_rs::drun::default_cache_path()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp/tofi-drun"));
            let mut entries =
                libtofi_rs::drun::entries_cached(&dirs, &cache_path).unwrap_or_default();
            #[cfg(feature = "history")]
            if settings.use_history
                && let Some(hp) =
                    history_utils::history_path(mode, settings.history_file.as_deref())
                && let Ok(hist) = crate::history::load(&hp)
            {
                history_utils::sort_drun_by_history(&mut entries, &hist);
            }
            let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
            state.drun_entries = entries;
            names
        }
    }
}

#[cfg(test)]
mod tests;
