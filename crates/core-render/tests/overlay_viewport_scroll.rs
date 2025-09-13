use core_model::{EditorModel, Layout};
use core_render::overlay::overlay_line_count;
use core_render::render_engine::{RenderEngine, build_status_line_with_ephemeral};
use core_state::{EditorState, METRICS_OVERLAY_DEFAULT_LINES};
use core_text::Buffer;

// Helper to fabricate a terminal size (w,h) and get overlay line count.
fn overlay_rows(state: &EditorState, w: u16) -> u16 {
    overlay_line_count(state, w)
}

#[test]
fn metrics_overlay_reduces_effective_text_height_scrolls() {
    // Build a buffer with enough lines to scroll.
    let mut content = String::new();
    for i in 0..50 {
        content.push_str(&format!("line{i}\n"));
    }
    let buf = Buffer::from_str("t", &content).unwrap();
    let state = EditorState::new(buf);
    let mut model = EditorModel::new(state);
    // Enable overlay.
    let st = model.state_mut();
    st.toggle_metrics_overlay(METRICS_OVERLAY_DEFAULT_LINES);

    // Simulate a terminal  (width arbitrary 80, height small) with overlay lines.
    let w = 80u16;
    let h = 12u16; // total rows incl status line
    let ov = overlay_rows(model.state(), w) as usize;
    assert!(ov > 0);
    let base_text_height = h as usize - 1; // exclude status line
    let effective_text_height = base_text_height - ov;
    assert!(
        effective_text_height < base_text_height,
        "overlay must reduce text height"
    );

    // Place cursor so that moving down into overlay boundary should trigger scroll when using effective height.
    {
        let view = model.active_view_mut();
        view.cursor.line = effective_text_height.saturating_sub(1); // last visible line before overlay area begins
    }

    // Force auto_scroll with effective height (mirrors updated main loop logic).
    let (st_mut, view_mut) = model.split_state_and_active_view();
    let changed = view_mut.auto_scroll(st_mut, effective_text_height);
    assert!(
        !changed,
        "Initial positioning within viewport should not scroll"
    );

    // Move cursor down step by step until just beyond last visible real text row.
    {
        let view = model.active_view_mut();
        view.cursor.line += 1; // crosses threshold (would require scroll if overlay consumed rows)
    }
    let (st_mut, view_mut) = model.split_state_and_active_view();
    let changed2 = view_mut.auto_scroll(st_mut, effective_text_height);
    assert!(
        changed2,
        "Crossing boundary above overlay should trigger scroll with overlay-aware height"
    );

    // Sanity: render paths still compute overlay line count; ensure engine can render without panic.
    let mut engine = RenderEngine::new();
    let layout = Layout::single(w, h);
    let status = build_status_line_with_ephemeral(model.state(), model.active_view(), w);
    // Full render should succeed (ignore result).
    let _ = engine.render_full(model.state(), model.active_view(), &layout, w, h, &status);
}
