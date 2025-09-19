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
    engine.render_full(&state, &view, &layout, w, h).unwrap();
    // Move cursor horizontally within same line several times; emulate CursorOnly partial path call.
    for col in 1..4 {
        // move to columns 1..3
        view.cursor.byte = col; // byte index equals col for ASCII
        let layout = core_model::Layout::single(w, h);
        engine
            .render_cursor_only(&state, &view, &layout, w, h)
            .unwrap();
        let snap = engine.metrics_snapshot();
        assert!(snap.cursor_only_frames >= col as u64); // monotonically increasing
    }
}
