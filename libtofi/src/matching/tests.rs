use super::*;

// ── match_words dispatch ──────────────────────────────────────────────────────

#[test]
fn dispatch_normal() {
    assert_ne!(
        match_words(MatchingAlgorithm::Normal, "fire", "firefox"),
        i32::MIN
    );
}

#[test]
fn dispatch_prefix() {
    assert_ne!(
        match_words(MatchingAlgorithm::Prefix, "fire", "firefox"),
        i32::MIN
    );
}

#[test]
fn dispatch_fuzzy() {
    assert_ne!(
        match_words(MatchingAlgorithm::Fuzzy, "ffx", "firefox"),
        i32::MIN
    );
}

// ── Normal (simple substring) ─────────────────────────────────────────────────

#[test]
fn normal_exact_match() {
    assert_ne!(simple_match_words("hello", "hello"), i32::MIN);
}

#[test]
fn normal_substring_match() {
    assert_ne!(simple_match_words("fox", "firefox"), i32::MIN);
}

#[test]
fn normal_case_insensitive() {
    assert_ne!(simple_match_words("FIRE", "firefox"), i32::MIN);
}

#[test]
fn normal_no_match() {
    assert_eq!(simple_match_words("chrome", "firefox"), i32::MIN);
}

#[test]
fn normal_multi_word_all_present() {
    assert_ne!(simple_match_words("fire fox", "firefox"), i32::MIN);
}

#[test]
fn normal_multi_word_one_missing() {
    assert_eq!(simple_match_words("fire chrome", "firefox"), i32::MIN);
}

#[test]
fn normal_earlier_match_scores_higher() {
    // "fox" appears at byte 3 in "firefox", byte 0 in "foxhound"
    let s1 = simple_match_words("fox", "firefox");
    let s2 = simple_match_words("fox", "foxhound");
    assert!(s2 > s1, "prefix match should score higher: {s2} vs {s1}");
}

#[test]
fn normal_nfc_normalizes_query() {
    // NFD query should still match NFC-normalised candidate.
    let nfd_query = "\u{0066}\u{0069}\u{0072}\u{0065}"; // "fire" in NFD (all ASCII, trivial)
    assert_ne!(simple_match_words(nfd_query, "firefox"), i32::MIN);
}

// ── Prefix ────────────────────────────────────────────────────────────────────

#[test]
fn prefix_match_at_start() {
    assert_ne!(prefix_match_words("fire", "firefox"), i32::MIN);
}

#[test]
fn prefix_no_match_midstring() {
    // "fox" is not a prefix of "firefox"
    assert_eq!(prefix_match_words("fox", "firefox"), i32::MIN);
}

#[test]
fn prefix_case_insensitive() {
    assert_ne!(prefix_match_words("FIRE", "firefox"), i32::MIN);
}

#[test]
fn prefix_exact_match() {
    assert_ne!(prefix_match_words("firefox", "firefox"), i32::MIN);
}

#[test]
fn prefix_longer_query_no_match() {
    assert_eq!(prefix_match_words("firefoxbrowser", "firefox"), i32::MIN);
}

#[test]
fn prefix_shorter_candidate_scores_higher() {
    // "fir" matches "fir" (suffix len 0) and "firefox" (suffix len 4).
    let s1 = prefix_match_words("fir", "fir");
    let s2 = prefix_match_words("fir", "firefox");
    assert!(s1 > s2, "shorter suffix should score higher: {s1} vs {s2}");
}

// ── Fuzzy ─────────────────────────────────────────────────────────────────────

#[test]
fn fuzzy_all_chars_present_in_order() {
    assert_ne!(fuzzy_match_words("ffx", "firefox"), i32::MIN);
}

#[test]
fn fuzzy_char_out_of_order_no_match() {
    assert_eq!(fuzzy_match_words("xff", "firefox"), i32::MIN);
}

#[test]
fn fuzzy_exact_match() {
    assert_ne!(fuzzy_match_words("firefox", "firefox"), i32::MIN);
}

#[test]
fn fuzzy_no_match() {
    assert_eq!(fuzzy_match_words("zzz", "firefox"), i32::MIN);
}

#[test]
fn fuzzy_case_insensitive() {
    assert_ne!(fuzzy_match_words("FFX", "firefox"), i32::MIN);
}

#[test]
fn fuzzy_empty_pattern_scores_zero() {
    assert_eq!(fuzzy_match("", "anything"), 0);
}

#[test]
fn fuzzy_pattern_longer_than_str_no_match() {
    assert_eq!(fuzzy_match("toolong", "hi"), i32::MIN);
}

#[test]
fn fuzzy_adjacent_matches_score_higher() {
    // "fi" adjacent in "fi_rest" vs scattered in "f_i_rest"
    let s1 = fuzzy_match("fi", "fi");
    let s2 = fuzzy_match("fi", "fXi");
    assert!(s1 > s2, "adjacent should score higher: {s1} vs {s2}");
}

#[test]
fn fuzzy_prefix_match_scores_higher_than_midstring() {
    let s_prefix = fuzzy_match("fo", "foot");
    let s_mid = fuzzy_match("fo", "info");
    assert!(
        s_prefix > s_mid,
        "prefix should score higher: {s_prefix} vs {s_mid}"
    );
}

#[test]
fn fuzzy_camel_case_bonus() {
    // "B" matching the uppercase B in "FooBar" should score higher than
    // matching a lowercase b elsewhere.
    let s_camel = fuzzy_match("b", "FooBar");
    let s_lower = fuzzy_match("b", "foobar");
    // camelCase match receives camel bonus; plain lowercase does not.
    assert!(
        s_camel > s_lower,
        "CamelCase bonus should apply: {s_camel} vs {s_lower}"
    );
}

#[test]
fn fuzzy_multi_word() {
    assert_ne!(fuzzy_match_words("fi ox", "firefox"), i32::MIN);
}

#[test]
fn fuzzy_multi_word_one_missing() {
    assert_eq!(fuzzy_match_words("fi zzz", "firefox"), i32::MIN);
}

// ── Score ordering invariants ─────────────────────────────────────────────────

#[test]
fn normal_no_match_is_min() {
    assert_eq!(simple_match_words("xyz", "abc"), i32::MIN);
}

#[test]
fn prefix_no_match_is_min() {
    assert_eq!(prefix_match_words("xyz", "abc"), i32::MIN);
}

#[test]
fn fuzzy_no_match_is_min() {
    assert_eq!(fuzzy_match_words("xyz", "abc"), i32::MIN);
}

// ── Integration with StringRefVec::filter ────────────────────────────────────

#[test]
fn filter_normal_matches_firefox_only() {
    use crate::string_table::StringRefVec;

    let mut v = StringRefVec::new();
    v.add("firefox");
    v.add("alacritty");
    v.add("foot");

    // Only "firefox" contains the substring "fi".
    let results = v.filter("fi", |s| {
        let score = match_words(MatchingAlgorithm::Normal, "fi", s);
        if score == i32::MIN { None } else { Some(score) }
    });

    assert_eq!(results.len(), 1);
    assert_eq!(results.entries()[0].string, "firefox");
}

#[test]
fn filter_fuzzy_ranks_prefix_match_higher() {
    use crate::string_table::StringRefVec;

    let mut v = StringRefVec::new();
    // "first": f[0] i[1] r[2] — all adjacent from start → highest score.
    // "cafir": f[2] i[3] r[4] — gap before first char → lower score.
    v.add("cafir");
    v.add("first");

    let results = v.filter("fir", |s| {
        let score = match_words(MatchingAlgorithm::Fuzzy, "fir", s);
        if score == i32::MIN { None } else { Some(score) }
    });

    assert_eq!(results.len(), 2);
    // "first" matches at position 0 with all adjacent chars → higher score.
    assert_eq!(results.entries()[0].string, "first");
    assert_eq!(results.entries()[1].string, "cafir");
}
