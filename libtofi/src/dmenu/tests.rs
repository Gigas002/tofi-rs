use super::read_lines_from;
use std::io::Cursor;

#[test]
fn read_lines_skips_empty_lines() {
    let input = "alpha\n\nbeta\n";
    let items = read_lines_from(Cursor::new(input), false);
    assert_eq!(items, ["alpha", "beta"]);
}

#[test]
fn read_lines_trims_trailing_newlines() {
    let input = "one\r\n\r\ntwo\n";
    let items = read_lines_from(Cursor::new(input), false);
    assert_eq!(items, ["one", "two"]);
}

#[test]
fn read_lines_normalizes_when_requested() {
    // LATIN SMALL LETTER A + COMBINING GRAVE ACCENT (NFD)
    let input = "a\u{0300}\n";
    let items = read_lines_from(Cursor::new(input), true);
    // NFC: precomposed LATIN SMALL LETTER A WITH GRAVE
    assert_eq!(items, ["\u{00e0}"]);
}
