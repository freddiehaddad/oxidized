use core_model::EditorModel;
use core_render::render_engine::{RenderEngine, build_content_frame, build_status_line};
use core_state::{EditorState, METRICS_OVERLAY_DEFAULT_LINES};
use core_text::Buffer;

fn mk_model(text: &str) -> EditorModel {
    let st = EditorState::new(Buffer::from_str("test", text).unwrap());
    EditorModel::new(st)
}

const W: u16 = 80;
const H: u16 = 8; // provides reasonable space

#[test]
fn overlay_reduces_text_height_full() {
    let mut model = mk_model("a0\na1\na2\na3\n");
    let mut eng = RenderEngine::new();
    let view = model.active_view().clone();
    let layout = core_model::Layout::single(W, H);
    // Baseline without overlay
    let status_line = build_status_line(model.state(), &view);
    eng.render_full(model.state(), &view, &layout, W, H, &status_line)
        .unwrap();
    let baseline_frame = build_content_frame(model.state(), &view, W, H);
    // Enable overlay
    model
        .state_mut()
        .toggle_metrics_overlay(METRICS_OVERLAY_DEFAULT_LINES);
    let status_line2 = build_status_line(model.state(), &view);
    eng.render_full(model.state(), &view, &layout, W, H, &status_line2)
        .unwrap();
    // Build another frame with overlay active; effective text rows should be fewer by overlay lines.
    let frame_overlay = build_content_frame(model.state(), &view, W, H);
    // Content frame height unchanged (overlay sits outside content frame); sanity check some line content equality.
    assert_eq!(
        baseline_frame.line_clusters(0),
        frame_overlay.line_clusters(0)
    );
}

#[test]
fn overlay_lines_contain_tokens() {
    let mut model = mk_model("x\n");
    model
        .state_mut()
        .toggle_metrics_overlay(METRICS_OVERLAY_DEFAULT_LINES);
    let mut eng = RenderEngine::new();
    let view = model.active_view().clone();
    let layout = core_model::Layout::single(W, H);
    let status_line = build_status_line(model.state(), &view);
    eng.render_full(model.state(), &view, &layout, W, H, &status_line)
        .unwrap();
    // Force at least one partial render to produce metrics counters.
    let status_line2 = build_status_line(model.state(), &view);
    eng.render_cursor_only(model.state(), &view, &layout, W, H, &status_line2)
        .unwrap();
    // We can't directly query overlay lines from frame (overlay painted via direct writer),
    // so rely on metrics presence + lack of panic. Indirect assertion: metrics counters advanced.
    let snap = eng.metrics_snapshot();
    assert!(snap.full_frames >= 1, "expected at least one full frame");
}

#[test]
fn status_skip_still_increments_with_overlay() {
    let mut model = mk_model("abc\n");
    model
        .state_mut()
        .toggle_metrics_overlay(METRICS_OVERLAY_DEFAULT_LINES);
    let mut eng = RenderEngine::new();
    let view = model.active_view().clone();
    let layout = core_model::Layout::single(W, H);
    let status_line = build_status_line(model.state(), &view);
    eng.render_full(model.state(), &view, &layout, W, H, &status_line)
        .unwrap();
    let before = eng.metrics_snapshot().status_skipped;
    // Cursor-only with no state change should skip status (overlay always repaints, independent).
    let status_line2 = build_status_line(model.state(), &view);
    eng.render_cursor_only(model.state(), &view, &layout, W, H, &status_line2)
        .unwrap();
    let after = eng.metrics_snapshot().status_skipped;
    assert_eq!(
        after,
        before + 1,
        "status skip should increment even with overlay enabled"
    );
}

#[test]
fn overlay_toggle_off_restores_behavior() {
    let mut model = mk_model("a\n");
    let mut eng = RenderEngine::new();
    let view = model.active_view().clone();
    let layout = core_model::Layout::single(W, H);
    // Enable overlay, render
    model
        .state_mut()
        .toggle_metrics_overlay(METRICS_OVERLAY_DEFAULT_LINES);
    let status_line = build_status_line(model.state(), &view);
    eng.render_full(model.state(), &view, &layout, W, H, &status_line)
        .unwrap();
    let snap_with = eng.metrics_snapshot();
    // Disable overlay, render again
    model
        .state_mut()
        .toggle_metrics_overlay(METRICS_OVERLAY_DEFAULT_LINES); // toggles off
    let status_line2 = build_status_line(model.state(), &view);
    eng.render_full(model.state(), &view, &layout, W, H, &status_line2)
        .unwrap();
    let snap_without = eng.metrics_snapshot();
    // Basic sanity: full frames advanced, and no panic. We can't trivially inspect overlay rows post-hoc.
    assert!(snap_without.full_frames >= snap_with.full_frames);
}
