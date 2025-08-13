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
fn g_dollar_with_wrap_moves_to_segment_end() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("A🙂BC")?; // 🙂 width 2

    // Enable wrapping and set a small window width so line wraps into segments of 3 cols
    editor.set_config_setting_ephemeral("wrap", "true");
    editor.set_config_setting_ephemeral("linebreak", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16; // total window width => 3 text columns
    }

    // Place cursor at start (in first segment) and press 'g$'
    if let Some(buf) = editor.current_buffer_mut() {
        buf.cursor.col = 0;
    }
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    // First segment is "A🙂"; end of display segment is after the emoji.
    // Cursor should land on the last grapheme in the segment, i.e., the emoji's start.
    let expected_pos = "A".len();
    assert_eq!(buf.cursor.row, 0);
    assert_eq!(buf.cursor.col, expected_pos);
    Ok(())
}

#[test]
fn g_dollar_without_wrap_does_not_scroll() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("🙂Xabc")?; // emoji then ASCII

    // Disable wrapping and set a small content width; start after the emoji
    editor.set_config_setting_ephemeral("wrap", "false");
    editor.set_config_setting_ephemeral("sidescrolloff", "0");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16; // 3 text columns visible
        win.horiz_offset = 1; // start after the first grapheme (emoji)
    }

    // Press 'g$'
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
    )?;

    // Expect: cursor at end of visible segment (3 cols from start = Xab -> on 'b')
    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.cursor.row, 0);
    let expected_b_start = "🙂Xa".len(); // start byte of 'b'
    assert_eq!(buf.cursor.col, expected_b_start);

    // And no horizontal scroll should have occurred
    let win = editor.window_manager.current_window().unwrap();
    assert_eq!(win.horiz_offset, 1);
    Ok(())
}

#[test]
fn g_dollar_extends_in_visual_mode_with_wrap() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("A🙂BC")?; // 🙂 width 2

    // Enable wrapping and set a small window width so line wraps into 3-col segment
    editor.set_config_setting_ephemeral("wrap", "true");
    editor.set_config_setting_ephemeral("linebreak", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16;
    }

    // Start at col 0, enter visual mode, then g$
    if let Some(buf) = editor.current_buffer_mut() {
        buf.cursor.col = 0;
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
        KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    // Selection end should be at the last grapheme in first segment (emoji start)
    let expected_end = "A".len();
    assert_eq!(sel.end.row, 0);
    assert_eq!(sel.end.col, expected_end);
    // Selection start stayed at 0
    assert_eq!(sel.start.col, 0);
    Ok(())
}

#[test]
fn g_dollar_extends_in_visual_mode_without_wrap_no_scroll() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("🙂Xabc")?;

    // No wrap; narrow content width with horiz_offset=1 so visible starts after emoji
    editor.set_config_setting_ephemeral("wrap", "false");
    editor.set_config_setting_ephemeral("sidescrolloff", "0");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16; // 3 text columns
        win.horiz_offset = 1;
    }

    // Place cursor at visible start (after emoji), enter visual, then g$
    if let Some(buf) = editor.current_buffer_mut() {
        buf.cursor.col = "🙂".len();
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
        KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    // End should be at last visible column: on 'b'
    let expected_end = "🙂Xa".len();
    assert_eq!(sel.end.row, 0);
    assert_eq!(sel.end.col, expected_end);
    // Start should be at visible start
    assert_eq!(sel.start.col, "🙂".len());

    // No horizontal scroll occurred
    let win = editor.window_manager.current_window().unwrap();
    assert_eq!(win.horiz_offset, 1);
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

#[test]
fn g0_extends_in_visual_mode_with_wrap() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("A🙂BC")?; // 🙂 width 2

    // Enable wrapping with 3 text columns so first segment is "A🙂"
    editor.set_config_setting_ephemeral("wrap", "true");
    editor.set_config_setting_ephemeral("linebreak", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16;
    }

    // Place cursor at end of line to ensure movement is visible
    if let Some(buf) = editor.current_buffer_mut()
        && let Some(line) = buf.get_line(0)
    {
        buf.cursor.col = line.len();
    }

    // Enter visual then g0
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
        KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    // Start at original cursor (end), end should be at start of second segment (after "A🙂")
    let second_seg_start = "A🙂".len();
    assert_eq!(sel.end.row, 0);
    assert_eq!(sel.end.col, second_seg_start);
    Ok(())
}

#[test]
fn g0_extends_in_visual_mode_without_wrap_no_scroll() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("🙂Xabc")?; // emoji then ASCII

    // No wrap; 3 text columns visible, offset 1 so visible starts after emoji
    editor.set_config_setting_ephemeral("wrap", "false");
    editor.set_config_setting_ephemeral("sidescrolloff", "0");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16;
        win.horiz_offset = 1;
    }

    // Put cursor near end so selection extends left
    if let Some(buf) = editor.current_buffer_mut() {
        buf.cursor.col = "🙂Xab".len(); // on 'b'
    }

    // Enter visual then g0
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
        KeyEvent::new(KeyCode::Char('0'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    // End should be at visible start (after emoji)
    let visible_start = "🙂".len();
    assert_eq!(sel.end.row, 0);
    assert_eq!(sel.end.col, visible_start);
    // No horizontal scroll occurred
    let win = editor.window_manager.current_window().unwrap();
    assert_eq!(win.horiz_offset, 1);
    Ok(())
}

#[test]
fn gj_with_wrap_moves_within_wrapped_segments() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("A🙂BC")?; // 🙂 width 2

    // Enable wrapping into 3 text columns so segments: "A🙂" | "BC"
    editor.set_config_setting_ephemeral("wrap", "true");
    editor.set_config_setting_ephemeral("linebreak", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(1);
        win.width = (3 + gutter) as u16; // total window width
    }

    // Start at column 1 (on the emoji start), then gj moves to same visual col in next segment
    if let Some(buf) = editor.current_buffer_mut() {
        buf.cursor.col = "A".len();
    }

    // Press 'g' then 'j'
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.cursor.row, 0); // same logical line
    // Next segment is "BC"; desired visual col from seg start was 1 (after 'A')
    // In next segment, that lands on 'C' (byte index of 'C' is "A🙂B".len())
    let expected_col = "A🙂B".len();
    assert_eq!(buf.cursor.col, expected_col);
    Ok(())
}

#[test]
fn gj_with_wrap_moves_to_next_line_when_last_segment() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("A🙂BC\nxyz")?; // two lines

    // Wrap into 3 columns
    editor.set_config_setting_ephemeral("wrap", "true");
    editor.set_config_setting_ephemeral("linebreak", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(2);
        win.width = (3 + gutter) as u16;
    }

    // Put cursor in last segment of first line (on 'C')
    if let Some(buf) = editor.current_buffer_mut() {
        buf.cursor.row = 0;
        buf.cursor.col = "A🙂B".len(); // on 'C'
    }

    // gj should move to line 1, first segment, similar visual column
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.cursor.row, 1);
    // With desired visual col = 1, on "xyz" it should be 'y'
    let expected = "x".len();
    assert_eq!(buf.cursor.col, expected);
    Ok(())
}

#[test]
fn gj_without_wrap_moves_down_one_buffer_line_preserving_byte_col() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("🙂ab\n🙂xyz")?;

    editor.set_config_setting_ephemeral("wrap", "false");
    editor.set_config_setting_ephemeral("sidescrolloff", "0");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(2);
        win.width = (5 + gutter) as u16;
        win.horiz_offset = 0;
    }

    if let Some(buf) = editor.current_buffer_mut() {
        buf.cursor.row = 0;
        buf.cursor.col = "🙂a".len(); // on 'b'
    }

    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.cursor.row, 1);
    // Should land at same byte col if valid; line starts with emoji then 'x'
    let expected = "🙂x".len();
    assert_eq!(buf.cursor.col, expected);
    Ok(())
}

#[test]
fn gj_extends_selection_in_visual_mode() -> Result<()> {
    let mut key_handler = KeyHandler::new();
    let mut editor = make_editor_with_text("A🙂BC\nDEF")?;

    // Wrap into 3 cols so first line segments: "A🙂" | "BC"
    editor.set_config_setting_ephemeral("wrap", "true");
    editor.set_config_setting_ephemeral("linebreak", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(2);
        win.width = (3 + gutter) as u16;
    }

    // Start on '🙂' (byte after 'A'), enter visual, then gj
    if let Some(buf) = editor.current_buffer_mut() {
        buf.cursor.row = 0;
        buf.cursor.col = "A".len();
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
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
    )?;

    let buf = editor.current_buffer().unwrap();
    assert!(buf.selection.is_some());
    let sel = buf.selection.as_ref().unwrap();
    assert!(sel.end.row >= sel.start.row);
    Ok(())
}
