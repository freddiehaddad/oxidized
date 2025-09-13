use core_model::EditorModel;
use core_render::render_engine::RenderEngine;
use core_state::EditorState;
use core_text::Buffer;

// Helper to build a model with N lines of simple text (each ending with \n)
fn model_with_lines(n: usize) -> EditorModel {
    let mut content = String::new();
    for i in 0..n {
        content.push_str(&format!("line{i}\n"));
    }
    let st = EditorState::new(Buffer::from_str("test", &content).unwrap());
    EditorModel::new(st)
}

#[test]
fn large_candidate_set_escalates_to_full_and_increments_metric() {
    // Choose a viewport height so that threshold math is deterministic.
    // visible_rows = h - 1. We'll use h = 11 (10 text rows). Threshold = 0.60 * 10 = 6.0.
    // So 6 or more candidates should escalate.
    let model = model_with_lines(50);
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    // Initial full render to warm cache.
    let layout = core_model::Layout::single(80, 11);
    let status_line = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 80, 11, &status_line)
        .unwrap();

    // Simulate dirty tracker with >= threshold candidates in viewport (lines 0..10 text rows).
    // We'll mark 6 distinct lines.
    let mut tracker = core_render::dirty::DirtyLinesTracker::new();
    for line in [0, 1, 2, 3, 4, 5] {
        tracker.mark(line);
    }

    // Call lines partial path directly (engine method) and expect escalation to full.
    // We check metric afterwards.
    let before = eng.metrics_snapshot();
    let layout = core_model::Layout::single(80, 11);
    eng.render_lines_partial(
        model.state(),
        &model.active_view().clone(),
        &layout,
        80,
        11,
        &mut tracker,
        &core_render::render_engine::build_status_line(model.state(), &model.active_view().clone()),
    )
    .unwrap();
    let after = eng.metrics_snapshot();

    // Because escalation path calls render_full internally, full_frames should increment by 1 while
    // lines_frames should NOT increment (partial path aborted early) and escalated_large_set increments.
    assert_eq!(
        after.full_frames,
        before.full_frames + 1,
        "full_frames not incremented on escalation"
    );
    assert_eq!(
        after.lines_frames, before.lines_frames,
        "lines_frames should not increment on escalation"
    );
    assert_eq!(
        after.escalated_large_set,
        before.escalated_large_set + 1,
        "escalation metric not incremented"
    );
}

#[test]
fn candidate_set_below_threshold_stays_partial() {
    // visible_rows = 10 again; threshold is 6. Using 5 should remain partial.
    let model = model_with_lines(50);
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    let layout = core_model::Layout::single(80, 11);
    let status_line2 = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 80, 11, &status_line2)
        .unwrap();

    let mut tracker = core_render::dirty::DirtyLinesTracker::new();
    for line in [0, 1, 2, 3, 4] {
        tracker.mark(line);
    }
    let before = eng.metrics_snapshot();
    let layout = core_model::Layout::single(80, 11);
    eng.render_lines_partial(
        model.state(),
        &model.active_view().clone(),
        &layout,
        80,
        11,
        &mut tracker,
        &core_render::render_engine::build_status_line(model.state(), &model.active_view().clone()),
    )
    .unwrap();
    let after = eng.metrics_snapshot();

    assert_eq!(
        after.full_frames, before.full_frames,
        "Should not escalate to full"
    );
    assert_eq!(
        after.lines_frames,
        before.lines_frames + 1,
        "lines_frames should increment on partial path"
    );
    assert_eq!(
        after.escalated_large_set, before.escalated_large_set,
        "escalation metric should not change"
    );
}
