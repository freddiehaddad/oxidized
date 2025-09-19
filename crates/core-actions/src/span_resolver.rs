//! Motion span resolver (Phase 4 Step 4).
//!
//! Given a starting cursor `Position`, a `MotionKind`, and a `count`, this
//! module computes a byte-range `[start, end)` suitable for operator
//! application (delete/yank/change). The semantics purposely mirror a
//! simplified subset of Vim:
//! * Horizontal forward motions (Right, WordForward, LineEnd) expand end.
//! * Horizontal backward motions (Left, WordBackward, LineStart) move start.
//! * Vertical motions (Up/Down/PageHalfUp/Down) produce the range covering
//!   the original cursor position to the final cursor position within its
//!   destination line (line-relative horizontal position preserved by the
//!   existing motion logic).
//! * Counts apply iteratively by reusing existing dispatcher motion helpers.
//!
//! Breadth-first: this resolver does NOT mutate editor state; it clones a
//! `Position` and replays motions on a temporary copy. Future enhancements
//! (text object semantics, inclusive/exclusive tweaks) can evolve here.
//!
//! Unicode correctness is delegated to `core_text::motion` primitives already
//! used by the dispatcher.

use crate::MotionKind;
use core_state::EditorState;
use core_text::{Buffer, Position, motion};

/// A resolved span for operator application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionSpan {
    pub start: usize, // absolute byte index
    pub end: usize,   // absolute byte index (exclusive)
}

impl MotionSpan {
    pub fn empty(at: usize) -> Self {
        Self { start: at, end: at }
    }
}

/// Resolve a motion repeated `count` times into a byte span. Start is the
/// minimum of original and final absolute byte indices; end is the maximum.
/// An empty span will be returned if the motion produced no movement.
pub fn resolve_span(
    state: &EditorState,
    start: Position,
    motion_kind: MotionKind,
    count: u32,
) -> MotionSpan {
    let count = count.max(1);
    let buffer = state.active_buffer();
    let mut pos = start; // working cursor copy
    let orig_abs = absolute_index(buffer, &pos);
    for _ in 0..count {
        apply_motion_once(buffer, &mut pos, motion_kind);
    }
    let final_abs = absolute_index(buffer, &pos);
    if final_abs == orig_abs {
        return MotionSpan::empty(orig_abs);
    }
    if final_abs > orig_abs {
        MotionSpan {
            start: orig_abs,
            end: final_abs,
        }
    } else {
        MotionSpan {
            start: final_abs,
            end: orig_abs,
        }
    }
}

fn apply_motion_once(buffer: &Buffer, pos: &mut Position, kind: MotionKind) {
    match kind {
        MotionKind::Left => motion::left(buffer, pos),
        MotionKind::Right => motion::right(buffer, pos),
        MotionKind::LineStart => motion::line_start(buffer, pos),
        MotionKind::LineEnd => motion::line_end(buffer, pos),
        MotionKind::WordForward => motion::word_forward(buffer, pos),
        MotionKind::WordBackward => motion::word_backward(buffer, pos),
        MotionKind::Up => {
            let _ = motion::up(buffer, pos, None);
        }
        MotionKind::Down => {
            let _ = motion::down(buffer, pos, None);
        }
        MotionKind::PageHalfUp => {
            let _ = motion::up(buffer, pos, None);
        } // simplified
        MotionKind::PageHalfDown => {
            let _ = motion::down(buffer, pos, None);
        }
    }
}

// Compute absolute byte index using only public Buffer APIs.
fn absolute_index(buffer: &Buffer, pos: &Position) -> usize {
    let mut total = 0usize;
    for line in 0..pos.line {
        total += buffer.line_byte_len(line);
        if let Some(l) = buffer.line(line)
            && l.ends_with('\n')
        {
            total += 1;
        }
    }
    total + pos.byte
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_state::EditorState;
    use core_text::Buffer;

    #[test]
    fn span_right_multiple() {
        let buf = Buffer::from_str("test", "abcdef\n").unwrap();
        let st = EditorState::new(buf);
        let span = resolve_span(&st, Position::origin(), MotionKind::Right, 3);
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 3);
    }

    #[test]
    fn span_word_forward() {
        let buf = Buffer::from_str("test", "one two three\n").unwrap();
        let st = EditorState::new(buf);
        let span = resolve_span(&st, Position::origin(), MotionKind::WordForward, 2);
        assert!(span.end > span.start);
    }

    #[test]
    fn span_left_no_movement() {
        let buf = Buffer::from_str("test", "abc\n").unwrap();
        let st = EditorState::new(buf);
        let span = resolve_span(&st, Position::origin(), MotionKind::Left, 5);
        assert_eq!(span.start, span.end);
    }
}
