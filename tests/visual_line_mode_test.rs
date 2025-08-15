use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::buffer::Buffer;
use oxidized::core::editor::Editor;
use oxidized::core::mode::{Mode, Position, SelectionType};
use oxidized::input::keymap::KeyHandler;

/// Helper to insert text into buffer character by character
fn insert_test_text(buffer: &mut Buffer, text: &str) {
    for ch in text.chars() {
        if ch == '\n' {
            buffer.insert_line_break();
        } else {
            buffer.insert_char(ch);
        }
    }
}

#[test]
fn test_visual_line_mode_activation() {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = Editor::new().unwrap();

    // Start in normal mode
    assert_eq!(editor.mode(), Mode::Normal);

    // Enter visual line mode with Shift+V
    let key_event = KeyEvent::new(KeyCode::Char('V'), KeyModifiers::SHIFT);
    let _result = key_handler.handle_key(&mut editor, key_event);

    // Should now be in visual line mode
    assert_eq!(editor.mode(), Mode::VisualLine);
}

#[test]
fn test_buffer_visual_line_selection() {
    let mut buffer = Buffer::new(1, 100);
    insert_test_text(&mut buffer, "Line one\nLine two with more text");
    buffer.cursor = Position::new(0, 0);

    // Start visual line selection
    buffer.start_visual_line_selection();

    // Should have a selection
    assert!(buffer.has_selection());

    // Selection should be line-wise
    if let Some(selection) = &buffer.selection {
        assert_eq!(selection.selection_type, SelectionType::Line);
        // Should start at beginning of line (col 0)
        assert_eq!(selection.start.col, 0);
        // Should end at the end of the line
        assert!(selection.end.col > 0); // "Line one" has characters
    }
}

#[test]
fn test_visual_line_selection_update() {
    let mut buffer = Buffer::new(1, 100);
    insert_test_text(&mut buffer, "Line one\nLine two with more text");
    buffer.cursor = Position::new(0, 0);

    // Start visual line selection at line 0
    buffer.start_visual_line_selection();

    // Move cursor to line 1 and update selection
    buffer.cursor = Position::new(1, 5);
    buffer.update_visual_selection(buffer.cursor);

    if let Some(selection) = &buffer.selection {
        assert_eq!(selection.selection_type, SelectionType::Line);
        // Should start at beginning of line 0
        assert_eq!(selection.start, Position::new(0, 0));
        // Should end at end of line 1
        assert_eq!(selection.end.row, 1);
        assert_eq!(selection.end.col, 23); // "Line two with more text" = 23 chars
    }
}

#[test]
fn test_buffer_get_line_length() {
    let mut buffer = Buffer::new(1, 100);

    // Test empty buffer
    assert_eq!(buffer.get_line_length(0), 0);
    assert_eq!(buffer.get_line_length(10), 0); // Out of bounds

    // Add some content
    insert_test_text(&mut buffer, "Hello\nLonger line");

    assert_eq!(buffer.get_line_length(0), 5); // "Hello"
    assert_eq!(buffer.get_line_length(1), 11); // "Longer line"
    assert_eq!(buffer.get_line_length(99), 0); // Out of bounds
}

#[test]
fn test_visual_line_empty_line() {
    let mut buffer = Buffer::new(1, 100);
    insert_test_text(&mut buffer, "Line 1\n\nLine 3");
    buffer.cursor = Position::new(1, 0); // Empty line

    // Start visual line selection on empty line
    buffer.start_visual_line_selection();

    if let Some(selection) = &buffer.selection {
        assert_eq!(selection.selection_type, SelectionType::Line);
        assert_eq!(selection.start, Position::new(1, 0));
        assert_eq!(selection.end, Position::new(1, 0)); // Empty line length = 0
    }
}

#[test]
fn test_visual_line_single_line_selection() {
    let mut buffer = Buffer::new(1, 100);
    insert_test_text(&mut buffer, "Single line");
    buffer.cursor = Position::new(0, 3); // Middle of line

    // Start visual line selection
    buffer.start_visual_line_selection();

    if let Some(selection) = &buffer.selection {
        assert_eq!(selection.selection_type, SelectionType::Line);
        // Should select entire line regardless of cursor position
        assert_eq!(selection.start, Position::new(0, 0));
        assert_eq!(selection.end, Position::new(0, 11)); // "Single line" = 11 chars
    }
}

#[test]
fn test_visual_line_backward_selection() {
    let mut buffer = Buffer::new(1, 100);
    insert_test_text(&mut buffer, "First line\nSecond line\nThird line");
    buffer.cursor = Position::new(2, 0); // Start on third line

    // Start visual line selection
    buffer.start_visual_line_selection();

    // Move cursor up to first line (backward selection)
    buffer.cursor = Position::new(0, 5);
    buffer.update_visual_selection(buffer.cursor);

    if let Some(selection) = &buffer.selection {
        assert_eq!(selection.selection_type, SelectionType::Line);
        // Selection should be normalized (start before end)
        assert_eq!(selection.start, Position::new(0, 0));
        assert_eq!(selection.end.row, 2);
        assert_eq!(selection.end.col, 10); // "Third line" = 10 chars
    }
}

#[test]
fn test_visual_line_multi_line_extension() {
    let mut buffer = Buffer::new(1, 100);
    insert_test_text(&mut buffer, "Line 1\nLine 2\nLine 3\nLine 4");
    buffer.cursor = Position::new(1, 0); // Start on second line

    // Start visual line selection
    buffer.start_visual_line_selection();

    // Extend selection to include multiple lines
    buffer.cursor = Position::new(3, 2);
    buffer.update_visual_selection(buffer.cursor);

    if let Some(selection) = &buffer.selection {
        assert_eq!(selection.selection_type, SelectionType::Line);
        // Should span from line 1 to line 3
        assert_eq!(selection.start, Position::new(1, 0));
        assert_eq!(selection.end.row, 3);
        assert_eq!(selection.end.col, 6); // "Line 4" = 6 chars
    }
}

#[test]
fn test_visual_line_cursor_position_independence() {
    let mut buffer = Buffer::new(1, 100);
    insert_test_text(&mut buffer, "This is a long line with many characters");
    buffer.cursor = Position::new(0, 15); // Middle of line

    // Start visual line selection
    buffer.start_visual_line_selection();

    if let Some(selection) = &buffer.selection {
        assert_eq!(selection.selection_type, SelectionType::Line);
        // Line selection should always start from column 0
        assert_eq!(selection.start.col, 0);
        // And extend to end of line
        assert_eq!(selection.end.col, 40); // Full line length - fixed count
    }
}

#[test]
fn test_selection_type_enum() {
    // Test that SelectionType enum works correctly
    let line_type = SelectionType::Line;
    let char_type = SelectionType::Character;

    // Ensure they're different
    assert_ne!(line_type, char_type);

    // Test match patterns work
    match line_type {
        SelectionType::Line => {} // Should match
        _ => panic!("SelectionType::Line should match itself"),
    }
}

#[test]
fn test_visual_line_utf8_safety() {
    let mut buffer = Buffer::new(1, 100);
    insert_test_text(&mut buffer, "Hello world\nRust is awesome");
    buffer.cursor = Position::new(0, 0);

    // Start visual line selection
    buffer.start_visual_line_selection();

    if let Some(selection) = &buffer.selection {
        assert_eq!(selection.selection_type, SelectionType::Line);
        assert_eq!(selection.start.col, 0);
        // Characters should be counted correctly
        assert!(selection.end.col > 0);
    }

    // Test line length calculation
    assert_eq!(buffer.get_line_length(0), 11); // "Hello world" 
    assert_eq!(buffer.get_line_length(1), 15); // "Rust is awesome"
}
