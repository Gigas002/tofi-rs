//! Unit tests for `libtofi::input` text editing helpers.
//!
//! These tests require no Wayland display, renderer, or XKB state — only
//! the pure string-manipulation functions.

mod input_tests {
    use crate::input::{add_char, clear_input, delete_char, delete_word};

    const MAX: usize = 256;

    // ── add_char ──────────────────────────────────────────────────────────────

    #[test]
    fn add_char_appends_when_cursor_at_end() {
        let mut s = String::from("ab");
        let mut cur = 2usize;
        add_char(&mut s, &mut cur, 'c', MAX);
        assert_eq!(s, "abc");
        assert_eq!(cur, 3);
    }

    #[test]
    fn add_char_inserts_at_cursor() {
        let mut s = String::from("ac");
        let mut cur = 1usize; // cursor after 'a'
        add_char(&mut s, &mut cur, 'b', MAX);
        assert_eq!(s, "abc");
        assert_eq!(cur, 2);
    }

    #[test]
    fn add_char_at_start() {
        let mut s = String::from("bc");
        let mut cur = 0usize;
        add_char(&mut s, &mut cur, 'a', MAX);
        assert_eq!(s, "abc");
        assert_eq!(cur, 1);
    }

    #[test]
    fn add_char_respects_max_len() {
        let mut s = String::from("ab");
        let mut cur = 2usize;
        add_char(&mut s, &mut cur, 'c', 2); // max=2, already full
        assert_eq!(s, "ab");
        assert_eq!(cur, 2);
    }

    #[test]
    fn add_char_multibyte_unicode() {
        let mut s = String::new();
        let mut cur = 0usize;
        add_char(&mut s, &mut cur, '€', MAX); // U+20AC, 3 bytes in UTF-8
        assert_eq!(s, "€");
        assert_eq!(cur, 1);
    }

    // ── delete_char ───────────────────────────────────────────────────────────

    #[test]
    fn delete_char_removes_before_cursor() {
        let mut s = String::from("abc");
        let mut cur = 3usize;
        delete_char(&mut s, &mut cur);
        assert_eq!(s, "ab");
        assert_eq!(cur, 2);
    }

    #[test]
    fn delete_char_from_middle() {
        let mut s = String::from("abc");
        let mut cur = 2usize; // cursor after 'b'
        delete_char(&mut s, &mut cur);
        assert_eq!(s, "ac");
        assert_eq!(cur, 1);
    }

    #[test]
    fn delete_char_noop_at_start() {
        let mut s = String::from("abc");
        let mut cur = 0usize;
        delete_char(&mut s, &mut cur);
        assert_eq!(s, "abc");
        assert_eq!(cur, 0);
    }

    #[test]
    fn delete_char_noop_on_empty() {
        let mut s = String::new();
        let mut cur = 0usize;
        delete_char(&mut s, &mut cur);
        assert_eq!(s, "");
        assert_eq!(cur, 0);
    }

    #[test]
    fn delete_char_multibyte() {
        let mut s = String::from("a€b");
        let mut cur = 2usize; // cursor after '€'
        delete_char(&mut s, &mut cur);
        assert_eq!(s, "ab");
        assert_eq!(cur, 1);
    }

    // ── delete_word ───────────────────────────────────────────────────────────

    #[test]
    fn delete_word_removes_last_word() {
        let mut s = String::from("hello world");
        let mut cur = 11usize;
        delete_word(&mut s, &mut cur);
        assert_eq!(s, "hello ");
        assert_eq!(cur, 6);
    }

    #[test]
    fn delete_word_skips_trailing_spaces() {
        let mut s = String::from("hello   ");
        let mut cur = 8usize;
        delete_word(&mut s, &mut cur);
        assert_eq!(s, "");
        assert_eq!(cur, 0);
    }

    #[test]
    fn delete_word_noop_at_start() {
        let mut s = String::from("hello");
        let mut cur = 0usize;
        delete_word(&mut s, &mut cur);
        assert_eq!(s, "hello");
        assert_eq!(cur, 0);
    }

    #[test]
    fn delete_word_single_word() {
        let mut s = String::from("hello");
        let mut cur = 5usize;
        delete_word(&mut s, &mut cur);
        assert_eq!(s, "");
        assert_eq!(cur, 0);
    }

    #[test]
    fn delete_word_from_middle_of_word() {
        let mut s = String::from("hello world");
        let mut cur = 8usize; // inside "world" after 'o' (h-e-l-l-o-spc-w-o = 8)
        delete_word(&mut s, &mut cur);
        assert_eq!(s, "hello rld");
        assert_eq!(cur, 6);
    }

    // ── clear_input ───────────────────────────────────────────────────────────

    #[test]
    fn clear_input_empties_string() {
        let mut s = String::from("hello world");
        let mut cur = 5usize;
        clear_input(&mut s, &mut cur);
        assert_eq!(s, "");
        assert_eq!(cur, 0);
    }

    #[test]
    fn clear_input_noop_on_empty() {
        let mut s = String::new();
        let mut cur = 0usize;
        clear_input(&mut s, &mut cur);
        assert_eq!(s, "");
        assert_eq!(cur, 0);
    }
}
