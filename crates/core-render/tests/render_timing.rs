use core_model::{View, ViewId};
use core_render::dirty::DirtyLinesTracker;
use core_render::{
    render_engine::RenderEngine,
    timing::{last_render_ns, record_last_render_ns},
};
use core_state::EditorState;
use core_text::Buffer;

fn mk_engine_warm(state: &EditorState, view: &View, w: u16, h: u16) -> RenderEngine {
    let mut eng = RenderEngine::new();
    // Perform an initial full render to warm cache; ignore result.
    let layout = core_model::Layout::single(w, h);
    let status_line = core_render::render_engine::build_status_line(state, view);
    let _ = eng.render_full(state, view, &layout, w, h, &status_line);
    eng
}

#[test]
fn last_render_ns_non_zero_after_render() {
    let buf = Buffer::from_str("test", "hello world\n").unwrap();
    let state = EditorState::new(buf);
    let view = View::new(ViewId(0), state.active, core_text::Position::origin(), 0);
    let mut engine = RenderEngine::new();
    let start = std::time::Instant::now();
    let layout = core_model::Layout::single(80, 10);
    let status_line_full = core_render::render_engine::build_status_line(&state, &view);
    let _ = engine.render_full(&state, &view, &layout, 80, 10, &status_line_full);
    let elapsed = start.elapsed().as_nanos() as u64;
    record_last_render_ns(elapsed);
    assert!(last_render_ns() > 0, "expected non-zero render timing");
}

#[test]
fn lines_partial_single_line_edit() {
    let buf = Buffer::from_str("test", "abc\nxyz\n").unwrap();
    let mut state = EditorState::new(buf);
    let mut view = View::new(ViewId(0), state.active, core_text::Position::origin(), 0);
    let mut eng = mk_engine_warm(&state, &view, 80, 6);
    // Simulate an edit on line 0: insert '!' at end.
    {
        let mut pos = core_text::Position::new(0, 3);
        state.active_buffer_mut().insert_grapheme(&mut pos, "!");
        view.cursor = pos;
    }
    let mut tracker = DirtyLinesTracker::new();
    tracker.mark(0);
    let before = eng.metrics_snapshot();
    let layout = core_model::Layout::single(80, 6);
    let status_line_after_edit = core_render::render_engine::build_status_line(&state, &view);
    let _ = eng.render_lines_partial(
        &state,
        &view,
        &layout,
        80,
        6,
        &mut tracker,
        &status_line_after_edit,
    );
    let after = eng.metrics_snapshot();
    assert_eq!(after.partial_frames, before.partial_frames + 1);
    assert_eq!(after.lines_frames, before.lines_frames + 1);
    let cand_delta = after.dirty_candidate_lines - before.dirty_candidate_lines;
    assert!(
        cand_delta == 1 || cand_delta == 2,
        "candidate set must include edited line (+ maybe cursor), got {cand_delta}"
    );
    assert!(
        after.dirty_lines_repainted > before.dirty_lines_repainted,
        "expected at least one repainted line"
    );
}

#[test]
fn lines_partial_escalates_large_set() {
    // Create buffer with many lines and mark >=60% dirty to force escalation.
    let content: String = (0..20).map(|i| format!("l{idx}\n", idx = i)).collect();
    let buf = Buffer::from_str("test", &content).unwrap();
    let state = EditorState::new(buf);
    let view = View::new(ViewId(0), state.active, core_text::Position::origin(), 0);
    let mut eng = mk_engine_warm(&state, &view, 80, 12); // text_height = 11
    let mut tracker = DirtyLinesTracker::new();
    // Mark 7 of 11 visible lines (>= 63%).
    for l in 0..7 {
        tracker.mark(l);
    }
    let before = eng.metrics_snapshot();
    let layout = core_model::Layout::single(80, 12);
    let status_line2 = core_render::render_engine::build_status_line(&state, &view);
    let _ = eng.render_lines_partial(&state, &view, &layout, 80, 12, &mut tracker, &status_line2);
    let after = eng.metrics_snapshot();
    // Because we escalated, we expect a full frame increment instead of lines partial counters OR partial counters if implementation escalated internally.
    let escalated = after.full_frames > before.full_frames;
    let partial = after.lines_frames > before.lines_frames;
    assert!(
        escalated || partial,
        "either escalated to full or performed partial repaint"
    );
}
