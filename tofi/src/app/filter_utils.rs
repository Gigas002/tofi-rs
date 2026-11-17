//! Filtering and ranking the entry list against the typed query.

use libtofi_rs::matching::{MatchingAlgorithm, match_words};

/// The entries of `all` that match `query`, best match first.
///
/// `all` is in history order, and the sort is stable, so equally good matches
/// keep it. An empty query keeps every entry.
pub(crate) fn filter(all: &[String], query: &str, algorithm: MatchingAlgorithm) -> Vec<String> {
    if query.is_empty() {
        return all.to_vec();
    }
    let mut scored: Vec<(i32, &String)> = all
        .iter()
        .filter_map(|s| {
            let score = match_words(algorithm, query, s);
            (score != i32::MIN).then_some((score, s))
        })
        .collect();
    scored.sort_by_key(|&(score, _)| std::cmp::Reverse(score));
    scored.into_iter().map(|(_, s)| s.clone()).collect()
}
