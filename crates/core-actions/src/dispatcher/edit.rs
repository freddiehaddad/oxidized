//! Text edit action handling (insert/backspace/delete/newline).
//!
//! Scope (R3 Step 1):
//! * Behavior-neutral extraction from monolithic dispatcher.
//! * Responsible for mutating buffer contents via `EditorState` helpers and
//!   updating the active view cursor.
//!
//! Design Tenet Alignment:
//! * Unicode: all insert/delete operations operate on grapheme boundaries
//!   through `core_text` APIs; no byte slicing here.
//! * Modularity: isolates mutation logic so later operator application or
//!   undo engine extraction can compose cleanly.
//! * Breadth-first: keeps logic synchronous and minimal; later async (e.g.,
//!   background formatting) will hook outside this layer.
//!
//! Forward Roadmap:
//! * Operator-driven edits (`dw`, `ciw`, etc.) will channel through augmented
//!   variants rather than inflating this function.
//! * Delta-based undo (future) will reduce snapshot costs; this module will
//!   simply request push events from an `UndoEngine` facade.
//! * Multi-view: edits will eventually broadcast minimal invalidation data
//!   to all affected views.

use super::DispatchResult;
use crate::EditKind;
use core_model::View;
use core_state::{EditorState, Mode};

pub(crate) fn handle_edit(
    kind: EditKind,
    state: &mut EditorState,
    view: &mut View,
) -> DispatchResult {
    match kind {
        EditKind::InsertGrapheme(g) => {
            if matches!(state.mode, Mode::Insert) {
                let span = tracing::trace_span!("edit_insert", grapheme = %g);
                let _e = span.enter();
                state.begin_insert_coalescing(view.cursor);
                state.note_insert_edit();
                let mut pos = view.cursor;
                state.active_buffer_mut().insert_grapheme(&mut pos, &g);
                view.cursor = pos;
                if !state.dirty {
                    state.dirty = true;
                }
                DispatchResult::dirty()
            } else {
                DispatchResult::clean()
            }
        }
        EditKind::InsertNewline => {
            if matches!(state.mode, Mode::Insert) {
                let span = tracing::trace_span!("edit_newline");
                let _e = span.enter();
                state.begin_insert_coalescing(view.cursor);
                state.note_insert_edit();
                let mut pos = view.cursor;
                state.active_buffer_mut().insert_newline(&mut pos);
                view.cursor = pos;
                state.end_insert_coalescing();
                if !state.dirty {
                    state.dirty = true;
                }
                DispatchResult::dirty()
            } else {
                DispatchResult::clean()
            }
        }
        EditKind::Backspace => {
            if matches!(state.mode, Mode::Insert) {
                let span = tracing::trace_span!("edit_backspace");
                let _e = span.enter();
                state.begin_insert_coalescing(view.cursor);
                state.note_insert_edit();
                let mut pos = view.cursor;
                state.active_buffer_mut().delete_grapheme_before(&mut pos);
                view.cursor = pos;
                if !state.dirty {
                    state.dirty = true;
                }
                DispatchResult::dirty()
            } else {
                DispatchResult::clean()
            }
        }
        EditKind::DeleteUnder => {
            if matches!(state.mode, Mode::Normal) {
                let span = tracing::trace_span!("edit_delete_under");
                let _e = span.enter();
                state.push_discrete_edit_snapshot(view.cursor);
                let mut pos = view.cursor;
                state.active_buffer_mut().delete_grapheme_at(&mut pos);
                view.cursor = pos;
                if !state.dirty {
                    state.dirty = true;
                }
                DispatchResult::dirty()
            } else {
                DispatchResult::clean()
            }
        }
    }
}
