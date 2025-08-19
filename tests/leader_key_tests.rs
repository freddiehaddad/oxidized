use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::input::keymap::KeyHandler;

fn editor() -> Result<Editor> {
    Editor::new()
}

#[test]
fn leader_space_m_p_toggles_markdown_preview() -> Result<()> {
    // Uses embedded keymaps.toml, which sets leader = "Space" and maps "leader m p"
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = editor()?;

    // Press <Space> m p
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
    )?;

    // Expect preview opened
    assert_eq!(editor.status_message(), "Markdown preview opened");

    // Press F8 should toggle it back (same action bound)
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE),
    )?;
    assert_eq!(editor.status_message(), "Markdown preview closed");

    Ok(())
}
