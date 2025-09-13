use core_model::{EditorModel, Layout};
use core_render::render_engine::RenderEngine;
use core_state::EditorState;
use core_text::Buffer;

fn mk(text: &str) -> EditorModel {
    let st = EditorState::new(Buffer::from_str("test", text).unwrap());
    EditorModel::new(st)
}

// Helper: perform a scroll shift after adjusting viewport_first_line and optional cursor line.
fn shift(
    eng: &mut RenderEngine,
    model: &mut EditorModel,
    layout: &Layout,
    old_first: usize,
    new_first: usize,
) {
    let view = model.active_view().clone();
    let status_line = core_render::render_engine::build_status_line(model.state(), &view);
    eng.render_scroll_shift(
        model.state(),
        &view,
        layout,
        80,
        layout.primary().height,
        old_first,
        new_first,
        &status_line,
    )
    .unwrap();
}

#[test]
fn scroll_shift_sequential_down_then_up_invariants() {
    // 25 lines, scroll small deltas within threshold.
    let mut model = mk(&(0..25).map(|i| format!("l{}\n", i)).collect::<String>());
    let mut eng = RenderEngine::new();
    let layout = Layout::single(80, 10); // text height 9
    // Initial state: cursor at line 0, full render to warm cache.
    let initial_view = model.active_view().clone();
    let status_line = core_render::render_engine::build_status_line(model.state(), &initial_view);
    eng.render_full(model.state(), &initial_view, &layout, 80, 10, &status_line)
        .unwrap();

    // Move cursor to bottom visible line (line 8) to exercise old cursor repaint on scroll.
    {
        let v = model.active_view_mut();
        v.cursor.line = 8;
    }
    let view_after_cursor = model.active_view().clone();
    let status_line_cursor =
        core_render::render_engine::build_status_line(model.state(), &view_after_cursor);
    eng.render_cursor_only(
        model.state(),
        &view_after_cursor,
        &layout,
        80,
        10,
        &status_line_cursor,
    )
    .unwrap();

    // Scroll down by 2 (old_first=0 -> new_first=2)
    {
        let v = model.active_view_mut();
        v.viewport_first_line = 2;
        v.cursor.line = 10; // keep cursor at bottom after scroll (line 10 now last visible line)
    }
    let metrics_before = eng.metrics_snapshot();
    shift(&mut eng, &mut model, &layout, 0, 2);
    let snap = eng.metrics_snapshot();
    assert_eq!(
        snap.scroll_region_shifts,
        metrics_before.scroll_region_shifts + 1
    );
    // visible_rows=9 entering=2 old cursor line (8) still visible? After scroll start=2 visible range 2..=10 so yes, should repaint entering(2)+old(8) => repaint set len ==3 or 2 if old cursor matched entering lines (it does not).
    let repainted = eng.test_last_repaint_lines();
    assert!(
        repainted.contains(&8),
        "old cursor line repainted when still visible"
    );
    assert!(
        repainted.contains(&10) || repainted.contains(&9) || repainted.contains(&11),
        "entering lines included"
    );
    // Lines saved should decrement by exactly entering + old cursor lines from previous value increment perspective.

    // Scroll up by 1 (old_first=2 -> new_first=1) with cursor moving accordingly.
    {
        let v = model.active_view_mut();
        v.viewport_first_line = 1;
        v.cursor.line = 9; // keep near bottom
    }
    let snap_before_up = eng.metrics_snapshot();
    shift(&mut eng, &mut model, &layout, 2, 1);
    let snap_after_up = eng.metrics_snapshot();
    assert_eq!(
        snap_after_up.scroll_region_shifts,
        snap_before_up.scroll_region_shifts + 1
    );
    // Ensure no duplicate cursor: old cursor line (10) repainted if still visible.
    let repainted2 = eng.test_last_repaint_lines();
    // old cursor (10) now visible range 1..=9? No, so not repainted; new entering top line 1 plus previous old cursor clearing not needed.
    assert!(
        repainted2.contains(&1),
        "entering top line repainted on upward scroll"
    );
}
