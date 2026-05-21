#[cfg(feature = "history")]
use crate::settings::LaunchMode;

#[cfg(feature = "history")]
pub(crate) fn history_path(
    mode: LaunchMode,
    history_file: Option<&str>,
) -> Option<std::path::PathBuf> {
    history_file
        .map(std::path::PathBuf::from)
        .or_else(|| match mode {
            LaunchMode::Dmenu => None,
            LaunchMode::Drun => crate::history::default_history_path(true),
            LaunchMode::Run => crate::history::default_history_path(false),
        })
}

#[cfg(feature = "history")]
pub(crate) fn sort_by_history(items: &mut [String], hist: &crate::history::AppHistory) {
    use std::collections::HashMap;
    let scores: HashMap<&str, i32> = hist
        .entries()
        .iter()
        .map(|p| (p.name.as_str(), p.run_count as i32))
        .collect();
    items.sort_by(|a, b| {
        scores
            .get(a.as_str())
            .copied()
            .unwrap_or(0)
            .cmp(&scores.get(b.as_str()).copied().unwrap_or(0))
            .reverse()
    });
}

#[cfg(feature = "history")]
pub(crate) fn sort_drun_by_history(
    entries: &mut [libtofi_rs::drun::DesktopEntry],
    hist: &crate::history::AppHistory,
) {
    use std::collections::HashMap;
    let scores: HashMap<&str, i32> = hist
        .entries()
        .iter()
        .map(|p| (p.name.as_str(), p.run_count as i32))
        .collect();
    entries.sort_by(|a, b| {
        scores
            .get(a.name.as_str())
            .copied()
            .unwrap_or(0)
            .cmp(&scores.get(b.name.as_str()).copied().unwrap_or(0))
            .reverse()
    });
}
