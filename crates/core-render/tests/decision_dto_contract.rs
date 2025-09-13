//! Tests asserting the stable Decision DTO shape behaves as expected.

use core_render::scheduler::{Decision, RenderDelta, RenderScheduler};

#[test]
fn decision_semantic_effective_fields_present() {
    let mut s = RenderScheduler::new();
    s.mark(RenderDelta::CursorOnly);
    let d: Decision = s.consume().expect("decision");
    // Cursor-only should remain cursor-only both semantically and effectively (partial path active)
    assert_eq!(d.semantic, RenderDelta::CursorOnly);
    assert_eq!(d.effective, RenderDelta::CursorOnly);
}

#[test]
fn decision_scroll_threshold_escalates_effective() {
    let mut s = RenderScheduler::new();
    s.mark(RenderDelta::Scroll {
        old_first: 0,
        new_first: RenderScheduler::SCROLL_SHIFT_MAX + 5,
    });
    let d = s.consume().unwrap();
    assert_eq!(
        d.semantic,
        RenderDelta::Scroll {
            old_first: 0,
            new_first: RenderScheduler::SCROLL_SHIFT_MAX + 5
        }
    );
    assert_eq!(d.effective, RenderDelta::Full);
}
