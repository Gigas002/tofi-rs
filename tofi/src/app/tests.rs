#[cfg(all(feature = "wayland", feature = "drun", feature = "run-commands"))]
use super::*;

// detect_mode: explicit CLI flags take priority over argv[0].
// These tests cover the flag paths; the argv[0] symlink path is exercised
// at runtime.

#[test]
#[cfg(all(feature = "wayland", feature = "drun", feature = "run-commands"))]
fn detect_mode_drun_flag_wins() {
    assert_eq!(detect_mode(true, false), LaunchMode::Drun);
}

#[test]
#[cfg(all(feature = "wayland", feature = "drun", feature = "run-commands"))]
fn detect_mode_run_flag_wins() {
    assert_eq!(detect_mode(false, true), LaunchMode::Run);
}

#[test]
#[cfg(all(feature = "wayland", feature = "drun", feature = "run-commands"))]
fn detect_mode_no_flags_falls_back_to_stdin() {
    // Test-runner argv[0] will not contain "-drun" or "-run".
    assert_eq!(detect_mode(false, false), LaunchMode::Stdin);
}

#[test]
#[cfg(all(feature = "wayland", feature = "drun", feature = "run-commands"))]
fn detect_mode_drun_flag_overrides_run_flag() {
    // drun is checked first; if both somehow set, drun wins.
    assert_eq!(detect_mode(true, true), LaunchMode::Drun);
}
