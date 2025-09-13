use core_model::{EditorModel, Layout};
use core_render::render_engine::RenderEngine;
use core_state::EditorState;
use core_text::Buffer;

fn mk_state(text: &str) -> EditorModel {
    let st = EditorState::new(Buffer::from_str("test", text).unwrap());
    EditorModel::new(st)
}

#[test]
fn scroll_shift_down_repaints_old_cursor_line() {
    // Buffer with enough lines.
    let mut model = mk_state("l0\nl1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\n");
    let mut eng = RenderEngine::new();
    // Layout: height 8 => text_height = 7 (rows 0..6)
    let layout = Layout::single(40, 8);
    {
        // Place cursor at bottom of initial viewport (line 6)
        let v = model.active_view_mut();
        v.cursor.line = 6;
        v.viewport_first_line = 0;
    }
    let view0 = model.active_view().clone();
    let status_line = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 40, 8, &status_line)
        .unwrap();
    // Now simulate scrolling down by 1 while cursor moves down one line staying at bottom.
    {
        let v = model.active_view_mut();
        v.viewport_first_line = 1; // old_first = 0, new_first = 1
        v.cursor.line = 7; // cursor moved with content
    }
    let view_after = model.active_view().clone();
    let status_line_after =
        core_render::render_engine::build_status_line(model.state(), &view_after);
    eng.render_scroll_shift(
        model.state(),
        &view_after,
        &layout,
        40,
        8,
        0,
        1,
        &status_line_after,
    )
    .unwrap();
    let snap = eng.metrics_snapshot();
    assert_eq!(snap.scroll_region_shifts, 1, "one shift executed");
    assert_eq!(snap.scroll_shift_degraded_full, 0, "no degradation");
    assert_eq!(snap.partial_frames, 1, "partial frame counted");
    // visible_rows=7, entering=1, plus old cursor line repaint => repainted_lines=2 => saved=5
    assert_eq!(
        snap.scroll_region_lines_saved, 5,
        "lines saved accounts for old cursor repaint"
    );
    let repainted = eng.test_last_repaint_lines();
    assert_eq!(
        repainted.len(),
        2,
        "two lines repainted: entering + old cursor"
    );
    assert!(repainted.contains(&6), "old cursor line repainted");
    assert!(
        repainted.contains(&7),
        "entering (and new cursor) line repainted"
    );
}
