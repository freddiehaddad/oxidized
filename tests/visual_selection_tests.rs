use anyhow::Result;
use oxidized::core::{buffer::Buffer, mode::Position};

#[cfg(test)]
mod visual_selection_tests {
    use super::*;

    fn insert_test_text(buffer: &mut Buffer, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                buffer.insert_line_break();
            } else {
                buffer.insert_char(ch);
            }
        }
        // Reset cursor to start for testing
        buffer.cursor = Position::new(0, 0);
    }

    #[test]
    fn test_start_visual_selection() {
        let mut buffer = Buffer::new(1, 100);
        insert_test_text(&mut buffer, "Hello, world!");
        buffer.cursor = Position::new(0, 5);

        // Start visual selection
        buffer.start_visual_selection();

        assert!(buffer.has_selection());
        if let Some(selection) = buffer.selection {
            assert_eq!(selection.start, Position::new(0, 5));
            assert_eq!(selection.end, Position::new(0, 5));
        }
    }

    #[test]
    fn test_update_visual_selection() {
        let mut buffer = Buffer::new(1, 100);
        insert_test_text(&mut buffer, "Hello, world!");

        // Start visual selection at position 0
        buffer.start_visual_selection();

        // Move cursor and update selection
        buffer.cursor = Position::new(0, 5);
        buffer.update_visual_selection(buffer.cursor);

        if let Some(selection) = buffer.selection {
            assert_eq!(selection.start, Position::new(0, 0));
            assert_eq!(selection.end, Position::new(0, 5));
        }
    }

    #[test]
    fn test_backward_visual_extension_includes_anchor_char() {
        let mut buffer = Buffer::new(1, 100);
        insert_test_text(&mut buffer, "abcdef");
        // Anchor on 'd' (col 3)
        buffer.cursor = Position::new(0, 3);
        buffer.start_visual_selection();
        // Move left to 'c' (col 2)
        buffer.cursor = Position::new(0, 2);
        buffer.update_visual_selection(buffer.cursor);
        let selected = buffer.get_selected_text().unwrap();
        assert_eq!(selected, "cd");
    }

    #[test]
    fn test_get_selection_range_normalized() {
        let mut buffer = Buffer::new(1, 100);
        insert_test_text(&mut buffer, "Hello, world!");

        // Test forward selection (start < end)
        buffer.selection = Some(oxidized::core::mode::Selection::new(
            Position::new(0, 2),
            Position::new(0, 7),
        ));

        if let Some((start, end)) = buffer.get_selection_range() {
            assert_eq!(start, Position::new(0, 2));
            assert_eq!(end, Position::new(0, 7));
        }

        // Test backward selection (start > end) - should be normalized
        buffer.selection = Some(oxidized::core::mode::Selection::new(
            Position::new(0, 7),
            Position::new(0, 2),
        ));

        if let Some((start, end)) = buffer.get_selection_range() {
            assert_eq!(start, Position::new(0, 2));
            // Backward selection now extends end by 1 to include anchor char
            assert_eq!(end, Position::new(0, 8));
        }
    }

    #[test]
    fn test_get_selected_text() {
        let mut buffer = Buffer::new(1, 100);
        insert_test_text(&mut buffer, "Hello, world!");

        // Test single line selection
        buffer.selection = Some(oxidized::core::mode::Selection::new(
            Position::new(0, 0),
            Position::new(0, 5),
        ));

        assert_eq!(buffer.get_selected_text(), Some("Hello".to_string()));
    }

    #[test]
    fn test_delete_selection() {
        let mut buffer = Buffer::new(1, 100);
        insert_test_text(&mut buffer, "Hello, world!");

        // Select "Hello"
        buffer.selection = Some(oxidized::core::mode::Selection::new(
            Position::new(0, 0),
            Position::new(0, 5),
        ));

        let deleted = buffer.delete_selection();
        assert_eq!(deleted, Some("Hello".to_string()));
        assert_eq!(buffer.lines[0], ", world!");
        assert!(buffer.selection.is_none());
    }

    #[test]
    fn test_yank_selection() {
        let mut buffer = Buffer::new(1, 100);
        insert_test_text(&mut buffer, "Hello, world!");

        // Select "world"
        buffer.selection = Some(oxidized::core::mode::Selection::new(
            Position::new(0, 7),
            Position::new(0, 12),
        ));

        let yanked = buffer.yank_selection();
        assert_eq!(yanked, Some("world".to_string()));
        assert_eq!(buffer.clipboard.text, "world");
        assert_eq!(buffer.lines[0], "Hello, world!"); // Original text unchanged
    }

    #[test]
    fn test_clear_visual_selection() {
        let mut buffer = Buffer::new(1, 100);
        insert_test_text(&mut buffer, "Hello, world!");
        buffer.start_visual_selection();

        assert!(buffer.has_selection());

        buffer.clear_visual_selection();
        assert!(!buffer.has_selection());
        assert!(buffer.selection.is_none());
    }

    #[test]
    fn test_multiline_selection() -> Result<()> {
        let mut buffer = Buffer::new(1, 100);
        insert_test_text(&mut buffer, "Line 1\nLine 2\nLine 3");

        // Select from middle of line 0 to middle of line 2
        buffer.selection = Some(oxidized::core::mode::Selection::new(
            Position::new(0, 2), // "ne 1"
            Position::new(2, 4), // "Line"
        ));

        let selected = buffer.get_selected_text().unwrap();
        // Should include: "ne 1\nLine 2\nLine"
        assert!(selected.contains("ne 1"));
        assert!(selected.contains("Line 2"));
        assert!(selected.contains("Line"));
        assert_eq!(selected.lines().count(), 3);

        Ok(())
    }

    #[test]
    fn test_selection_boundary_conditions() {
        let mut buffer = Buffer::new(1, 100);
        insert_test_text(&mut buffer, "Hello");

        // Test selection at end of line
        buffer.selection = Some(oxidized::core::mode::Selection::new(
            Position::new(0, 3),
            Position::new(0, 5), // End of "Hello"
        ));

        assert_eq!(buffer.get_selected_text(), Some("lo".to_string()));

        // Test selection beyond line length (should be clamped)
        buffer.selection = Some(oxidized::core::mode::Selection::new(
            Position::new(0, 0),
            Position::new(0, 100), // Way beyond line length
        ));

        assert_eq!(buffer.get_selected_text(), Some("Hello".to_string()));
    }
}
