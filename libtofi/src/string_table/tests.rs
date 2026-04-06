use std::collections::HashMap;

use super::*;

// ── StringVec::add ────────────────────────────────────────────────────────────

#[test]
fn add_stores_nfc_normalized() {
    let mut v = StringVec::new();
    // NFD: e + combining acute → NFC: é (U+00E9)
    v.add("\u{0065}\u{0301}");
    assert_eq!(v.entries()[0].string, "\u{00E9}");
}

#[test]
fn add_already_nfc_unchanged() {
    let mut v = StringVec::new();
    v.add("hello");
    assert_eq!(v.entries()[0].string, "hello");
}

#[test]
fn add_initializes_scores_to_zero() {
    let mut v = StringVec::new();
    v.add("x");
    assert_eq!(v.entries()[0].search_score, 0);
    assert_eq!(v.entries()[0].history_score, 0);
}

#[test]
fn add_multiple() {
    let mut v = StringVec::new();
    v.add("b");
    v.add("a");
    v.add("c");
    assert_eq!(v.len(), 3);
}

// ── StringVec::sort_alpha ─────────────────────────────────────────────────────

#[test]
fn sort_alpha_orders_ascending() {
    let mut v = StringVec::new();
    v.add("banana");
    v.add("apple");
    v.add("cherry");
    v.sort_alpha();
    let names: Vec<&str> = v.entries().iter().map(|e| e.string.as_str()).collect();
    assert_eq!(names, ["apple", "banana", "cherry"]);
}

#[test]
fn sort_alpha_already_sorted() {
    let mut v = StringVec::new();
    v.add("a");
    v.add("b");
    v.sort_alpha();
    assert_eq!(v.entries()[0].string, "a");
}

// ── StringVec::dedup ──────────────────────────────────────────────────────────

#[test]
fn dedup_removes_duplicates() {
    let mut v = StringVec::new();
    v.add("b");
    v.add("a");
    v.add("b");
    v.add("a");
    v.dedup();
    assert_eq!(v.len(), 2);
    let names: Vec<&str> = v.entries().iter().map(|e| e.string.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
}

#[test]
fn dedup_no_duplicates_unchanged_count() {
    let mut v = StringVec::new();
    v.add("x");
    v.add("y");
    v.add("z");
    v.dedup();
    assert_eq!(v.len(), 3);
}

// ── StringVec::find_sorted ────────────────────────────────────────────────────

#[test]
fn find_sorted_found() {
    let mut v = StringVec::new();
    v.add("apple");
    v.add("banana");
    v.add("cherry");
    v.sort_alpha();
    assert!(v.find_sorted("banana").is_some());
}

#[test]
fn find_sorted_not_found() {
    let mut v = StringVec::new();
    v.add("apple");
    v.sort_alpha();
    assert!(v.find_sorted("mango").is_none());
}

#[test]
fn find_sorted_empty() {
    let v = StringVec::new();
    assert!(v.find_sorted("anything").is_none());
}

// ── StringRefVec::from_lines ──────────────────────────────────────────────────

#[test]
fn from_lines_splits_on_newline() {
    let buf = "foo\nbar\nbaz";
    let v = StringRefVec::from_lines(buf);
    assert_eq!(v.len(), 3);
    let names: Vec<&str> = v.entries().iter().map(|e| e.string).collect();
    assert_eq!(names, ["foo", "bar", "baz"]);
}

#[test]
fn from_lines_skips_empty_lines() {
    let buf = "foo\n\nbar\n";
    let v = StringRefVec::from_lines(buf);
    assert_eq!(v.len(), 2);
}

#[test]
fn from_lines_empty_buffer() {
    let v = StringRefVec::from_lines("");
    assert!(v.is_empty());
}

#[test]
fn from_lines_borrows_original_strings() {
    let buf = String::from("hello\nworld");
    let v = StringRefVec::from_lines(&buf);
    assert_eq!(v.entries()[0].string, "hello");
    assert_eq!(v.entries()[1].string, "world");
}

// ── StringRefVec::clone_shallow ───────────────────────────────────────────────

#[test]
fn clone_shallow_same_count() {
    let mut v = StringRefVec::new();
    v.add("a");
    v.add("b");
    let copy = v.clone_shallow();
    assert_eq!(copy.len(), 2);
}

#[test]
fn clone_shallow_independent_scores() {
    let mut v = StringRefVec::new();
    v.add("a");
    let mut copy = v.clone_shallow();
    copy.entries[0].search_score = 99;
    assert_eq!(v.entries()[0].search_score, 0);
}

// ── StringRefVec::apply_history_scores ───────────────────────────────────────

#[test]
fn apply_history_scores_sets_scores() {
    let mut v = StringRefVec::new();
    v.add("firefox");
    v.add("alacritty");
    let mut scores = HashMap::new();
    scores.insert("firefox", 5);
    v.apply_history_scores(&scores);
    let firefox = v.entries().iter().find(|e| e.string == "firefox").unwrap();
    assert_eq!(firefox.history_score, 5);
}

#[test]
fn apply_history_scores_sorts_by_history_descending() {
    let mut v = StringRefVec::new();
    v.add("rarely");
    v.add("often");
    let mut scores = HashMap::new();
    scores.insert("often", 10);
    scores.insert("rarely", 1);
    v.apply_history_scores(&scores);
    assert_eq!(v.entries()[0].string, "often");
    assert_eq!(v.entries()[1].string, "rarely");
}

#[test]
fn apply_history_scores_unknown_entries_stay_zero() {
    let mut v = StringRefVec::new();
    v.add("unknown");
    v.apply_history_scores(&HashMap::new());
    assert_eq!(v.entries()[0].history_score, 0);
}

// ── StringRefVec::filter ──────────────────────────────────────────────────────

#[test]
fn filter_empty_query_returns_full_clone() {
    let mut v = StringRefVec::new();
    v.add("foo");
    v.add("bar");
    let result = v.filter("", |_| Some(0));
    assert_eq!(result.len(), 2);
}

#[test]
fn filter_excludes_non_matching() {
    let mut v = StringRefVec::new();
    v.add("firefox");
    v.add("alacritty");
    v.add("foot");
    let result = v.filter("fi", |s| if s.contains("fi") { Some(1) } else { None });
    assert_eq!(result.len(), 1);
    assert_eq!(result.entries()[0].string, "firefox");
}

#[test]
fn filter_sorts_by_combined_score_descending() {
    let mut v = StringRefVec::new();
    v.add("low");
    v.add("high");
    // Manually set history scores.
    v.entries[0].history_score = 1;
    v.entries[1].history_score = 10;
    let result = v.filter("x", |_| Some(5));
    // "high": search=5, history=10, total=15
    // "low":  search=5, history=1,  total=6
    assert_eq!(result.entries()[0].string, "high");
    assert_eq!(result.entries()[1].string, "low");
}

#[test]
fn filter_no_matches_returns_empty() {
    let mut v = StringRefVec::new();
    v.add("foo");
    let result = v.filter("zzz", |_| None);
    assert!(result.is_empty());
}

// ── StringRefVec::find_sorted ─────────────────────────────────────────────────

#[test]
fn ref_find_sorted_found() {
    let strings = vec!["apple", "banana", "cherry"];
    let mut v = StringRefVec::new();
    for s in &strings {
        v.add(s);
    }
    v.entries.sort_by(|a, b| a.string.cmp(b.string));
    assert!(v.find_sorted("banana").is_some());
}

#[test]
fn ref_find_sorted_not_found() {
    let mut v = StringRefVec::new();
    v.add("apple");
    v.entries.sort_by(|a, b| a.string.cmp(b.string));
    assert!(v.find_sorted("mango").is_none());
}

// ── Deterministic ordering ────────────────────────────────────────────────────

#[test]
fn sort_alpha_is_deterministic() {
    let input = ["zebra", "mango", "apple", "mango"];
    let mut v1 = StringVec::new();
    let mut v2 = StringVec::new();
    for s in &input {
        v1.add(s);
        v2.add(s);
    }
    v1.sort_alpha();
    v2.sort_alpha();
    let names1: Vec<&str> = v1.entries().iter().map(|e| e.string.as_str()).collect();
    let names2: Vec<&str> = v2.entries().iter().map(|e| e.string.as_str()).collect();
    assert_eq!(names1, names2);
}
