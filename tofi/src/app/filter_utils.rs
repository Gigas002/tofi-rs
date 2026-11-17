//! Filtering and ranking the entry list against the typed query.

use libtofi_rs::matching::{MatchingAlgorithm, match_words};

/// How much worse a keyword match ranks than the same name match (upstream's
/// value).
const KEYWORD_PENALTY: i32 = 20;

/// The entries of `all` that match `query`, best match first.
///
/// An entry that doesn't match by name may match by its `keywords` (same
/// index; drun entries' `Keywords`, or empty), ranked lower than a name match.
/// `all` is in history order, and the sort is stable, so equally good matches
/// keep it. An empty query keeps every entry.
pub(crate) fn filter(
    all: &[String],
    keywords: &[String],
    query: &str,
    algorithm: MatchingAlgorithm,
) -> Vec<String> {
    if query.is_empty() {
        return all.to_vec();
    }
    let mut scored: Vec<(i32, &String)> = all
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let mut score = match_words(algorithm, query, s);
            if score == i32::MIN
                && let Some(kw) = keywords.get(i).filter(|kw| !kw.is_empty())
            {
                score = match_words(algorithm, query, kw);
                if score != i32::MIN {
                    score = score.saturating_sub(KEYWORD_PENALTY);
                }
            }
            (score != i32::MIN).then_some((score, s))
        })
        .collect();
    scored.sort_by_key(|&(score, _)| std::cmp::Reverse(score));
    scored.into_iter().map(|(_, s)| s.clone()).collect()
}
