use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::input::keymap::KeyHandler;

// Helper function to create a test editor instance
fn create_test_editor() -> Result<Editor> {
    Editor::new()
}

#[test]
fn test_keyhandler_creation() -> Result<()> {
    let _key_handler = KeyHandler::test_with_embedded();
    Ok(())
}

#[test]
fn test_basic_navigation() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test h key (left)
    let h_key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, h_key);

    // Test j key (down)
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, j_key);

    // Test k key (up)
    let k_key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, k_key);

    // Test l key (right)
    let l_key = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, l_key);

    Ok(())
}

#[test]
fn test_insert_mode_transition() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test i key (insert)
    let i_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, i_key);

    // Test a key (append)
    let a_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, a_key);

    // Test o key (open line below)
    let o_key = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, o_key);

    Ok(())
}

#[test]
fn test_visual_mode() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test v key (visual)
    let v_key = KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, v_key);

    // Test V key (visual line)
    let v_shift_key = KeyEvent::new(KeyCode::Char('V'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, v_shift_key);

    Ok(())
}

#[test]
fn test_command_mode() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test colon key (command mode)
    let colon_key = KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, colon_key);

    // Test escape to exit command mode
    let escape_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, escape_key);

    Ok(())
}

#[test]
fn test_character_input() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // First enter insert mode
    let i_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, i_key);

    // Test typing characters
    let chars = ['h', 'e', 'l', 'l', 'o'];
    for ch in &chars {
        let char_key = KeyEvent::new(KeyCode::Char(*ch), KeyModifiers::NONE);
        let _result = key_handler.handle_key(&mut editor, char_key);
    }

    Ok(())
}

#[test]
fn test_backspace_and_enter() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Enter insert mode
    let i_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, i_key);

    // Test backspace
    let backspace_key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, backspace_key);

    // Test enter
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, enter_key);

    Ok(())
}

#[test]
fn test_tab_and_shift_tab() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test tab
    let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, tab_key);

    // Test shift+tab
    let shift_tab_key = KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, shift_tab_key);

    Ok(())
}

#[test]
fn test_function_keys() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test F1 key
    let f1_key = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, f1_key);

    // Test F12 key
    let f12_key = KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, f12_key);

    Ok(())
}

#[test]
fn test_arrow_keys() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test arrow keys
    let up_key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, up_key);

    let down_key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, down_key);

    let left_key = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, left_key);

    let right_key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, right_key);

    Ok(())
}

#[test]
fn test_modifier_keys() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test Ctrl+key combinations
    let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let _result = key_handler.handle_key(&mut editor, ctrl_c);

    let ctrl_v = KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL);
    let _result = key_handler.handle_key(&mut editor, ctrl_v);

    // Test Alt+key combinations
    let alt_f = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT);
    let _result = key_handler.handle_key(&mut editor, alt_f);

    Ok(())
}

#[test]
fn test_command_sequence() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test entering command mode and typing a command
    let colon_key = KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, colon_key);

    // Type "quit" command
    let command_chars = ['q', 'u', 'i', 't'];
    for ch in command_chars {
        let char_key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
        let _result = key_handler.handle_key(&mut editor, char_key);
    }

    // Test enter to execute command
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, enter_key);

    Ok(())
}

#[test]
fn test_delete_operations() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test x key (delete character)
    let x_key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, x_key);

    // Test X key (delete character backward)
    let x_shift_key = KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, x_shift_key);

    Ok(())
}

#[test]
fn test_search_keys() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test forward slash (search forward)
    let slash_key = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, slash_key);

    // Test question mark (search backward)
    let question_key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, question_key);

    // Test n key (search next)
    let n_key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, n_key);

    // Test N key (search previous)
    let n_shift_key = KeyEvent::new(KeyCode::Char('N'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, n_shift_key);

    Ok(())
}

#[test]
fn test_word_movement() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test w key (word forward)
    let w_key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, w_key);

    // Test b key (word backward)
    let b_key = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, b_key);

    // Test e key (end of word)
    let e_key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, e_key);

    Ok(())
}

#[test]
fn test_line_movement() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test 0 key (beginning of line)
    let zero_key = KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, zero_key);

    // Test $ key (end of line)
    let dollar_key = KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, dollar_key);

    Ok(())
}

#[test]
fn test_page_movement() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test Ctrl+f (page down)
    let ctrl_f = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
    let _result = key_handler.handle_key(&mut editor, ctrl_f);

    // Test Ctrl+b (page up)
    let ctrl_b = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL);
    let _result = key_handler.handle_key(&mut editor, ctrl_b);

    Ok(())
}

#[test]
fn test_file_operations() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = create_test_editor()?;

    // Test Ctrl+s (save - if mapped)
    let ctrl_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
    let _result = key_handler.handle_key(&mut editor, ctrl_s);

    // Test Ctrl+o (open - if mapped)
    let ctrl_o = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
    let _result = key_handler.handle_key(&mut editor, ctrl_o);

    Ok(())
}

#[test]
fn test_undo_redo() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = create_test_editor()?;

    // Test u key (undo)
    let u_key = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, u_key);

    // Test Ctrl+r (redo)
    let ctrl_r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
    let _result = key_handler.handle_key(&mut editor, ctrl_r);

    Ok(())
}

#[test]
fn test_copy_paste() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = create_test_editor()?;

    // Test y key (yank/copy)
    let y_key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, y_key);

    // Test p key (paste)
    let p_key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, p_key);

    Ok(())
}

#[test]
fn test_special_keys() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = create_test_editor()?;

    // Test Home key
    let home_key = KeyEvent::new(KeyCode::Home, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, home_key);

    // Test End key
    let end_key = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, end_key);

    // Test Page Up key
    let page_up_key = KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, page_up_key);

    // Test Page Down key
    let page_down_key = KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, page_down_key);

    // Test Delete key
    let delete_key = KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE);
    let _result = key_handler.handle_key(&mut editor, delete_key);

    Ok(())
}

#[test]
fn test_complex_key_combinations() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = create_test_editor()?;

    // Test Ctrl+Shift combinations
    let ctrl_shift_p = KeyEvent::new(
        KeyCode::Char('p'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    let _result = key_handler.handle_key(&mut editor, ctrl_shift_p);

    // Test Alt+Shift combinations
    let alt_shift_f = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT | KeyModifiers::SHIFT);
    let _result = key_handler.handle_key(&mut editor, alt_shift_f);

    // Test unmapped key combinations
    let ctrl_shift_z = KeyEvent::new(
        KeyCode::Char('z'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );
    let _result = key_handler.handle_key(&mut editor, ctrl_shift_z);

    Ok(())
}
