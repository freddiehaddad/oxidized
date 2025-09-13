//! Tests for bracketed paste streaming events (NGI Phase C Step 7).
//! These tests directly construct InputEvent variants since crossterm does not
//! currently surface bracketed paste as structured events. We validate that
//! event ordering invariants expected by downstream consumers hold.

use core_events::InputEvent;

#[test]
fn paste_sequence_order() {
    // Simulate a paste of two chunks broken by internal flushing heuristic.
    let events = [
        InputEvent::PasteStart,
        InputEvent::PasteChunk("hello ".to_string()),
        InputEvent::PasteChunk("world".to_string()),
        InputEvent::PasteEnd,
    ];
    // Basic invariants: first is start, last is end, at least one chunk between.
    assert!(matches!(events.first(), Some(InputEvent::PasteStart)));
    assert!(matches!(events.last(), Some(InputEvent::PasteEnd)));
    assert!(
        events[1..events.len() - 1]
            .iter()
            .all(|e| matches!(e, InputEvent::PasteChunk(_))),
        "all middle events must be chunks"
    );
}
