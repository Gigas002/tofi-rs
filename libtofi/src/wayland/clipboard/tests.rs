use super::decode_chunk;

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
