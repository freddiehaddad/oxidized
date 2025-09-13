use core_model::{EditorModel, Layout};
use core_render::dirty::DirtyLinesTracker;
use core_render::render_engine::RenderEngine;
use core_state::EditorState;
use core_text::{Buffer, Position};

fn mk_state(text: &str) -> (EditorModel, Layout) {
    let st = EditorState::new(Buffer::from_str("test", text).unwrap());
    let model = EditorModel::new(st);
    let layout = Layout::single(80, 8); // text height 7
    (model, layout)
}

#[test]
fn trim_middle_interior_success() {
    let (mut model, layout) = mk_state("alpha bravo charlie\nsecond line here\nthird line\n");
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    let status_line = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 80, 8, &status_line)
        .unwrap();
    // Edit first line interior: replace single 'b' with 'B' preserving large prefix+suffix so trimming should trigger.
    {
        let buf = model.state_mut().active_buffer_mut();
        // Find byte offset of 'b' in first line: "alpha bravo charlie" => 'b' after "alpha ": 6
        let mut pos = Position::new(0, 6); // 'b' in "bravo"
        buf.delete_grapheme_at(&mut pos); // remove lowercase b
        buf.insert_grapheme(&mut pos, "B"); // insert uppercase B (same length)
    }
    // Mark line 0 dirty
    let mut dirty = DirtyLinesTracker::new();
    dirty.mark(0);
    let view_after = model.active_view().clone();
    let status_line_after =
        core_render::render_engine::build_status_line(model.state(), &view_after);
    eng.render_lines_partial(
        model.state(),
        &view_after,
        &layout,
        80,
        8,
        &mut dirty,
        &status_line_after,
    )
    .unwrap();
    let snap = eng.metrics_snapshot();
    assert!(snap.trim_attempts >= 1, "should attempt trim");
    assert!(snap.trim_success >= 1, "trim should succeed");
    assert!(snap.cols_saved_total > 0, "should save some columns");
}

#[test]
fn trim_below_threshold_fallback() {
    // Short line so savings < threshold (prefix 'a' + suffix 'cd' = 3 < 4 threshold) when changing interior 'b'
    let (mut model, layout) = mk_state("abcd\nzz\n");
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    let status_line2 = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 80, 8, &status_line2)
        .unwrap();
    // Small interior change: replace 'b' with 'X' (savings should be 3 < threshold 4)
    {
        let buf = model.state_mut().active_buffer_mut();
        let mut pos = Position::new(0, 1); // 'b'
        buf.delete_grapheme_at(&mut pos);
        buf.insert_grapheme(&mut pos, "X");
    }
    let mut dirty = DirtyLinesTracker::new();
    dirty.mark(0);
    let view_after = model.active_view().clone();
    let status_line_after2 =
        core_render::render_engine::build_status_line(model.state(), &view_after);
    eng.render_lines_partial(
        model.state(),
        &view_after,
        &layout,
        80,
        8,
        &mut dirty,
        &status_line_after2,
    )
    .unwrap();
    let snap = eng.metrics_snapshot();
    // Attempt increments but success should not.
    assert!(snap.trim_attempts >= 1);
    assert_eq!(snap.trim_success, 0, "trim should fallback below threshold");
}

#[test]
fn trim_unicode_wide_cluster() {
    let (mut model, layout) = mk_state("😀😀😀 wide test\nline2\n");
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    let status_line3 = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 80, 8, &status_line3)
        .unwrap();
    // Replace middle emoji cluster with another emoji of same cluster width (no length change)
    {
        let buf = model.state_mut().active_buffer_mut();
        // After first emoji boundary
        let mut pos = Position::new(0, "😀".len());
        // delete second emoji cluster and insert another emoji (same byte length) to allow trimming
        buf.delete_grapheme_at(&mut pos);
        buf.insert_grapheme(&mut pos, "😎");
    }
    let mut dirty = DirtyLinesTracker::new();
    dirty.mark(0);
    let view_after = model.active_view().clone();
    let status_line_after3 =
        core_render::render_engine::build_status_line(model.state(), &view_after);
    eng.render_lines_partial(
        model.state(),
        &view_after,
        &layout,
        80,
        8,
        &mut dirty,
        &status_line_after3,
    )
    .unwrap();
    let snap = eng.metrics_snapshot();
    assert!(snap.trim_success >= 1, "unicode trim should succeed");
}

#[test]
fn trim_line_shrink_behavior() {
    // Deletion inside line may still be treated as trim if sufficient unchanged prefix/suffix remain.
    let (mut model, layout) = mk_state("prefix MIDDLE suffix\nline2\n");
    let mut eng = RenderEngine::new();
    let v_clone = model.active_view().clone();
    let status_line4 = core_render::render_engine::build_status_line(model.state(), &v_clone);
    eng.render_full(model.state(), &v_clone, &layout, 80, 8, &status_line4)
        .unwrap();
    {
        let buf = model.state_mut().active_buffer_mut();
        // Delete MIDDLE (6 chars) leaving space between prefix and suffix removed
        let mut pos = Position::new(0, 7); // start of MIDDLE
        for _ in 0..6 {
            buf.delete_grapheme_at(&mut pos);
        }
    }
    let mut dirty = DirtyLinesTracker::new();
    dirty.mark(0);
    let status_line_after4 =
        core_render::render_engine::build_status_line(model.state(), &model.active_view().clone());
    eng.render_lines_partial(
        model.state(),
        &model.active_view().clone(),
        &layout,
        80,
        8,
        &mut dirty,
        &status_line_after4,
    )
    .unwrap();
    let snap = eng.metrics_snapshot();
    assert!(snap.trim_attempts >= 1);
    // We accept either outcome depending on heuristic; ensure no panic and prev_text updated.
    assert!(eng.test_prev_text(0).is_some());
}

#[test]
fn scroll_shift_preserves_prev_text_for_trim() {
    // Large line that will be trimmed after a scroll shift
    let (mut model, layout) = mk_state(
        "line0xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\nline1 common tail AAA\nline2\nline3\nline4\nline5\nline6\nline7 target mutate AAA\nline8\n",
    );
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    let status_line5 = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 80, 8, &status_line5)
        .unwrap();
    // Scroll viewport down by 1 (simulate scheduler choosing scroll shift path)
    {
        let v = model.active_view_mut();
        v.viewport_first_line = 1;
    }
    let view_scrolled = model.active_view().clone();
    let status_line_scrolled =
        core_render::render_engine::build_status_line(model.state(), &view_scrolled);
    eng.render_scroll_shift(
        model.state(),
        &view_scrolled,
        &layout,
        80,
        8,
        0,
        1,
        &status_line_scrolled,
    )
    .unwrap();
    // Mutate line now at relative row 6 (original buffer line 7) interior
    {
        let buf = model.state_mut().active_buffer_mut();
        let mut pos = Position::new(7, 7); // inside "target mutate AAA"
        buf.insert_grapheme(&mut pos, "ZZZ");
    }
    let mut dirty = DirtyLinesTracker::new();
    dirty.mark(7);
    let status_line_after5 =
        core_render::render_engine::build_status_line(model.state(), &model.active_view().clone());
    eng.render_lines_partial(
        model.state(),
        &model.active_view().clone(),
        &layout,
        80,
        8,
        &mut dirty,
        &status_line_after5,
    )
    .unwrap();
    let snap = eng.metrics_snapshot();
    assert!(snap.trim_attempts >= 1);
    // May or may not succeed depending on heuristic, but prev_text must be present for the relative row.
    assert!(
        eng.test_prev_text(6).is_some(),
        "prev_text should exist after scroll shift for preserved line"
    );
}

#[test]
fn cache_shift_invariants() {
    // Build several lines and perform scroll shift, verifying prev_text moves appropriately.
    let (mut model, layout) = mk_state("l0\nl1\nl2\nl3\nl4\nl5\nl6\n");
    let mut eng = RenderEngine::new();
    let view0 = model.active_view().clone();
    let status_line6 = core_render::render_engine::build_status_line(model.state(), &view0);
    eng.render_full(model.state(), &view0, &layout, 80, 8, &status_line6)
        .unwrap();
    // After full render, row 0 text should be l0 (trimmed of newline)
    assert_eq!(eng.test_prev_text(0), Some("l0"));
    // Scroll viewport down by 2
    {
        let v = model.active_view_mut();
        v.viewport_first_line = 2;
    }
    let view_after = model.active_view().clone();
    let status_line_shift2 =
        core_render::render_engine::build_status_line(model.state(), &view_after);
    eng.render_scroll_shift(
        model.state(),
        &view_after,
        &layout,
        80,
        8,
        0,
        2,
        &status_line_shift2,
    )
    .unwrap();
    // Row 0 should now correspond to original l2
    assert_eq!(eng.test_prev_text(0), Some("l2"));
}

#[test]
fn resize_invalidation_repopulates_prev_text() {
    let (model, layout) = mk_state("aa\nbb\ncc\n");
    let mut eng = RenderEngine::new();
    let v_clone2 = model.active_view().clone();
    let status_line7 = core_render::render_engine::build_status_line(model.state(), &v_clone2);
    eng.render_full(model.state(), &v_clone2, &layout, 80, 8, &status_line7)
        .unwrap();
    assert!(eng.test_prev_text(0).is_some());
    eng.invalidate_for_resize();
    // After invalidation, a new full render should repopulate prev_text entries.
    let v_clone3 = model.active_view().clone();
    let status_line8 = core_render::render_engine::build_status_line(model.state(), &v_clone3);
    eng.render_full(model.state(), &v_clone3, &layout, 100, 8, &status_line8)
        .unwrap();
    assert!(eng.test_prev_text(0).is_some());
}
