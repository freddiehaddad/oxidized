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
                let before = view.cursor;
                state.begin_insert_coalescing(view.cursor);
                state.note_insert_edit();
                let mut pos = view.cursor;
                state.active_buffer_mut().insert_grapheme(&mut pos, &g);
                view.cursor = pos;
                tracing::trace!(target: "actions.dispatch", op="insert_grapheme", grapheme=%g, line=before.line, byte=before.byte, to_line=view.cursor.line, to_byte=view.cursor.byte, "edit");
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
                let before = view.cursor;
                state.begin_insert_coalescing(view.cursor);
                state.note_insert_edit();
                let mut pos = view.cursor;
                state.active_buffer_mut().insert_newline(&mut pos);
                view.cursor = pos;
                state.end_insert_coalescing();
                tracing::trace!(target: "actions.dispatch", op="insert_newline", line=before.line, byte=before.byte, to_line=view.cursor.line, to_byte=view.cursor.byte, "edit");
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
                let before = view.cursor;
                state.begin_insert_coalescing(view.cursor);
                state.note_insert_edit();
                let mut pos = view.cursor;
                state.active_buffer_mut().delete_grapheme_before(&mut pos);
                view.cursor = pos;
                tracing::trace!(target: "actions.dispatch", op="backspace", line=before.line, byte=before.byte, to_line=view.cursor.line, to_byte=view.cursor.byte, "edit");
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
                let before = view.cursor;
                state.push_discrete_edit_snapshot(view.cursor);
                // Capture grapheme under cursor (if any) for register integration (Step 1.1)
                let mut pos = view.cursor;
                let line_len = state.active_buffer().line_byte_len(pos.line);
                if pos.byte < line_len
                    && let Some(mut line_owned) = state.active_buffer().line(pos.line)
                {
                    if line_owned.ends_with('\n') {
                        line_owned.pop();
                    }
                    let next = core_text::grapheme::next_boundary(&line_owned, pos.byte);
                    if next > pos.byte {
                        let removed = &line_owned[pos.byte..next];
                        {
                            let mut regs = state.registers_facade();
                            regs.write_delete(removed.to_string(), None);
                        }
                    }
                }
                state.active_buffer_mut().delete_grapheme_at(&mut pos);
                view.cursor = pos;
                tracing::trace!(target: "actions.dispatch", op="delete_under", line=before.line, byte=before.byte, to_line=view.cursor.line, to_byte=view.cursor.byte, "edit");
                if !state.dirty {
                    state.dirty = true;
                }
                DispatchResult::dirty()
            } else {
                DispatchResult::clean()
            }
        }
        EditKind::DeleteLeft => {
            if matches!(state.mode, Mode::Normal) {
                let before = view.cursor;
                state.push_discrete_edit_snapshot(view.cursor);
                // Capture grapheme before cursor for register integration (future)
                if let Some(line) = state.active_buffer().line(view.cursor.line) {
                    let trimmed = line.strip_suffix('\n').unwrap_or(&line);
                    let prev = core_text::grapheme::prev_boundary(trimmed, view.cursor.byte);
                    if prev < view.cursor.byte {
                        let removed = &trimmed[prev..view.cursor.byte];
                        {
                            let mut regs = state.registers_facade();
                            regs.write_delete(removed.to_string(), None);
                        }
                    }
                }
                let mut pos = view.cursor;
                state.active_buffer_mut().delete_grapheme_before(&mut pos);
                view.cursor = pos;
                tracing::trace!(target: "actions.dispatch", op="delete_left", line=before.line, byte=before.byte, to_line=view.cursor.line, to_byte=view.cursor.byte, "edit");
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
