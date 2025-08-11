#[cfg(test)]
mod ui_tests {
    use oxidized::ui::UI;

    #[test]
    fn test_ui_creation() {
        let ui = UI::new();

        // Test default settings
        assert!(ui.show_line_numbers);
        assert!(!ui.show_relative_numbers);
        assert!(ui.show_cursor_line);
        assert_eq!(ui.viewport_top(), 0);

        // New: show marks in number column default
        assert!(ui.show_marks);
    }

    #[test]
    fn test_theme_name_detection() {
        let ui = UI::new();
        let theme_name = ui.theme_name();

        // Should return one of the expected theme names
        assert!(matches!(
            theme_name,
            "default" | "dark" | "light" | "ferris"
        ));
    }

    #[test]
    fn test_set_theme() {
        let mut ui = UI::new();

        // Test setting a known theme
        ui.set_theme("dark");
        // Since we can't easily test internal theme state without exposing it,
        // we just verify the method doesn't panic

        ui.set_theme("light");
        ui.set_theme("ferris");

        // Test fallback for unknown theme
        ui.set_theme("nonexistent_theme");
    }

    #[test]
    fn test_line_number_settings() {
        let mut ui = UI::new();

        // Test initial values
        assert!(ui.show_line_numbers);
        assert!(!ui.show_relative_numbers);

        // Test toggling
        ui.show_line_numbers = false;
        assert!(!ui.show_line_numbers);

        ui.show_relative_numbers = true;
        assert!(ui.show_relative_numbers);
    }

    #[test]
    fn test_gutter_exists_when_only_marks_enabled() {
        let mut ui = UI::new();
        // Simulate config: hide numbers, show marks
        ui.show_line_numbers = false;
        ui.show_relative_numbers = false;
        ui.show_marks = true;

        // With no numbers, gutter should still reserve minimal width for marks
        let w = ui.compute_gutter_width(1234);
        assert_eq!(w, 2, "Expected minimal gutter width for marks-only mode");

        // If numbers are enabled, width should scale with total lines
        ui.show_line_numbers = true;
        let w2 = ui.compute_gutter_width(1234);
        assert!(w2 >= 4);
    }

    #[test]
    fn test_cursor_line_setting() {
        let mut ui = UI::new();

        assert!(ui.show_cursor_line);

        ui.show_cursor_line = false;
        assert!(!ui.show_cursor_line);
    }

    #[test]
    fn test_viewport_top() {
        let mut ui = UI::new();

        assert_eq!(ui.viewport_top(), 0);

        ui.set_viewport_top(10);
        assert_eq!(ui.viewport_top(), 10);
    }

    #[test]
    fn test_multiple_theme_changes() {
        let mut ui = UI::new();

        // Test rapid theme changes don't cause issues
        let themes = ["default", "dark", "light", "ferris", "nonexistent"];

        for theme in themes.iter() {
            ui.set_theme(theme);
            // Verify theme_name still returns a valid value
            let current_name = ui.theme_name();
            assert!(matches!(
                current_name,
                "default" | "dark" | "light" | "ferris"
            ));
        }
    }

    #[test]
    fn test_ui_settings_independence() {
        let mut ui1 = UI::new();
        let ui2 = UI::new();

        // Modify one UI instance
        ui1.show_line_numbers = false;
        ui1.show_relative_numbers = true;
        ui1.set_viewport_top(5);

        // Other instance should be unaffected
        assert!(ui2.show_line_numbers);
        assert!(!ui2.show_relative_numbers);
        assert_eq!(ui2.viewport_top(), 0);
    }

    #[test]
    fn test_theme_configuration_loading() {
        // Test that UI creation loads theme configuration without panicking
        let ui = UI::new();

        // These should all work without errors
        let theme_name = ui.theme_name();
        assert!(!theme_name.is_empty());

        // Theme name should be one of the valid options
        assert!(
            theme_name == "default"
                || theme_name == "dark"
                || theme_name == "light"
                || theme_name == "ferris",
            "Theme name '{}' is not one of the expected values",
            theme_name
        );
    }

    #[test]
    fn test_viewport_boundaries() {
        let mut ui = UI::new();

        // Test setting viewport to large values
        ui.set_viewport_top(usize::MAX);
        assert_eq!(ui.viewport_top(), usize::MAX);

        ui.set_viewport_top(0);
        assert_eq!(ui.viewport_top(), 0);
    }

    #[test]
    fn test_default_behavior_matches_vim() {
        let ui = UI::new();

        // Test that defaults match Vim-like behavior
        assert!(
            ui.show_line_numbers,
            "Line numbers should be enabled by default like Vim"
        );
        assert!(
            !ui.show_relative_numbers,
            "Relative numbers should be disabled by default"
        );
        assert!(
            ui.show_cursor_line,
            "Cursor line highlighting should be enabled by default"
        );
    }

    #[test]
    fn test_boolean_settings_toggle() {
        let mut ui = UI::new();

        // Test all boolean settings can be toggled
        let original_line_numbers = ui.show_line_numbers;
        ui.show_line_numbers = !ui.show_line_numbers;
        assert_ne!(ui.show_line_numbers, original_line_numbers);

        let original_relative = ui.show_relative_numbers;
        ui.show_relative_numbers = !ui.show_relative_numbers;
        assert_ne!(ui.show_relative_numbers, original_relative);

        let original_cursor_line = ui.show_cursor_line;
        ui.show_cursor_line = !ui.show_cursor_line;
        assert_ne!(ui.show_cursor_line, original_cursor_line);
    }

    #[test]
    fn test_consecutive_ui_instances() {
        // Test creating multiple UI instances doesn't cause issues
        for _ in 0..5 {
            let ui = UI::new();
            assert!(ui.show_line_numbers);
            assert_eq!(ui.viewport_top(), 0);

            let theme_name = ui.theme_name();
            assert!(!theme_name.is_empty());
        }
    }

    #[test]
    fn test_theme_persistence_across_methods() {
        let mut ui = UI::new();

        ui.set_theme("dark");
        let name1 = ui.theme_name();

        ui.set_theme("light");
        let name2 = ui.theme_name();

        // Both calls should succeed and potentially return different values
        // (though the implementation might not actually change the theme name
        // returned by theme_name() since it reads from config)
        assert!(!name1.is_empty());
        assert!(!name2.is_empty());
    }
}
