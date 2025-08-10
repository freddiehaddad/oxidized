use crossterm::style::Color;
use oxidized::config::theme::{ThemeConfig, parse_color};

#[test]
fn test_parse_color() {
    assert!(matches!(
        parse_color("#ff0000"),
        Color::Rgb { r: 255, g: 0, b: 0 }
    ));
    assert!(matches!(
        parse_color("#00ff00"),
        Color::Rgb { r: 0, g: 255, b: 0 }
    ));
    assert!(matches!(
        parse_color("#0000ff"),
        Color::Rgb { r: 0, g: 0, b: 255 }
    ));
    assert!(matches!(parse_color("invalid"), Color::White));
}

#[test]
fn test_emergency_theme_config() {
    let config = ThemeConfig::create_emergency_config();
    // Emergency config now mirrors the repository's default theme exactly
    assert_eq!(config.theme.current, "default");
    assert!(config.themes.contains_key("default"));
}
