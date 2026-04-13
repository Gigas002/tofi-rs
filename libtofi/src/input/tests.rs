//! Unit tests for `libtofi::input` — text editing helpers and key binding
//! classification.
//!
//! These tests require no Wayland display, renderer, or XKB state — only
//! pure functions.

mod classify_tests {
    use crate::input::{
        KEY_B, KEY_BACKSPACE, KEY_C, KEY_DOWN, KEY_ENTER, KEY_ESC, KEY_F, KEY_G, KEY_H, KEY_HOME,
        KEY_J, KEY_K, KEY_KPENTER, KEY_L, KEY_LEFT, KEY_LEFTBRACE, KEY_M, KEY_N, KEY_P,
        KEY_PAGEDOWN, KEY_PAGEUP, KEY_RIGHT, KEY_TAB, KEY_U, KEY_UP, KEY_V, KEY_W, KeyAction,
        classify_keypress,
    };

    // Convenience wrappers — (ctrl, alt, shift, key, ch).
    fn plain(key: u32, ch: u32) -> KeyAction {
        classify_keypress(false, false, false, key, ch)
    }
    fn ctrl(key: u32) -> KeyAction {
        classify_keypress(true, false, false, key, 0)
    }
    fn alt(key: u32) -> KeyAction {
        classify_keypress(false, true, false, key, 0)
    }
    fn shift(key: u32) -> KeyAction {
        classify_keypress(false, false, true, key, 0)
    }
    fn ctrl_alt(key: u32) -> KeyAction {
        classify_keypress(true, true, false, key, 0)
    }

    // ── Printable characters ──────────────────────────────────────────────────

    #[test]
    fn printable_ascii_inserts() {
        assert_eq!(plain(0, 'a' as u32), KeyAction::InsertChar('a'));
    }

    #[test]
    fn printable_unicode_inserts() {
        assert_eq!(plain(0, '€' as u32), KeyAction::InsertChar('€'));
    }

    #[test]
    fn ctrl_suppresses_insert() {
        // Ctrl+A: 'a' is printable but ctrl is held → not InsertChar.
        let action = classify_keypress(true, false, false, 0, 'a' as u32);
        assert_ne!(action, KeyAction::InsertChar('a'));
    }

    #[test]
    fn alt_suppresses_insert() {
        let action = classify_keypress(false, true, false, 0, 'a' as u32);
        assert_ne!(action, KeyAction::InsertChar('a'));
    }

    #[test]
    fn null_ch_not_inserted() {
        // ch=0 is not printable.
        let action = plain(0, 0);
        assert_eq!(action, KeyAction::Unknown);
    }

    // ── Text editing ──────────────────────────────────────────────────────────

    #[test]
    fn ctrl_w_deletes_word() {
        assert_eq!(ctrl(KEY_W), KeyAction::DeleteWord);
    }

    #[test]
    fn ctrl_backspace_deletes_word() {
        assert_eq!(ctrl(KEY_BACKSPACE), KeyAction::DeleteWord);
    }

    #[test]
    fn backspace_deletes_char() {
        assert_eq!(plain(KEY_BACKSPACE, 0), KeyAction::DeleteChar);
    }

    #[test]
    fn ctrl_h_deletes_char() {
        assert_eq!(ctrl(KEY_H), KeyAction::DeleteChar);
    }

    #[test]
    fn ctrl_u_clears_input() {
        assert_eq!(ctrl(KEY_U), KeyAction::ClearInput);
    }

    #[test]
    fn ctrl_v_pastes() {
        assert_eq!(ctrl(KEY_V), KeyAction::Paste);
    }

    // ── Cursor / result navigation ────────────────────────────────────────────

    #[test]
    fn left_prev_cursor_or_result() {
        assert_eq!(plain(KEY_LEFT, 0), KeyAction::PrevCursorOrResult);
    }

    #[test]
    fn right_next_cursor_or_result() {
        assert_eq!(plain(KEY_RIGHT, 0), KeyAction::NextCursorOrResult);
    }

    #[test]
    fn up_prev_result() {
        assert_eq!(plain(KEY_UP, 0), KeyAction::PrevResult);
    }

    #[test]
    fn shift_tab_prev_result() {
        assert_eq!(shift(KEY_TAB), KeyAction::PrevResult);
    }

    #[test]
    fn alt_h_prev_result() {
        assert_eq!(alt(KEY_H), KeyAction::PrevResult);
    }

    #[test]
    fn ctrl_k_prev_result() {
        assert_eq!(ctrl(KEY_K), KeyAction::PrevResult);
    }

    #[test]
    fn alt_k_prev_result() {
        assert_eq!(alt(KEY_K), KeyAction::PrevResult);
    }

    #[test]
    fn ctrl_p_prev_result() {
        assert_eq!(ctrl(KEY_P), KeyAction::PrevResult);
    }

    #[test]
    fn ctrl_b_prev_result() {
        assert_eq!(ctrl(KEY_B), KeyAction::PrevResult);
    }

    #[test]
    fn alt_b_prev_result() {
        assert_eq!(alt(KEY_B), KeyAction::PrevResult);
    }

    #[test]
    fn ctrl_alt_k_prev_result() {
        assert_eq!(ctrl_alt(KEY_K), KeyAction::PrevResult);
    }

    #[test]
    fn down_next_result() {
        assert_eq!(plain(KEY_DOWN, 0), KeyAction::NextResult);
    }

    #[test]
    fn tab_next_result() {
        assert_eq!(plain(KEY_TAB, 0), KeyAction::NextResult);
    }

    #[test]
    fn alt_l_next_result() {
        assert_eq!(alt(KEY_L), KeyAction::NextResult);
    }

    #[test]
    fn ctrl_j_next_result() {
        assert_eq!(ctrl(KEY_J), KeyAction::NextResult);
    }

    #[test]
    fn alt_j_next_result() {
        assert_eq!(alt(KEY_J), KeyAction::NextResult);
    }

    #[test]
    fn ctrl_n_next_result() {
        assert_eq!(ctrl(KEY_N), KeyAction::NextResult);
    }

    #[test]
    fn alt_n_next_result() {
        assert_eq!(alt(KEY_N), KeyAction::NextResult);
    }

    #[test]
    fn ctrl_f_next_result() {
        assert_eq!(ctrl(KEY_F), KeyAction::NextResult);
    }

    #[test]
    fn alt_f_next_result() {
        assert_eq!(alt(KEY_F), KeyAction::NextResult);
    }

    #[test]
    fn home_resets_selection() {
        assert_eq!(plain(KEY_HOME, 0), KeyAction::ResetSelection);
    }

    #[test]
    fn pageup_prev_page() {
        assert_eq!(plain(KEY_PAGEUP, 0), KeyAction::PrevPage);
    }

    #[test]
    fn pagedown_next_page() {
        assert_eq!(plain(KEY_PAGEDOWN, 0), KeyAction::NextPage);
    }

    // ── Close ─────────────────────────────────────────────────────────────────

    #[test]
    fn esc_closes() {
        assert_eq!(plain(KEY_ESC, 0), KeyAction::Close);
    }

    #[test]
    fn ctrl_c_closes() {
        assert_eq!(ctrl(KEY_C), KeyAction::Close);
    }

    #[test]
    fn ctrl_leftbrace_closes() {
        assert_eq!(ctrl(KEY_LEFTBRACE), KeyAction::Close);
    }

    #[test]
    fn ctrl_g_closes() {
        assert_eq!(ctrl(KEY_G), KeyAction::Close);
    }

    // ── Submit ────────────────────────────────────────────────────────────────

    #[test]
    fn enter_submits() {
        assert_eq!(plain(KEY_ENTER, 0), KeyAction::Submit);
    }

    #[test]
    fn kpenter_submits() {
        assert_eq!(plain(KEY_KPENTER, 0), KeyAction::Submit);
    }

    #[test]
    fn ctrl_m_submits() {
        assert_eq!(ctrl(KEY_M), KeyAction::Submit);
    }

    // ── Unknown / no-ops ─────────────────────────────────────────────────────

    #[test]
    fn unbound_key_is_unknown() {
        // KEY_F1 = 59, not bound to anything.
        assert_eq!(plain(59, 0), KeyAction::Unknown);
    }

    #[test]
    fn plain_g_not_close() {
        // 'g' without ctrl is a printable insert, not close.
        assert_eq!(
            classify_keypress(false, false, false, KEY_G, 'g' as u32),
            KeyAction::InsertChar('g')
        );
    }
}

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
