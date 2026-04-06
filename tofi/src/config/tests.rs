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
