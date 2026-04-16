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

// ── keyboard tests ────────────────────────────────────────────────────────────

#[cfg(feature = "wayland")]
mod keyboard_tests {
    use std::time::{Duration, Instant};

    use xkbcommon::xkb::{Keysym, keysyms};

    use super::super::keyboard::{KeyboardState, RepeatInfo, keysym_to_linux_key};
    use super::super::{
        KEY_B, KEY_BACKSPACE, KEY_C, KEY_DOWN, KEY_ENTER, KEY_ESC, KEY_F, KEY_G, KEY_H, KEY_HOME,
        KEY_J, KEY_K, KEY_KPENTER, KEY_L, KEY_LEFT, KEY_LEFTBRACE, KEY_M, KEY_N, KEY_P,
        KEY_PAGEDOWN, KEY_PAGEUP, KEY_RIGHT, KEY_TAB, KEY_U, KEY_UP, KEY_V, KEY_W,
    };

    fn sym(raw: u32) -> Keysym {
        Keysym::new(raw)
    }

    // ── RepeatInfo ────────────────────────────────────────────────────────────

    #[test]
    fn repeat_info_default_fields() {
        let r = RepeatInfo::default();
        assert_eq!(r.rate, 0);
        assert_eq!(r.delay_ms, 200);
        assert!(!r.active);
        assert_eq!(r.keycode, 0);
    }

    #[test]
    fn timeout_inactive_is_none() {
        let r = RepeatInfo::default();
        assert!(r.timeout().is_none());
    }

    #[test]
    fn timeout_active_rate_zero_is_none() {
        let mut r = RepeatInfo::default();
        r.active = true;
        r.rate = 0;
        assert!(r.timeout().is_none());
    }

    #[test]
    fn timeout_future_deadline_some_nonzero() {
        let mut r = RepeatInfo::default();
        r.active = true;
        r.rate = 30;
        r.next = Instant::now() + Duration::from_secs(10);
        let t = r.timeout();
        assert!(t.is_some());
        assert!(t.unwrap() > Duration::ZERO);
    }

    #[test]
    fn timeout_past_deadline_some_zero() {
        let mut r = RepeatInfo::default();
        r.active = true;
        r.rate = 30;
        r.next = Instant::now() - Duration::from_secs(1);
        assert_eq!(r.timeout(), Some(Duration::ZERO));
    }

    // ── KeyboardState ─────────────────────────────────────────────────────────

    #[test]
    fn new_physical_does_not_panic() {
        let _ = KeyboardState::new(true);
    }

    #[test]
    fn new_logical_does_not_panic() {
        let _ = KeyboardState::new(false);
    }

    #[test]
    fn is_ready_false_before_keymap() {
        assert!(!KeyboardState::new(true).is_ready());
    }

    #[test]
    fn key_get_utf32_zero_before_keymap() {
        assert_eq!(KeyboardState::new(true).key_get_utf32(42), 0);
    }

    #[test]
    fn mod_ctrl_false_before_keymap() {
        assert!(!KeyboardState::new(true).mod_ctrl());
    }

    #[test]
    fn mod_alt_false_before_keymap() {
        assert!(!KeyboardState::new(true).mod_alt());
    }

    #[test]
    fn mod_shift_false_before_keymap() {
        assert!(!KeyboardState::new(true).mod_shift());
    }

    #[test]
    fn key_repeats_false_before_keymap() {
        assert!(!KeyboardState::new(true).key_repeats(42));
    }

    #[test]
    fn set_repeat_info_updates_fields() {
        let mut ks = KeyboardState::new(true);
        ks.set_repeat_info(30, 300);
        assert_eq!(ks.repeat.rate, 30);
        assert_eq!(ks.repeat.delay_ms, 300);
    }

    #[test]
    fn arm_repeat_sets_active_and_keycode() {
        let mut ks = KeyboardState::new(true);
        ks.set_repeat_info(30, 200);
        let before = Instant::now();
        ks.arm_repeat(42);
        assert!(ks.repeat.active);
        assert_eq!(ks.repeat.keycode, 42);
        assert!(ks.repeat.next >= before + Duration::from_millis(200));
    }

    #[test]
    fn disarm_repeat_matching_keycode_clears_active() {
        let mut ks = KeyboardState::new(true);
        ks.arm_repeat(42);
        ks.disarm_repeat(42);
        assert!(!ks.repeat.active);
    }

    #[test]
    fn disarm_repeat_nonmatching_keycode_stays_active() {
        let mut ks = KeyboardState::new(true);
        ks.arm_repeat(42);
        ks.disarm_repeat(99);
        assert!(ks.repeat.active);
    }

    #[test]
    fn advance_repeat_moves_next_forward() {
        let mut ks = KeyboardState::new(true);
        ks.set_repeat_info(10, 200);
        ks.arm_repeat(42);
        let before = ks.repeat.next;
        ks.advance_repeat();
        assert!(ks.repeat.next > before);
    }

    #[test]
    fn advance_repeat_noop_when_rate_zero() {
        let mut ks = KeyboardState::new(true);
        ks.set_repeat_info(0, 200);
        ks.arm_repeat(42);
        let before = ks.repeat.next;
        ks.advance_repeat();
        assert_eq!(ks.repeat.next, before);
    }

    #[test]
    fn keycode_to_linux_key_physical() {
        let ks = KeyboardState::new(true);
        assert_eq!(ks.keycode_to_linux_key(36), 28);
    }

    #[test]
    fn keycode_to_linux_key_logical_no_state_fallback() {
        let ks = KeyboardState::new(false);
        assert_eq!(ks.keycode_to_linux_key(36), 28);
    }

    // ── keysym_to_linux_key — every arm ──────────────────────────────────────

    #[test]
    fn keysym_backspace() {
        assert_eq!(
            keysym_to_linux_key(sym(keysyms::KEY_BackSpace)),
            KEY_BACKSPACE
        );
    }
    #[test]
    fn keysym_w() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_w)), KEY_W);
    }
    #[test]
    fn keysym_u() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_u)), KEY_U);
    }
    #[test]
    fn keysym_v() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_v)), KEY_V);
    }
    #[test]
    fn keysym_left() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_Left)), KEY_LEFT);
    }
    #[test]
    fn keysym_right() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_Right)), KEY_RIGHT);
    }
    #[test]
    fn keysym_up() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_Up)), KEY_UP);
    }
    #[test]
    fn keysym_iso_left_tab() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_ISO_Left_Tab)), KEY_TAB);
    }
    #[test]
    fn keysym_h() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_h)), KEY_H);
    }
    #[test]
    fn keysym_k() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_k)), KEY_K);
    }
    #[test]
    fn keysym_p() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_p)), KEY_P);
    }
    #[test]
    fn keysym_down() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_Down)), KEY_DOWN);
    }
    #[test]
    fn keysym_tab() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_Tab)), KEY_TAB);
    }
    #[test]
    fn keysym_l() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_l)), KEY_L);
    }
    #[test]
    fn keysym_j() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_j)), KEY_J);
    }
    #[test]
    fn keysym_n() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_n)), KEY_N);
    }
    #[test]
    fn keysym_b() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_b)), KEY_B);
    }
    #[test]
    fn keysym_f() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_f)), KEY_F);
    }
    #[test]
    fn keysym_home() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_Home)), KEY_HOME);
    }
    #[test]
    fn keysym_page_up() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_Page_Up)), KEY_PAGEUP);
    }
    #[test]
    fn keysym_page_down() {
        assert_eq!(
            keysym_to_linux_key(sym(keysyms::KEY_Page_Down)),
            KEY_PAGEDOWN
        );
    }
    #[test]
    fn keysym_escape() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_Escape)), KEY_ESC);
    }
    #[test]
    fn keysym_c() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_c)), KEY_C);
    }
    #[test]
    fn keysym_bracketleft() {
        assert_eq!(
            keysym_to_linux_key(sym(keysyms::KEY_bracketleft)),
            KEY_LEFTBRACE
        );
    }
    #[test]
    fn keysym_g() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_g)), KEY_G);
    }
    #[test]
    fn keysym_return() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_Return)), KEY_ENTER);
    }
    #[test]
    fn keysym_kp_enter() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_KP_Enter)), KEY_KPENTER);
    }
    #[test]
    fn keysym_m() {
        assert_eq!(keysym_to_linux_key(sym(keysyms::KEY_m)), KEY_M);
    }
    #[test]
    fn keysym_unmapped_returns_max() {
        assert_eq!(keysym_to_linux_key(sym(0x1008_FF14)), u32::MAX);
    }
}
