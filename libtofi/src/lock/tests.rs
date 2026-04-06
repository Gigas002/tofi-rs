//! Unit tests for `libtofi::lock`.

use super::*;

// ── resolve_lock_path (pure) ──────────────────────────────────────────────────

#[test]
fn path_prefers_xdg_runtime_dir() {
    let path = resolve_lock_path(
        Some("/run/user/1000".into()),
        Some("/home/user/.cache".into()),
        Some("/home/user".into()),
    )
    .expect("path");
    assert_eq!(path, PathBuf::from("/run/user/1000/tofi.lock"));
}

#[test]
fn path_falls_back_to_xdg_cache_home() {
    let path = resolve_lock_path(
        None,
        Some("/home/user/.cache".into()),
        Some("/home/user".into()),
    )
    .expect("path");
    assert_eq!(path, PathBuf::from("/home/user/.cache/tofi.lock"));
}

#[test]
fn path_falls_back_to_home_cache() {
    let path = resolve_lock_path(None, None, Some("/home/user".into())).expect("path");
    assert_eq!(path, PathBuf::from("/home/user/.cache/tofi.lock"));
}

#[test]
fn path_returns_none_when_no_env_set() {
    let path = resolve_lock_path(None, None, None);
    assert!(path.is_none());
}

// ── try_acquire ───────────────────────────────────────────────────────────────

#[test]
fn acquire_succeeds_on_fresh_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("tofi.lock");
    let lock = try_acquire(&path).expect("try_acquire");
    assert!(lock.is_some(), "expected lock to be acquired");
}

#[test]
fn acquire_creates_parent_directories() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sub").join("dir").join("tofi.lock");
    let lock = try_acquire(&path).expect("try_acquire");
    assert!(lock.is_some());
    assert!(path.exists());
}

#[test]
fn second_acquire_returns_none_while_first_held() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("tofi.lock");

    let _lock1 = try_acquire(&path)
        .expect("first acquire")
        .expect("first lock");

    // A second open file description on the same path should fail with EWOULDBLOCK.
    let lock2 = try_acquire(&path).expect("try_acquire did not error");
    assert!(
        lock2.is_none(),
        "expected second acquire to return None (already locked)"
    );
}

#[test]
fn lock_released_on_drop_allows_reacquire() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("tofi.lock");

    {
        let _lock = try_acquire(&path)
            .expect("first acquire")
            .expect("first lock");
        // lock dropped here
    }

    let lock2 = try_acquire(&path)
        .expect("second acquire")
        .expect("second lock should succeed after drop");
    drop(lock2);
}
