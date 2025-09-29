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
                let before_line_count = state.active_buffer().line_count();
                state.begin_insert_coalescing(view.cursor);
                state.note_insert_edit();
                let mut pos = view.cursor;
                {
                    let buffer = state.active_buffer_mut();
                    buffer.insert_newline(&mut pos);
                }
                view.cursor = pos;
                state.end_insert_coalescing();
                let after_line_count = state.active_buffer().line_count();
                let structural =
                    after_line_count > before_line_count || view.cursor.line > before.line;
                tracing::trace!(target: "actions.dispatch", op="insert_newline", line=before.line, byte=before.byte, to_line=view.cursor.line, to_byte=view.cursor.byte, structural, "edit");
                if !state.dirty {
                    state.dirty = true;
                }
                if structural {
                    DispatchResult::buffer_replaced()
                } else {
                    DispatchResult::dirty()
                }
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
        EditKind::DeleteUnder { count, register } => {
            if !matches!(state.mode, Mode::Normal) {
                return DispatchResult::clean();
            }
            let repeat = count.max(1);
            let mut removed = String::new();
            let mut structural = false;
            let mut any = false;
            let target_register = register;
            for _ in 0..repeat {
                if !has_grapheme_under(state, view) {
                    break;
                }
                if !any {
                    state.push_discrete_edit_snapshot(view.cursor);
                }
                if let Some((chunk, chunk_structural)) = delete_under_once(state, view) {
                    removed.push_str(&chunk);
                    structural |= chunk_structural;
                    any = true;
                } else {
                    break;
                }
            }
            if any {
                if !removed.is_empty() {
                    let mut regs = state.registers_facade();
                    regs.write_delete(removed.clone(), target_register);
                }
                tracing::trace!(target: "actions.dispatch", op="delete_under", count=repeat, structural, "edit");
                if !state.dirty {
                    state.dirty = true;
                }
                if structural {
                    DispatchResult::buffer_replaced()
                } else {
                    DispatchResult::dirty()
                }
            } else {
                DispatchResult::clean()
            }
        }
        EditKind::DeleteLeft { count, register } => {
            if !matches!(state.mode, Mode::Normal) {
                return DispatchResult::clean();
            }
            let repeat = count.max(1);
            let mut chunks: Vec<String> = Vec::new();
            let mut any = false;
            let target_register = register;
            for _ in 0..repeat {
                if !has_grapheme_left(state, view) {
                    break;
                }
                if !any {
                    state.push_discrete_edit_snapshot(view.cursor);
                }
                if let Some(chunk) = delete_left_once(state, view) {
                    chunks.push(chunk);
                    any = true;
                } else {
                    break;
                }
            }
            if any {
                let mut removed = String::new();
                for chunk in chunks.iter().rev() {
                    removed.push_str(chunk);
                }
                if !removed.is_empty() {
                    let mut regs = state.registers_facade();
                    regs.write_delete(removed, target_register);
                }
                tracing::trace!(target: "actions.dispatch", op="delete_left", count=repeat, "edit");
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

fn has_grapheme_under(state: &EditorState, view: &View) -> bool {
    let cursor = view.cursor;
    let buffer = state.active_buffer();
    if cursor.line >= buffer.line_count() {
        return false;
    }
    if let Some(line) = buffer.line(cursor.line) {
        let has_newline = line.ends_with('\n');
        let trimmed_len = if has_newline {
            line.len().saturating_sub(1)
        } else {
            line.len()
        };
        if cursor.byte < trimmed_len {
            true
        } else {
            has_newline
        }
    } else {
        false
    }
}

fn delete_under_once(state: &mut EditorState, view: &mut View) -> Option<(String, bool)> {
    let cursor = view.cursor;
    let buffer = state.active_buffer();
    if cursor.line >= buffer.line_count() {
        return None;
    }
    let mut line_owned = buffer.line(cursor.line)?;
    let had_newline = if line_owned.ends_with('\n') {
        line_owned.pop();
        true
    } else {
        false
    };
    if cursor.byte < line_owned.len() {
        let next = core_text::grapheme::next_boundary(&line_owned, cursor.byte);
        if next <= cursor.byte {
            return None;
        }
        let removed = line_owned[cursor.byte..next].to_string();
        let structural = removed.contains('\n');
        let mut pos = view.cursor;
        state.active_buffer_mut().delete_grapheme_at(&mut pos);
        view.cursor = pos;
        Some((removed, structural))
    } else if had_newline {
        let mut pos = view.cursor;
        state.active_buffer_mut().delete_grapheme_at(&mut pos);
        view.cursor = pos;
        Some(("\n".to_string(), true))
    } else {
        None
    }
}

fn has_grapheme_left(state: &EditorState, view: &View) -> bool {
    if view.cursor.byte == 0 {
        return false;
    }
    let buffer = state.active_buffer();
    if view.cursor.line >= buffer.line_count() {
        return false;
    }
    if let Some(line) = buffer.line(view.cursor.line) {
        let trimmed = line.strip_suffix('\n').unwrap_or(&line);
        core_text::grapheme::prev_boundary(trimmed, view.cursor.byte) < view.cursor.byte
    } else {
        false
    }
}

fn delete_left_once(state: &mut EditorState, view: &mut View) -> Option<String> {
    if view.cursor.byte == 0 {
        return None;
    }
    if let Some(line) = state.active_buffer().line(view.cursor.line) {
        let trimmed = line.strip_suffix('\n').unwrap_or(&line);
        let prev = core_text::grapheme::prev_boundary(trimmed, view.cursor.byte);
        if prev >= view.cursor.byte {
            return None;
        }
        let removed = trimmed[prev..view.cursor.byte].to_string();
        let mut pos = view.cursor;
        state.active_buffer_mut().delete_grapheme_before(&mut pos);
        view.cursor = pos;
        Some(removed)
    } else {
        None
    }
}
