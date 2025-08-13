use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::input::keymap::KeyHandler;

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
