use super::*;

// ── Constants ─────────────────────────────────────────────────────────────────

#[test]
fn max_input_length_value() {
    assert_eq!(MAX_INPUT_LENGTH, 256);
}

// ── utf8_validate ─────────────────────────────────────────────────────────────

#[test]
fn validate_ascii() {
    assert!(utf8_validate(b"hello"));
}

#[test]
fn validate_multibyte() {
    assert!(utf8_validate("héllo".as_bytes()));
}

#[test]
fn validate_empty() {
    assert!(utf8_validate(b""));
}

#[test]
fn validate_invalid_sequence() {
    assert!(!utf8_validate(&[0xFF, 0xFE]));
}

#[test]
fn validate_truncated_multibyte() {
    // First byte of a 2-byte sequence without continuation byte.
    assert!(!utf8_validate(&[0xC3]));
}

// ── utf8_to_utf32 ─────────────────────────────────────────────────────────────

#[test]
fn utf8_to_utf32_ascii() {
    assert_eq!(utf8_to_utf32("A"), Some('A'));
}

#[test]
fn utf8_to_utf32_multibyte() {
    assert_eq!(utf8_to_utf32("é"), Some('é'));
}

#[test]
fn utf8_to_utf32_emoji() {
    assert_eq!(utf8_to_utf32("🦀"), Some('🦀'));
}

#[test]
fn utf8_to_utf32_empty() {
    assert_eq!(utf8_to_utf32(""), None);
}

// ── utf8_to_utf32_validate ────────────────────────────────────────────────────

#[test]
fn validate_and_decode_valid() {
    assert_eq!(utf8_to_utf32_validate("é".as_bytes()), Some('é'));
}

#[test]
fn validate_and_decode_invalid() {
    assert_eq!(utf8_to_utf32_validate(&[0xFF, 0xFE]), None);
}

// ── utf32_to_utf8 ─────────────────────────────────────────────────────────────

#[test]
fn encode_ascii() {
    let (buf, len) = utf32_to_utf8('A');
    assert_eq!(len, 1);
    assert_eq!(&buf[..len], b"A");
}

#[test]
fn encode_two_byte() {
    let (buf, len) = utf32_to_utf8('é');
    assert_eq!(len, 2);
    assert_eq!(&buf[..len], "é".as_bytes());
}

#[test]
fn encode_four_byte() {
    let (buf, len) = utf32_to_utf8('🦀');
    assert_eq!(len, 4);
    assert_eq!(&buf[..len], "🦀".as_bytes());
}

// ── utf8_string_to_utf32_string ───────────────────────────────────────────────

#[test]
fn string_to_codepoints() {
    let v = utf8_string_to_utf32_string("héllo");
    assert_eq!(v, vec!['h', 'é', 'l', 'l', 'o']);
}

#[test]
fn string_to_codepoints_empty() {
    assert_eq!(utf8_string_to_utf32_string(""), vec![]);
}

// ── utf8_strlen ───────────────────────────────────────────────────────────────

#[test]
fn strlen_ascii() {
    assert_eq!(utf8_strlen("hello"), 5);
}

#[test]
fn strlen_multibyte() {
    // "héllo" = h(1) é(2 bytes) l l o → 5 codepoints
    assert_eq!(utf8_strlen("héllo"), 5);
}

#[test]
fn strlen_emoji() {
    assert_eq!(utf8_strlen("🦀🦀"), 2);
}

#[test]
fn strlen_empty() {
    assert_eq!(utf8_strlen(""), 0);
}

// ── utf8_next_char ────────────────────────────────────────────────────────────

#[test]
fn next_char_ascii() {
    assert_eq!(utf8_next_char("abc"), Some(('a', "bc")));
}

#[test]
fn next_char_multibyte() {
    let (c, rest) = utf8_next_char("ébc").unwrap();
    assert_eq!(c, 'é');
    assert_eq!(rest, "bc");
}

#[test]
fn next_char_single() {
    assert_eq!(utf8_next_char("x"), Some(('x', "")));
}

#[test]
fn next_char_empty() {
    assert_eq!(utf8_next_char(""), None);
}

// ── utf8_strchr ───────────────────────────────────────────────────────────────

#[test]
fn strchr_found() {
    assert_eq!(utf8_strchr("hello", 'l'), Some(2));
}

#[test]
fn strchr_not_found() {
    assert_eq!(utf8_strchr("hello", 'z'), None);
}

#[test]
fn strchr_multibyte() {
    let s = "héllo";
    let offset = utf8_strchr(s, 'é').unwrap();
    assert_eq!(&s[offset..].chars().next(), &Some('é'));
}

// ── utf8_strcasechr ───────────────────────────────────────────────────────────

#[test]
fn strcasechr_same_case() {
    assert!(utf8_strcasechr("Hello", 'h').is_some());
}

#[test]
fn strcasechr_different_case() {
    assert!(utf8_strcasechr("Hello", 'H').is_some());
}

#[test]
fn strcasechr_not_found() {
    assert_eq!(utf8_strcasechr("Hello", 'z'), None);
}

// ── utf8_strcasestr ───────────────────────────────────────────────────────────

#[test]
fn strcasestr_exact() {
    assert!(utf8_strcasestr("Firefox", "fire").is_some());
}

#[test]
fn strcasestr_case_mismatch() {
    assert!(utf8_strcasestr("Firefox", "FIRE").is_some());
}

#[test]
fn strcasestr_not_found() {
    assert_eq!(utf8_strcasestr("Firefox", "chrome"), None);
}

#[test]
fn strcasestr_empty_needle() {
    assert_eq!(utf8_strcasestr("anything", ""), Some(0));
}

// ── character classification ──────────────────────────────────────────────────

#[test]
fn isprint_printable() {
    assert!(utf32_isprint('A'));
    assert!(utf32_isprint('é'));
    assert!(utf32_isprint(' '));
}

#[test]
fn isprint_control() {
    assert!(!utf32_isprint('\x00'));
    assert!(!utf32_isprint('\n'));
    assert!(!utf32_isprint('\t'));
}

#[test]
fn isspace_whitespace() {
    assert!(utf32_isspace(' '));
    assert!(utf32_isspace('\t'));
    assert!(utf32_isspace('\n'));
}

#[test]
fn isspace_non_whitespace() {
    assert!(!utf32_isspace('A'));
}

#[test]
fn isupper_upper() {
    assert!(utf32_isupper('A'));
    assert!(utf32_isupper('É'));
}

#[test]
fn isupper_lower() {
    assert!(!utf32_isupper('a'));
}

#[test]
fn islower_lower() {
    assert!(utf32_islower('a'));
    assert!(utf32_islower('é'));
}

#[test]
fn islower_upper() {
    assert!(!utf32_islower('A'));
}

#[test]
fn isalnum_alpha() {
    assert!(utf32_isalnum('a'));
    assert!(utf32_isalnum('Z'));
}

#[test]
fn isalnum_digit() {
    assert!(utf32_isalnum('5'));
}

#[test]
fn isalnum_symbol() {
    assert!(!utf32_isalnum('!'));
}

#[test]
fn toupper_ascii() {
    assert_eq!(utf32_toupper('a'), 'A');
}

#[test]
fn toupper_already_upper() {
    assert_eq!(utf32_toupper('A'), 'A');
}

#[test]
fn tolower_ascii() {
    assert_eq!(utf32_tolower('A'), 'a');
}

#[test]
fn tolower_already_lower() {
    assert_eq!(utf32_tolower('a'), 'a');
}

// ── NFC normalization ─────────────────────────────────────────────────────────

#[test]
fn normalize_already_nfc() {
    // "é" as a single precomposed codepoint (U+00E9) is already NFC.
    let s = "\u{00E9}";
    assert_eq!(utf8_normalize(s), s);
}

#[test]
fn normalize_nfd_to_nfc() {
    // NFD form: e (U+0065) + combining acute (U+0301)
    let nfd = "\u{0065}\u{0301}";
    let nfc = utf8_normalize(nfd);
    assert_eq!(nfc, "\u{00E9}");
}

#[test]
fn compose_nfd_to_nfc() {
    let nfd = "\u{0065}\u{0301}";
    assert_eq!(utf8_compose(nfd), "\u{00E9}");
}

#[test]
fn normalize_ascii_unchanged() {
    assert_eq!(utf8_normalize("hello"), "hello");
}
