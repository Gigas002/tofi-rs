#[cfg(feature = "renderer")]
pub(crate) fn run_commands_cached() -> Vec<String> {
    let path_var = std::env::var("PATH").unwrap_or_default();
    commands_maybe_cached(&path_var, run_commands_cache_path().as_deref())
}

/// The commands on `path_var`, cached at `cache_path` if there is one.
///
/// Without a cache directory nothing is cached: a fixed path in a shared
/// directory such as /tmp could be planted or replaced by another user.
#[cfg(feature = "renderer")]
pub(crate) fn commands_maybe_cached(
    path_var: &str,
    cache_path: Option<&std::path::Path>,
) -> Vec<String> {
    match cache_path {
        Some(path) => commands_cached(path_var, path).unwrap_or_default(),
        None => scan(path_var),
    }
}

#[cfg(feature = "renderer")]
fn run_commands_cache_path() -> Option<std::path::PathBuf> {
    if let Some(c) = std::env::var_os("XDG_CACHE_HOME") {
        Some(std::path::PathBuf::from(c).join("tofi-compgen"))
    } else {
        Some(std::path::PathBuf::from(std::env::var_os("HOME")?).join(".cache/tofi-compgen"))
    }
}

#[cfg(feature = "renderer")]
pub(crate) fn scan(path_var: &str) -> Vec<String> {
    use std::collections::BTreeSet;
    use std::os::unix::fs::PermissionsExt as _;
    let mut names: BTreeSet<String> = BTreeSet::new();
    for dir in path_var.split(':').filter(|s| !s.is_empty()) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || meta.permissions().mode() & 0o111 == 0 {
                continue;
            }
            if let Some(name) = entry.file_name().to_str().map(str::to_owned) {
                names.insert(name);
            }
        }
    }
    names.into_iter().collect()
}

#[cfg(feature = "renderer")]
pub(crate) fn save_cache(commands: &[String], path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        commands
            .iter()
            .map(|s| format!("{s}\n"))
            .collect::<String>(),
    )
}

#[cfg(feature = "renderer")]
pub(crate) fn load_cache(path: &std::path::Path) -> std::io::Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    Ok(text
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

#[cfg(feature = "renderer")]
pub(crate) fn commands_cached(
    path_var: &str,
    cache_path: &std::path::Path,
) -> std::io::Result<Vec<String>> {
    use std::io;
    match std::fs::metadata(cache_path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let commands = scan(path_var);
            let _ = save_cache(&commands, cache_path);
            Ok(commands)
        }
        Err(e) => Err(e),
        Ok(cache_meta) => {
            let cache_mtime = cache_meta.modified()?;
            let is_stale = path_var
                .split(':')
                .filter(|s| !s.is_empty())
                .filter_map(|dir| std::fs::metadata(dir).ok()?.modified().ok())
                .max()
                .map(|dir_mtime| dir_mtime > cache_mtime)
                .unwrap_or(false);
            if is_stale {
                let commands = scan(path_var);
                let _ = save_cache(&commands, cache_path);
                Ok(commands)
            } else {
                load_cache(cache_path)
            }
        }
    }
}
