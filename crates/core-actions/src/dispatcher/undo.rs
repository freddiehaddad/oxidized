//! Undo / Redo handling extraction.
//!
//! Scope (R3 Step 1): delegate calls into the existing snapshot-based undo
//! stacks inside `EditorState`. Isolation here prepares for Step 8 where an
//! `UndoEngine` wrapper will own this logic and expose a smaller surface.
//!
//! Forward Roadmap:
//! * Replace snapshot cloning with delta (operation log) model.
//! * Provide structured metadata for future time-travel / tree history.
//! * Expose observer hook for plugins (e.g. to display undo tree).

use super::DispatchResult;
use core_model::View;
use core_state::EditorState;

pub(crate) fn handle_undo(state: &mut EditorState, view: &mut View) -> DispatchResult {
    let before = state.active_buffer().line_count();
    if state.undo(&mut view.cursor) {
        let after = state.active_buffer().line_count();
        tracing::trace!(target: "actions.dispatch", op="undo", buffer_changed = (before != after), "undo");
        if before != after {
            DispatchResult::buffer_replaced()
        } else {
            DispatchResult::dirty()
        }
    } else {
        DispatchResult::clean()
    }
}

pub(crate) fn handle_redo(state: &mut EditorState, view: &mut View) -> DispatchResult {
    let before = state.active_buffer().line_count();
    if state.redo(&mut view.cursor) {
        let after = state.active_buffer().line_count();
        tracing::trace!(target: "actions.dispatch", op="redo", buffer_changed = (before != after), "redo");
        if before != after {
            DispatchResult::buffer_replaced()
        } else {
            DispatchResult::dirty()
        }
    } else {
        DispatchResult::clean()
    }
}
