use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::input::keymap::KeyHandler;

fn editor_with_buffer() -> Result<(Editor, KeyHandler)> {
    let mut editor = Editor::new()?;
    editor.create_buffer(None)?; // ensure a buffer exists
    let key_handler = KeyHandler::test_with_embedded();
    Ok((editor, key_handler))
}

#[test]
fn test_smart_tab_uses_shiftwidth_at_line_start() -> Result<()> {
    let (mut editor, mut kh) = editor_with_buffer()?;
    editor.set_config_setting_ephemeral("expandtab", "true");
    editor.set_config_setting_ephemeral("smarttab", "true");
    editor.set_config_setting_ephemeral("shiftwidth", "6");
    editor.set_config_setting_ephemeral("softtabstop", "3");

    // Enter insert mode (direct)
    editor.set_mode(oxidized::core::mode::Mode::Insert);
    // Press Tab
    kh.handle_key(&mut editor, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.get_line(0).unwrap(), "      "); // 6 spaces
    Ok(())
}

#[test]
fn test_tab_uses_softtabstop_mid_line() -> Result<()> {
    let (mut editor, mut kh) = editor_with_buffer()?;
    editor.set_config_setting_ephemeral("expandtab", "true");
    editor.set_config_setting_ephemeral("smarttab", "true");
    editor.set_config_setting_ephemeral("shiftwidth", "6");
    editor.set_config_setting_ephemeral("softtabstop", "3");

    // Enter insert mode and type characters
    editor.set_mode(oxidized::core::mode::Mode::Insert); // insert mode
    for ch in ['a', 'b'] {
        kh.handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
        )?;
    }
    kh.handle_key(&mut editor, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?; // should add 3 spaces

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.get_line(0).unwrap(), "ab "); // advances to next softtabstop boundary (3) from col 2
    Ok(())
}

#[test]
fn test_backspace_removes_to_previous_softtabstop_boundary() -> Result<()> {
    let (mut editor, mut kh) = editor_with_buffer()?;
    editor.set_config_setting_ephemeral("expandtab", "true");
    editor.set_config_setting_ephemeral("smarttab", "true");
    editor.set_config_setting_ephemeral("shiftwidth", "6");
    editor.set_config_setting_ephemeral("softtabstop", "3");

    editor.set_mode(oxidized::core::mode::Mode::Insert); // insert mode
    kh.handle_key(&mut editor, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?; // 6 spaces
    // Backspace once -> should remove 3 spaces leaving 3
    kh.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
    )?;
    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.get_line(0).unwrap(), "   ");
    Ok(())
}

// NOTE: Operator-based indentation (>>) not currently covered; pending dedicated motion support.

#[test]
fn test_smartindent_gates_block_indent() -> Result<()> {
    let (mut editor, mut kh) = editor_with_buffer()?;
    editor.set_config_setting_ephemeral("expandtab", "true");
    editor.set_config_setting_ephemeral("shiftwidth", "4");
    editor.set_config_setting_ephemeral("autoindent", "true");
    // First with smartindent ON (default)
    editor.set_mode(oxidized::core::mode::Mode::Insert); // insert mode
    kh.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('{'), KeyModifiers::NONE),
    )?;
    kh.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    )?;
    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.get_line(0).unwrap(), "{");
    assert_eq!(buf.get_line(1).unwrap(), "    "); // extra indent added

    // Disable smartindent and try again on a new buffer
    editor.create_buffer(None)?; // switches to new empty buffer
    editor.set_config_setting_ephemeral("smartindent", "false");
    editor.set_mode(oxidized::core::mode::Mode::Insert); // insert mode
    kh.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('{'), KeyModifiers::NONE),
    )?;
    kh.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
    )?;
    let buf2 = editor.current_buffer().unwrap();
    assert_eq!(buf2.get_line(0).unwrap(), "{");
    assert_eq!(buf2.get_line(1).unwrap(), ""); // no extra indent when smartindent disabled
    Ok(())
}
