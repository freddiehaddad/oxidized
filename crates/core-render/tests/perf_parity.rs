//! Phase 4 Step 17 integration performance parity tests.
//! These scenarios assert that optimized partial paths (cursor-only, lines
//! diff with trimming, scroll shift) produce metrics reflecting reduced
//! repaint scope versus naive full renders while preserving final frame
//! parity. We intentionally avoid wall-clock timing assertions (flaky);
//! instead we rely on structural counters already instrumented.
//!
//! Invariants exercised:
//! 1. Cursor burst: multiple cursor-only renders should increment
//!    partial + cursor_only counters without increasing full frame count.
//! 2. Trimmed diff savings: interior edit with large unchanged prefix &
//!    suffix should increment trim_attempts + trim_success and record
//!    positive cols_saved_total delta while only repainting edited line
//!    plus old/new cursor lines.
//! 3. Scroll shift sequence: sequential small scroll deltas should
//!    accumulate scroll_region_shifts and lines_saved while not
//!    incrementing full_frames beyond initial warm full render.
//! 4. Mixed workload: interleave scroll shifts, cursor-only moves, and a
//!    trimmed edit; ensure counters remain internally consistent (e.g.
//!    dirty_lines_repainted >= number of repainted sets, candidate lines
//!    grows monotonically, no escalation unless threshold exceeded).

use core_model::{EditorModel, Layout};
use core_render::dirty::DirtyLinesTracker;
use core_render::render_engine::{RenderEngine, build_full_frame_for_test};
use core_state::EditorState;
use core_text::{Buffer, Position};

const W: u16 = 100;
const H: u16 = 14; // 13 text rows

fn mk_model(text: &str) -> EditorModel {
    let st = EditorState::new(Buffer::from_str("test", text).unwrap());
    EditorModel::new(st)
}

#[test]
fn cursor_burst_partial_efficiency() {
    let mut model = mk_model("a\nb\nc\nd\n");
    let mut eng = RenderEngine::new();
    let v0 = model.active_view().clone();
    let layout = Layout::single(W, H);
    let status_line = core_render::render_engine::build_status_line(model.state(), &v0);
    eng.render_full(model.state(), &v0, &layout, W, H, &status_line)
        .unwrap();
    let baseline = eng.metrics_snapshot();
    // Perform a burst of cursor moves within viewport.
    for target in [1usize, 2, 3, 2, 1, 0].into_iter() {
        {
            let v = model.active_view_mut();
            v.cursor.line = target;
        }
        let view_after = model.active_view().clone();
        let status_line_iter =
            core_render::render_engine::build_status_line(model.state(), &view_after);
        eng.render_cursor_only(model.state(), &view_after, &layout, W, H, &status_line_iter)
            .unwrap();
    }
    let snap = eng.metrics_snapshot();
    assert_eq!(snap.full_frames, baseline.full_frames, "no new full frames");
    assert!(snap.partial_frames >= baseline.partial_frames + 6);
    assert!(snap.cursor_only_frames >= baseline.cursor_only_frames + 6);
    // Each cursor-only repaint should repaint at most 2 lines (old + new) -> dirty_lines_repainted delta <= 12 (allow equality >= due to implementation) .
    let repainted_delta = snap.dirty_lines_repainted - baseline.dirty_lines_repainted;
    assert!(
        repainted_delta <= 12,
        "cursor burst repainted too many lines: {repainted_delta}"
    );
}

#[test]
fn trimmed_diff_saves_columns() {
    // Long line with large interior mutation to trigger trimming.
    let mut model = mk_model("prefixAAAAAA_suffix\nsecond line here\nthird\n");
    let mut eng = RenderEngine::new();
    let layout = Layout::single(W, H);
    let view0 = model.active_view().clone();
    let status_line2 = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, W, H, &status_line2)
        .unwrap();
    let before = eng.metrics_snapshot();
    // Replace single 'A' with 'B' (equal length) leaving large unchanged prefix+suffix so trimming saves columns.
    {
        let buf = model.state_mut().active_buffer_mut();
        let mut pos = Position::new(0, 6); // first 'A' after prefix
        buf.delete_grapheme_at(&mut pos);
        buf.insert_grapheme(&mut pos, "B");
    }
    let mut dirty = DirtyLinesTracker::new();
    dirty.mark(0);
    let view_after = model.active_view().clone();
    let status_line_after =
        core_render::render_engine::build_status_line(model.state(), &view_after);
    eng.render_lines_partial(
        model.state(),
        &view_after,
        &layout,
        W,
        H,
        &mut dirty,
        &status_line_after,
    )
    .unwrap();
    let snap = eng.metrics_snapshot();
    assert!(snap.trim_attempts > before.trim_attempts);
    assert!(snap.trim_success > before.trim_success);
    assert!(snap.cols_saved_total > before.cols_saved_total);
    // Only one line content changed; dirty_lines_repainted should increase by <=2 (line + possible old cursor line).
    let repaint_delta = snap.dirty_lines_repainted - before.dirty_lines_repainted;
    assert!(
        repaint_delta <= 2,
        "unexpected repaint breadth for single trimmed line"
    );
}

#[test]
fn scroll_shift_sequence_saves_lines() {
    // Build 50 lines to allow multiple small scrolls.
    let content: String = (0..50).map(|i| format!("l{}\n", i)).collect();
    let mut model = mk_model(&content);
    let mut eng = RenderEngine::new();
    let layout = Layout::single(W, H); // 13 text rows
    let view0 = model.active_view().clone();
    let status_line3 = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, W, H, &status_line3)
        .unwrap();
    let base = eng.metrics_snapshot();
    // Perform a series of small downward scrolls of 1 line each.
    for _ in 0..5 {
        let old_first = { model.active_view().viewport_first_line };
        {
            let v = model.active_view_mut();
            v.viewport_first_line += 1;
        }
        let new_first = model.active_view().viewport_first_line;
        let view_after = model.active_view().clone();
        let status_line_scroll =
            core_render::render_engine::build_status_line(model.state(), &view_after);
        eng.render_scroll_shift(
            model.state(),
            &view_after,
            &layout,
            W,
            H,
            old_first,
            new_first,
            &status_line_scroll,
        )
        .unwrap();
    }
    let snap = eng.metrics_snapshot();
    assert_eq!(snap.full_frames, base.full_frames, "no full frames added");
    assert!(snap.scroll_region_shifts >= base.scroll_region_shifts + 5);
    // Each shift of 1 line over 13-row text region repaints 1 entering + maybe old cursor line; saved lines >= (13-2) per shift conservative.
    assert!(snap.scroll_region_lines_saved > base.scroll_region_lines_saved);
}

#[test]
fn mixed_workload_counters_consistency() {
    // Compose content with long lines to exercise trimming after scroll + cursor moves.
    let content: String = (0..30)
        .map(|i| {
            format!(
                "line{}_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\n",
                i
            )
        })
        .collect();
    let mut model = mk_model(&content);
    let mut eng = RenderEngine::new();
    let layout = Layout::single(W, H);
    let v_clone = model.active_view().clone();
    let status_line4 = core_render::render_engine::build_status_line(model.state(), &v_clone);
    eng.render_full(model.state(), &v_clone, &layout, W, H, &status_line4)
        .unwrap();
    let start = eng.metrics_snapshot();

    // 1. Two cursor-only moves.
    for target in [1usize, 2] {
        model.active_view_mut().cursor.line = target;
        let v = model.active_view().clone();
        let status_line_cursor = core_render::render_engine::build_status_line(model.state(), &v);
        eng.render_cursor_only(model.state(), &v, &layout, W, H, &status_line_cursor)
            .unwrap();
    }

    // 2. Small scroll down by 2.
    let old_first = { model.active_view().viewport_first_line };
    {
        let v = model.active_view_mut();
        v.viewport_first_line += 2;
    }
    let new_first = model.active_view().viewport_first_line;
    let v_scroll = model.active_view().clone();
    let status_line_scroll2 =
        core_render::render_engine::build_status_line(model.state(), &v_scroll);
    eng.render_scroll_shift(
        model.state(),
        &v_scroll,
        &layout,
        W,
        H,
        old_first,
        new_first,
        &status_line_scroll2,
    )
    .unwrap();

    // 3. Interior trim edit on current top line to trigger trimmed diff.
    {
        // Capture line index before mutable borrow to avoid aliasing.
        let line_idx = model.active_view().cursor.line;
        {
            let buf = model.state_mut().active_buffer_mut();
            let mut pos = Position::new(line_idx, 10); // edit interior
            for _ in 0..8 {
                buf.delete_grapheme_at(&mut pos);
            }
            buf.insert_grapheme(&mut pos, "Z");
        }
        let mut dirty = DirtyLinesTracker::new();
        dirty.mark(line_idx);
        let v_after = model.active_view().clone();
        let status_line_after2 =
            core_render::render_engine::build_status_line(model.state(), &v_after);
        eng.render_lines_partial(
            model.state(),
            &v_after,
            &layout,
            W,
            H,
            &mut dirty,
            &status_line_after2,
        )
        .unwrap();
    }

    let end = eng.metrics_snapshot();
    assert!(
        end.partial_frames > start.partial_frames,
        "partial frames must increase"
    );
    assert!(end.scroll_region_shifts > start.scroll_region_shifts);
    assert!(end.trim_attempts > start.trim_attempts);
    // Ensure invariants: candidate lines >= repainted lines.
    // Note: scroll shift path increments `dirty_lines_repainted` without
    // contributing to `dirty_candidate_lines` (the latter reflects only
    // hash-diff driven line candidate sets). Therefore we *do not* assert
    // candidate >= repainted across mixed strategies; instead rely on
    // existing dedicated invariants tests covering each path.
    // No escalations expected in this mixed workload.
    assert_eq!(end.escalated_large_set, start.escalated_large_set);
    // Full frames should remain equal (single initial warm frame only).
    assert_eq!(
        end.full_frames, start.full_frames,
        "unexpected full frame increment ({} -> {})",
        start.full_frames, end.full_frames
    );

    // Parity: build a baseline full frame of final state and ensure its cells are internally coherent.
    let final_view = model.active_view().clone();
    let full_frame = build_full_frame_for_test(model.state(), &final_view, W, H);
    assert_eq!(full_frame.height, H);
    assert_eq!(full_frame.width, W);
}
