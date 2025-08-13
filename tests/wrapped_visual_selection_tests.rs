use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::input::keymap::KeyHandler;
use oxidized::ui::UI;

/// Regression test: when entering visual mode on a wrapped line then pressing 'j',
/// the visual selection on the first logical line should span all of its wrapped
/// segments (not just the first segment).
#[test]
fn visual_v_then_j_highlights_all_wrapped_segments_of_first_line() -> Result<()> {
    // Prepare editor with two lines; first line will wrap into two segments with width=3
    let mut editor = Editor::new()?;
    let _ = editor.create_buffer(None)?;
    if let Some(buf) = editor.current_buffer_mut() {
        buf.lines = vec!["A🙂BC".to_string(), "zz".to_string()]; // 🙂 width 2
        buf.cursor.row = 0;
        buf.cursor.col = 0;
    }

    // Enable wrapping and set total window width so text area = 3 columns
    editor.set_config_setting_ephemeral("wrap", "true");
    editor.set_config_setting_ephemeral("linebreak", "false");
    if let Some(win) = editor.window_manager.current_window_mut() {
        let gutter = UI::new().compute_gutter_width(2);
        win.width = (3 + gutter) as u16; // 3 text columns
    }

    // Enter visual mode at (0,0), then press 'j' to extend selection to next line
    let mut key_handler = KeyHandler::new();
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE),
    )?;
    key_handler.handle_key(
        &mut editor,
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
    )?;

    // Get selection and verify it's present
    let buf = editor.current_buffer().unwrap();
    let selection = buf.get_selection().expect("selection should be active");
    assert_eq!(selection.start.row.min(selection.end.row), 0);

    // Compute wrapped segments for the first logical line using the same UI logic
    let ui = UI::new();
    let line0 = &buf.lines[0];
    let text_width = 3usize;
    let mut segments: Vec<(usize, usize)> = Vec::new(); // byte start..end for each segment
    let mut start = 0usize;
    loop {
        let (end, _c) = ui.wrap_next_end_byte(line0, start, text_width, false);
        segments.push((start, end));
        if end >= line0.len() {
            break;
        }
        start = end;
    }

    // Sanity: we expect exactly two segments for "A🙂BC" at width 3: "A🙂" and "BC"
    assert!(
        segments.len() >= 2,
        "expected at least two wrapped segments"
    );

    // Get the total char count of the full logical line
    let total_line_chars = line0.chars().count();

    // For each segment, compute the local highlight span by subtracting base_offset
    // from the core selection span on this line; ensure both segments have non-empty overlap
    let mut seg_has_highlight = Vec::new();
    for (seg_start_b, seg_end_b) in segments.iter().copied() {
        // base_offset in character columns from BOL to segment start
        let base_offset = line0[..seg_start_b].chars().count();
        // segment length in characters
        let seg_len = line0[seg_start_b..seg_end_b].chars().count();
        let span = selection
            .highlight_span_for_line(0, total_line_chars)
            .map(|(s, e)| (s.saturating_sub(base_offset), e.saturating_sub(base_offset)));
        let has_overlap = if let Some((s, e)) = span {
            // Overlap exists if [s,e) intersects [0, seg_len)
            s < seg_len && e > 0 && s < e
        } else {
            false
        };
        seg_has_highlight.push(has_overlap);
    }

    // Expect both the first and second wrapped segment of line 0 to be highlighted
    assert!(seg_has_highlight[0], "first segment should be highlighted");
    assert!(seg_has_highlight[1], "second segment should be highlighted");

    Ok(())
}
