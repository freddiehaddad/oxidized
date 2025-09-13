use core_model::{EditorModel, Layout};
use core_render::render_engine::RenderEngine;
use core_state::EditorState;
use core_text::Buffer;

fn mk(text: &str) -> EditorModel {
    let st = EditorState::new(Buffer::from_str("test", text).unwrap());
    EditorModel::new(st)
}

// Helper: perform an initial full render to populate cache.
fn warm(eng: &mut RenderEngine, model: &EditorModel, layout: &Layout) {
    let view = model.active_view().clone();
    let status_line = core_render::render_engine::build_status_line(model.state(), &view);
    eng.render_full(
        model.state(),
        &view,
        layout,
        layout.primary().width,
        layout.primary().height,
        &status_line,
    )
    .unwrap();
}

#[test]
#[allow(clippy::needless_range_loop)]
fn cache_shift_down_reuses_and_recomputes() {
    // Build 15 lines so we can scroll within a 10-row (9 text + 1 status) viewport.
    let text: String = (0..15).map(|i| format!("L{:#02}\n", i)).collect();
    let mut model = mk(&text);
    let mut eng = RenderEngine::new();
    let layout = Layout::single(80, 10); // text rows = 9
    // Position viewport at line 0 first.
    warm(&mut eng, &model, &layout);
    // Capture initial hashes for rows 0..9 (text rows 0..8).
    let initial_hashes: Vec<_> = eng.test_cache_hashes().to_vec();
    // Cache stores only text rows (excludes status) so length == layout.primary().height - 1.
    assert_eq!(initial_hashes.len(), (layout.primary().height - 1) as usize);

    // Scroll viewport down by 2 lines (delta=+2).
    {
        let v = model.active_view_mut();
        v.viewport_first_line = 2;
    }
    let view = model.active_view().clone();
    let status_line_shift = core_render::render_engine::build_status_line(model.state(), &view);
    eng.render_scroll_shift(
        model.state(),
        &view,
        &layout,
        80,
        10,
        0,
        2,
        &status_line_shift,
    )
    .unwrap();

    // After shift, hashes[0] should equal old hashes[2]; hashes[6] == old hashes[8]; bottom two rows recomputed.
    for i in 0..7 {
        // reused rows count = 9 - 2
        assert_eq!(
            eng.test_cache_hashes()[i],
            initial_hashes[i + 2],
            "row {} reused from old row {}",
            i,
            i + 2
        );
    }
    // Bottom two new lines correspond to original lines 9 and 10.
    let buf = model.state().active_buffer();
    for (i, global_line) in (9..=10).enumerate() {
        // recomputed rows indices 7,8
        let cache_idx = 7 + i;
        let raw = buf.line(global_line).unwrap();
        let trimmed = raw.trim_end_matches(['\n', '\r']);
        let expected = core_render::partial_cache::PartialCache::compute_hash(trimmed);
        assert_eq!(
            eng.test_cache_hashes()[cache_idx],
            expected,
            "recomputed hash mismatch at cache row {}",
            cache_idx
        );
    }
    assert_eq!(eng.test_cache_viewport_start(), 2);
}
#[test]
#[allow(clippy::needless_range_loop)]
fn cache_shift_up_reuses_and_recomputes() {
    let text: String = (0..15).map(|i| format!("L{:#02}\n", i)).collect();
    let mut model = mk(&text);
    let mut eng = RenderEngine::new();
    let layout = Layout::single(80, 10); // 9 text rows
    // Warm at viewport starting 3 to give space to scroll up.
    {
        let v = model.active_view_mut();
        v.viewport_first_line = 3;
    }
    warm(&mut eng, &model, &layout);
    let initial_hashes: Vec<_> = eng.test_cache_hashes().to_vec();
    // Cache stores only text rows.
    assert_eq!(initial_hashes.len(), (layout.primary().height - 1) as usize);
    {
        let v = model.active_view_mut();
        v.viewport_first_line = 1;
    }
    let view = model.active_view().clone();
    let status_line_shift2 = core_render::render_engine::build_status_line(model.state(), &view);
    eng.render_scroll_shift(
        model.state(),
        &view,
        &layout,
        80,
        10,
        3,
        1,
        &status_line_shift2,
    )
    .unwrap();

    // After upward shift, bottom reused row (index 8) should equal initial row 6 (old index 6 -> new 8).
    // More directly: reused span size = 9 - 2 = 7 lines; they shift down by 2.
    for i in 0..7 {
        // original rows 0..6 move to 2..8
        let dst = i + 2;
        assert_eq!(
            eng.test_cache_hashes()[dst],
            initial_hashes[i],
            "old row {} should move to {}",
            i,
            dst
        );
    }
    // Entering top rows (0,1) correspond to global lines 1 and 2.
    let buf = model.state().active_buffer();
    for (i, global_line) in (1..=2).enumerate() {
        // cache rows 0,1
        let raw = buf.line(global_line).unwrap();
        let trimmed = raw.trim_end_matches(['\n', '\r']);
        let expected = core_render::partial_cache::PartialCache::compute_hash(trimmed);
        assert_eq!(
            eng.test_cache_hashes()[i],
            expected,
            "top entering row {} recompute mismatch",
            i
        );
    }
    assert_eq!(eng.test_cache_viewport_start(), 1);
}
