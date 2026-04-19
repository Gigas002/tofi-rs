use super::*;

// ---------------------------------------------------------------------------
// resolve_path
// ---------------------------------------------------------------------------

#[test]
fn resolve_absolute_unchanged() {
    let result = resolve_path("/etc/tofi/themes/theme.toml", None);
    assert_eq!(
        result,
        Some(std::path::PathBuf::from("/etc/tofi/themes/theme.toml"))
    );
}

#[test]
fn resolve_relative_with_slash_uses_config_dir() {
    let dir = std::path::Path::new("/home/user/.config/tofi");
    let result = resolve_path("themes/theme.toml", Some(dir));
    assert_eq!(result, Some(dir.join("themes/theme.toml")));
}

// ---------------------------------------------------------------------------
// Dim parsing
// ---------------------------------------------------------------------------

#[test]
fn dim_integer_becomes_pixels() {
    assert_eq!(dim_to_unit(&Dim::Int(42)), UnitValue::pixels(42));
}

#[test]
fn dim_percent_string_becomes_percent() {
    assert_eq!(
        dim_to_unit(&Dim::Str("45%".to_owned())),
        UnitValue::percent(45)
    );
}

#[test]
fn dim_plain_string_becomes_pixels() {
    assert_eq!(
        dim_to_unit(&Dim::Str("100".to_owned())),
        UnitValue::pixels(100)
    );
}

// ---------------------------------------------------------------------------
// parse_hex_color
// ---------------------------------------------------------------------------

#[test]
fn parse_hex_six_digit() {
    let c = parse_hex_color("#FF0000").unwrap();
    assert!((c.r - 1.0).abs() < 1e-3);
    assert!(c.g.abs() < 1e-3);
    assert!(c.b.abs() < 1e-3);
    assert_eq!(c.a, 1.0);
}

#[test]
fn parse_hex_eight_digit_with_alpha() {
    let c = parse_hex_color("#000000bb").unwrap();
    assert!(c.r.abs() < 1e-3);
    assert!((c.a - 187.0 / 255.0).abs() < 1e-2);
}

#[test]
fn parse_hex_invalid_returns_none() {
    assert!(parse_hex_color("notacolor").is_none());
    assert!(parse_hex_color("#ZZZZZZ").is_none());
}

// ---------------------------------------------------------------------------
// TOML round-trips
// ---------------------------------------------------------------------------

#[test]
fn input_color_hyphen_alias() {
    let toml_str = r##"
[text]
input-color = "#00c1e4"
"##;
    let theme: Theme = toml::from_str(toml_str).unwrap();
    assert_eq!(theme.text.input_color.as_deref(), Some("#00c1e4"));
}

#[test]
fn dim_deserializes_string_and_int() {
    let toml_str = r##"
[window]
width = "45%"
padding_bottom = 0
"##;
    let theme: Theme = toml::from_str(toml_str).unwrap();
    assert_eq!(
        dim_to_unit(theme.window.width.as_ref().unwrap()),
        UnitValue::percent(45)
    );
    assert_eq!(
        dim_to_unit(theme.window.padding_bottom.as_ref().unwrap()),
        UnitValue::pixels(0)
    );
}

// ---------------------------------------------------------------------------
// Example themes
// ---------------------------------------------------------------------------

fn examples_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
}

#[test]
fn load_example_theme_parses() {
    let path = examples_dir().join("theme/theme.toml");
    if !path.exists() {
        return;
    }
    let theme = load(&path);

    assert_eq!(theme.results.spacing, Some(25));
    assert_eq!(
        theme.font.name.as_deref(),
        Some("CaskaydiaCove Nerd Font Mono")
    );
    assert_eq!(
        dim_to_unit(theme.window.width.as_ref().unwrap()),
        UnitValue::percent(45)
    );
    assert_eq!(
        dim_to_unit(theme.window.height.as_ref().unwrap()),
        UnitValue::percent(60)
    );
    assert_eq!(theme.window.corner_radius, Some(50));
    assert_eq!(theme.text.input_color.as_deref(), Some("#00c1e4"));
}

#[test]
fn load_minimal_theme_has_all_none() {
    let path = examples_dir().join("theme/minimal.toml");
    if !path.exists() {
        return;
    }
    let theme = load(&path);

    assert!(theme.prompt.text.is_none());
    assert!(theme.prompt.padding.is_none());
    assert!(theme.results.count.is_none());
    assert!(theme.results.spacing.is_none());
    assert!(theme.results.mode.is_none());
    assert!(theme.font.name.is_none());
    assert!(theme.font.size.is_none());
    assert!(theme.window.width.is_none());
    assert!(theme.window.height.is_none());
    assert!(theme.window.corner_radius.is_none());
    assert!(theme.text.color.is_none());
    assert!(theme.text.selection_color.is_none());
    assert!(theme.cursor.hide.is_none());
    assert!(theme.input.cursor.is_none());
}

#[test]
fn load_maximal_theme_sets_all_fields() {
    let path = examples_dir().join("theme/maximal.toml");
    if !path.exists() {
        return;
    }
    let theme = load(&path);

    assert_eq!(theme.prompt.text.as_deref(), Some("> "));
    assert_eq!(theme.prompt.padding, Some(4));
    assert_eq!(theme.results.count, Some(8));
    assert_eq!(theme.results.spacing, Some(6));
    assert_eq!(theme.results.mode.as_deref(), Some("horizontal"));
    assert_eq!(theme.font.name.as_deref(), Some("Monospace"));
    assert_eq!(theme.font.size, Some(16));
    assert_eq!(theme.font.hint, Some(true));
    assert_eq!(theme.window.anchor.as_deref(), Some("top"));
    assert_eq!(theme.window.margin_top, Some(10));
    assert_eq!(
        dim_to_unit(theme.window.width.as_ref().unwrap()),
        UnitValue::percent(80)
    );
    assert_eq!(
        dim_to_unit(theme.window.height.as_ref().unwrap()),
        UnitValue::percent(5)
    );
    assert_eq!(theme.window.corner_radius, Some(4));
    assert_eq!(theme.window.border_width, Some(1));
    assert_eq!(theme.window.clip_to_padding, Some(false));
    assert_eq!(theme.cursor.hide, Some(true));
    assert_eq!(theme.input.cursor, Some(true));
    assert_eq!(theme.input.cursor_style.as_deref(), Some("block"));
    assert_eq!(theme.input.cursor_corner_radius, Some(2));
}
