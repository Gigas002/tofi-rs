#[cfg(any(feature = "wayland", feature = "history"))]
use super::*;

// ── config_anchor_to_layer — all 9 variants ───────────────────────────────

#[cfg(feature = "wayland")]
#[test]
fn anchor_top_left_contains_top_and_left() {
    use crate::config::Anchor as CA;
    use libtofi_rs::wayland::Anchor;
    let result = config_anchor_to_layer(CA::TopLeft);
    assert!(result.contains(Anchor::Top), "should contain Top");
    assert!(result.contains(Anchor::Left), "should contain Left");
    assert!(!result.contains(Anchor::Right), "should not contain Right");
    assert!(
        !result.contains(Anchor::Bottom),
        "should not contain Bottom"
    );
}

#[cfg(feature = "wayland")]
#[test]
fn anchor_top_spans_full_width() {
    use crate::config::Anchor as CA;
    use libtofi_rs::wayland::Anchor;
    let result = config_anchor_to_layer(CA::Top);
    assert!(result.contains(Anchor::Top));
    assert!(result.contains(Anchor::Left));
    assert!(result.contains(Anchor::Right));
    assert!(!result.contains(Anchor::Bottom));
}

#[cfg(feature = "wayland")]
#[test]
fn anchor_top_right_contains_top_and_right() {
    use crate::config::Anchor as CA;
    use libtofi_rs::wayland::Anchor;
    let result = config_anchor_to_layer(CA::TopRight);
    assert!(result.contains(Anchor::Top));
    assert!(result.contains(Anchor::Right));
    assert!(!result.contains(Anchor::Left));
    assert!(!result.contains(Anchor::Bottom));
}

#[cfg(feature = "wayland")]
#[test]
fn anchor_right_spans_full_height() {
    use crate::config::Anchor as CA;
    use libtofi_rs::wayland::Anchor;
    let result = config_anchor_to_layer(CA::Right);
    assert!(result.contains(Anchor::Right));
    assert!(result.contains(Anchor::Top));
    assert!(result.contains(Anchor::Bottom));
    assert!(!result.contains(Anchor::Left));
}

#[cfg(feature = "wayland")]
#[test]
fn anchor_bottom_right_contains_bottom_and_right() {
    use crate::config::Anchor as CA;
    use libtofi_rs::wayland::Anchor;
    let result = config_anchor_to_layer(CA::BottomRight);
    assert!(result.contains(Anchor::Bottom));
    assert!(result.contains(Anchor::Right));
    assert!(!result.contains(Anchor::Top));
    assert!(!result.contains(Anchor::Left));
}

#[cfg(feature = "wayland")]
#[test]
fn anchor_bottom_spans_full_width() {
    use crate::config::Anchor as CA;
    use libtofi_rs::wayland::Anchor;
    let result = config_anchor_to_layer(CA::Bottom);
    assert!(result.contains(Anchor::Bottom));
    assert!(result.contains(Anchor::Left));
    assert!(result.contains(Anchor::Right));
    assert!(!result.contains(Anchor::Top));
}

#[cfg(feature = "wayland")]
#[test]
fn anchor_bottom_left_contains_bottom_and_left() {
    use crate::config::Anchor as CA;
    use libtofi_rs::wayland::Anchor;
    let result = config_anchor_to_layer(CA::BottomLeft);
    assert!(result.contains(Anchor::Bottom));
    assert!(result.contains(Anchor::Left));
    assert!(!result.contains(Anchor::Top));
    assert!(!result.contains(Anchor::Right));
}

#[cfg(feature = "wayland")]
#[test]
fn anchor_left_spans_full_height() {
    use crate::config::Anchor as CA;
    use libtofi_rs::wayland::Anchor;
    let result = config_anchor_to_layer(CA::Left);
    assert!(result.contains(Anchor::Left));
    assert!(result.contains(Anchor::Top));
    assert!(result.contains(Anchor::Bottom));
    assert!(!result.contains(Anchor::Right));
}

#[cfg(feature = "wayland")]
#[test]
fn anchor_center_is_empty() {
    use crate::config::Anchor as CA;
    let result = config_anchor_to_layer(CA::Center);
    assert!(result.is_empty(), "Center should map to empty anchor flags");
}

// ── config_theme_to_entry ─────────────────────────────────────────────────

#[cfg(all(feature = "wayland", feature = "renderer"))]
#[test]
fn theme_to_entry_maps_all_some_fields() {
    use crate::config::{Directional, TextTheme};
    use libtofi_rs::color::Color;

    let fg = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let bg = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 0.5,
    };
    let pad = Directional {
        top: 1,
        right: 2,
        bottom: 3,
        left: 4,
    };
    let theme = TextTheme {
        foreground_color: Some(fg),
        background_color: Some(bg),
        padding: Some(pad),
        background_corner_radius: Some(8),
    };

    let entry_theme = config_theme_to_entry(&theme);

    assert_eq!(entry_theme.foreground_color, Some(fg));
    assert_eq!(entry_theme.background_color, Some(bg));
    let p = entry_theme.padding.expect("padding should be Some");
    assert_eq!(p.top, 1);
    assert_eq!(p.right, 2);
    assert_eq!(p.bottom, 3);
    assert_eq!(p.left, 4);
    assert_eq!(entry_theme.background_corner_radius, Some(8));
}

#[cfg(all(feature = "wayland", feature = "renderer"))]
#[test]
fn theme_to_entry_all_none_stays_none() {
    use crate::config::TextTheme;
    let entry_theme = config_theme_to_entry(&TextTheme::default());
    assert!(entry_theme.foreground_color.is_none());
    assert!(entry_theme.background_color.is_none());
    assert!(entry_theme.padding.is_none());
    assert!(entry_theme.background_corner_radius.is_none());
}

// ── config_cursor_to_entry ────────────────────────────────────────────────

#[cfg(all(feature = "wayland", feature = "renderer"))]
#[test]
fn cursor_to_entry_bar_style() {
    use crate::config::{CursorStyle, CursorTheme};
    use libtofi_rs::entry::CursorStyle as EntryStyle;

    let theme = CursorTheme {
        show: true,
        style: CursorStyle::Bar,
        color: None,
        text_color: None,
        corner_radius: 0,
        thickness: None,
    };
    let result = config_cursor_to_entry(&theme);
    assert!(matches!(result.style, EntryStyle::Bar));
    assert!(result.show);
}

#[cfg(all(feature = "wayland", feature = "renderer"))]
#[test]
fn cursor_to_entry_block_style() {
    use crate::config::{CursorStyle, CursorTheme};
    use libtofi_rs::entry::CursorStyle as EntryStyle;

    let theme = CursorTheme {
        show: false,
        style: CursorStyle::Block,
        color: None,
        text_color: None,
        corner_radius: 4,
        thickness: Some(3),
    };
    let result = config_cursor_to_entry(&theme);
    assert!(matches!(result.style, EntryStyle::Block));
    assert!(!result.show);
    assert_eq!(result.corner_radius, 4);
    assert_eq!(result.thickness, Some(3));
}

#[cfg(all(feature = "wayland", feature = "renderer"))]
#[test]
fn cursor_to_entry_underscore_style() {
    use crate::config::{CursorStyle, CursorTheme};
    use libtofi_rs::entry::CursorStyle as EntryStyle;

    let theme = CursorTheme {
        show: true,
        style: CursorStyle::Underscore,
        color: None,
        text_color: None,
        corner_radius: 0,
        thickness: None,
    };
    let result = config_cursor_to_entry(&theme);
    assert!(matches!(result.style, EntryStyle::Underscore));
}

#[cfg(all(feature = "wayland", feature = "renderer"))]
#[test]
fn cursor_to_entry_passes_colors_through() {
    use crate::config::{CursorStyle, CursorTheme};
    use libtofi_rs::color::Color;

    let fg = Color {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    };
    let text = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let theme = CursorTheme {
        show: true,
        style: CursorStyle::Bar,
        color: Some(fg),
        text_color: Some(text),
        corner_radius: 0,
        thickness: None,
    };
    let result = config_cursor_to_entry(&theme);
    assert_eq!(result.color, Some(fg));
    assert_eq!(result.text_color, Some(text));
}

// ── sort_by_history ───────────────────────────────────────────────────────

#[cfg(feature = "history")]
#[test]
fn sort_by_history_orders_by_run_count_descending() {
    use crate::history::History;

    let mut hist = History::new();
    hist.add("b");
    hist.add("b");
    hist.add("a");

    let mut items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    sort_by_history(&mut items, &hist);

    assert_eq!(items[0], "b");
    assert_eq!(items[1], "a");
    assert_eq!(items[2], "c");
}

#[cfg(feature = "history")]
#[test]
fn sort_by_history_empty_history_preserves_order() {
    use crate::history::History;

    let hist = History::new();
    let mut items = vec!["x".to_string(), "y".to_string(), "z".to_string()];
    sort_by_history(&mut items, &hist);
    assert_eq!(items, vec!["x", "y", "z"]);
}

// ── sort_drun_by_history ──────────────────────────────────────────────────

#[cfg(all(feature = "history", feature = "drun"))]
#[test]
fn sort_drun_by_history_orders_by_run_count() {
    use crate::history::History;
    use libtofi_rs::drun::DesktopEntry;
    use std::path::PathBuf;

    let make_entry = |name: &str| DesktopEntry {
        id: name.to_string(),
        name: name.to_string(),
        path: PathBuf::from("/tmp"),
        keywords: String::new(),
        exec: name.to_string(),
        icon: String::new(),
        terminal: false,
    };

    let mut entries = vec![
        make_entry("app-a"),
        make_entry("app-b"),
        make_entry("app-c"),
    ];

    let mut hist = History::new();
    hist.add("app-b");
    hist.add("app-b");

    sort_drun_by_history(&mut entries, &hist);

    assert_eq!(entries[0].name, "app-b");
    assert!(entries[1..].iter().any(|e| e.name == "app-a"));
    assert!(entries[1..].iter().any(|e| e.name == "app-c"));
}

// ── drun_print ────────────────────────────────────────────────────────────

#[cfg(all(feature = "wayland", feature = "drun"))]
#[test]
fn drun_print_non_terminal_app_does_not_panic() {
    use libtofi_rs::drun::DesktopEntry;
    use std::path::PathBuf;

    let entry = DesktopEntry {
        id: "myapp.desktop".to_string(),
        name: "My App".to_string(),
        path: PathBuf::from("/usr/share/applications/myapp.desktop"),
        keywords: String::new(),
        exec: "myapp".to_string(),
        icon: String::new(),
        terminal: false,
    };
    drun_print(&entry, None);
}

#[cfg(all(feature = "wayland", feature = "drun"))]
#[test]
fn drun_print_terminal_app_with_terminal_set_does_not_panic() {
    use libtofi_rs::drun::DesktopEntry;
    use std::path::PathBuf;

    let entry = DesktopEntry {
        id: "termapp.desktop".to_string(),
        name: "Term App".to_string(),
        path: PathBuf::from("/usr/share/applications/termapp.desktop"),
        keywords: String::new(),
        exec: "termapp --flag".to_string(),
        icon: String::new(),
        terminal: true,
    };
    drun_print(&entry, Some("xterm"));
}

// ── drun_launch ───────────────────────────────────────────────────────────

#[cfg(all(feature = "wayland", feature = "drun"))]
#[test]
fn drun_launch_echo_does_not_panic() {
    use libtofi_rs::drun::DesktopEntry;
    use std::path::PathBuf;

    let entry = DesktopEntry {
        id: "echo.desktop".to_string(),
        name: "Echo".to_string(),
        path: PathBuf::from("/tmp/echo.desktop"),
        keywords: String::new(),
        exec: "echo hello".to_string(),
        icon: String::new(),
        terminal: false,
    };
    drun_launch(&entry, None);
}
