use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::input::keymap::KeyHandler;

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

// Regression: gm then entering visual mode and moving left should not panic and
// should create a small (1 char) selection regardless of internal start/end ordering.
#[test]
fn gm_then_visual_left_creates_valid_selection() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = make_editor_with_text("ABCDEFGHIJKLMNOPQRSTUVWXYZ")?;
    editor.set_config_setting_ephemeral("wrap", "false");

    // gm
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
    )?;

    // Enter visual
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
    )?;
    // Move left
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    let span = sel.end.col.abs_diff(sel.start.col);
    assert!(span <= 1, "Selection unexpectedly large: {}", span);
    Ok(())
}

// Regression: gm then Visual Line then left should not panic and selection should remain line-wise
#[test]
fn gm_then_visual_line_left_creates_valid_line_selection() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    let mut editor = make_editor_with_text("ABCDEFGHIJKLMNOPQRSTUVWXYZ\nsecond line")?;
    editor.set_config_setting_ephemeral("wrap", "false");

    // gm on first line
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
    )?;

    // Enter visual line mode (V)
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('V'), KeyModifiers::NONE),
    )?;
    // Move left (should keep full line selected)
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    // In line mode selection start.col == 0 and end.col == line length
    let line_len = buf.get_line(0).unwrap().len();
    assert_eq!(sel.start.row, 0);
    assert_eq!(sel.end.row, 0);
    assert_eq!(sel.start.col, 0);
    assert_eq!(sel.end.col, line_len);
    Ok(())
}

// Regression: gm then Visual Block then left should not panic and block should shrink/adjust safely
#[test]
fn gm_then_visual_block_left_creates_valid_block_selection() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    // Two lines to give block height context
    let mut editor = make_editor_with_text("ABCDEFGHIJKLMNOPQRSTUVWXYZ\nABCDEFGHIJKLMNOPQRSTUVWX")?;
    editor.set_config_setting_ephemeral("wrap", "false");

    // gm on first line
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
    )?;

    // Enter visual block mode (Ctrl+v not easily simulated; use alternative Alt+v if mapped, else assume mapping) -> here we simulate by sending the configured mapping: Ctrl+v
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
    )?;
    // Move left
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    // Block selection type should maintain same rows; columns differ by at most 1 after moving left
    let col_diff = sel.end.col.abs_diff(sel.start.col);
    assert!(
        col_diff <= 1,
        "Block selection width too large: {}",
        col_diff
    );
    Ok(())
}

// Regression follow-up: ensure multi-line visual block selection maintains a
// consistent width across all covered lines when expanding right/down.
#[test]
fn gm_visual_block_multi_line_width_consistent() -> Result<()> {
    let mut key_handler = KeyHandler::test_with_embedded();
    // Three lines, all sufficiently long and uniform ASCII for simple indexing
    let text = "ABCDEFGHIJKL\nMNOPQRSTUVWX\nYZABCDEFGHIJK"; // lengths: 12,12,12
    let mut editor = make_editor_with_text(text)?;
    editor.set_config_setting_ephemeral("wrap", "false");

    // Move to middle of first line via gm (roughly column 6 for 12 chars)
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
    )?;

    // Enter visual block mode (Ctrl+v mapping)
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL),
    )?;
    // Extend block two lines down
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
    )?;
    // Expand block two columns to the right
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some(), "Expected block selection");
    let sel = buf.selection.as_ref().unwrap();
    // Normalize coordinates
    let start_row = sel.start.row.min(sel.end.row);
    let end_row = sel.start.row.max(sel.end.row);
    let min_col = sel.start.col.min(sel.end.col);
    let max_col = sel.start.col.max(sel.end.col);
    let width = max_col.saturating_sub(min_col);
    // We moved two 'l' steps; expect width >= 2 (exact depends on initial gm landing). Allow >=2.
    assert!(width >= 2, "Block width too small: {}", width);
    // Assert each covered line can accommodate the block and implied substring has same width
    for r in start_row..=end_row {
        let line = buf.get_line(r).unwrap();
        assert!(line.len() >= max_col, "Line {} shorter than block end", r);
        // Count chars in slice (since ASCII, byte len equals char count)
        let slice = &line[min_col..max_col];
        assert_eq!(
            slice.chars().count(),
            width,
            "Inconsistent width at line {}",
            r
        );
    }
    Ok(())
}
