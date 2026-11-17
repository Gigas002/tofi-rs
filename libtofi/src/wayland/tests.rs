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
