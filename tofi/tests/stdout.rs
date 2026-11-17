//! stdout carries only the selection; logs must never land there.
#![cfg(feature = "logging")]

use std::process::{Command, Stdio};

#[test]
fn logs_go_to_stderr_not_stdout() {
    let dir = tempfile::tempdir().unwrap();
    // No compositor at this socket, so tofi logs its start-up, then fails to
    // connect; everything it printed is a log line.
    let out = Command::new(env!("CARGO_BIN_EXE_tofi"))
        .args(["--log-level", "debug", "--mode", "dmenu"])
        .env_remove("RUST_LOG")
        .env("XDG_RUNTIME_DIR", dir.path())
        .env("XDG_CONFIG_HOME", dir.path())
        .env("WAYLAND_DISPLAY", "no-such-display")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stdout.is_empty(), "stdout should be empty, got: {stdout}");
    assert!(stderr.contains("tofi starting"), "stderr: {stderr}");
    assert!(
        !stderr.contains('\x1b'),
        "no colour codes when not a terminal: {stderr}"
    );
}
