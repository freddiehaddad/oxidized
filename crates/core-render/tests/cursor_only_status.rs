use core_model::View;
use core_render::render_engine::RenderEngine;
use core_state::{EditorState, Mode};
use core_text::{Buffer, Position};

// Helper to build a minimal state + view.
fn setup(buffer_content: &str) -> (EditorState, View, RenderEngine, u16, u16) {
    let buf = Buffer::from_str("test", buffer_content).unwrap();
    let mut state = EditorState::new(buf);
    state.mode = Mode::Normal;
    state.last_text_height = 5; // pretend 6 rows incl status
    let view = View {
        id: core_model::ViewId(0),
        buffer_index: 0,
        cursor: Position::new(0, 0),
        viewport_first_line: 0,
    };
    (state, view, RenderEngine::new(), 40, 6)
}

#[test]
fn cursor_only_horizontal_updates_status() {
    let (state, mut view, mut engine, w, h) = setup("abcdef\n");
    // Initial full render to warm cache.
    let layout = core_model::Layout::single(w, h);
    let status_line = core_render::render_engine::build_status_line(&state, &view);
    engine
        .render_full(&state, &view, &layout, w, h, &status_line)
        .unwrap();
    // Move cursor horizontally within same line several times; emulate CursorOnly partial path call.
    for col in 1..4 {
        // move to columns 1..3
        view.cursor.byte = col; // byte index equals col for ASCII
        let layout = core_model::Layout::single(w, h);
        let status_line_iter = core_render::render_engine::build_status_line(&state, &view);
        engine
            .render_cursor_only(&state, &view, &layout, w, h, &status_line_iter)
            .unwrap();
        let snap = engine.metrics_snapshot();
        assert!(snap.cursor_only_frames >= col as u64); // monotonically increasing
    }
}

// Regression test: cursor moves onto an empty line; cursor-only partial render
// path should still treat cursor as occupying at least one cell (span width >=1)
// and update last_cursor_line metadata accordingly.
#[test]
fn cursor_only_partial_empty_line_cursor_span_width() {
    let (state, mut view, mut engine, w, h) = setup("hello\n\nworld\n");
    let layout = core_model::Layout::single(w, h);
    // Warm full render.
    let status0 = core_render::render_engine::build_status_line(&state, &view);
    engine
        .render_full(&state, &view, &layout, w, h, &status0)
        .unwrap();
    // Move to empty second line.
    view.cursor.line = 1;
    view.cursor.byte = 0;
    let status1 = core_render::render_engine::build_status_line(&state, &view);
    engine
        .render_cursor_only(&state, &view, &layout, w, h, &status1)
        .unwrap();
    assert_eq!(engine.last_cursor_line(), Some(1));
}

// Regression test for lines-partial path: after marking a line dirty and moving cursor
// onto an empty line, the partial render should preserve cursor metadata.
#[test]
fn lines_partial_empty_line_cursor_span_width() {
    let (mut state, mut view, mut engine, w, h) = setup("alpha\nsecond\n\nthird\n");
    let layout = core_model::Layout::single(w, h);
    // Full render first.
    let status0 = core_render::render_engine::build_status_line(&state, &view);
    engine
        .render_full(&state, &view, &layout, w, h, &status0)
        .unwrap();
    // Simulate an edit to mark first line dirty.
    {
        let buf = state.active_buffer_mut();
        let mut pos = Position { line: 0, byte: 5 };
        buf.insert_grapheme(&mut pos, "x");
    }
    // Move cursor to empty line (index 2)
    view.cursor.line = 2;
    view.cursor.byte = 0;
    let status1 = core_render::render_engine::build_status_line(&state, &view);
    let mut dirty = core_render::dirty::DirtyLinesTracker::default();
    dirty.mark(0);
    engine
        .render_lines_partial(&state, &view, &layout, w, h, &mut dirty, &status1)
        .unwrap();
    assert_eq!(engine.last_cursor_line(), Some(2));
}

#[test]
fn status_line_includes_ephemeral_open_failure() {
    use core_model::EditorModel;
    use core_render::render_engine::build_status_line_with_ephemeral;
    use core_state::EditorState;
    use core_text::Buffer;
    let buf = Buffer::from_str("test", "").unwrap();
    let mut model = EditorModel::new(EditorState::new(buf));
    {
        let st = model.state_mut();
        st.set_ephemeral("Open failed", std::time::Duration::from_secs(3));
    }
    let view = model.active_view().clone();
    // Choose width large enough to fit base + ephemeral (80 to ensure padding path).
    let status = build_status_line_with_ephemeral(model.state(), &view, 80);
    assert!(
        status.contains("Open failed"),
        "Ephemeral message should be present in status line"
    );
}
