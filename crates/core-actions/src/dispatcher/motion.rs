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
use core_text::{Buffer, Position, motion};

pub(crate) fn handle_motion(
    kind: MotionKind,
    state: &mut EditorState,
    view: &mut View,
    sticky_visual_col: &mut Option<usize>,
) -> DispatchResult {
    let span = tracing::trace_span!("motion", kind = ?kind, line = view.cursor.line, byte = view.cursor.byte);
    let _e = span.enter();
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
    if before != view.cursor {
        DispatchResult::dirty()
    } else {
        DispatchResult::clean()
    }
}

fn page_half_down(state: &EditorState, view: &mut View, sticky_visual_col: &mut Option<usize>) {
    let h = state.last_text_height.max(1);
    let jump = (h / 2).max(1);
    let target =
        (view.cursor.line + jump).min(state.active_buffer().line_count().saturating_sub(1));
    let mut moved = false;
    while view.cursor.line < target {
        *sticky_visual_col =
            apply_vertical_motion(state, &mut view.cursor, *sticky_visual_col, motion::down);
        moved = true;
    }
    if !moved {
        *sticky_visual_col =
            apply_vertical_motion(state, &mut view.cursor, *sticky_visual_col, motion::down);
    }
}

fn page_half_up(state: &EditorState, view: &mut View, sticky_visual_col: &mut Option<usize>) {
    let h = state.last_text_height.max(1);
    let jump = (h / 2).max(1);
    let target = view.cursor.line.saturating_sub(jump);
    let mut moved = false;
    while view.cursor.line > target {
        *sticky_visual_col =
            apply_vertical_motion(state, &mut view.cursor, *sticky_visual_col, motion::up);
        moved = true;
    }
    if !moved {
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
