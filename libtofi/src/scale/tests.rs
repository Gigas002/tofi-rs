use super::{scale_apply, scale_apply_inverse};

#[test]
fn scale_1x_unchanged() {
    assert_eq!(scale_apply(1920, 120), 1920);
    assert_eq!(scale_apply(1080, 120), 1080);
}

#[test]
fn scale_2x_doubles() {
    assert_eq!(scale_apply(1920, 240), 3840);
    assert_eq!(scale_apply(1080, 240), 2160);
}

#[test]
fn scale_1_5x() {
    // 1920 × 1.5 = 2880
    assert_eq!(scale_apply(1920, 180), 2880);
}

#[test]
fn scale_fractional_175() {
    // 1000 × (175/120) = 1458.33… → rounds to 1458
    assert_eq!(scale_apply(1000, 175), 1458);
}

#[test]
fn inverse_1x_unchanged() {
    assert_eq!(scale_apply_inverse(1920, 120), 1920);
}

#[test]
fn inverse_2x_halves() {
    assert_eq!(scale_apply_inverse(3840, 240), 1920);
}

#[test]
fn roundtrip_1_25x() {
    let logical: u32 = 600;
    let scale: u32 = 150; // 1.25×
    let phys = scale_apply(logical, scale);
    assert_eq!(scale_apply_inverse(phys, scale), logical);
}

#[test]
fn scale_zero_base() {
    assert_eq!(scale_apply(0, 180), 0);
    assert_eq!(scale_apply_inverse(0, 180), 0);
}
