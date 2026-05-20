#[cfg(all(feature = "wayland", feature = "renderer"))]
use crate::settings::LaunchMode;
#[cfg(all(feature = "wayland", feature = "renderer"))]
use crate::settings::Settings;

#[cfg(all(feature = "wayland", feature = "renderer"))]
pub(crate) fn do_submit(
    state: &libtofi_rs::wayland::WaylandState,
    settings: &Settings,
    mode: LaunchMode,
) -> bool {
    let Some(entry) = state.entry.as_ref() else {
        return false;
    };

    if entry.results.is_empty() {
        if matches!(mode, LaunchMode::Drun) {
            return false;
        }
        if settings.require_match {
            return false;
        }
        println!("{}", entry.input);
        return true;
    }

    let abs_idx = (entry.first_result + entry.selection).min(entry.results.len() - 1);
    let result = &entry.results[abs_idx];

    if matches!(mode, LaunchMode::Drun) {
        let app = state.drun_entries.iter().find(|e| e.name == *result);
        let Some(app) = app else {
            tracing::error!("Couldn't find application '{result}' in drun_entries");
            return false;
        };
        launch(app, settings.default_terminal.as_deref());
    } else {
        println!("{result}");
    }

    #[cfg(feature = "history")]
    if settings.use_history
        && let Some(hp) = super::history_utils::history_path(mode, settings.history_file.as_deref())
    {
        let mut hist = crate::history::load(&hp).unwrap_or_default();
        hist.add(result);
        if let Err(e) = crate::history::save(&hist, &hp) {
            tracing::warn!("Failed to save history: {e}");
        }
    }

    true
}

#[cfg(feature = "wayland")]
fn launch(entry: &libtofi_rs::drun::DesktopEntry, terminal: Option<&str>) {
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

#[cfg(all(feature = "wayland", feature = "renderer"))]
pub(crate) fn text_style_to_entry(t: &crate::theme::TextStyle) -> libtofi_rs::entry::TextTheme {
    libtofi_rs::entry::TextTheme {
        foreground_color: t.foreground_color,
        background_color: t.background_color,
        padding: t.padding.map(|p| libtofi_rs::entry::Directional {
            top: p.top as i32,
            right: p.right as i32,
            bottom: p.bottom as i32,
            left: p.left as i32,
        }),
        background_corner_radius: t.background_corner_radius,
    }
}

#[cfg(all(feature = "wayland", feature = "renderer"))]
pub(crate) fn cursor_to_entry(c: &crate::theme::Cursor) -> libtofi_rs::entry::CursorTheme {
    use crate::theme::CursorKind;
    use libtofi_rs::entry::CursorStyle;
    libtofi_rs::entry::CursorTheme {
        color: c.color,
        text_color: c.text_color,
        style: match c.kind {
            CursorKind::Bar => CursorStyle::Bar,
            CursorKind::Block => CursorStyle::Block,
            CursorKind::Underscore => CursorStyle::Underscore,
        },
        corner_radius: c.corner_radius,
        thickness: c.thickness,
        show: c.show,
    }
}

#[cfg(feature = "wayland")]
pub(crate) fn anchor_to_layer(anchor: crate::theme::Anchor) -> libtofi_rs::wayland::Anchor {
    use crate::theme::Anchor as A;
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
        A::Center => Anchor::empty(),
    }
}
