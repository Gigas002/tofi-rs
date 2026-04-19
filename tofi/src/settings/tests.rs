use libtofi_rs::matching::MatchingAlgorithm;

use super::{LaunchMode, build, detect_mode};
use crate::cli::Cli;
use crate::config::Config;
use crate::theme::{Theme, UnitValue};

// ── detect_mode ───────────────────────────────────────────────────────────────

#[test]
fn detect_mode_drun_string() {
    assert_eq!(detect_mode(Some("drun")), LaunchMode::Drun);
}

#[test]
fn detect_mode_run_string() {
    assert_eq!(detect_mode(Some("run")), LaunchMode::Run);
}

#[test]
fn detect_mode_none_defaults_to_drun() {
    assert_eq!(detect_mode(None), LaunchMode::Drun);
}

#[test]
fn detect_mode_unknown_defaults_to_drun() {
    assert_eq!(detect_mode(Some("bogus")), LaunchMode::Drun);
}

fn default_cli() -> Cli {
    use clap::Parser as _;
    Cli::try_parse_from(["tofi"]).unwrap()
}

#[test]
fn defaults_when_nothing_set() {
    let s = build(&default_cli(), &Config::default(), &Theme::default());
    assert_eq!(s.algorithm, MatchingAlgorithm::Normal);
    assert!(s.require_match);
    assert!(!s.ascii_input);
    assert!(s.use_history);
    assert!(s.history_file.is_none());
    assert!(s.target_output.is_empty());
    assert!(s.default_terminal.is_none());
    assert_eq!(s.width, UnitValue::pixels(1280));
    assert_eq!(s.height, UnitValue::pixels(720));
    assert_eq!(s.font, "Sans");
    assert_eq!(s.font_size, 24);
    assert!(s.hint_font);
    assert_eq!(s.hidden_character, '*');
    assert!(!s.hide_cursor);
    assert!(!s.hide_input);
    assert!(!s.horizontal);
    assert_eq!(s.num_results, 0);
    assert_eq!(s.result_spacing, 0);
}

#[test]
fn cli_algorithm_overrides_config() {
    use clap::Parser as _;
    let cli = Cli::try_parse_from(["tofi", "--algorithm", "fuzzy"]).unwrap();
    let mut config = Config::default();
    config.matching.algorithm = Some("normal".to_owned());
    let s = build(&cli, &config, &Theme::default());
    assert_eq!(s.algorithm, MatchingAlgorithm::Fuzzy);
}

#[test]
fn cli_history_false_overrides_config() {
    use clap::Parser as _;
    let cli = Cli::try_parse_from(["tofi", "--history", "false"]).unwrap();
    let mut config = Config::default();
    config.history.history = Some(true);
    let s = build(&cli, &config, &Theme::default());
    assert!(!s.use_history);
}

#[test]
fn cli_output_overrides_config() {
    use clap::Parser as _;
    let cli = Cli::try_parse_from(["tofi", "--output", "HDMI-1"]).unwrap();
    let s = build(&cli, &Config::default(), &Theme::default());
    assert_eq!(s.target_output, "HDMI-1");
}

#[test]
fn config_output_empty_string_is_ignored() {
    let mut config = Config::default();
    config.base.output = Some(String::new());
    let s = build(&default_cli(), &config, &Theme::default());
    assert!(s.target_output.is_empty());
}

#[test]
fn theme_sets_window_geometry() {
    use crate::theme::Dim;
    let mut theme = Theme::default();
    theme.window.width = Some(Dim::Str("45%".to_owned()));
    theme.window.height = Some(Dim::Str("60%".to_owned()));
    theme.window.corner_radius = Some(50);
    let s = build(&default_cli(), &Config::default(), &theme);
    assert_eq!(s.width, UnitValue::percent(45));
    assert_eq!(s.height, UnitValue::percent(60));
    assert_eq!(s.corner_radius, 50);
}

#[test]
fn theme_sets_font() {
    let mut theme = Theme::default();
    theme.font.name = Some("Monospace".to_owned());
    theme.font.size = Some(18);
    theme.font.hint = Some(false);
    let s = build(&default_cli(), &Config::default(), &theme);
    assert_eq!(s.font, "Monospace");
    assert_eq!(s.font_size, 18);
    assert!(!s.hint_font);
}

#[test]
fn theme_sets_colors() {
    let mut theme = Theme::default();
    theme.text.color = Some("#FF0000".to_owned());
    theme.text.selection_color = Some("#00FF00".to_owned());
    let s = build(&default_cli(), &Config::default(), &theme);
    assert!((s.foreground_color.r - 1.0).abs() < 1e-3);
    assert!(s.foreground_color.g.abs() < 1e-3);
    let sc = s.selection_style.foreground_color.unwrap();
    assert!(sc.r.abs() < 1e-3);
    assert!((sc.g - 1.0).abs() < 1e-3);
}

#[test]
fn default_selection_color_fallback() {
    let s = build(&default_cli(), &Config::default(), &Theme::default());
    let sc = s.selection_style.foreground_color.unwrap();
    assert!((sc.r - 0.976).abs() < 1e-3);
}

fn examples_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
}

#[test]
fn build_from_example_files() {
    use clap::Parser as _;
    let config_path = examples_dir().join("config/config.toml");
    let theme_path = examples_dir().join("theme/theme.toml");
    if !config_path.exists() || !theme_path.exists() {
        return;
    }

    let config = crate::config::load(&config_path);
    let theme = crate::theme::load(&theme_path);
    let cli = Cli::try_parse_from(["tofi"]).unwrap();
    let s = build(&cli, &config, &theme);

    assert!(s.require_match);
    assert!(!s.ascii_input);
    assert!(s.use_history);
    assert_eq!(s.width, UnitValue::percent(45));
    assert_eq!(s.height, UnitValue::percent(60));
    assert_eq!(s.font, "CaskaydiaCove Nerd Font Mono");
    assert_eq!(s.result_spacing, 25);
}

#[test]
fn build_from_minimal_files_uses_defaults() {
    use clap::Parser as _;
    let config_path = examples_dir().join("config/minimal.toml");
    let theme_path = examples_dir().join("theme/minimal.toml");
    if !config_path.exists() || !theme_path.exists() {
        return;
    }

    let config = crate::config::load(&config_path);
    let theme = crate::theme::load(&theme_path);
    let cli = Cli::try_parse_from(["tofi"]).unwrap();
    let s = build(&cli, &config, &theme);

    assert_eq!(s.algorithm, MatchingAlgorithm::Normal);
    assert!(s.require_match);
    assert!(!s.ascii_input);
    assert!(s.use_history);
    assert!(s.history_file.is_none());
    assert!(s.target_output.is_empty());
    assert!(s.default_terminal.is_none());
    assert_eq!(s.width, UnitValue::pixels(1280));
    assert_eq!(s.height, UnitValue::pixels(720));
    assert_eq!(s.font, "Sans");
    assert_eq!(s.font_size, 24);
    assert!(!s.horizontal);
    assert_eq!(s.num_results, 0);
    assert_eq!(s.result_spacing, 0);
}

#[test]
fn build_from_maximal_files_sets_all_options() {
    use clap::Parser as _;
    let config_path = examples_dir().join("config/maximal.toml");
    let theme_path = examples_dir().join("theme/maximal.toml");
    if !config_path.exists() || !theme_path.exists() {
        return;
    }

    let config = crate::config::load(&config_path);
    let theme = crate::theme::load(&theme_path);
    let cli = Cli::try_parse_from(["tofi"]).unwrap();
    let s = build(&cli, &config, &theme);

    // behavioral — from maximal config
    assert_eq!(s.algorithm, MatchingAlgorithm::Fuzzy);
    assert!(!s.require_match);
    assert!(s.ascii_input);
    assert!(!s.use_history);
    assert_eq!(s.target_output, "eDP-1");
    assert_eq!(s.default_terminal.as_deref(), Some("foot"));

    // visual — from maximal theme
    assert_eq!(s.font, "Monospace");
    assert_eq!(s.font_size, 16);
    assert_eq!(s.width, UnitValue::percent(80));
    assert_eq!(s.height, UnitValue::percent(5));
    assert_eq!(s.num_results, 8);
    assert_eq!(s.result_spacing, 6);
    assert!(s.horizontal);
    assert!(s.hide_cursor);
    assert!(s.cursor.show);
}
