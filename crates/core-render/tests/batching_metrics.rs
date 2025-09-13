use core_model::EditorModel;
use core_render::render_engine::RenderEngine;
use core_state::EditorState;
use core_text::Buffer;

fn mk_model(text: &str) -> EditorModel {
    let st = EditorState::new(Buffer::from_str("test", text).unwrap());
    EditorModel::new(st)
}

#[test]
fn batched_full_frame_print_commands_not_exceed_cells() {
    let model = mk_model("hello world\nsecond line here\nthird line\n");
    let mut eng = RenderEngine::new();
    let view = model.active_view().clone();
    let layout = core_model::Layout::single(80, 10);
    let status_line_initial = core_render::render_engine::build_status_line(model.state(), &view);
    eng.render_full(model.state(), &view, &layout, 80, 10, &status_line_initial)
        .unwrap();
    let status_line = core_render::render_engine::build_status_line(model.state(), &view);
    eng.render_full(model.state(), &view, &layout, 100, 8, &status_line)
        .unwrap();
    let snap = eng.metrics_snapshot();
    assert!(
        snap.print_commands <= snap.cells_printed,
        "print commands must not exceed cells printed"
    );
    // Expect some batching: at least one reduction vs naive 80*height possible. Just assert > 0 cells.
    assert!(snap.cells_printed > 0);
}
