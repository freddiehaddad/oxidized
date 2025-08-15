use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::input::keymap::KeyHandler;
use oxidized::ui::UI;

fn make_editor_with_text(text: &str) -> Result<Editor> {
    let mut editor = Editor::new()?;
    let _ = editor.create_buffer(None)?;
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
fn g_caret_with_wrap_moves_to_segment_first_nonblank() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    // Leading spaces ensure first-nonblank isn't at segment start
    let mut editor = make_editor_with_text("  A🙂B")?; // 🙂 width 2, wraps into 2 segments

    // Enable wrapping and set a small window width so line wraps
    editor.set_config_setting_ephemeral("wrap", "true");
    editor.set_config_setting_ephemeral("linebreak", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16; // 3 text columns
    }

    // Place cursor in second segment by moving to end of line first
    if let Some(buf) = editor.current_buffer_mut()
        && let Some(line) = buf.get_line(0)
    {
        buf.cursor.col = line.len();
    }

    // Press 'g' then '^'
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('^'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    // With 3 columns, segments: "  A" and "🙂B". Cursor at EOL -> in segment 2.
    // First nonblank in segment 2 is the emoji at byte index 3.
    assert_eq!(buf.cursor.col, 3);
    Ok(())
}

#[test]
fn g_caret_without_wrap_uses_visible_first_nonblank() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = make_editor_with_text("  abcd")?;

    // No wrap; set horiz_offset so visible window starts at first space, 3 text cols width
    editor.set_config_setting_ephemeral("wrap", "false");
    editor.set_config_setting_ephemeral("sidescrolloff", "0");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16; // 3 columns visible
        win.horiz_offset = 0; // start at beginning
    }

    // Cursor somewhere else
    if let Some(buf) = editor.current_buffer_mut() {
        buf.cursor.col = 5;
    }

    // Press g^
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('^'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    // First three columns are two spaces and 'a'; first non-blank at byte index 2.
    assert_eq!(buf.cursor.col, 2);
    Ok(())
}

#[test]
fn g_caret_extends_in_visual_mode_with_wrap() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = make_editor_with_text("  A🙂B")?;

    // Enable wrapping
    editor.set_config_setting_ephemeral("wrap", "true");
    editor.set_config_setting_ephemeral("linebreak", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16;
    }

    // Put cursor at line end, enter visual, then g^
    if let Some(buf) = editor.current_buffer_mut()
        && let Some(line) = buf.get_line(0)
    {
        buf.cursor.col = line.len();
    }
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('^'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    assert_eq!(sel.end.row, 0);
    assert_eq!(sel.end.col, 3); // first non-blank in segment 2 (emoji) at byte index 3
    Ok(())
}
