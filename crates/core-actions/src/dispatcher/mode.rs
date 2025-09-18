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
use core_state::{EditorState, Mode};

pub(crate) fn handle_mode_change(mc: ModeChange, state: &mut EditorState) -> DispatchResult {
    match mc {
        ModeChange::EnterInsert => {
            state.end_insert_coalescing();
            state.mode = Mode::Insert;
        }
        ModeChange::LeaveInsert => {
            state.end_insert_coalescing();
            state.mode = Mode::Normal;
        }
    }
    DispatchResult::dirty()
}
