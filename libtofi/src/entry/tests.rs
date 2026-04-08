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
}
