//! Selection submission, output helpers, history sorting, and config converters.

#[cfg(feature = "wayland")]
use crate::config;

// ── do_submit ─────────────────────────────────────────────────────────────────

/// Handle selection acceptance — print to stdout, update history, optionally
/// launch the app.  Returns `true` when the selection was accepted and the
/// launcher should exit; `false` when no match is available and the loop
/// should continue.
#[cfg(all(feature = "wayland", feature = "renderer"))]
pub fn do_submit(
    state: &libtofi_rs::wayland::WaylandState,
    config: &config::TofiConfig,
    mode: crate::app::LaunchMode,
    all_commands: &[String],
) -> bool {
    let Some(entry) = state.entry.as_ref() else {
        return false;
    };

    // ── No results ────────────────────────────────────────────────────────────
    if entry.results.is_empty() {
        #[cfg(feature = "drun")]
        if matches!(mode, crate::app::LaunchMode::Drun) {
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
    if matches!(mode, crate::app::LaunchMode::Drun) {
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
        if matches!(mode, crate::app::LaunchMode::Stdin) && config.print_index {
            if let Some(idx) = all_commands.iter().position(|s| s == result) {
                println!("{}", idx + 1);
            }
        } else {
            println!("{result}");
        }
    }
    #[cfg(not(feature = "drun"))]
    {
        if matches!(mode, crate::app::LaunchMode::Stdin) && config.print_index {
            if let Some(idx) = all_commands.iter().position(|s| s == result) {
                println!("{}", idx + 1);
            }
        } else {
            println!("{result}");
        }
    }

    // ── History ───────────────────────────────────────────────────────────────
    #[cfg(feature = "history")]
    if config.use_history {
        let is_drun = cfg!(feature = "drun") && matches!(mode, crate::app::LaunchMode::Drun);
        let hist_path = config
            .history_file
            .as_deref()
            .map(std::path::PathBuf::from)
            .or_else(|| crate::history::default_history_path(is_drun));
        if let Some(hp) = hist_path {
            let mut hist = crate::history::load(&hp).unwrap_or_default();
            hist.add(result);
            if let Err(e) = crate::history::save(&hist, &hp) {
                tracing::warn!("Failed to save history: {e}");
            }
        }
    }

    true
}

// ── drun output ───────────────────────────────────────────────────────────────

/// Print the expanded exec command for a desktop entry to stdout.
///
/// Prepends the terminal command when `entry.terminal` is `true`.
#[cfg(all(feature = "wayland", feature = "drun"))]
pub fn drun_print(entry: &libtofi_rs::drun::DesktopEntry, terminal: Option<&str>) {
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
/// shell-quoting parser (e.g. `shlex`) would be more robust for complex exec
/// strings, but is sufficient for typical `.desktop` file entries.
#[cfg(all(feature = "wayland", feature = "drun"))]
pub fn drun_launch(entry: &libtofi_rs::drun::DesktopEntry, terminal: Option<&str>) {
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
#[cfg(feature = "history")]
pub fn sort_by_history(items: &mut [String], hist: &crate::history::History) {
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
#[cfg(all(feature = "history", feature = "drun"))]
pub fn sort_drun_by_history(
    entries: &mut [libtofi_rs::drun::DesktopEntry],
    hist: &crate::history::History,
) {
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

// ── Config converters ─────────────────────────────────────────────────────────

/// Convert a [`config::TextTheme`] to [`libtofi_rs::entry::TextTheme`].
#[cfg(all(feature = "wayland", feature = "renderer"))]
pub fn config_theme_to_entry(t: &config::TextTheme) -> libtofi_rs::entry::TextTheme {
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
pub fn config_cursor_to_entry(c: &config::CursorTheme) -> libtofi_rs::entry::CursorTheme {
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
#[cfg(feature = "wayland")]
pub fn config_anchor_to_layer(anchor: config::Anchor) -> libtofi_rs::wayland::Anchor {
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
