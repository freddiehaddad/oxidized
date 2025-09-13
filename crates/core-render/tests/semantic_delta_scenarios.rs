//! Semantic delta scenario tests (Refactor R2 Step 10)
//!
//! These tests exercise realistic sequences of render delta marks to validate
//! precedence, coalescing, and metrics behavior prior to activating partial
//! rendering strategies in Phase 3.

use core_render::scheduler::{RenderDelta, RenderScheduler};

fn semantic(s: &mut RenderScheduler) -> Option<RenderDelta> {
    s.consume().map(|d| d.semantic)
}

#[test]
fn cursor_moves_only_produce_cursor_semantics() {
    let mut sch = RenderScheduler::new();
    sch.mark(RenderDelta::CursorOnly);
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::CursorOnly)));
    sch.mark(RenderDelta::CursorOnly);
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::CursorOnly)));
}

#[test]
fn edit_then_motion_promotes_lines_not_cursor() {
    let mut sch = RenderScheduler::new();
    sch.mark(RenderDelta::Lines(5..6));
    sch.mark(RenderDelta::CursorOnly); // should be absorbed by Lines precedence
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::Lines(r)) if r== (5..6)));
    sch.mark(RenderDelta::CursorOnly);
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::CursorOnly)));
}

#[test]
fn multiple_scrolls_coalesce_single_semantic() {
    let mut sch = RenderScheduler::new();
    sch.mark(RenderDelta::Scroll {
        old_first: 10,
        new_first: 11,
    });
    sch.mark(RenderDelta::Scroll {
        old_first: 11,
        new_first: 15,
    });
    sch.mark(RenderDelta::Scroll {
        old_first: 15,
        new_first: 17,
    });
    assert!(matches!(
        semantic(&mut sch),
        Some(RenderDelta::Scroll {
            old_first: 10,
            new_first: 17
        })
    ));
    let snap = sch.metrics_snapshot();
    assert_eq!(snap.scroll, 1);
    assert_eq!(snap.collapsed_scroll, 2); // three events -> two collapses
}

#[test]
fn scroll_precedence_suppresses_lines() {
    // Refactor R3 Step 5 precedence change: scroll outranks lines.
    let mut sch = RenderScheduler::new();
    sch.mark(RenderDelta::Scroll {
        old_first: 0,
        new_first: 1,
    });
    sch.mark(RenderDelta::Lines(20..21));
    assert!(matches!(
        semantic(&mut sch),
        Some(RenderDelta::Scroll {
            old_first: 0,
            new_first: 1
        })
    ));
    let snap = sch.metrics_snapshot();
    assert_eq!(snap.scroll, 1, "scroll semantic recorded");
    assert_eq!(snap.lines, 0, "lines suppressed by scroll precedence");
    assert_eq!(snap.suppressed_scroll, 1, "suppressed metric increments");
}

#[test]
fn status_and_cursor_collapse_status_line() {
    let mut sch = RenderScheduler::new();
    sch.mark(RenderDelta::CursorOnly);
    sch.mark(RenderDelta::StatusLine);
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::StatusLine)));
}

#[test]
fn mixed_scroll_status_cursor_prefers_scroll() {
    let mut sch = RenderScheduler::new();
    sch.mark(RenderDelta::CursorOnly);
    sch.mark(RenderDelta::StatusLine);
    sch.mark(RenderDelta::Scroll {
        old_first: 3,
        new_first: 4,
    });
    assert!(matches!(
        semantic(&mut sch),
        Some(RenderDelta::Scroll {
            old_first: 3,
            new_first: 4
        })
    ));
}

#[test]
fn full_overrides_everything() {
    let mut sch = RenderScheduler::new();
    sch.mark(RenderDelta::Lines(0..1));
    sch.mark(RenderDelta::Scroll {
        old_first: 0,
        new_first: 1,
    });
    sch.mark(RenderDelta::StatusLine);
    sch.mark(RenderDelta::CursorOnly);
    sch.mark(RenderDelta::Full);
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::Full)));
}

#[test]
fn line_range_merge_expansive() {
    let mut sch = RenderScheduler::new();
    sch.mark(RenderDelta::Lines(10..11));
    sch.mark(RenderDelta::Lines(13..15));
    // Policy: merge uses min(start) .. max(end) (bridge over gaps)
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::Lines(r)) if r == (10..15)));
}

#[test]
fn frame_without_marks_returns_none() {
    let mut sch = RenderScheduler::new();
    assert!(sch.consume().is_none());
}

#[test]
fn multi_frame_sequence_churn() {
    let mut sch = RenderScheduler::new();
    // Frame 1: edit -> Lines
    sch.mark(RenderDelta::Lines(2..3));
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::Lines(_))));
    // Frame 2: cursor
    sch.mark(RenderDelta::CursorOnly);
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::CursorOnly)));
    // Frame 3: scroll sequence
    sch.mark(RenderDelta::Scroll {
        old_first: 5,
        new_first: 6,
    });
    sch.mark(RenderDelta::Scroll {
        old_first: 6,
        new_first: 7,
    });
    assert!(matches!(
        semantic(&mut sch),
        Some(RenderDelta::Scroll {
            old_first: 5,
            new_first: 7
        })
    ));
    // Frame 4: status change
    sch.mark(RenderDelta::StatusLine);
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::StatusLine)));
    // Frame 5: cursor again
    sch.mark(RenderDelta::CursorOnly);
    assert!(matches!(semantic(&mut sch), Some(RenderDelta::CursorOnly)));
    let snap = sch.metrics_snapshot();
    assert_eq!(snap.lines, 1);
    assert_eq!(snap.scroll, 1);
    assert_eq!(snap.status_line, 1);
    assert_eq!(snap.cursor_only, 2);
    assert_eq!(snap.semantic_frames, 5);
}
