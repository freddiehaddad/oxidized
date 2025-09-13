//! Motion span resolver (Phase 4 Step 4).
//!
//! Given a starting cursor `Position`, a `MotionKind`, and a `count`, this
//! module computes a byte-range `[start, end)` suitable for operator
//! application (delete/yank/change). The semantics purposely mirror a
//! simplified subset of Vim and are centralized here to guarantee parity
//! between future text-object aware motions, operator application, and visual
//! selection expansion.
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
//! (text object semantics, inclusive/exclusive tweaks, inner/around delimiter
//! handling) can evolve in this single locus without touching operator or
//! visual mode code paths.
//!
//! Unicode correctness is delegated to `core_text::motion` primitives already
//! used by the dispatcher.

use crate::MotionKind;
use core_state::{EditorState, SelectionKind, SelectionSpan};
use core_text::{Buffer, Position, motion};

/// Resolved selection returning a `SelectionSpan` (characterwise or linewise).
/// Vertical motions (Up/Down/PageHalfUp/PageHalfDown) spanning multiple lines
/// produce a linewise span covering entire lines touched. Horizontal/word motions
/// produce a characterwise span with normalized ordering.
pub fn resolve_selection(
    state: &EditorState,
    start: Position,
    motion_kind: MotionKind,
    count: u32,
) -> SelectionSpan {
    let count = count.max(1);
    let buffer = state.active_buffer();
    // Vertical detection (linewise semantics) first
    let vertical = matches!(
        motion_kind,
        MotionKind::Up | MotionKind::Down | MotionKind::PageHalfUp | MotionKind::PageHalfDown
    );
    if vertical {
        let mut tmp = start;
        for _ in 0..count {
            match motion_kind {
                MotionKind::Up | MotionKind::PageHalfUp => {
                    let _ = motion::up(buffer, &mut tmp, None);
                }
                MotionKind::Down | MotionKind::PageHalfDown => {
                    let _ = motion::down(buffer, &mut tmp, None);
                }
                _ => {}
            }
        }
        let line_start = start.line.min(tmp.line);
        let line_end = start.line.max(tmp.line);
        // Compute absolute byte indices spanning full lines inclusive.
        let mut abs_start = 0usize;
        for l in 0..line_start {
            abs_start += buffer.line_byte_len(l);
            if let Some(s) = buffer.line(l)
                && s.ends_with('\n')
            {
                abs_start += 1;
            }
        }
        let mut abs_after_last = abs_start;
        for l in line_start..=line_end {
            abs_after_last += buffer.line_byte_len(l);
            if let Some(s) = buffer.line(l)
                && s.ends_with('\n')
            {
                abs_after_last += 1;
            }
        }
        // If no movement, treat as empty characterwise span (future visual may keep linewise empty)
        if abs_start == abs_after_last {
            return SelectionSpan::new(start, start, SelectionKind::Characterwise);
        }
        return SelectionSpan::new(
            start_of_abs(buffer, abs_start),
            start_of_abs(buffer, abs_after_last),
            SelectionKind::Linewise,
        );
    }
    // Characterwise path replicating previous MotionSpan logic.
    let mut pos = start;
    let orig_abs = absolute_index(buffer, &pos);
    for _ in 0..count {
        apply_motion_once(buffer, &mut pos, motion_kind);
    }
    let final_abs = absolute_index(buffer, &pos);
    if final_abs == orig_abs {
        return SelectionSpan::new(start, start, SelectionKind::Characterwise);
    }
    let (sa, sb) = if final_abs > orig_abs {
        (orig_abs, final_abs)
    } else {
        (final_abs, orig_abs)
    };
    SelectionSpan::new(
        start_of_abs(buffer, sa),
        start_of_abs(buffer, sb),
        SelectionKind::Characterwise,
    )
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

// Map absolute byte index to Position (line scan; mirrors logic in dispatcher change mapping).
fn start_of_abs(buffer: &Buffer, abs: usize) -> Position {
    let mut total = 0usize;
    for line in 0..buffer.line_count() {
        let mut line_total = buffer.line_byte_len(line);
        let ends_nl = buffer
            .line(line)
            .map(|l| l.ends_with('\n'))
            .unwrap_or(false);
        if ends_nl {
            line_total += 1;
        }
        if total + line_total > abs {
            // within this line
            let byte = abs - total;
            return Position {
                line,
                byte: byte.min(buffer.line_byte_len(line)),
            };
        }
        total += line_total;
    }
    Position {
        line: buffer.line_count().saturating_sub(1),
        byte: buffer.line_byte_len(buffer.line_count().saturating_sub(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_state::EditorState;
    use core_text::Buffer;

    #[test]
    fn selection_right_multiple() {
        let buf = Buffer::from_str("test", "abcdef\n").unwrap();
        let st = EditorState::new(buf);
        let sel = resolve_selection(&st, Position::origin(), MotionKind::Right, 3);
        assert_eq!(sel.start.byte, 0); // start position
        assert_eq!(sel.end.byte, 3); // end byte within line
    }

    #[test]
    fn selection_word_forward() {
        let buf = Buffer::from_str("test", "one two three\n").unwrap();
        let st = EditorState::new(buf);
        let sel = resolve_selection(&st, Position::origin(), MotionKind::WordForward, 2);
        assert!(sel.end.byte > sel.start.byte);
    }

    #[test]
    fn selection_left_no_movement() {
        let buf = Buffer::from_str("test", "abc\n").unwrap();
        let st = EditorState::new(buf);
        let sel = resolve_selection(&st, Position::origin(), MotionKind::Left, 5);
        assert_eq!(sel.start, sel.end);
    }
}
