use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::input::keymap::KeyHandler;

fn create_editor_with_text(lines: &[&str]) -> Result<Editor> {
    let mut editor = Editor::new()?;
    // Create an empty buffer and populate with lines
    let _ = editor.create_buffer(None)?;
    if let Some(buf) = editor.current_buffer_mut() {
        buf.lines = lines.iter().map(|s| s.to_string()).collect();
        buf.cursor.row = 0;
        buf.cursor.col = 0;
    }
    Ok(editor)
}

#[test]
fn count_moves_cursor_down() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor =
        create_editor_with_text(&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"])?;

    // 1 0 j  (10j)
    let k1 = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
    key_handler.handle_key(&mut editor, k1)?;
    let k0 = KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE);
    key_handler.handle_key(&mut editor, k0)?;
    let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    key_handler.handle_key(&mut editor, j)?;

    assert_eq!(editor.current_buffer().unwrap().cursor.row, 10);
    Ok(())
}

#[test]
fn count_deletes_multiple_lines() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_editor_with_text(&["l1", "l2", "l3", "l4", "l5"])?;

    // 3 d d  (3dd)
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines.len(), 2);
    // Cursor stays on current line 0 after deleting first three
    assert_eq!(buf.cursor.row, 0);
    Ok(())
}

#[test]
fn count_deletes_chars_with_x() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_editor_with_text(&["abcdef"])?;

    // Move to start to be explicit
    {
        let b = editor.current_buffer_mut().unwrap();
        b.cursor.row = 0;
        b.cursor.col = 0;
    }

    // 3 x
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines[0].as_str(), "def");
    assert_eq!(buf.cursor.col, 0);
    Ok(())
}
