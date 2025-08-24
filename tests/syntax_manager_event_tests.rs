use std::sync::mpsc;
use std::time::{Duration, Instant};

use oxidized::features::syntax_manager::SyntaxManager;
use oxidized::input::events::EditorEvent;

// Helper: wait for at least one SyntaxReady event or timeout
fn wait_for_syntax_event(rx: &mpsc::Receiver<EditorEvent>, timeout: Duration) -> bool {
    let start = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(EditorEvent::SyntaxReady) => return true,
            Ok(_) => { /* ignore other events */ }
            Err(mpsc::RecvTimeoutError::Timeout) => { /* keep looping until overall timeout */ }
            Err(_) => return false,
        }
        if start.elapsed() >= timeout {
            return false;
        }
    }
}

#[test]
fn syntax_basic_highlight_and_event() {
    let (tx, rx) = mpsc::channel();
    let mut sm = SyntaxManager::new_with_event_sender(Some(tx)).expect("syntax manager");

    let buffer_id = 1usize;
    let code = "fn main() {\n    println!(\"hi\");\n}\n"; // simple Rust snippet
    sm.ensure_lines(buffer_id, &[0, 1, 2], code, "rust");

    assert!(
        wait_for_syntax_event(&rx, Duration::from_millis(500)),
        "did not receive SyntaxReady event in time"
    );

    // Poll results until no more
    let _ = sm.poll_results();

    // Lines should now have highlight data (may be empty for some, but at least one should be non-empty)
    let mut non_empty = 0;
    for li in 0..3 {
        if let Some(spans) = sm.get_line(buffer_id, li)
            && !spans.is_empty()
        {
            non_empty += 1;
        }
    }
    assert!(
        non_empty >= 1,
        "expected at least one line with non-empty highlight spans"
    );
}

#[test]
fn syntax_reuse_unchanged_line_promotes_previous_spans() {
    let (tx, rx) = mpsc::channel();
    let mut sm = SyntaxManager::new_with_event_sender(Some(tx)).expect("syntax manager");
    let buffer_id = 2usize;
    let code = "fn main() { }\n";
    sm.ensure_lines(buffer_id, &[0], code, "rust");
    assert!(wait_for_syntax_event(&rx, Duration::from_millis(500)));
    sm.poll_results();
    let original = sm.get_line(buffer_id, 0).expect("original spans");
    assert!(!original.is_empty(), "expected some initial spans");

    // Mark edit (version bump) but keep identical text to force reuse path producing LineUnchanged
    sm.notify_edit(buffer_id, 1); // marks line stale & version++
    sm.ensure_lines(buffer_id, &[0], code, "rust");
    assert!(wait_for_syntax_event(&rx, Duration::from_millis(500)));
    sm.poll_results();
    let reused = sm.get_line(buffer_id, 0).expect("reused spans");
    assert_eq!(
        original.len(),
        reused.len(),
        "reused span count should match original"
    );
}

#[test]
fn syntax_incremental_single_edit_updates_metrics() {
    let (tx, rx) = mpsc::channel();
    let mut sm = SyntaxManager::new_with_event_sender(Some(tx)).expect("syntax manager");
    let buffer_id = 3usize;
    let mut code = String::from("fn add(a: i32, b: i32) -> i32 { a + b }\n");
    sm.ensure_lines(buffer_id, &[0], &code, "rust");
    assert!(wait_for_syntax_event(&rx, Duration::from_millis(500)));
    sm.poll_results();

    let initial_incremental = sm.metrics.incremental;

    // Perform a tiny edit: change operator '+' to '-' to exercise single contiguous edit path
    code = code.replace("+", "-");
    sm.notify_edit(buffer_id, 1);
    sm.ensure_lines(buffer_id, &[0], &code, "rust");
    assert!(wait_for_syntax_event(&rx, Duration::from_millis(500)));
    sm.poll_results();

    assert!(
        sm.metrics.incremental > initial_incremental,
        "expected incremental metric to increase ({} -> {})",
        initial_incremental,
        sm.metrics.incremental
    );
}

#[test]
fn syntax_spans_non_overlapping_per_line() {
    let (tx, rx) = mpsc::channel();
    let mut sm = SyntaxManager::new_with_event_sender(Some(tx)).expect("syntax manager");
    let buffer_id = 4usize;
    let code = "fn outer() { fn inner() { let x = 1; } }\n";
    sm.ensure_lines(buffer_id, &[0], code, "rust");
    assert!(wait_for_syntax_event(&rx, Duration::from_millis(500)));
    sm.poll_results();
    let spans = sm.get_line(buffer_id, 0).expect("spans");
    // Verify non-overlap and sorted
    let mut last_end = 0usize;
    for s in spans.iter() {
        assert!(
            s.start >= last_end,
            "overlap detected: last_end={} next_start={}",
            last_end,
            s.start
        );
        assert!(s.end > s.start);
        last_end = s.end;
    }
}

#[test]
fn syntax_provisional_delimiter_has_span() {
    let (tx, rx) = mpsc::channel();
    let mut sm = SyntaxManager::new_with_event_sender(Some(tx)).expect("syntax manager");
    let buffer_id = 5usize;
    // Include some tokens plus an unmatched brace to encourage provisional insertion if parser omits it.
    let code = "fn demo( x: i32 ) {\n"; // single line with opening brace
    sm.ensure_lines(buffer_id, &[0], code, "rust");
    assert!(wait_for_syntax_event(&rx, Duration::from_millis(500)));
    sm.poll_results();
    if let Some(spans) = sm.get_line(buffer_id, 0) {
        let brace_pos = code.find('{').unwrap();
        let brace_rel = brace_pos;
        let has_brace = spans
            .iter()
            .any(|s| s.start <= brace_rel && s.end > brace_rel);
        assert!(
            has_brace,
            "expected a span covering '{{' at position {}",
            brace_rel
        );
    } else {
        panic!("no spans returned for line");
    }
}
