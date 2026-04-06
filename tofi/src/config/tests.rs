use std::io::Write as _;

use super::load::{apply_key, load};
use super::*;

// ---------------------------------------------------------------------------
// UnitValue
// ---------------------------------------------------------------------------

#[test]
fn unit_value_pixels() {
    let v = UnitValue::pixels(42);
    assert_eq!(v.value, 42);
    assert!(!v.is_percent);
}

#[test]
fn unit_value_percent() {
    let v = UnitValue::percent(50);
    assert_eq!(v.value, 50);
    assert!(v.is_percent);
}

// ---------------------------------------------------------------------------
// Directional
// ---------------------------------------------------------------------------

#[test]
fn directional_default_is_zero() {
    let d = Directional::default();
    assert_eq!(d.top, 0);
    assert_eq!(d.right, 0);
    assert_eq!(d.bottom, 0);
    assert_eq!(d.left, 0);
}

#[test]
fn directional_uniform() {
    let d = Directional::uniform(5);
    assert_eq!(d.top, 5);
    assert_eq!(d.right, 5);
    assert_eq!(d.bottom, 5);
    assert_eq!(d.left, 5);
}

// ---------------------------------------------------------------------------
// HiddenCharacter
// ---------------------------------------------------------------------------

#[test]
fn hidden_character_default_is_asterisk() {
    let hc = HiddenCharacter::default();
    assert_eq!(hc.0, Some('*'));
}

// ---------------------------------------------------------------------------
// CursorTheme
// ---------------------------------------------------------------------------

#[test]
fn cursor_theme_defaults() {
    let ct = CursorTheme::default();
    assert!(!ct.show);
    assert_eq!(ct.style, CursorStyle::Bar);
    assert!(ct.color.is_none());
    assert!(ct.text_color.is_none());
    assert_eq!(ct.corner_radius, 0);
    assert!(ct.thickness.is_none());
}

// ---------------------------------------------------------------------------
// TextTheme
// ---------------------------------------------------------------------------

#[test]
fn text_theme_default_all_none() {
    let tt = TextTheme::default();
    assert!(tt.foreground_color.is_none());
    assert!(tt.background_color.is_none());
    assert!(tt.padding.is_none());
    assert!(tt.background_corner_radius.is_none());
}

// ---------------------------------------------------------------------------
// TofiConfig::default — geometry
// ---------------------------------------------------------------------------

#[test]
fn default_window_size() {
    let cfg = TofiConfig::default();
    assert_eq!(cfg.width, UnitValue::pixels(1280));
    assert_eq!(cfg.height, UnitValue::pixels(720));
}

#[test]
fn default_anchor_is_center() {
    assert_eq!(TofiConfig::default().anchor, Anchor::Center);
}

#[test]
fn default_exclusive_zone_is_minus_one() {
    let cfg = TofiConfig::default();
    assert_eq!(cfg.exclusive_zone, -1);
    assert!(!cfg.exclusive_zone_is_percent);
}

#[test]
fn default_margins_are_zero_pixels() {
    let cfg = TofiConfig::default();
    assert_eq!(cfg.margin_top, UnitValue::pixels(0));
    assert_eq!(cfg.margin_bottom, UnitValue::pixels(0));
    assert_eq!(cfg.margin_left, UnitValue::pixels(0));
    assert_eq!(cfg.margin_right, UnitValue::pixels(0));
}

#[test]
fn default_scale_is_true() {
    assert!(TofiConfig::default().scale);
}

#[test]
fn default_target_output_is_empty() {
    assert!(TofiConfig::default().target_output.is_empty());
}

// ---------------------------------------------------------------------------
// TofiConfig::default — font
// ---------------------------------------------------------------------------

#[test]
fn default_font() {
    let cfg = TofiConfig::default();
    assert_eq!(cfg.font, "Sans");
    assert_eq!(cfg.font_size, 24);
    assert!(cfg.font_features.is_empty());
    assert!(cfg.font_variations.is_empty());
    assert!(cfg.hint_font);
}

// ---------------------------------------------------------------------------
// TofiConfig::default — colors
// ---------------------------------------------------------------------------

#[test]
fn default_background_color() {
    let c = TofiConfig::default().background_color;
    // #1B1D1E ≈ (0.106, 0.114, 0.118, 1.0)
    assert!((c.r - 0.106).abs() < 1e-3);
    assert!((c.g - 0.114).abs() < 1e-3);
    assert!((c.b - 0.118).abs() < 1e-3);
    assert_eq!(c.a, 1.0);
}

#[test]
fn default_foreground_color_is_white() {
    let c = TofiConfig::default().foreground_color;
    assert_eq!(c.r, 1.0);
    assert_eq!(c.g, 1.0);
    assert_eq!(c.b, 1.0);
    assert_eq!(c.a, 1.0);
}

#[test]
fn default_border_color() {
    let c = TofiConfig::default().border_color;
    // #F92672 ≈ (0.976, 0.149, 0.447, 1.0)
    assert!((c.r - 0.976).abs() < 1e-3);
    assert!((c.g - 0.149).abs() < 1e-3);
    assert!((c.b - 0.447).abs() < 1e-3);
    assert_eq!(c.a, 1.0);
}

#[test]
fn default_outline_color() {
    let c = TofiConfig::default().outline_color;
    // #080800 ≈ (0.031, 0.031, 0.0, 1.0)
    assert!((c.r - 0.031).abs() < 1e-3);
    assert!((c.g - 0.031).abs() < 1e-3);
    assert!((c.b - 0.0).abs() < 1e-3);
    assert_eq!(c.a, 1.0);
}

#[test]
fn default_selection_highlight_is_transparent() {
    let c = TofiConfig::default().selection_highlight_color;
    assert_eq!(c.a, 0.0);
}

#[test]
fn default_decoration() {
    let cfg = TofiConfig::default();
    assert_eq!(cfg.corner_radius, 0);
    assert_eq!(cfg.border_width, 12);
    assert_eq!(cfg.outline_width, 4);
}

// ---------------------------------------------------------------------------
// TofiConfig::default — padding
// ---------------------------------------------------------------------------

#[test]
fn default_padding_is_8px() {
    let cfg = TofiConfig::default();
    assert_eq!(cfg.padding_top, UnitValue::pixels(8));
    assert_eq!(cfg.padding_bottom, UnitValue::pixels(8));
    assert_eq!(cfg.padding_left, UnitValue::pixels(8));
    assert_eq!(cfg.padding_right, UnitValue::pixels(8));
}

#[test]
fn default_clip_to_padding_is_true() {
    assert!(TofiConfig::default().clip_to_padding);
}

// ---------------------------------------------------------------------------
// TofiConfig::default — text layout
// ---------------------------------------------------------------------------

#[test]
fn default_prompt_text() {
    assert_eq!(TofiConfig::default().prompt_text, "run: ");
}

#[test]
fn default_placeholder_text_is_empty() {
    assert!(TofiConfig::default().placeholder_text.is_empty());
}

#[test]
fn default_num_results_is_zero() {
    assert_eq!(TofiConfig::default().num_results, 0);
}

#[test]
fn default_layout_flags() {
    let cfg = TofiConfig::default();
    assert!(!cfg.horizontal);
    assert_eq!(cfg.result_spacing, 0);
    assert_eq!(cfg.min_input_width, 0);
}

// ---------------------------------------------------------------------------
// TofiConfig::default — themes
// ---------------------------------------------------------------------------

#[test]
fn default_placeholder_theme_has_foreground() {
    let fg = TofiConfig::default().placeholder_theme.foreground_color;
    let c = fg.expect("placeholder foreground should be pre-set");
    // #FFFFFFA8 ≈ (1.0, 1.0, 1.0, 0.659)
    assert_eq!(c.r, 1.0);
    assert_eq!(c.g, 1.0);
    assert_eq!(c.b, 1.0);
    assert!((c.a - 0.659).abs() < 1e-3);
}

#[test]
fn default_selection_theme_has_foreground() {
    let fg = TofiConfig::default().selection_theme.foreground_color;
    let c = fg.expect("selection foreground should be pre-set");
    // #F92672
    assert!((c.r - 0.976).abs() < 1e-3);
    assert!((c.g - 0.149).abs() < 1e-3);
    assert!((c.b - 0.447).abs() < 1e-3);
    assert_eq!(c.a, 1.0);
}

#[test]
fn default_other_themes_all_none() {
    let cfg = TofiConfig::default();
    assert!(cfg.prompt_theme.foreground_color.is_none());
    assert!(cfg.input_theme.foreground_color.is_none());
    assert!(cfg.default_result_theme.foreground_color.is_none());
    assert!(cfg.alternate_result_theme.foreground_color.is_none());
}

// ---------------------------------------------------------------------------
// TofiConfig::default — behaviour
// ---------------------------------------------------------------------------

#[test]
fn default_behaviour_flags() {
    let cfg = TofiConfig::default();
    assert!(!cfg.hide_cursor);
    assert!(cfg.use_history);
    assert!(cfg.history_file.is_none());
    assert_eq!(cfg.matching_algorithm, MatchingAlgorithm::Normal);
    assert!(cfg.require_match);
    assert!(!cfg.auto_accept_single);
    assert!(!cfg.hide_input);
    assert_eq!(cfg.hidden_character, HiddenCharacter(Some('*')));
    assert!(cfg.physical_keybindings);
    assert!(!cfg.print_index);
    assert!(!cfg.drun_launch);
    assert!(cfg.default_terminal.is_none());
    assert!(!cfg.late_keyboard_init);
    assert!(!cfg.multi_instance);
    assert!(!cfg.ascii_input);
}

// ---------------------------------------------------------------------------
// apply_key — value parsers
// ---------------------------------------------------------------------------

fn cfg() -> TofiConfig {
    TofiConfig::default()
}

#[test]
fn apply_bool_true_false() {
    let mut c = cfg();
    apply_key(&mut c, "hide-cursor", "true").unwrap();
    assert!(c.hide_cursor);
    apply_key(&mut c, "hide-cursor", "false").unwrap();
    assert!(!c.hide_cursor);
}

#[test]
fn apply_bool_case_insensitive() {
    let mut c = cfg();
    apply_key(&mut c, "hide-cursor", "True").unwrap();
    assert!(c.hide_cursor);
    apply_key(&mut c, "hide-cursor", "FALSE").unwrap();
    assert!(!c.hide_cursor);
}

#[test]
fn apply_bool_invalid_returns_err() {
    let mut c = cfg();
    assert!(apply_key(&mut c, "hide-cursor", "yes").is_err());
    assert!(apply_key(&mut c, "hide-cursor", "1").is_err());
}

#[test]
fn apply_u32_font_size() {
    let mut c = cfg();
    apply_key(&mut c, "font-size", "36").unwrap();
    assert_eq!(c.font_size, 36);
}

#[test]
fn apply_font_size_zero_returns_err() {
    let mut c = cfg();
    assert!(apply_key(&mut c, "font-size", "0").is_err());
}

#[test]
fn apply_i32_result_spacing() {
    let mut c = cfg();
    apply_key(&mut c, "result-spacing", "-4").unwrap();
    assert_eq!(c.result_spacing, -4);
}

#[test]
fn apply_unit_value_pixels() {
    let mut c = cfg();
    apply_key(&mut c, "width", "800").unwrap();
    assert_eq!(c.width, UnitValue::pixels(800));
}

#[test]
fn apply_unit_value_percent() {
    let mut c = cfg();
    apply_key(&mut c, "width", "50%").unwrap();
    assert_eq!(c.width, UnitValue::percent(50));
}

#[test]
fn apply_color_text_color() {
    let mut c = cfg();
    apply_key(&mut c, "text-color", "#FF0000").unwrap();
    assert_eq!(c.foreground_color.r, 1.0);
    assert_eq!(c.foreground_color.g, 0.0);
    assert_eq!(c.foreground_color.b, 0.0);
}

#[test]
fn apply_color_invalid_returns_err() {
    let mut c = cfg();
    assert!(apply_key(&mut c, "text-color", "notacolor").is_err());
}

#[test]
fn apply_anchor_all_variants() {
    let pairs = [
        ("top-left", Anchor::TopLeft),
        ("top", Anchor::Top),
        ("top-right", Anchor::TopRight),
        ("right", Anchor::Right),
        ("bottom-right", Anchor::BottomRight),
        ("bottom", Anchor::Bottom),
        ("bottom-left", Anchor::BottomLeft),
        ("left", Anchor::Left),
        ("center", Anchor::Center),
    ];
    for (s, expected) in pairs {
        let mut c = cfg();
        apply_key(&mut c, "anchor", s).unwrap();
        assert_eq!(c.anchor, expected, "anchor = {s}");
    }
}

#[test]
fn apply_anchor_case_insensitive() {
    let mut c = cfg();
    apply_key(&mut c, "anchor", "TOP-LEFT").unwrap();
    assert_eq!(c.anchor, Anchor::TopLeft);
}

#[test]
fn apply_anchor_invalid_returns_err() {
    let mut c = cfg();
    assert!(apply_key(&mut c, "anchor", "diagonal").is_err());
}

#[test]
fn apply_cursor_style_variants() {
    let pairs = [
        ("bar", CursorStyle::Bar),
        ("block", CursorStyle::Block),
        ("underscore", CursorStyle::Underscore),
    ];
    for (s, expected) in pairs {
        let mut c = cfg();
        apply_key(&mut c, "text-cursor-style", s).unwrap();
        assert_eq!(c.cursor_theme.style, expected, "cursor-style = {s}");
    }
}

#[test]
fn apply_matching_algorithm_variants() {
    use libtofi_rs::matching::MatchingAlgorithm;
    let pairs = [
        ("normal", MatchingAlgorithm::Normal),
        ("prefix", MatchingAlgorithm::Prefix),
        ("fuzzy", MatchingAlgorithm::Fuzzy),
    ];
    for (s, expected) in pairs {
        let mut c = cfg();
        apply_key(&mut c, "matching-algorithm", s).unwrap();
        assert_eq!(c.matching_algorithm, expected, "matching-algorithm = {s}");
    }
}

#[test]
fn apply_directional_single_value() {
    let mut c = cfg();
    apply_key(&mut c, "prompt-background-padding", "4").unwrap();
    let d = c.prompt_theme.padding.unwrap();
    assert_eq!(d, Directional::uniform(4));
}

#[test]
fn apply_directional_two_values() {
    let mut c = cfg();
    apply_key(&mut c, "prompt-background-padding", "2,8").unwrap();
    let d = c.prompt_theme.padding.unwrap();
    assert_eq!(
        d,
        Directional {
            top: 2,
            right: 8,
            bottom: 2,
            left: 8
        }
    );
}

#[test]
fn apply_directional_four_values() {
    let mut c = cfg();
    apply_key(&mut c, "prompt-background-padding", "1,2,3,4").unwrap();
    let d = c.prompt_theme.padding.unwrap();
    assert_eq!(
        d,
        Directional {
            top: 1,
            right: 2,
            bottom: 3,
            left: 4
        }
    );
}

#[test]
fn apply_directional_too_many_values_returns_err() {
    let mut c = cfg();
    assert!(apply_key(&mut c, "prompt-background-padding", "1,2,3,4,5").is_err());
}

#[test]
fn apply_hidden_character_asterisk() {
    let mut c = cfg();
    apply_key(&mut c, "hidden-character", "*").unwrap();
    assert_eq!(c.hidden_character, HiddenCharacter(Some('*')));
}

#[test]
fn apply_hidden_character_empty_means_hidden() {
    let mut c = cfg();
    apply_key(&mut c, "hidden-character", "").unwrap();
    assert_eq!(c.hidden_character, HiddenCharacter(None));
}

#[test]
fn apply_hidden_character_multi_char_returns_err() {
    let mut c = cfg();
    assert!(apply_key(&mut c, "hidden-character", "ab").is_err());
}

#[test]
fn apply_exclusive_zone_minus_one() {
    let mut c = cfg();
    apply_key(&mut c, "exclusive-zone", "100").unwrap();
    assert_eq!(c.exclusive_zone, 100);
    apply_key(&mut c, "exclusive-zone", "-1").unwrap();
    assert_eq!(c.exclusive_zone, -1);
}

#[test]
fn apply_history_file() {
    let mut c = cfg();
    apply_key(&mut c, "history-file", "/tmp/tofi-history").unwrap();
    assert_eq!(c.history_file.as_deref(), Some("/tmp/tofi-history"));
}

#[test]
fn apply_unknown_key_returns_err() {
    let mut c = cfg();
    assert!(apply_key(&mut c, "no-such-option", "value").is_err());
}

// ---------------------------------------------------------------------------
// load — file-level tests
// ---------------------------------------------------------------------------

/// Write `content` to a named temp file and return its path.
fn tmp_config(content: &str) -> (tempfile::NamedTempFile, std::path::PathBuf) {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    let path = f.path().to_owned();
    (f, path)
}

#[test]
fn load_empty_file_gives_default() {
    let (_f, path) = tmp_config("");
    let mut c = TofiConfig::default();
    let errs = load(&path, &mut c).unwrap();
    assert!(errs.is_empty());
    assert_eq!(c, TofiConfig::default());
}

#[test]
fn load_comment_only_file_gives_default() {
    let (_f, path) = tmp_config("# This is a comment\n; Another comment\n[section]\n");
    let mut c = TofiConfig::default();
    let errs = load(&path, &mut c).unwrap();
    assert!(errs.is_empty());
    assert_eq!(c, TofiConfig::default());
}

#[test]
fn load_simple_key_value() {
    let (_f, path) = tmp_config("font-size = 36\n");
    let mut c = TofiConfig::default();
    load(&path, &mut c).unwrap();
    assert_eq!(c.font_size, 36);
}

#[test]
fn load_strips_whitespace_and_quotes() {
    let (_f, path) = tmp_config("  font  =  \"Monospace\"  \n");
    let mut c = TofiConfig::default();
    load(&path, &mut c).unwrap();
    assert_eq!(c.font, "Monospace");
}

#[test]
fn load_unknown_key_produces_error() {
    let (_f, path) = tmp_config("unknown-option = value\n");
    let mut c = TofiConfig::default();
    let errs = load(&path, &mut c).unwrap();
    assert_eq!(errs.len(), 1);
    assert!(errs[0].message.contains("Unknown option"));
}

#[test]
fn load_stops_after_max_errors() {
    let content = (0..10)
        .map(|i| format!("bad-key-{i} = val\n"))
        .collect::<String>();
    let (_f, path) = tmp_config(&content);
    let mut c = TofiConfig::default();
    let errs = load(&path, &mut c).unwrap();
    // MAX_ERRORS (5) errors + 1 "too many errors" sentinel
    assert!(errs.len() <= 7, "errs.len() = {}", errs.len());
}

#[test]
fn load_include_relative() {
    // Included file sets font-size to 99.
    let mut inc = tempfile::NamedTempFile::new().unwrap();
    inc.write_all(b"font-size = 99\n").unwrap();
    let inc_name = inc
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let inc_dir = inc.path().parent().unwrap().to_string_lossy().into_owned();

    let main_content = format!("include = {inc_name}\n");
    let mut main = tempfile::Builder::new()
        .suffix(".tofi")
        .tempfile_in(&inc_dir)
        .unwrap();
    main.write_all(main_content.as_bytes()).unwrap();
    let main_path = main.path().to_owned();

    let mut c = TofiConfig::default();
    let errs = load(&main_path, &mut c).unwrap();
    assert!(errs.is_empty(), "errors: {errs:?}");
    assert_eq!(c.font_size, 99);
}

#[test]
fn load_doc_config_fixture() {
    // The repo's doc/config sets all options to their defaults.
    // Verify a representative set of values.
    let doc_config = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("doc/config");
    if !doc_config.exists() {
        // Skip if running outside the repo.
        return;
    }
    let mut c = TofiConfig::default();
    let errs = load(&doc_config, &mut c).unwrap();
    assert!(errs.is_empty(), "parse errors in doc/config: {errs:?}");

    // Window geometry
    assert_eq!(c.width, UnitValue::pixels(1280));
    assert_eq!(c.height, UnitValue::pixels(720));
    assert_eq!(c.anchor, Anchor::Center);
    assert_eq!(c.exclusive_zone, -1);

    // Font
    assert_eq!(c.font, "Sans");
    assert_eq!(c.font_size, 24);
    assert!(c.hint_font);

    // Colors
    assert!((c.foreground_color.r - 1.0).abs() < 1e-3);
    assert!((c.border_width as i32 - 12).abs() == 0);

    // Behaviour
    assert!(c.use_history);
    assert!(c.require_match);
    assert!(c.physical_keybindings);
    assert!(!c.multi_instance);
}
