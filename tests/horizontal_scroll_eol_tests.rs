use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::ui::UI;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn make_editor_with_line(len: usize) -> Result<Editor> {
    let mut editor = Editor::new()?;
    let _ = editor.create_buffer(None)?;
    if let Some(buf) = editor.current_buffer_mut() {
        buf.lines = vec!["a".repeat(len)];
        buf.cursor.row = 0;
        buf.cursor.col = 0;
    }
    Ok(editor)
}

fn visible_cols_from_offset_to_cursor(editor: &oxidized::core::editor::Editor) -> usize {
    let buf = editor.current_buffer().unwrap();
    let win = editor.window_manager.current_window().unwrap();
    let line = buf.get_line(buf.cursor.row).unwrap();

    let start_byte = UI::floor_char_boundary(line, win.horiz_offset);
    let cursor_byte = buf.cursor.col.min(line.len());
    if start_byte >= cursor_byte {
        return 0;
    }
    let mut cols = 0usize;
    for g in line[start_byte..cursor_byte].graphemes(true) {
        cols += UnicodeWidthStr::width(g);
    }
    cols
}

fn text_width(editor: &oxidized::core::editor::Editor) -> usize {
    let buf = editor.current_buffer().unwrap();
    let win = editor.window_manager.current_window().unwrap();
    // Use a fresh UI instance; gutter width depends only on display flags,
    // which are at their defaults for this test (numbers on, marks on).
    let gutter = UI::new().compute_gutter_width(buf.lines.len());
    (win.width as usize).saturating_sub(gutter).max(1)
}

#[test]
fn zero_reveals_bol_in_no_wrap() -> Result<()> {
    // Long line and narrow window; start scrolled right, then go to BOL
    let mut editor = make_editor_with_line(50)?;
    editor.set_config_setting_ephemeral("wrap", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        win.width = 10;
        win.horiz_offset = 15; // simulate being scrolled right
    }

    // Move cursor to a later column first
    if let Some(buf) = editor.current_buffer_mut()
        && let Some(line) = buf.get_line(0)
    {
        buf.cursor.col = line.len().min(20);
    }

    // Press '0' (beginning of line)
    let mut key_handler = oxidized::input::keymap::KeyHandler::test_with_embedded();
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
    )?;

    // Trigger viewport update
    editor.render()?;

    // Cursor at start and visible
    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.cursor.col, 0);
    let cols = visible_cols_from_offset_to_cursor(&editor);
    assert_eq!(
        cols, 0,
        "At BOL, cursor visual cols from offset should be 0"
    );
    // Expect horiz_offset to be at the leftmost or near-leftmost position.
    // With default siso=0 in test config, we expect scrolling fully left.
    let win = editor.window_manager.current_window().unwrap();
    assert!(win.horiz_offset <= 1);
    Ok(())
}

#[test]
fn caret_reveals_first_nonblank_in_no_wrap() -> Result<()> {
    // Line with leading spaces; narrow window; ensure ^ goes to first non-blank and reveals it
    let mut editor = Editor::new()?;
    let _ = editor.create_buffer(None)?;
    if let Some(buf) = editor.current_buffer_mut() {
        buf.lines = vec!["    hello world".to_string()];
        buf.cursor.row = 0;
        buf.cursor.col = 0;
    }
    editor.set_config_setting_ephemeral("wrap", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        win.width = 10;
        win.horiz_offset = 8; // simulate scrolled right
    }

    // Move cursor near end
    if let Some(buf) = editor.current_buffer_mut()
        && let Some(line) = buf.get_line(0)
    {
        buf.cursor.col = line.len().saturating_sub(1);
    }

    // Map '^' to line_first_char per keymaps; action currently routes to line_start fallback
    let mut key_handler = oxidized::input::keymap::KeyHandler::test_with_embedded();
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('^'), KeyModifiers::NONE),
    )?;

    editor.render()?;

    // First non-blank should be at index 4
    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.cursor.col, 4);
    let cols = visible_cols_from_offset_to_cursor(&editor);
    let tw = text_width(&editor);
    assert!(cols < tw);
    Ok(())
}

#[test]
fn wrap_l_moves_to_next_line_at_eol() -> Result<()> {
    // Even with wrap enabled, pressing 'l' at EOL should go to next logical line
    let mut editor = Editor::new()?;
    let _ = editor.create_buffer(None)?;
    if let Some(buf) = editor.current_buffer_mut() {
        buf.lines = vec![
            // long enough to wrap for narrow window, but logic should still go to next line
            "abcdefghijabcdefghijabcdefghij".to_string(),
            "second".to_string(),
        ];
        buf.cursor.row = 0;
        buf.cursor.col = buf.lines[0].len();
    }
    editor.set_config_setting_ephemeral("wrap", "true");
    if let Some(win) = editor.window_manager.current_window_mut() {
        win.width = 10;
    }

    let mut key_handler = oxidized::input::keymap::KeyHandler::test_with_embedded();
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 0);
    Ok(())
}

#[test]
fn dollar_reveals_eol_in_no_wrap() -> Result<()> {
    // Setup: long line, small window, no wrapping
    let mut editor = make_editor_with_line(40)?;
    editor.set_config_setting_ephemeral("wrap", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        win.width = 10; // small window to force horizontal scroll (gutter eats ~4)
        win.horiz_offset = 0;
    }

    // Press '$' (end of line)
    let mut key_handler = oxidized::input::keymap::KeyHandler::test_with_embedded();
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
    )?;

    // Trigger viewport/horizontal offset update
    editor.render()?;

    // Cursor is at end of line
    let buf = editor.current_buffer().unwrap();
    let line = buf.get_line(0).unwrap();
    assert_eq!(buf.cursor.col, line.len());

    // And the end is visible within the text region
    let cols = visible_cols_from_offset_to_cursor(&editor);
    let tw = text_width(&editor);
    // With siso=0, renderer keeps cursor strictly inside right edge
    assert!(
        cols < tw,
        "cursor cols {} should be within text width {}",
        cols,
        tw
    );
    Ok(())
}

#[test]
fn l_reaches_eol_in_no_wrap() -> Result<()> {
    // Setup: long line, small window, no wrapping
    let mut editor = make_editor_with_line(30)?;
    editor.set_config_setting_ephemeral("wrap", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        win.width = 12;
        win.horiz_offset = 0;
    }

    // Move near end
    if let Some(buf) = editor.current_buffer_mut()
        && let Some(line) = buf.get_line(0)
    {
        buf.cursor.col = line.len().saturating_sub(1);
    }

    // Press 'l' to step to EOL
    let mut key_handler = oxidized::input::keymap::KeyHandler::test_with_embedded();
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
    )?;

    // Trigger viewport/horizontal offset update
    editor.render()?;

    // Cursor reached end of line and is visible
    let buf = editor.current_buffer().unwrap();
    let line = buf.get_line(0).unwrap();
    assert_eq!(buf.cursor.col, line.len());

    let cols = visible_cols_from_offset_to_cursor(&editor);
    let tw = text_width(&editor);
    assert!(
        cols < tw,
        "cursor cols {} should be within text width {}",
        cols,
        tw
    );
    Ok(())
}
