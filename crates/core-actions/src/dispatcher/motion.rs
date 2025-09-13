//! Motion sub-dispatch (cursor movement).
//!
//! Scope (R3 Step 1):
//! * Pure, synchronous cursor movement logic (no side-effects beyond cursor & sticky column).
//! * Emits only `DispatchResult::{dirty,clean}`; future steps will extend with semantic deltas.
//!
//! Design Tenet Alignment:
//! * Modularity: isolates motion rules from edits & command parsing.
//! * Unicode correctness: delegates to `core_text::motion::*` which operate on grapheme clusters.
//! * Evolution: future operator-pending motions and counts will layer here without touching unrelated code.
//!
//! Forward Roadmap (beyond R3):
//! * Count-aware motions (e.g. `5j`) applied before invoking the underlying motion primitive.
//! * Operator-target resolution (e.g. in `dw`, motion result will be paired with an operator kind).
//! * Horizontal scroll & fold awareness once those concepts enter `View`.
//! * Scroll-region optimization hints (when combined with layout + capabilities) to minimize redraw.
//!
//! Testing: parity covered indirectly via original dispatcher tests moved intact. Additional
//! focused motion tests will be added when count/operator semantics arrive.

use super::DispatchResult;
use crate::MotionKind;
use core_model::View;
use core_state::EditorState;
use core_state::Mode;
use core_text::{Buffer, Position, motion};

pub(crate) fn handle_motion(
    kind: MotionKind,
    state: &mut EditorState,
    view: &mut View,
    sticky_visual_col: &mut Option<usize>,
) -> DispatchResult {
    let before = view.cursor;
    match kind {
        MotionKind::Left => {
            apply_horizontal_motion(state, &mut view.cursor, motion::left);
            *sticky_visual_col = None;
        }
        MotionKind::Right => {
            apply_horizontal_motion(state, &mut view.cursor, motion::right);
            *sticky_visual_col = None;
        }
        MotionKind::LineStart => {
            apply_horizontal_motion(state, &mut view.cursor, motion::line_start);
            *sticky_visual_col = None;
        }
        MotionKind::LineEnd => {
            apply_horizontal_motion(state, &mut view.cursor, motion::line_end);
            *sticky_visual_col = None;
        }
        MotionKind::Up => {
            *sticky_visual_col =
                apply_vertical_motion(state, &mut view.cursor, *sticky_visual_col, motion::up);
        }
        MotionKind::Down => {
            *sticky_visual_col =
                apply_vertical_motion(state, &mut view.cursor, *sticky_visual_col, motion::down);
        }
        MotionKind::WordForward => {
            apply_horizontal_motion(state, &mut view.cursor, motion::word_forward);
            *sticky_visual_col = None;
        }
        MotionKind::WordBackward => {
            apply_horizontal_motion(state, &mut view.cursor, motion::word_backward);
            *sticky_visual_col = None;
        }
        MotionKind::PageHalfDown => page_half_down(state, view, sticky_visual_col),
        MotionKind::PageHalfUp => page_half_up(state, view, sticky_visual_col),
    }
    // Apply Normal-mode cursor normalization (Vim semantics: block cursor rests on a real cell)
    if matches!(state.mode, Mode::Normal) {
        motion::normalize_normal_mode_position(state.active_buffer(), &mut view.cursor);
    }
    // VisualChar selection expansion (Phase 5 / Step 3): if in VisualChar and cursor moved, update selection span.
    if matches!(state.mode, Mode::VisualChar) && before != view.cursor {
        expand_visual_char_selection(state, before, view.cursor);
    } else if matches!(state.mode, Mode::VisualChar) && state.selection.active.is_none() {
        // Defensive: guarantee an active zero-length selection exists while in VisualChar.
        use core_state::{SelectionKind, SelectionSpan};
        let anchor = view.cursor;
        state.selection.set(SelectionSpan::new(
            anchor,
            anchor,
            SelectionKind::Characterwise,
        ));
        if state.selection.anchor.is_none() {
            state.selection.anchor = Some(anchor);
        }
    }
    if before != view.cursor {
        tracing::trace!(target: "actions.dispatch", motion=?kind, line=before.line, byte=before.byte, to_line=view.cursor.line, to_byte=view.cursor.byte, "motion");
        DispatchResult::dirty()
    } else {
        DispatchResult::clean()
    }
}

fn expand_visual_char_selection(
    state: &mut EditorState,
    anchor_candidate: Position,
    new_cursor: Position,
) {
    use core_state::{SelectionKind, SelectionSpan};
    // Initialize anchor if not already set (defensive for legacy entry paths)
    if state.selection.anchor.is_none() {
        state.selection.anchor = Some(anchor_candidate);
    }
    let anchor = state.selection.anchor.expect("anchor just ensured");
    // Keep span normalized (ordering) while anchor is preserved separately.
    let span = SelectionSpan::new(anchor, new_cursor, SelectionKind::Characterwise);
    state.selection.set(span);
}

fn page_half_down(state: &EditorState, view: &mut View, sticky_visual_col: &mut Option<usize>) {
    // New semantics (Phase 5 / Step 0.2): explicit half-page scroll independent of
    // margin-based auto_scroll. We shift the viewport by half the last known text
    // height and advance the cursor by the same jump, clamping at buffer end.
    let buffer = state.active_buffer();
    let total_lines = buffer.line_count();
    if total_lines == 0 {
        return;
    }
    let h = state.last_text_height.max(1).min(total_lines); // guard small files
    let jump = (h / 2).max(1);
    // Compute new viewport first line; clamp so last page is fully visible.
    let max_first = total_lines.saturating_sub(h);
    let candidate_first = view.viewport_first_line.saturating_add(jump);
    let new_first = candidate_first.min(max_first);
    view.viewport_first_line = new_first;
    // Target cursor line; clamp to last line. Always attempt full jump even when viewport already clamped.
    let target_line = (view.cursor.line + jump).min(total_lines.saturating_sub(1));
    while view.cursor.line < target_line {
        *sticky_visual_col =
            apply_vertical_motion(state, &mut view.cursor, *sticky_visual_col, motion::down);
    }
}

fn page_half_up(state: &EditorState, view: &mut View, sticky_visual_col: &mut Option<usize>) {
    let buffer = state.active_buffer();
    let total_lines = buffer.line_count();
    if total_lines == 0 {
        return;
    }
    let h = state.last_text_height.max(1).min(total_lines);
    let jump = (h / 2).max(1);
    // Compute new viewport first line with saturating subtraction.
    let new_first = view.viewport_first_line.saturating_sub(jump);
    view.viewport_first_line = new_first;
    // Move cursor up by jump lines (clamped at 0).
    let target_line = view.cursor.line.saturating_sub(jump);
    while view.cursor.line > target_line {
        *sticky_visual_col =
            apply_vertical_motion(state, &mut view.cursor, *sticky_visual_col, motion::up);
    }
}

fn apply_horizontal_motion(
    state: &EditorState,
    cursor: &mut Position,
    f: fn(&Buffer, &mut Position),
) {
    f(state.active_buffer(), cursor);
}
fn apply_vertical_motion(
    state: &EditorState,
    cursor: &mut Position,
    sticky: Option<usize>,
    f: fn(&Buffer, &mut Position, Option<usize>) -> Option<usize>,
) -> Option<usize> {
    f(state.active_buffer(), cursor, sticky)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::View;
    use core_state::{EditorState, Mode};
    use core_text::Buffer;

    fn setup(text: &str) -> (EditorState, View, Option<usize>) {
        let buffer = Buffer::from_str("t", text).unwrap();
        let state = EditorState::new(buffer);
        // Mirror EditorModel::new logic: single view bound to active buffer index 0, cursor origin
        let view = View::new(core_model::ViewId(0), 0, Position::origin(), 0);
        (state, view, None)
    }

    #[test]
    fn normal_mode_line_end_clamps() {
        let (mut state, mut view, mut sticky) = setup("abc\n");
        assert_eq!(view.cursor.byte, 0);
        // Move to end
        let res = handle_motion(MotionKind::LineEnd, &mut state, &mut view, &mut sticky);
        assert!(res.dirty);
        // Original line length (excluding newline) = 3; last grapheme starts at byte 2.
        assert_eq!(state.active_buffer().line_byte_len(0), 3);
        assert_eq!(
            view.cursor.byte, 2,
            "cursor should clamp to last grapheme start in Normal mode"
        );
    }

    #[test]
    fn insert_mode_line_end_not_clamped() {
        let (mut state, mut view, mut sticky) = setup("abc\n");
        state.mode = Mode::Insert; // simulate insert mode
        let res = handle_motion(MotionKind::LineEnd, &mut state, &mut view, &mut sticky);
        assert!(res.dirty);
        // In insert mode we retain insertion-point semantics at end-of-line (byte == line_len)
        assert_eq!(view.cursor.byte, 3, "insert mode should not clamp at EOL");
    }

    // --- Phase 5 / Step 0.2 tests: half-page motion semantics ---

    fn mk_buffer(lines: usize) -> String {
        // produce lines numbered 0..lines-1 each ending with \n
        let mut s = String::new();
        for i in 0..lines {
            s.push_str(&format!("{i}\n"));
        }
        s
    }

    #[test]
    fn page_half_down_two_invocations_scrolls() {
        let text = mk_buffer(100);
        let (mut state, mut view, mut sticky) = setup(&text);
        state.last_text_height = 20; // simulate prior auto_scroll height
        // First
        handle_motion(MotionKind::PageHalfDown, &mut state, &mut view, &mut sticky);
        assert_eq!(
            view.viewport_first_line, 10,
            "first half-page scroll viewport"
        );
        assert_eq!(view.cursor.line, 10, "cursor advanced half page");
        // Second
        handle_motion(MotionKind::PageHalfDown, &mut state, &mut view, &mut sticky);
        assert_eq!(
            view.viewport_first_line, 20,
            "second half-page scroll viewport"
        );
        assert_eq!(view.cursor.line, 20, "cursor advanced second half page");
    }

    #[test]
    fn page_half_up_two_invocations_scrolls() {
        let text = mk_buffer(100);
        let (mut state, mut view, mut sticky) = setup(&text);
        state.last_text_height = 20;
        // Seed viewport somewhere lower with cursor.
        view.viewport_first_line = 40;
        view.cursor.line = 50;
        handle_motion(MotionKind::PageHalfUp, &mut state, &mut view, &mut sticky);
        assert_eq!(view.viewport_first_line, 30);
        assert_eq!(view.cursor.line, 40);
        handle_motion(MotionKind::PageHalfUp, &mut state, &mut view, &mut sticky);
        assert_eq!(view.viewport_first_line, 20);
        assert_eq!(view.cursor.line, 30);
    }

    #[test]
    fn page_half_down_clamps_near_eof() {
        // 35 lines with height 20 -> max_first = 15
        let text = mk_buffer(35);
        let (mut state, mut view, mut sticky) = setup(&text);
        state.last_text_height = 20;
        handle_motion(MotionKind::PageHalfDown, &mut state, &mut view, &mut sticky);
        assert_eq!(view.viewport_first_line, 10);
        handle_motion(MotionKind::PageHalfDown, &mut state, &mut view, &mut sticky);
        let total_lines = state.active_buffer().line_count();
        let expected_max_first =
            total_lines.saturating_sub(state.last_text_height.min(total_lines));
        assert_eq!(
            view.viewport_first_line, expected_max_first,
            "clamped to last full page"
        );
        // Additional invocation should keep viewport fixed and cursor should advance by jump (10) but clamp at EOF.
        let prev_cursor = view.cursor.line;
        handle_motion(MotionKind::PageHalfDown, &mut state, &mut view, &mut sticky);
        assert_eq!(
            view.viewport_first_line, expected_max_first,
            "viewport remains clamped"
        );
        assert!(
            view.cursor.line > prev_cursor,
            "cursor continues moving toward EOF"
        );
    }
}
