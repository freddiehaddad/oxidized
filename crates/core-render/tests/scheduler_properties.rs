//! Property-based tests for RenderScheduler merge semantics (Step 2)

use core_render::scheduler::{RenderDelta, RenderScheduler};
use proptest::prelude::*;

proptest! {
    // CursorOnly combined with Lines should yield Lines (precedence)
    #[test]
    fn cursor_plus_lines_yields_lines(start in 0usize..1000, len in 1usize..50) {
        let mut s = RenderScheduler::new();
        let end = start + len;
        s.mark(RenderDelta::CursorOnly);
        s.mark(RenderDelta::Lines(start..end));
        let d = s.consume().unwrap();
        prop_assert!(matches!(d.semantic, RenderDelta::Lines(r) if r.start == start && r.end == end));
    }

    // Lines combined with Full should yield Full
    #[test]
    fn lines_plus_full_is_full(start in 0usize..1000, len in 1usize..50) {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::Lines(start..start+len));
        s.mark(RenderDelta::Full);
        let d = s.consume().unwrap();
        prop_assert_eq!(d.semantic, RenderDelta::Full);
    }

    // Scroll precedence over Lines
    #[test]
    fn scroll_supersedes_lines(of in 0usize..500, nf in 0usize..500, lstart in 0usize..500, llen in 1usize..100) {
        let mut s = RenderScheduler::new();
        let new_first = nf.max(of+1); // ensure forward progress
        s.mark(RenderDelta::Scroll { old_first: of, new_first });
        s.mark(RenderDelta::Lines(lstart..lstart+llen));
        let d = s.consume().unwrap();
        match d.semantic {
            RenderDelta::Scroll { old_first, new_first: nf2 } => {
                prop_assert!(old_first == of && nf2 == new_first);
            }
            _ => prop_assert!(false),
        }
    }

    // Multiple Lines merge into min..max
    #[test]
    fn lines_merge_min_max(a_start in 0usize..500, a_len in 1usize..100, b_start in 0usize..500, b_len in 1usize..100) {
        let mut s = RenderScheduler::new();
        let a_end = a_start + a_len;
        let b_end = b_start + b_len;
        s.mark(RenderDelta::Lines(a_start..a_end));
        s.mark(RenderDelta::Lines(b_start..b_end));
        let d = s.consume().unwrap();
        let expect_start = a_start.min(b_start);
        let expect_end = a_end.max(b_end);
        prop_assert!(matches!(d.semantic, RenderDelta::Lines(r) if r.start == expect_start && r.end == expect_end));
    }
}
