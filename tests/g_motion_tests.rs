use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::input::keymap::KeyHandler;
use oxidized::ui::UI;

fn make_editor_with_text(text: &str) -> Result<Editor> {
    let mut editor = Editor::new()?;
    let _ = editor.create_buffer(None)?;
    // Populate buffer lines
    if let Some(buffer) = editor.current_buffer_mut() {
        buffer.lines = text.lines().map(|s| s.to_string()).collect();
        if buffer.lines.is_empty() {
            buffer.lines.push(String::new());
        }
        buffer.cursor.row = 0;
        buffer.cursor.col = 0;
    }
    Ok(editor)
}

#[test]
fn g0_with_wrap_moves_to_segment_start() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("A🙂BC")?; // 🙂 width 2

    // Enable wrapping and set a small window width so line wraps
    editor.set_config_setting_ephemeral("wrap", "true");
    editor.set_config_setting_ephemeral("linebreak", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        // UI text width is window.width - gutter, so include gutter to get 3 text columns
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16; // total window width
    }

    // Place cursor in second segment by moving to end of line first
    if let Some(buf) = editor.current_buffer_mut()
        && let Some(line) = buf.get_line(0)
    {
        buf.cursor.col = line.len();
    }

    // Press 'g' then '0'
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    // First segment should be "A🙂" (3 cols). Start of second segment is after that.
    let first_seg_end = "A🙂".len();
    assert_eq!(buf.cursor.col, first_seg_end);
    Ok(())
}

#[test]
fn g0_without_wrap_uses_horiz_offset_start() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    // String with an emoji (4 bytes, 1 grapheme) then ASCII
    let mut editor = make_editor_with_text("🙂Xabc")?;

    // Disable wrapping and set a non-zero horizontal offset
    editor.set_config_setting_ephemeral("wrap", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        win.horiz_offset = 1; // one grapheme to the right
    }

    // Put cursor somewhere not at start
    if let Some(buf) = editor.current_buffer_mut()
        && let Some(line) = buf.get_line(0)
    {
        buf.cursor.col = line.len();
    }

    // Press 'g' then '0'
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    // Start should be at the byte boundary after the first emoji grapheme
    let expected = "🙂".len();
    assert_eq!(buf.cursor.col, expected);
    Ok(())
}
#[test]
fn g0_moves_to_line_start_even_with_digit() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("hello")?;

    // Put cursor away from start
    if let Some(buf) = editor.current_buffer_mut() {
        buf.cursor.col = 3;
    }

    // Press 'g' then '0'
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.cursor.col, 0);
    Ok(())
}

#[test]
fn g_joins_moves_to_last_non_blank_in_normal_mode() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("abc   ")?; // trailing spaces

    // Move to end first to ensure we're not already there
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
    )?;

    // Press 'g' then '_'
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('_'), KeyModifiers::NONE),
    )?;

    // Expect cursor at index 2 (character 'c')
    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, 2);
    Ok(())
}

#[test]
fn g_joins_extends_in_visual_mode() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("hey   ")?;

    // Enter visual mode
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
    )?;
    // Move cursor right twice
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
    )?;

    // Press g_
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('_'), KeyModifiers::NONE),
    )?;

    // Expect selection to end at last non-blank index (2)
    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.end.row, 0);
    assert_eq!(sel.end.col, 2);
    Ok(())
}
