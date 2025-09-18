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
    let span = tracing::trace_span!("undo");
    let _e = span.enter();
    if state.undo(&mut view.cursor) {
        DispatchResult::dirty()
    } else {
        DispatchResult::clean()
    }
}

pub(crate) fn handle_redo(state: &mut EditorState, view: &mut View) -> DispatchResult {
    let span = tracing::trace_span!("redo");
    let _e = span.enter();
    if state.redo(&mut view.cursor) {
        DispatchResult::dirty()
    } else {
        DispatchResult::clean()
    }
}
