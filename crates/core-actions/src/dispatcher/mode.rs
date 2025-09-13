//! Mode transition handling (Normal <-> Insert).
//!
//! Scope (R3 Step 1): minimal synchronous state transition + insert run
//! coalescing finalization. This keeps mode logic isolated for future
//! expansions (Visual, Command, Operator-Pending, etc.).
//!
//! Forward Roadmap:
//! * Introduce additional modes (Visual, VisualLine, Replace) without
//!   inflating unrelated dispatcher code.
//! * Mode change side-effects (cursor shape, status line delta emission,
//!   semantic render delta) will hook here in Step 6 when `StatusLine`
//!   deltas are introduced.

use super::DispatchResult;
use crate::ModeChange;
use core_model::View;
use core_state::InsertRun;
use core_state::{EditorState, Mode};

pub(crate) fn handle_mode_change(
    mc: ModeChange,
    state: &mut EditorState,
    view: &mut View,
) -> DispatchResult {
    match mc {
        ModeChange::EnterInsert => {
            // Starting fresh insert run; ensure any previous run was ended defensively.
            state.end_insert_coalescing();
            state.mode = Mode::Insert;
            DispatchResult::dirty()
        }
        ModeChange::LeaveInsert => {
            // Determine if we should retreat cursor (Vim parity) BEFORE ending run; consult insert_run.
            let should_retreat =
                matches!(state.insert_run(), InsertRun::Active { edits, .. } if *edits > 0);
            state.end_insert_coalescing();
            if should_retreat && let Some(line) = state.active_buffer().line(view.cursor.line) {
                let raw = line.as_str();
                let trimmed = raw.strip_suffix('\n').unwrap_or(raw);
                if view.cursor.byte > 0 && view.cursor.byte <= trimmed.len() {
                    let prev = core_text::grapheme::prev_boundary(trimmed, view.cursor.byte);
                    view.cursor.byte = prev;
                }
            }
            state.mode = Mode::Normal;
            DispatchResult::dirty()
        }
        ModeChange::EnterVisualChar => {
            // Initialize anchored empty selection at cursor.
            use core_state::{SelectionKind, SelectionSpan};
            let pos = view.cursor;
            let span = SelectionSpan::new(pos, pos, SelectionKind::Characterwise);
            state.selection.set(span);
            state.selection.anchor = Some(pos);
            state.mode = Mode::VisualChar;
            DispatchResult::dirty()
        }
        ModeChange::LeaveVisualChar => {
            state.selection.clear();
            state.mode = Mode::Normal;
            DispatchResult::dirty()
        }
    }
}
