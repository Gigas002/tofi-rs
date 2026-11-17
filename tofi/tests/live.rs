//! End-to-end tests: tofi in a headless sway, driven by a virtual keyboard.
//!
//! See `common/mod.rs` for the harness; without `sway` these are skipped.
#![cfg(all(feature = "wayland", feature = "renderer"))]

mod common;

use common::{Home, Keyboard, Tofi};

const ITEMS: &str = "alpha\nbeta\ngamma\n";

#[test]
fn dmenu_prints_the_typed_match() {
    let sway = sway_or_skip!();
    let mut kb = Keyboard::new(&sway);
    let home = Home::new();
    let mut tofi = Tofi::start(&sway, &home, &["--mode", "dmenu"], ITEMS);
    kb.type_text("bet");
    kb.key("Return");
    assert!(tofi.wait().success());
    assert_eq!(tofi.stdout(), "beta\n");
}

#[test]
fn dmenu_arrow_keys_move_the_selection() {
    let sway = sway_or_skip!();
    let mut kb = Keyboard::new(&sway);
    let home = Home::new();
    let mut tofi = Tofi::start(&sway, &home, &["--mode", "dmenu"], ITEMS);
    kb.key("Down");
    kb.key("Down");
    kb.key("Up");
    kb.key("Return");
    assert!(tofi.wait().success());
    assert_eq!(tofi.stdout(), "beta\n");
}

#[test]
fn escape_cancels_with_failure() {
    let sway = sway_or_skip!();
    let mut kb = Keyboard::new(&sway);
    let home = Home::new();
    let mut tofi = Tofi::start(&sway, &home, &["--mode", "dmenu"], ITEMS);
    kb.key("Escape");
    assert_eq!(tofi.wait().code(), Some(1));
    assert_eq!(tofi.stdout(), "");
}

#[test]
fn dmenu_without_require_match_prints_the_raw_input() {
    let sway = sway_or_skip!();
    let mut kb = Keyboard::new(&sway);
    let home = Home::new();
    home.config("[matching]\nrequire = false\n");
    let mut tofi = Tofi::start(&sway, &home, &["--mode", "dmenu"], ITEMS);
    kb.type_text("zeta");
    kb.key("Return");
    assert!(tofi.wait().success());
    assert_eq!(tofi.stdout(), "zeta\n");
}

#[test]
fn drun_launches_the_selected_entry() {
    let sway = sway_or_skip!();
    let mut kb = Keyboard::new(&sway);
    let home = Home::new();
    let marker = home.file("launched");
    home.desktop_entry(
        "test-app",
        &format!("Name=Test App\nExec=touch {}", marker.display()),
    );
    let mut tofi = Tofi::start(&sway, &home, &["--mode", "drun"], "");
    kb.type_text("test");
    kb.key("Return");
    assert!(tofi.wait().success());
    common::wait_for("the app to run", || marker.exists().then_some(()));
}

#[test]
fn history_ranks_the_last_pick_first() {
    let sway = sway_or_skip!();
    let home = Home::new();
    let hist = home.file("history");
    home.config(&format!("[history]\npath = \"{}\"\n", hist.display()));
    for keys in [&["Down", "Down", "Return"][..], &["Return"]] {
        let mut kb = Keyboard::new(&sway);
        let mut tofi = Tofi::start(&sway, &home, &["--mode", "dmenu"], ITEMS);
        for k in keys {
            kb.key(k);
        }
        assert!(tofi.wait().success());
        // The first run picks gamma; the second takes the top entry, which
        // history now makes gamma.
        assert_eq!(tofi.stdout(), "gamma\n");
    }
}

#[test]
fn edge_anchors_log_no_warnings() {
    let sway = sway_or_skip!();
    let mut kb = Keyboard::new(&sway);
    let home = Home::new();
    // "top" anchors top, left and right; sway keeps the width and centres it.
    home.theme("[window]\nanchor = \"top\"\nwidth = 640\nheight = 300\n");
    let mut tofi = Tofi::start(&sway, &home, &["--mode", "dmenu"], ITEMS);
    kb.key("Escape");
    tofi.wait();
    let log = tofi.stderr();
    assert!(log.contains("compositor configure → 640×300"), "{log}");
    assert!(!log.contains("WARN"), "{log}");
}

#[test]
fn percentages_are_of_the_logical_output_size() {
    // 1280×720 at scale 1.5 is 853×480 logical.
    let sway = sway_or_skip!("1.5");
    let mut kb = Keyboard::new(&sway);
    let home = Home::new();
    home.theme("[window]\nwidth = \"50%\"\nheight = \"50%\"\n");
    let mut tofi = Tofi::start(&sway, &home, &["--mode", "dmenu"], ITEMS);
    kb.key("Escape");
    tofi.wait();
    let log = tofi.stderr();
    assert!(log.contains("compositor configure → 426×240"), "{log}");
    assert!(log.contains("Physical buffer: 639×360"), "{log}");
}
