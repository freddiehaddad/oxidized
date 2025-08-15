use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::input::keymap::KeyHandler;

fn make_editor() -> Result<Editor> {
    Editor::new()
}

fn make_editor_with_buffer() -> Result<Editor> {
    let mut editor = Editor::new()?;
    editor.create_buffer(None)?;
    Ok(editor)
}

#[test]
fn macro_register_selection_does_not_trigger_mappings() -> Result<()> {
    let mut handler = KeyHandler::test_with_embedded();
    let mut editor = make_editor()?;

    // Press 'q' to arm register selection
    let q_key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    handler.handle_key(&mut editor, q_key)?;

    assert!(
        handler.pending_macro_register,
        "expected pending register after 'q'"
    );
    assert!(
        !editor.is_macro_recording(),
        "should not be recording until register provided"
    );

    // Next 'a' should be consumed as register, not as normal-mode 'a' (insert_after)
    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    handler.handle_key(&mut editor, a_key)?;

    // Should now be recording, still in normal mode (no accidental insert mode)
    assert!(
        editor.is_macro_recording(),
        "should start recording after providing register"
    );
    assert_eq!(
        editor.mode(),
        oxidized::core::mode::Mode::Normal,
        "register selection must not trigger mappings"
    );

    Ok(())
}

#[test]
fn macro_q_toggles_stop() -> Result<()> {
    let mut handler = KeyHandler::test_with_embedded();
    let mut editor = make_editor()?;

    // Start recording: q a
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
    )?;
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
    )?;
    assert!(editor.is_macro_recording(), "recording should be active");

    // Press q again to stop
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
    )?;
    assert!(!editor.is_macro_recording(), "recording should stop on 'q'");

    Ok(())
}

#[test]
fn macro_pending_register_can_cancel_with_esc() -> Result<()> {
    let mut handler = KeyHandler::test_with_embedded();
    let mut editor = make_editor()?;

    // Arm register selection
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
    )?;
    assert!(handler.pending_macro_register);

    // Press Esc cancels selection
    handler.handle_key(&mut editor, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;

    assert!(
        !handler.pending_macro_register,
        "pending register should be cleared after Esc"
    );
    assert!(
        !editor.is_macro_recording(),
        "should not start recording on cancel"
    );

    Ok(())
}

#[test]
fn macro_at_arms_pending_and_executes_on_register() -> Result<()> {
    let mut handler = KeyHandler::test_with_embedded();
    let mut editor = make_editor_with_buffer()?;

    // Record a simple macro 'a' that inserts 'x'
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
    )?;
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
    )?;
    assert!(editor.is_macro_recording());

    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
    )?;
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
    )?;
    handler.handle_key(&mut editor, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;

    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
    )?;
    assert!(!editor.is_macro_recording());

    // Execute macro via '@' 'a'
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE),
    )?;
    // Should be awaiting register now
    assert!(
        handler.pending_macro_execute,
        "expected pending execute after '@'"
    );
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
    )?;

    // After execution, buffer should contain 'x' on first line
    let buf = editor.current_buffer().unwrap();
    let line = buf.get_line(0).map_or("", |s| s.as_str());
    assert!(line.contains('x'));
    Ok(())
}

#[test]
fn macro_at_at_repeats_last_macro() -> Result<()> {
    let mut handler = KeyHandler::test_with_embedded();
    let mut editor = make_editor_with_buffer()?;

    // Record macro 'b' that inserts 'y'
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
    )?;
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
    )?;
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
    )?;
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
    )?;
    handler.handle_key(&mut editor, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
    )?;

    // Play with '@b'
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE),
    )?;
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
    )?;

    // Now '@@' should repeat last macro (register 'b')
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE),
    )?;
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE),
    )?;

    // Buffer should contain two 'y' characters inserted total
    let buf = editor.current_buffer().unwrap();
    let line = buf.get_line(0).map_or("", |s| s.as_str());
    let y_count = line.chars().filter(|&c| c == 'y').count();
    assert!(
        y_count >= 2,
        "expected at least two 'y' after executing and repeating macro"
    );
    Ok(())
}

#[test]
fn macro_single_at_only_arms_pending_no_execution() -> Result<()> {
    let mut handler = KeyHandler::test_with_embedded();
    let mut editor = make_editor()?;

    // Ensure no last macro exists
    assert!(editor.get_last_played_macro_register().is_none());

    // Single '@' should arm pending and not error or execute
    handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('@'), KeyModifiers::NONE),
    )?;
    assert!(handler.pending_macro_execute);

    // Cancel with Esc
    handler.handle_key(&mut editor, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert!(!handler.pending_macro_execute);
    Ok(())
}
