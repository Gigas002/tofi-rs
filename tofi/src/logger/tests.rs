use super::filter;

#[test]
fn level_becomes_the_filter_without_rust_log() {
    // RUST_LOG would take precedence; the filter is only predictable without it.
    if std::env::var_os("RUST_LOG").is_some() {
        return;
    }
    assert_eq!(filter("debug").to_string(), "debug");
    assert_eq!(filter("warn").to_string(), "warn");
}
