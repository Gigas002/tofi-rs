use super::*;

// Approximate float equality for channel values.
fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

// ── 6-digit RRGGBB (opaque) ───────────────────────────────────────────────────

#[test]
fn rrggbb_no_hash() {
    let c = Color::from_hex("ff0000").unwrap();
    assert!(approx(c.r, 1.0));
    assert!(approx(c.g, 0.0));
    assert!(approx(c.b, 0.0));
    assert!(approx(c.a, 1.0));
}

#[test]
fn rrggbb_with_hash() {
    let c = Color::from_hex("#00ff00").unwrap();
    assert!(approx(c.r, 0.0));
    assert!(approx(c.g, 1.0));
    assert!(approx(c.b, 0.0));
    assert!(approx(c.a, 1.0));
}

#[test]
fn rrggbb_black() {
    let c = Color::from_hex("#000000").unwrap();
    assert!(approx(c.r, 0.0));
    assert!(approx(c.g, 0.0));
    assert!(approx(c.b, 0.0));
    assert!(approx(c.a, 1.0));
}

#[test]
fn rrggbb_white() {
    let c = Color::from_hex("#ffffff").unwrap();
    assert!(approx(c.r, 1.0));
    assert!(approx(c.g, 1.0));
    assert!(approx(c.b, 1.0));
    assert!(approx(c.a, 1.0));
}

#[test]
fn rrggbb_uppercase() {
    let c = Color::from_hex("#FF8000").unwrap();
    assert!(approx(c.r, 1.0));
    assert!(approx(c.g, 128.0 / 255.0));
    assert!(approx(c.b, 0.0));
    assert!(approx(c.a, 1.0));
}

// ── 8-digit RRGGBBAA ──────────────────────────────────────────────────────────

#[test]
fn rrggbbaa_with_hash() {
    let c = Color::from_hex("#ff000080").unwrap();
    assert!(approx(c.r, 1.0));
    assert!(approx(c.g, 0.0));
    assert!(approx(c.b, 0.0));
    assert!(approx(c.a, 128.0 / 255.0));
}

#[test]
fn rrggbbaa_fully_transparent() {
    let c = Color::from_hex("#ffffff00").unwrap();
    assert!(approx(c.a, 0.0));
}

#[test]
fn rrggbbaa_no_hash() {
    let c = Color::from_hex("aabbccdd").unwrap();
    assert!(approx(c.r, 0xAA as f32 / 255.0));
    assert!(approx(c.g, 0xBB as f32 / 255.0));
    assert!(approx(c.b, 0xCC as f32 / 255.0));
    assert!(approx(c.a, 0xDD as f32 / 255.0));
}

// ── 3-digit RGB (shorthand) ───────────────────────────────────────────────────

#[test]
fn rgb_shorthand_no_hash() {
    // "f00" → "ff0000" + alpha FF
    let c = Color::from_hex("f00").unwrap();
    assert!(approx(c.r, 1.0));
    assert!(approx(c.g, 0.0));
    assert!(approx(c.b, 0.0));
    assert!(approx(c.a, 1.0));
}

#[test]
fn rgb_shorthand_with_hash() {
    // "#abc" → "#aabbcc" + alpha FF
    let c = Color::from_hex("#abc").unwrap();
    assert!(approx(c.r, 0xAA as f32 / 255.0));
    assert!(approx(c.g, 0xBB as f32 / 255.0));
    assert!(approx(c.b, 0xCC as f32 / 255.0));
    assert!(approx(c.a, 1.0));
}

#[test]
fn rgb_shorthand_white() {
    let c = Color::from_hex("#fff").unwrap();
    assert!(approx(c.r, 1.0));
    assert!(approx(c.g, 1.0));
    assert!(approx(c.b, 1.0));
    assert!(approx(c.a, 1.0));
}

// ── 4-digit RGBA (shorthand) ──────────────────────────────────────────────────

#[test]
fn rgba_shorthand_no_hash() {
    // "f00f" → "ff0000ff"
    let c = Color::from_hex("f00f").unwrap();
    assert!(approx(c.r, 1.0));
    assert!(approx(c.g, 0.0));
    assert!(approx(c.b, 0.0));
    assert!(approx(c.a, 1.0));
}

#[test]
fn rgba_shorthand_semi_transparent() {
    // "#f008" → "#ff000088"
    let c = Color::from_hex("#f008").unwrap();
    assert!(approx(c.r, 1.0));
    assert!(approx(c.g, 0.0));
    assert!(approx(c.b, 0.0));
    assert!(approx(c.a, 0x88 as f32 / 255.0));
}

// ── FromStr ───────────────────────────────────────────────────────────────────

#[test]
fn from_str_trait() {
    let c: Color = "#ff0000".parse().unwrap();
    assert!(approx(c.r, 1.0));
}

// ── Error cases ───────────────────────────────────────────────────────────────

#[test]
fn invalid_length_empty() {
    assert!(Color::from_hex("").is_err());
}

#[test]
fn invalid_length_five() {
    assert!(Color::from_hex("12345").is_err());
}

#[test]
fn invalid_length_seven() {
    assert!(Color::from_hex("#1234567").is_err());
}

#[test]
fn invalid_hex_digits() {
    assert!(Color::from_hex("#zzzzzz").is_err());
}

#[test]
fn invalid_hex_digits_short() {
    assert!(Color::from_hex("#ggg").is_err());
}
