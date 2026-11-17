//! `--completions` only prints a script, so it must work while tofi is open.
#![cfg(feature = "completions")]

use std::process::{Command, Stdio};

#[test]
fn completions_work_while_another_tofi_holds_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let held = libtofi_rs::lock::try_acquire(&dir.path().join("tofi.lock"))
        .unwrap()
        .expect("the lock is free");
    let out = Command::new(env!("CARGO_BIN_EXE_tofi"))
        .args(["--completions", "bash"])
        .env("XDG_RUNTIME_DIR", dir.path())
        .env("XDG_CONFIG_HOME", dir.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    drop(held);
    assert!(out.status.success(), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("tofi"));
}
