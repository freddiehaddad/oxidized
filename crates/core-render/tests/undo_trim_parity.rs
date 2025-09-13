use core_model::{Layout, View, ViewId};
use core_render::render_engine::RenderEngine;
use core_state::EditorState;
use core_text::{Buffer, Position};

// Regression (Phase 5 / Step 0.5): After undoing an insert run, subsequent cursor-only
// repaints must not leave the line truncated. We simulate an insert run mid-line,
// undo it, then perform cursor-only renders.
#[test]
fn undo_after_insert_then_cursor_motion_preserves_line() {
    let original = "# Oxidized\n";
    let buf = Buffer::from_str("test", original).unwrap();
    let mut state = EditorState::new(buf);
    let mut view = View::new(ViewId(0), 0, Position::origin(), 0);

    // Insert run at byte 2 (after '# ').
    view.cursor.byte = 2;
    state.begin_insert_coalescing(view.cursor);
    {
        let active = state.active_buffer_mut();
        active.insert_grapheme(&mut view.cursor, "h");
        active.insert_grapheme(&mut view.cursor, "e");
        active.insert_grapheme(&mut view.cursor, "l");
        active.insert_grapheme(&mut view.cursor, "l");
        active.insert_grapheme(&mut view.cursor, "o");
    }
    state.note_insert_edit();
    state.end_insert_coalescing();

    let mut engine = RenderEngine::new();
    let layout = Layout::single(120, 20);
    engine
        .render_full(&state, &view, &layout, 120, 20, "")
        .unwrap();

    // Undo run
    let mut cur = view.cursor;
    assert!(state.undo(&mut cur));
    view.cursor = cur; // restored (byte 2)

    engine
        .render_full(&state, &view, &layout, 120, 20, "")
        .unwrap();

    // Cursor movement simulation: move to start, repaint, then back to insert point.
    view.cursor.byte = 0;
    engine
        .render_cursor_only(&state, &view, &layout, 120, 20, "")
        .unwrap();
    view.cursor.byte = 2;
    engine
        .render_cursor_only(&state, &view, &layout, 120, 20, "")
        .unwrap();

    let line = state.active_buffer().line(0).unwrap();
    assert_eq!(line, original, "expected original line, got: {line}");
}
