//! Editor state: buffer collection, mode, and caret position (Cursor moved to core-text as Position).

use core_text::{Buffer, Position};
use tracing::trace;

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

    /// Mutable accessor for the active buffer (Phase 1: single buffer only).
    /// All text mutations in editing paths should flow through this to keep
    /// future invariants (multi-buffer, dirty tracking) centralized.
    pub fn active_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.active]
    }

    /// Capture a snapshot of the current editable state (active single buffer only in Phase 1).
    pub fn push_snapshot(&mut self) {
        let snap = EditSnapshot {
            buffer: self.active_buffer().clone(),
            position: self.position,
            mode: self.mode,
        };
        let rope_chars_before = self.active_buffer().line_count(); // coarse metric (lines)
        self.undo_stack.push(snap);
        trace!(
            undo_depth = self.undo_stack.len(),
            redo_depth = self.redo_stack.len(),
            lines = rope_chars_before,
            "push_snapshot"
        );
        if self.undo_stack.len() > UNDO_HISTORY_MAX {
            let _ = self.undo_stack.remove(0);
            trace!("undo_stack_trimmed oldest removed");
        }
        // New edit invalidates redo stack.
        self.redo_stack.clear();
        trace!("redo_stack_cleared_on_new_edit");
    }

    /// Begin an Insert-mode coalescing run: push a pre-edit snapshot only once.
    ///
    /// Call this right before applying the *first* grapheme insertion after entering
    /// Insert mode (or after a newline / Esc boundary ends a prior run).
    /// Subsequent inserts in the same run MUST NOT call this again until
    /// `end_insert_coalescing` is invoked.
    pub fn begin_insert_coalescing(&mut self) {
        if !self.insert_run_active {
            self.push_snapshot();
            self.insert_run_active = true;
        }
    }

    /// Ends the current Insert-mode coalescing run. Boundary triggers:
    /// * Leaving Insert mode (Esc)
    /// * Inserting a newline (treated as a boundary in 5b)
    pub fn end_insert_coalescing(&mut self) {
        self.insert_run_active = false;
    }

    /// Push a discrete edit snapshot (used for Normal mode edits like `x` or
    /// other non-coalesced operations). Always pushes regardless of coalescing flag.
    pub fn push_discrete_edit_snapshot(&mut self) {
        self.push_snapshot();
    }

    /// Restore previously captured snapshot (caller ensures existence). Returns true if restored.
    pub fn undo(&mut self) -> bool {
        if let Some(last) = self.undo_stack.pop() {
            trace!(
                undo_depth = self.undo_stack.len(),
                redo_depth = self.redo_stack.len(),
                "undo_pop"
            );
            // Push current state to redo before replacing.
            let current = EditSnapshot {
                buffer: self.active_buffer().clone(),
                position: self.position,
                mode: self.mode,
            };
            self.redo_stack.push(current);
            trace!(redo_depth = self.redo_stack.len(), "redo_push_from_undo");
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
            trace!(
                redo_depth = self.redo_stack.len(),
                undo_depth = self.undo_stack.len(),
                "redo_pop"
            );
            // Save current to undo stack.
            let current = EditSnapshot {
                buffer: self.active_buffer().clone(),
                position: self.position,
                mode: self.mode,
            };
            self.undo_stack.push(current);
            trace!(undo_depth = self.undo_stack.len(), "undo_push_from_redo");
            self.buffers[self.active] = next.buffer;
            self.position = next.position;
            self.mode = next.mode;
            true
        } else {
            false
        }
    }
}

// NOTE (4.11 Deferred): Time-based coalescing for Insert runs is intentionally not implemented yet.
// Future work: maintain timestamp of last inserted grapheme; if elapsed > THRESHOLD_MS, begin a new coalescing run.
// This will integrate with an async timer or action producer feeding a boundary Action without blocking input.

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

    #[test]
    fn coalescing_run_only_pushes_once() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        st.mode = Mode::Insert;
        st.begin_insert_coalescing(); // should push snapshot
        let len_after_first = st.undo_stack.len();
        assert_eq!(len_after_first, 1);

        // Simulate multiple inserts inside same run (no further snapshots expected)
        for ch in ["a", "b", "c"] {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, ch);
            st.buffers[st.active] = modified;
            st.position = pos;
            st.begin_insert_coalescing(); // no-op after first
        }
        assert_eq!(
            st.undo_stack.len(),
            1,
            "coalescing inserted multiple snapshots"
        );

        // End run and start new one -> pushes again
        st.end_insert_coalescing();
        st.begin_insert_coalescing();
        assert_eq!(
            st.undo_stack.len(),
            2,
            "second run did not create new snapshot"
        );
    }

    #[test]
    fn redo_cleared_after_new_coalesced_edit() {
        let buf = Buffer::from_str("t", "Hello").unwrap();
        let mut st = EditorState::new(buf);
        st.push_snapshot(); // baseline snapshot
        // Apply an edit by direct mutation and push snapshot to simulate earlier approach
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, "!");
            st.buffers[st.active] = modified;
            st.position = pos;
        }
        st.push_snapshot();
        assert!(st.undo()); // creates one entry in redo
        assert_eq!(st.redo_stack.len(), 1);

        // New coalesced run should clear redo stack
        st.mode = Mode::Insert;
        st.begin_insert_coalescing();
        assert_eq!(st.redo_stack.len(), 0, "redo stack not cleared on new edit");
    }

    #[test]
    fn undo_stack_capped() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        // Push more than max
        for i in 0..(UNDO_HISTORY_MAX + 10) {
            // Apply a trivial mutation to differentiate snapshots
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, "x");
            st.buffers[st.active] = modified;
            st.position = pos;
            st.push_snapshot();
            // ensure length never exceeds cap + 1 transiently (since we remove after push)
            assert!(st.undo_stack.len() <= UNDO_HISTORY_MAX);
            // Reset to allow next iteration to mutate from previous base (simplistic)
            let _ = i; // silence unused warning if any
        }
        assert_eq!(st.undo_stack.len(), UNDO_HISTORY_MAX);
    }

    #[test]
    fn coalesced_insert_run_undo_redo() {
        // Simulate minimal insert mode run: enter Insert, type 'a','b','c', Esc boundary.
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        st.mode = Mode::Insert;
        // First insert triggers snapshot
        st.begin_insert_coalescing();
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, "a");
            st.buffers[st.active] = modified;
            st.position = pos;
        }
        // Additional inserts in same run (should not push extra snapshots)
        for ch in ["b", "c"] {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, ch);
            st.buffers[st.active] = modified;
            st.position = pos;
            st.begin_insert_coalescing(); // no-op while active
        }
        // End run via Esc boundary semantics
        st.end_insert_coalescing();
        assert_eq!(st.undo_stack.len(), 1, "expected single snapshot for run");
        // Perform undo: buffer should become empty again
        assert!(st.undo());
        assert_eq!(st.active_buffer().line(0).unwrap_or_default(), "");
        // Redo should restore 'abc'
        assert!(st.redo());
        assert!(
            st.active_buffer()
                .line(0)
                .unwrap_or_default()
                .starts_with("abc")
        );
    }
}
