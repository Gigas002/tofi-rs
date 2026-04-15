//! Single-instance lock (feature **`single-instance-lock`**).
#![deny(unsafe_code)]
//!
//! Calls `flock(2)` with `LOCK_EX | LOCK_NB` on a well-known file so that
//! only one `tofi` process holds the lock at a time.  The lock is released
//! automatically when the [`Lock`] value is dropped (i.e. when the owning
//! `File` is closed).
//!
//! # Path resolution
//!
//! | Condition | Lock file |
//! |---|---|
//! | `$XDG_RUNTIME_DIR` is set | `$XDG_RUNTIME_DIR/tofi.lock` |
//! | `$XDG_CACHE_HOME` is set | `$XDG_CACHE_HOME/tofi.lock` |
//! | Fallback | `$HOME/.cache/tofi.lock` |

use std::fs;
use std::path::{Path, PathBuf};

use rustix::fs::{FlockOperation, flock};
use rustix::io::Errno;

use crate::Result;

const LOCK_FILENAME: &str = "tofi.lock";

/// An acquired single-instance lock.
///
/// Holds the open [`fs::File`] whose `flock` is released automatically on
/// [`Drop`] via `rustix::fs::flock(Unlock)`.
#[must_use]
pub struct Lock {
    file: fs::File,
}

impl Drop for Lock {
    fn drop(&mut self) {
        // Best-effort unlock; ignore errors (process exit also releases flock).
        let _ = flock(&self.file, FlockOperation::Unlock);
    }
}

/// Try to acquire the single-instance lock at `path`.
///
/// - Returns `Ok(Some(lock))` when the lock was acquired successfully.
/// - Returns `Ok(None)` when another process already holds the lock
///   (`EWOULDBLOCK`), i.e. another `tofi` instance is running.
/// - Returns `Err(_)` for unexpected I/O failures (e.g. permission denied).
pub fn try_acquire(path: &Path) -> Result<Option<Lock>> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;

    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(Some(Lock { file })),
        // EAGAIN == EWOULDBLOCK on Linux; match just AGAIN.
        Err(Errno::AGAIN) => Ok(None),
        Err(e) => Err(std::io::Error::from(e).into()),
    }
}

/// Acquire the lock using the platform-default path.
///
/// Resolves the path via [`default_lock_path`] and calls [`try_acquire`].
pub fn try_acquire_default() -> Result<Option<Lock>> {
    match default_lock_path() {
        Some(path) => try_acquire(&path),
        None => Err(crate::Error::InvalidValue(
            "could not determine lock file path: HOME not set".into(),
        )),
    }
}

/// Resolve the default lock file path.
///
/// Priority: `XDG_RUNTIME_DIR` → `XDG_CACHE_HOME` → `$HOME/.cache/`.
pub fn default_lock_path() -> Option<PathBuf> {
    resolve_lock_path(
        std::env::var_os("XDG_RUNTIME_DIR"),
        std::env::var_os("XDG_CACHE_HOME"),
        std::env::var_os("HOME"),
    )
}

/// Pure path-resolution logic, extracted for testing without env mutation.
fn resolve_lock_path(
    xdg_runtime_dir: Option<std::ffi::OsString>,
    xdg_cache_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    if let Some(runtime) = xdg_runtime_dir {
        Some(PathBuf::from(runtime).join(LOCK_FILENAME))
    } else if let Some(cache) = xdg_cache_home {
        Some(PathBuf::from(cache).join(LOCK_FILENAME))
    } else {
        Some(PathBuf::from(home?).join(".cache").join(LOCK_FILENAME))
    }
}

#[cfg(test)]
mod tests;
