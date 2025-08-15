use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::core::mode::{Mode, Position, SelectionType};
use oxidized::input::keymap::KeyHandler;

#[test]
fn test_visual_line_cursor_movement_updates_selection() {
    let mut editor = Editor::new().unwrap();
    let mut key_handler = KeyHandler::test_with_embedded();

    // Insert some test content
    let test_content = vec![
        "First line of text".to_string(),
        "Second line of text".to_string(),
        "Third line of text".to_string(),
        "Fourth line of text".to_string(),
    ];

    if let Some(buffer) = editor.current_buffer_mut() {
        buffer.lines = test_content;
        buffer.cursor = Position::new(0, 0);
    }

    // Start visual line mode with Shift+V
    let shift_v = KeyEvent::new(KeyCode::Char('V'), KeyModifiers::SHIFT);
    key_handler.handle_key(&mut editor, shift_v).unwrap();

    // Should be in visual line mode now
    assert_eq!(editor.mode(), Mode::VisualLine);

    // Check initial selection
    if let Some(buffer) = editor.current_buffer() {
        assert!(buffer.selection.is_some());
        let selection = buffer.selection.as_ref().unwrap();
        assert_eq!(selection.selection_type, SelectionType::Line);
        assert_eq!(selection.start.row, 0);
        assert_eq!(selection.end.row, 0);
    }

    // Move down with 'j' - this should extend the selection
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    key_handler.handle_key(&mut editor, j_key).unwrap();

    // Check that selection was extended to include line 1
    if let Some(buffer) = editor.current_buffer() {
        let selection = buffer.selection.as_ref().unwrap();
        assert_eq!(selection.selection_type, SelectionType::Line);
        assert_eq!(selection.start.row, 0);
        assert_eq!(selection.end.row, 1); // Should now include line 1
    }

    // Move down again with 'j' - should extend to line 2
    key_handler.handle_key(&mut editor, j_key).unwrap();

    // Check that selection now includes line 2
    if let Some(buffer) = editor.current_buffer() {
        let selection = buffer.selection.as_ref().unwrap();
        assert_eq!(selection.selection_type, SelectionType::Line);
        assert_eq!(selection.start.row, 0);
        assert_eq!(selection.end.row, 2); // Should now include line 2
    }

    // Move down one more time with 'j' - should extend to line 3
    key_handler.handle_key(&mut editor, j_key).unwrap();

    // Check final selection state
    if let Some(buffer) = editor.current_buffer() {
        let selection = buffer.selection.as_ref().unwrap();
        assert_eq!(selection.selection_type, SelectionType::Line);
        assert_eq!(selection.start.row, 0);
        assert_eq!(selection.end.row, 3); // Should now include line 3
    }
}

#[test]
fn test_visual_line_yank_multiple_lines() {
    let mut editor = Editor::new().unwrap();
    let mut key_handler = KeyHandler::test_with_embedded();

    // Insert test content
    let test_content = vec![
        "Line 1".to_string(),
        "Line 2".to_string(),
        "Line 3".to_string(),
        "Line 4".to_string(),
    ];

    if let Some(buffer) = editor.current_buffer_mut() {
        buffer.lines = test_content;
        buffer.cursor = Position::new(0, 0);
    }

    // Start visual line mode
    let shift_v = KeyEvent::new(KeyCode::Char('V'), KeyModifiers::SHIFT);
    key_handler.handle_key(&mut editor, shift_v).unwrap();

    // Move down twice to select 3 lines (0, 1, 2)
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    key_handler.handle_key(&mut editor, j_key).unwrap();
    key_handler.handle_key(&mut editor, j_key).unwrap();

    // Yank the selection
    let y_key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
    key_handler.handle_key(&mut editor, y_key).unwrap();

    // Should return to normal mode after yank
    assert_eq!(editor.mode(), Mode::Normal);

    // Move to line 3 and paste
    if let Some(buffer) = editor.current_buffer_mut() {
        buffer.cursor = Position::new(3, 0);
    }

    let p_key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
    key_handler.handle_key(&mut editor, p_key).unwrap();

    // Check that 3 lines were pasted
    if let Some(buffer) = editor.current_buffer() {
        assert_eq!(buffer.lines.len(), 7); // 4 original + 3 pasted
        assert_eq!(buffer.lines[4], "Line 1");
        assert_eq!(buffer.lines[5], "Line 2");
        assert_eq!(buffer.lines[6], "Line 3");
    }
}
