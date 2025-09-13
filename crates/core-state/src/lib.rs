//! Editor state: buffer collection, mode, and caret position (Cursor moved to core-text as Position).

use core_text::{Buffer, Position};

/// Maximum number of snapshots retained in undo history.
const UNDO_HISTORY_MAX: usize = 200;

/// A full-state snapshot for undo/redo (Phase 1: coarse clone for simplicity).
#[derive(Clone)]
pub struct EditSnapshot {
    pub buffer: Buffer,
    pub position: Position,
    pub mode: Mode,
}

/// Current editor mode.
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    /// Normal command/navigation mode.
    Normal,
    /// Insert text mode (appends / inserts grapheme clusters into the active buffer).
    Insert,
}

/// Top-level editor state container (single-buffer in Phase 0).
pub struct EditorState {
    /// All loaded buffers (Phase 0: exactly one).
    pub buffers: Vec<Buffer>,
    /// Index into `buffers` of the active buffer.
    pub active: usize,
    /// Current editor mode.
    pub mode: Mode,
    /// Primary caret position (grapheme boundary) within active buffer.
    pub position: Position,
    /// Undo history (older at lower indices). Most recent snapshot is at the end.
    pub undo_stack: Vec<EditSnapshot>,
    /// Redo history (most recently undone states). Cleared on new edit.
    pub redo_stack: Vec<EditSnapshot>,
    /// Indicates we are in the middle of an Insert coalescing run (snapshot already taken).
    pub insert_run_active: bool,
}

impl EditorState {
    /// Create a new state with a single active buffer.
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffers: vec![buffer],
            active: 0,
            mode: Mode::Normal,
            position: Position::origin(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            insert_run_active: false,
        }
    }

    /// Borrow the currently active buffer.
    pub fn active_buffer(&self) -> &Buffer {
        &self.buffers[self.active]
    }

    #[allow(dead_code)]
    /// Temporary mutable accessor for the active buffer.
    ///
    /// This is intentionally unused until Insert mode (Task 5a) wires edit
    /// actions through a dispatcher that mutates the buffer. Keeping it now
    /// avoids repeating direct `self.buffers[self.active]` indexing patterns
    /// across early mutation call sites and clarifies the intended single
    /// mutation choke point. The `#[allow(dead_code)]` will be removed once
    /// the first edit path (printable grapheme insertion) lands.
    fn active_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.active]
    }

    /// Capture a snapshot of the current editable state (active single buffer only in Phase 1).
    pub fn push_snapshot(&mut self) {
        let snap = EditSnapshot {
            buffer: self.active_buffer().clone(),
            position: self.position,
            mode: self.mode,
        };
        self.undo_stack.push(snap);
        if self.undo_stack.len() > UNDO_HISTORY_MAX {
            let _ = self.undo_stack.remove(0);
        }
        // New edit invalidates redo stack.
        self.redo_stack.clear();
    }

    /// Restore previously captured snapshot (caller ensures existence). Returns true if restored.
    pub fn undo(&mut self) -> bool {
        if let Some(last) = self.undo_stack.pop() {
            // Push current state to redo before replacing.
            let current = EditSnapshot {
                buffer: self.active_buffer().clone(),
                position: self.position,
                mode: self.mode,
            };
            self.redo_stack.push(current);
            self.buffers[self.active] = last.buffer;
            self.position = last.position;
            self.mode = last.mode; // mode restore is simplistic; acceptable Phase 1.
            true
        } else {
            false
        }
    }

    /// Redo previously undone snapshot. Returns true if applied.
    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            // Save current to undo stack.
            let current = EditSnapshot {
                buffer: self.active_buffer().clone(),
                position: self.position,
                mode: self.mode,
            };
            self.undo_stack.push(current);
            self.buffers[self.active] = next.buffer;
            self.position = next.position;
            self.mode = next.mode;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_text::Buffer;

    #[test]
    fn cursor_initializes_at_origin() {
        let buf = Buffer::from_str("test", "Hello").unwrap();
        let st = EditorState::new(buf);
        assert_eq!(st.position.line, 0);
        assert_eq!(st.position.byte, 0);
        assert!(matches!(st.mode, Mode::Normal));
    }

    #[test]
    fn cursor_clamp() {
        let buf = Buffer::from_str("test", "Hello\nWorld").unwrap();
        let mut st = EditorState::new(buf);
        st.position.line = 10; // beyond
        st.position.byte = 999;
        let line_count = st.active_buffer().line_count();
        let last_len = st.active_buffer().line_byte_len(line_count - 1);
        // Provide a closure that does not borrow `st` to satisfy borrow checker.
        st.position.clamp_to(line_count, |_| last_len);
        assert_eq!(st.position.line, line_count - 1); // last valid line index
        assert_eq!(st.position.byte, last_len);
    }

    #[test]
    fn snapshot_push_and_undo_redo() {
        let buf = Buffer::from_str("t", "one").unwrap();
        let mut st = EditorState::new(buf);
        // initial snapshot
        st.push_snapshot();
        // mutate buffer by inserting (simulate via direct buffer clone replace for now)
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, "X");
            st.buffers[st.active] = modified;
            st.position = pos;
        }
        st.push_snapshot();
        assert!(st.undo());
        // After undo we can redo
        assert!(st.redo());
    }
}
