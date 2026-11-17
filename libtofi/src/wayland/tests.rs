use super::{logical_size, rotated_size};
use wayland_client::protocol::wl_output::Transform;

#[test]
fn rotation_by_90_or_270_swaps_the_size() {
    assert_eq!(rotated_size(1920, 1080, Transform::Normal), (1920, 1080));
    assert_eq!(rotated_size(1920, 1080, Transform::_180), (1920, 1080));
    assert_eq!(rotated_size(1920, 1080, Transform::_90), (1080, 1920));
    assert_eq!(
        rotated_size(1920, 1080, Transform::Flipped270),
        (1080, 1920)
    );
}

#[test]
fn negative_mode_sizes_count_as_zero() {
    assert_eq!(rotated_size(-1, 1080, Transform::Normal), (0, 1080));
}

#[test]
fn xdg_output_gives_the_fractional_logical_size() {
    // 1920×1080 at 1.5: wl_output can only say "scale 2".
    assert_eq!(
        logical_size((1920, 1080), 2, Some((1280, 720))),
        (1280, 720)
    );
}

#[test]
fn without_xdg_output_the_integer_scale_divides() {
    assert_eq!(logical_size((3840, 2160), 2, None), (1920, 1080));
    assert_eq!(logical_size((1920, 1080), 1, None), (1920, 1080));
}

#[test]
fn bogus_scales_and_sizes_fall_back() {
    assert_eq!(logical_size((1920, 1080), 0, None), (1920, 1080));
    assert_eq!(logical_size((1920, 1080), 1, Some((0, 0))), (1920, 1080));
}

#[cfg(feature = "clipboard")]
mod clipboard {
    use super::super::clipboard::decode_chunk;

    #[test]
    fn whole_text_decodes_at_once() {
        let mut pending = Vec::new();
        assert_eq!(decode_chunk(&mut pending, "héllo".as_bytes()), "héllo");
        assert!(pending.is_empty());
    }

    #[test]
    fn a_character_split_across_reads_is_kept_whole() {
        let bytes = "aé€😀b".as_bytes();
        // Split at every position, including inside each multi-byte character.
        for at in 0..=bytes.len() {
            let mut pending = Vec::new();
            let mut text = decode_chunk(&mut pending, &bytes[..at]);
            text += &decode_chunk(&mut pending, &bytes[at..]);
            assert_eq!(text, "aé€😀b", "split at {at}");
            assert!(pending.is_empty());
        }
    }

    #[test]
    fn invalid_bytes_are_dropped() {
        let mut pending = Vec::new();
        assert_eq!(decode_chunk(&mut pending, b"a\xffb\xc3("), "ab(");
        assert!(pending.is_empty());
    }
}
