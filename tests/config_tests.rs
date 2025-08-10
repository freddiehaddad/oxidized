use oxidized::config::editor::LanguageConfig;
use oxidized::config::{EditorConfig, theme::ThemeConfig};
use oxidized::ui::UI;

#[test]
fn test_language_config_default() {
    let lang_config = LanguageConfig::default();

    // Default should be empty - all config comes from editor.toml
    assert!(lang_config.extensions.is_empty());
    assert!(lang_config.content_patterns.is_empty());
}

#[test]
fn test_language_config_from_file() {
    // Test that we can load language config from the actual editor.toml file
    let config = EditorConfig::load();

    // Should have extensions from editor.toml
    assert_eq!(
        config.languages.detect_language_from_extension("test.rs"),
        Some("rust".to_string())
    );
    assert_eq!(
        config
            .languages
            .detect_language_from_extension("config.toml"),
        Some("toml".to_string())
    );
    assert_eq!(
        config.languages.detect_language_from_extension("readme.md"),
        Some("markdown".to_string())
    );
    assert_eq!(
        config
            .languages
            .detect_language_from_extension("unknown.xyz"),
        None
    );

    // Should have content patterns from editor.toml
    assert_eq!(
        config
            .languages
            .detect_language_from_content("fn main() { let x = 5; }"),
        Some("rust".to_string())
    );
    assert_eq!(
        config
            .languages
            .detect_language_from_content("[package]\nname = \"test\""),
        Some("toml".to_string())
    );
    assert_eq!(
        config
            .languages
            .detect_language_from_content("# Heading\n## Subheading"),
        Some("markdown".to_string())
    );
    assert_eq!(
        config.languages.detect_language_from_content("plain text"),
        None
    );
}

#[test]
fn test_editor_config_has_languages() {
    let config = EditorConfig::load(); // Load from actual file
    assert!(!config.languages.extensions.is_empty());
    assert!(!config.languages.content_patterns.is_empty());
}

#[test]
fn test_language_config_fallbacks() {
    let config = EditorConfig::load();

    // Should have language support
    assert!(config.languages.has_language_support());

    // Should have a fallback language (first configured language)
    assert!(config.languages.get_fallback_language().is_some());

    // Fallback should be one of the configured languages
    let fallback = config.languages.get_fallback_language().unwrap();
    assert!(
        config
            .languages
            .extensions
            .values()
            .any(|lang| lang == &fallback)
    );
}

// Integration tests for theme configuration
#[test]
fn test_theme_integration_editor_to_themes() {
    // Load editor configuration
    let editor_config = EditorConfig::load();

    // Check that color_scheme is loaded from editor.toml
    assert!(!editor_config.display.color_scheme.is_empty());
    println!(
        "Editor config color_scheme: {}",
        editor_config.display.color_scheme
    );

    // Load theme configuration with the color scheme from editor config
    let theme_config = ThemeConfig::load_with_default_theme(&editor_config.display.color_scheme);

    // Check that the theme exists in themes.toml
    assert!(
        theme_config
            .themes
            .contains_key(&editor_config.display.color_scheme),
        "Theme '{}' from editor.toml should exist in themes.toml",
        editor_config.display.color_scheme
    );

    // Verify UI can set the theme
    let mut ui = UI::new();
    ui.set_theme(&editor_config.display.color_scheme);

    // Test that the theme was applied (by checking it doesn't panic and colors are set)
    // Note: If we reached here, applying the theme didn't panic.
}

#[test]
fn test_theme_config_current_matches_editor_config() {
    let editor_config = EditorConfig::load();
    let theme_config = ThemeConfig::load_with_default_theme(&editor_config.display.color_scheme);

    // The theme config should either use the editor's color scheme or fallback gracefully
    assert!(
        theme_config.theme.current == editor_config.display.color_scheme
            || theme_config
                .themes
                .contains_key(&theme_config.theme.current),
        "Theme config current '{}' should match editor config color_scheme '{}' or exist in themes",
        theme_config.theme.current,
        editor_config.display.color_scheme
    );
}

#[test]
fn test_available_themes_in_themes_toml() {
    let theme_config = ThemeConfig::load();

    // Should have at least one theme
    assert!(
        !theme_config.themes.is_empty(),
        "themes.toml should contain at least one theme"
    );

    // Check if "default" theme exists (as set in editor.toml)
    println!(
        "Available themes: {:?}",
        theme_config.themes.keys().collect::<Vec<_>>()
    );

    if theme_config.themes.contains_key("default") {
        let default_theme = &theme_config.themes["default"];
        assert!(
            !default_theme.name.is_empty(),
            "Default theme should have a name"
        );
        println!("Default theme name: {}", default_theme.name);
    }
}

#[test]
fn test_editor_config_parsing() {
    let config = EditorConfig::load();

    // Verify the config parsed correctly from editor.toml
    println!("Editor config display settings:");
    println!("  color_scheme: {}", config.display.color_scheme);
    println!("  show_line_numbers: {}", config.display.show_line_numbers);
    println!(
        "  show_relative_numbers: {}",
        config.display.show_relative_numbers
    );
    println!("  show_cursor_line: {}", config.display.show_cursor_line);
    println!(
        "  syntax_highlighting: {}",
        config.display.syntax_highlighting
    );

    // Should match the values in editor.toml
    assert_eq!(config.display.color_scheme, "default");
    assert!(!config.display.show_line_numbers);
    assert!(config.display.show_relative_numbers);
    assert!(config.display.show_cursor_line);
    assert!(config.display.syntax_highlighting);
}

#[test]
fn test_showmarks_setting_defaults_and_set() {
    let mut config = EditorConfig::load();
    // Default from editor.toml should be true
    assert!(config.display.show_marks_in_number_column);

    // Toggle off via :set equivalent
    let res = config.set_setting("showmarks", "false");
    assert!(res.is_ok());
    assert!(!config.display.show_marks_in_number_column);

    // Toggle on via alias
    let res = config.set_setting("smk", "true");
    assert!(res.is_ok());
    assert!(config.display.show_marks_in_number_column);
}
