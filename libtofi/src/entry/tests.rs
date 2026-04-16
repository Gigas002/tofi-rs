//! Unit tests for `libtofi::entry`.
//!
//! Creates in-memory ARGB buffers (no Wayland display needed) and verifies
//! that [`Entry`] draws non-zero pixels and produces sane clip rectangles.

#[cfg(feature = "renderer")]
mod entry_tests {
    use crate::entry::{Entry, EntryConfig};

    /// Allocate a double-buffered ARGB buffer and return (`Vec<u8>`, ptr).
    fn make_buf(w: u32, h: u32) -> Vec<u8> {
        vec![0u8; (w * h * 4 * 2) as usize]
    }

    /// Minimal `Entry` with `400 × 200` physical pixels at 1× scale.
    fn make_entry(buf: &mut Vec<u8>, w: u32, h: u32) -> Entry {
        let config = EntryConfig {
            font_name: "Sans".into(),
            font_size: 18,
            border_width: 2,
            outline_width: 1,
            padding_top: 4,
            padding_bottom: 4,
            padding_left: 4,
            padding_right: 4,
            clip_to_padding: true,
            prompt_text: "> ".into(),
            ..EntryConfig::default()
        };
        // SAFETY: buf lives longer than the returned Entry; no aliasing.
        unsafe { Entry::new(buf.as_mut_ptr(), w, h, 120, config).expect("Entry::new") }
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    /// After `Entry::new` the buffer should not be entirely zero.
    #[test]
    fn entry_new_writes_pixels() {
        let (w, h) = (400u32, 200u32);
        let mut buf = make_buf(w, h);
        let entry = make_entry(&mut buf, w, h);
        entry.flush();
        drop(entry);

        let all_zero = buf.iter().all(|&b| b == 0);
        assert!(!all_zero, "Entry::new should paint non-zero pixels");
    }

    /// Clip dimensions must be strictly less than the physical frame.
    #[test]
    fn clip_rect_is_inset() {
        let (w, h) = (400u32, 200u32);
        let mut buf = make_buf(w, h);
        let entry = make_entry(&mut buf, w, h);

        assert!(
            entry.clip_width < w,
            "clip_width ({}) should be < width ({})",
            entry.clip_width,
            w
        );
        assert!(
            entry.clip_height < h,
            "clip_height ({}) should be < height ({})",
            entry.clip_height,
            h
        );
    }

    /// Updating the entry with input text should write pixels.
    #[test]
    fn entry_update_with_input_writes_pixels() {
        let (w, h) = (400u32, 200u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        entry.input = "hello world".into();
        entry.cursor_position = entry.input.chars().count();
        entry.update();
        entry.flush();

        let all_zero = buf.iter().all(|&b| b == 0);
        assert!(!all_zero, "entry_update should paint pixels");
    }

    /// Results should be tracked after an update.
    #[test]
    fn entry_update_with_results() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        entry.results = vec!["alpha".into(), "beta".into(), "gamma".into()];
        entry.selection = 1;
        entry.update();
        entry.flush();

        assert!(
            entry.num_results_drawn <= 3,
            "num_results_drawn should be ≤ result count"
        );
    }

    /// 2× scale produces logical dimensions half the physical.
    #[test]
    fn entry_scale_2x() {
        let (pw, ph) = (800u32, 400u32);
        let mut buf = make_buf(pw, ph);
        let config = EntryConfig {
            border_width: 0,
            outline_width: 0,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
            ..EntryConfig::default()
        };
        // SAFETY: buf lives longer than the entry.
        let entry =
            unsafe { Entry::new(buf.as_mut_ptr(), pw, ph, 240, config).expect("2× Entry::new") };

        // With no border/padding the clip should match logical dimensions.
        assert_eq!(entry.clip_width, pw / 2);
        assert_eq!(entry.clip_height, ph / 2);
    }

    /// `ready_index` returns the complement of `index`.
    #[test]
    fn ready_index_is_complement() {
        let (w, h) = (400u32, 200u32);
        let mut buf = make_buf(w, h);
        let entry = make_entry(&mut buf, w, h);
        assert_eq!(entry.ready_index(), entry.index ^ 1);
    }

    // ── Scrolling / selection navigation ─────────────────────────────────────

    /// `reset_selection` zeroes `selection` and `first_result`.
    #[test]
    fn reset_selection_clears_state() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        entry.results = vec!["a".into(), "b".into(), "c".into()];
        entry.selection = 2;
        entry.first_result = 1;
        entry.reset_selection();

        assert_eq!(entry.selection, 0);
        assert_eq!(entry.first_result, 0);
    }

    /// `select_next` increments selection within the visible page.
    #[test]
    fn select_next_increments_within_page() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        entry.results = vec!["a".into(), "b".into(), "c".into()];
        entry.num_results_drawn = 3;
        entry.selection = 0;
        entry.first_result = 0;

        entry.select_next();
        assert_eq!(entry.selection, 1);
        assert_eq!(entry.first_result, 0);

        entry.select_next();
        assert_eq!(entry.selection, 2);
        assert_eq!(entry.first_result, 0);
    }

    /// `select_next` wraps `first_result` forward when reaching page end.
    #[test]
    fn select_next_scrolls_forward() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        // 5 results, page size = 2.
        entry.results = (0..5).map(|i| i.to_string()).collect();
        entry.num_results_drawn = 2;
        entry.selection = 0;
        entry.first_result = 0;

        // Move past the page boundary.
        entry.select_next(); // selection = 1
        entry.select_next(); // selection wraps → 0, first_result += 2 → 2

        assert_eq!(entry.selection, 0);
        assert_eq!(entry.first_result, 2);
    }

    /// `select_next` wraps `first_result` back to 0 when at the end of all results.
    #[test]
    fn select_next_wraps_to_beginning() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        // 3 results, page size = 3 → one page; wrapping goes back to 0.
        entry.results = vec!["a".into(), "b".into(), "c".into()];
        entry.num_results_drawn = 3;
        entry.selection = 2;
        entry.first_result = 0;

        entry.select_next(); // selection wraps → 0, first_result = (0+3) % 3 = 0

        assert_eq!(entry.selection, 0);
        assert_eq!(entry.first_result, 0);
    }

    /// `select_prev` decrements selection within the page.
    #[test]
    fn select_prev_decrements_within_page() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        entry.results = vec!["a".into(), "b".into(), "c".into()];
        entry.num_results_drawn = 3;
        entry.selection = 2;
        entry.first_result = 0;

        entry.select_prev();
        assert_eq!(entry.selection, 1);
        assert_eq!(entry.first_result, 0);
    }

    /// `select_prev` scrolls `first_result` back when at the top of the page and
    /// there are previous results to show.
    #[test]
    fn select_prev_scrolls_backward() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        // 5 results, page size = 2; we are on the second page.
        entry.results = (0..5).map(|i| i.to_string()).collect();
        entry.num_results_drawn = 2;
        entry.last_num_results_drawn = 2;
        entry.selection = 0;
        entry.first_result = 2; // showing results [2, 3]

        entry.select_prev(); // first_result goes back; selection = last-1

        // first_result (2) == nsel (2), not > nsel, so falls into the `> 0` branch.
        assert_eq!(entry.first_result, 0);
        assert_eq!(entry.selection, 1); // first_result was 2, so selection = 2 - 1 = 1
    }

    /// `select_prev` at the very first result (selection=0, first_result=0) is a
    /// no-op — the caller stays at position 0.
    #[test]
    fn select_prev_noop_at_start() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        entry.results = vec!["a".into(), "b".into()];
        entry.num_results_drawn = 2;
        entry.selection = 0;
        entry.first_result = 0;

        entry.select_prev();

        assert_eq!(entry.selection, 0);
        assert_eq!(entry.first_result, 0);
    }

    /// `select_next_page` advances `first_result` by `num_results_drawn`.
    #[test]
    fn select_next_page_advances_window() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        entry.results = (0..10).map(|i| i.to_string()).collect();
        entry.num_results_drawn = 3;
        entry.selection = 1;
        entry.first_result = 0;

        entry.select_next_page();

        assert_eq!(entry.selection, 0);
        assert_eq!(entry.first_result, 3);
        assert_eq!(entry.last_num_results_drawn, 3);
    }

    /// `select_next_page` wraps back to 0 when past the end.
    #[test]
    fn select_next_page_wraps() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        entry.results = (0..4).map(|i| i.to_string()).collect();
        entry.num_results_drawn = 3;
        entry.selection = 0;
        entry.first_result = 2; // 2 + 3 = 5 ≥ 4 → wraps to 0

        entry.select_next_page();

        assert_eq!(entry.first_result, 0);
        assert_eq!(entry.selection, 0);
    }

    /// `select_prev_page` moves `first_result` back by `last_num_results_drawn`.
    #[test]
    fn select_prev_page_moves_back() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        entry.results = (0..10).map(|i| i.to_string()).collect();
        entry.num_results_drawn = 3;
        entry.last_num_results_drawn = 3;
        entry.selection = 2;
        entry.first_result = 6;

        entry.select_prev_page();

        assert_eq!(entry.first_result, 3);
        assert_eq!(entry.selection, 0);
    }

    /// `select_prev_page` clamps to 0 when `first_result` < `last_num_results_drawn`.
    #[test]
    fn select_prev_page_clamps_to_zero() {
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);

        entry.results = (0..10).map(|i| i.to_string()).collect();
        entry.num_results_drawn = 3;
        entry.last_num_results_drawn = 5;
        entry.first_result = 2; // 2 < 5 → clamps to 0

        entry.select_prev_page();

        assert_eq!(entry.first_result, 0);
        assert_eq!(entry.selection, 0);
    }

    // ── Additional rendering paths ────────────────────────────────────────────

    /// hide_input = true replaces text with hidden character.
    #[test]
    fn entry_update_hide_input() {
        let (w, h) = (400u32, 200u32);
        let mut buf = make_buf(w, h);
        let config = EntryConfig {
            font_name: "Sans".into(),
            font_size: 18,
            border_width: 2,
            outline_width: 1,
            padding_top: 4,
            padding_bottom: 4,
            padding_left: 4,
            padding_right: 4,
            clip_to_padding: true,
            prompt_text: "> ".into(),
            hide_input: true,
            hidden_character: "*".into(),
            ..EntryConfig::default()
        };
        let mut entry =
            unsafe { Entry::new(buf.as_mut_ptr(), w, h, 120, config).expect("Entry::new") };
        entry.input = "secret".into();
        entry.cursor_position = entry.input.chars().count();
        entry.update();
        entry.flush();
        let all_zero = buf.iter().all(|&b| b == 0);
        assert!(!all_zero, "hide_input should still paint pixels");
    }

    /// Selection highlight with non-transparent color triggers render_selected_result.
    #[test]
    fn entry_update_with_selection_highlight() {
        use crate::color::Color;
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let config = EntryConfig {
            font_name: "Sans".into(),
            font_size: 18,
            border_width: 2,
            outline_width: 1,
            padding_top: 4,
            padding_bottom: 4,
            padding_left: 4,
            padding_right: 4,
            clip_to_padding: true,
            num_results: 5,
            selection_highlight_color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            ..EntryConfig::default()
        };
        let mut entry =
            unsafe { Entry::new(buf.as_mut_ptr(), w, h, 120, config).expect("Entry::new") };
        entry.results = vec!["hello".into()];
        entry.selection = 0;
        entry.num_results_drawn = 1;
        entry.update();
        entry.flush();
        let all_zero = buf.iter().all(|&b| b == 0);
        assert!(!all_zero, "selection highlight should paint pixels");
    }

    /// Horizontal layout renders without panic.
    #[test]
    fn entry_update_horizontal() {
        let (w, h) = (800u32, 100u32);
        let mut buf = make_buf(w, h);
        let config = EntryConfig {
            font_name: "Sans".into(),
            font_size: 16,
            border_width: 2,
            outline_width: 1,
            padding_top: 4,
            padding_bottom: 4,
            padding_left: 4,
            padding_right: 4,
            clip_to_padding: true,
            horizontal: true,
            num_results: 5,
            ..EntryConfig::default()
        };
        let mut entry =
            unsafe { Entry::new(buf.as_mut_ptr(), w, h, 120, config).expect("Entry::new") };
        entry.results = vec!["alpha".into(), "beta".into(), "gamma".into()];
        entry.selection = 0;
        entry.update();
        entry.flush();
        let all_zero = buf.iter().all(|&b| b == 0);
        assert!(!all_zero, "horizontal layout should paint pixels");
    }

    /// Non-zero cursor position renders correctly.
    #[test]
    fn entry_update_cursor_middle() {
        let (w, h) = (400u32, 200u32);
        let mut buf = make_buf(w, h);
        let mut entry = make_entry(&mut buf, w, h);
        entry.input = "hello".into();
        entry.cursor_position = 2;
        entry.update();
        entry.flush();
        let all_zero = buf.iter().all(|&b| b == 0);
        assert!(!all_zero, "cursor in middle should paint pixels");
    }

    /// Block cursor style renders without panic.
    #[test]
    fn entry_update_block_cursor() {
        use crate::entry::{CursorStyle, CursorTheme};
        let (w, h) = (400u32, 200u32);
        let mut buf = make_buf(w, h);
        let config = EntryConfig {
            font_name: "Sans".into(),
            font_size: 18,
            border_width: 2,
            outline_width: 1,
            padding_top: 4,
            padding_bottom: 4,
            padding_left: 4,
            padding_right: 4,
            clip_to_padding: true,
            cursor_theme: CursorTheme {
                style: CursorStyle::Block,
                show: true,
                ..CursorTheme::default()
            },
            ..EntryConfig::default()
        };
        let mut entry =
            unsafe { Entry::new(buf.as_mut_ptr(), w, h, 120, config).expect("Entry::new") };
        entry.input = "abc".into();
        entry.cursor_position = 1;
        entry.update();
        entry.flush();
        let all_zero = buf.iter().all(|&b| b == 0);
        assert!(!all_zero, "block cursor should paint pixels");
    }

    /// Underscore cursor style renders without panic.
    #[test]
    fn entry_update_underscore_cursor() {
        use crate::entry::{CursorStyle, CursorTheme};
        let (w, h) = (400u32, 200u32);
        let mut buf = make_buf(w, h);
        let config = EntryConfig {
            font_name: "Sans".into(),
            font_size: 18,
            border_width: 2,
            outline_width: 1,
            padding_top: 4,
            padding_bottom: 4,
            padding_left: 4,
            padding_right: 4,
            clip_to_padding: true,
            cursor_theme: CursorTheme {
                style: CursorStyle::Underscore,
                show: true,
                ..CursorTheme::default()
            },
            ..EntryConfig::default()
        };
        let mut entry =
            unsafe { Entry::new(buf.as_mut_ptr(), w, h, 120, config).expect("Entry::new") };
        entry.input = "abc".into();
        entry.cursor_position = 3;
        entry.update();
        entry.flush();
        let all_zero = buf.iter().all(|&b| b == 0);
        assert!(!all_zero, "underscore cursor should paint pixels");
    }

    /// Selection theme with a non-transparent background exercises the themed background path.
    #[test]
    fn entry_update_selection_theme_with_background() {
        use crate::color::Color;
        use crate::entry::TextTheme;
        let (w, h) = (600u32, 400u32);
        let mut buf = make_buf(w, h);
        let config = EntryConfig {
            font_name: "Sans".into(),
            font_size: 18,
            border_width: 2,
            outline_width: 1,
            padding_top: 4,
            padding_bottom: 4,
            padding_left: 4,
            padding_right: 4,
            clip_to_padding: true,
            num_results: 5,
            selection_theme: TextTheme {
                background_color: Some(Color {
                    r: 0.2,
                    g: 0.4,
                    b: 0.8,
                    a: 1.0,
                }),
                ..TextTheme::default()
            },
            ..EntryConfig::default()
        };
        let mut entry =
            unsafe { Entry::new(buf.as_mut_ptr(), w, h, 120, config).expect("Entry::new") };
        entry.results = vec!["one".into(), "two".into(), "three".into()];
        entry.selection = 1;
        entry.num_results_drawn = 3;
        entry.update();
        entry.flush();
        let all_zero = buf.iter().all(|&b| b == 0);
        assert!(
            !all_zero,
            "selection theme with background should paint pixels"
        );
    }
}

// ── pango_backend tests ───────────────────────────────────────────────────────

#[cfg(feature = "renderer")]
mod pango_backend_tests {
    use super::super::pango_backend::find_match_position;

    #[test]
    fn empty_needle_returns_none() {
        assert_eq!(find_match_position("hello", ""), None);
    }

    #[test]
    fn needle_not_in_haystack_returns_none() {
        assert_eq!(find_match_position("hello", "xyz"), None);
    }

    #[test]
    fn exact_ascii_match() {
        assert_eq!(find_match_position("foo", "foo"), Some((0, 3)));
    }

    #[test]
    fn case_insensitive_world() {
        assert_eq!(find_match_position("Hello World", "world"), Some((6, 11)));
    }

    #[test]
    fn case_insensitive_fire() {
        assert_eq!(find_match_position("Firefox", "fire"), Some((0, 4)));
    }

    #[test]
    fn match_in_middle() {
        assert_eq!(find_match_position("abcdef", "cd"), Some((2, 4)));
    }

    #[test]
    fn needle_longer_than_haystack_returns_none() {
        assert_eq!(find_match_position("hi", "hello world"), None);
    }

    #[test]
    fn unicode_haystack_match_is_some() {
        assert!(find_match_position("café", "afé").is_some());
    }
}
