//! Editor state: buffer collection, mode, and caret position.
//!
//! Insert Coalescing (Phase 1):
//! - A contiguous run of Insert-mode text entry (grapheme inserts and backspaces) is captured
//!   by a *single* undo snapshot taken lazily at the first mutation in the run.
//! - Coalescing boundaries: pressing `Esc` to leave Insert mode or inserting a newline.
//!   Backspace does NOT end a run; it mutates within the same coalesced unit.
//! - After a boundary, the next Insert edit begins a fresh run and takes a new snapshot.
//! - Rationale: Most typing bursts should undo as atomic units while keeping implementation
//!   simple (no timers yet). Newline chosen as a structural boundary for intuitive multi-line undo.
//! - Future: time-based boundaries and differential snapshots (to avoid whole-buffer cloning).
//!
//! Normal Mode discrete edits (currently just `x`) always push an immediate snapshot per action
//! so each delete can be undone individually. This fulfills Task 4.9/6.2 semantics.
//!
//! SnapshotKind & Mode Restoration:
//! - `SnapshotKind::Edit` ignores restoring the editor mode on undo/redo so that leaving Insert
//!   mode with `Esc` then undoing the last run does not unexpectedly re-enter Insert mode.
//! - Future kinds (mode transitions, structural operations) can opt-in to mode restoration.
//!
//! Telemetry Integration:
//! - Snapshot lifecycle emits trace events (`push_snapshot`, `undo_pop`, `redo_pop`, stack trims, redo clear).
//! - Edit application spans (`edit_insert`, `edit_newline`, `edit_backspace`, `edit_delete_under`) and
//!   navigation (`motion`) live in the dispatcher; undo/redo spans wrap calls into this module.

use core_text::{Buffer, Position};
use tracing::trace;

/// Maximum number of snapshots retained in undo history.
const UNDO_HISTORY_MAX: usize = 200;

/// Snapshot classification controlling restore semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotKind {
    /// Text edit snapshot (coalesced insert run or discrete edit). Mode is not restored.
    Edit,
    // Future: ModeTransition, Structural, etc.
}

/// A full-state snapshot for undo/redo (Phase 1: coarse clone for simplicity).
#[derive(Clone)]
pub struct EditSnapshot {
    pub kind: SnapshotKind,
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
    /// Optional file name associated with the active buffer (Phase 2 Step 1).
    /// None for unnamed / scratch buffer until a path is assigned via open or write-as.
    pub file_name: Option<std::path::PathBuf>,
    /// Dirty flag indicating buffer has unsaved modifications relative to last open/save.
    /// Phase 2 Step 1: plumbed but mutations do not yet toggle this (handled in later steps).
    pub dirty: bool,
    /// Undo history (older at lower indices). Most recent snapshot is at the end.
    pub undo_stack: Vec<EditSnapshot>,
    /// Redo history (most recently undone states). Cleared on new edit.
    pub redo_stack: Vec<EditSnapshot>,
    /// Insert coalescing run state (Refactor R1 Step 6): tracks active run start & edit count.
    pub insert_run: InsertRun,
    /// Command-line (":" style) transient input buffer state (Refactor R1 Step 2).
    pub command_line: CommandLineState,
}

/// Insert run state tracking (Refactor R1 Step 6).
///
/// In Phase 1 this is informational beyond boundary detection and counting edits. Future
/// heuristics (time-based split, telemetry) can leverage `started_at` and `edits`.
#[derive(Debug, Clone)]
pub enum InsertRun {
    Inactive,
    Active {
        started_at: std::time::Instant,
        edits: u32,
    },
}

/// Minimal command-line state container (Refactor R1 Step 2).
/// Breadth-first: only stores raw buffer including leading ':' when active.
/// Future (Phase 2+): history, cursor within command line, validation status, suggestion UI.
#[derive(Debug, Default, Clone)]
pub struct CommandLineState {
    buf: String,
}

impl CommandLineState {
    /// Returns true if a command is being entered (buffer starts with ':').
    pub fn is_active(&self) -> bool {
        self.buf.starts_with(':')
    }
    /// Expose raw buffer for rendering/translation.
    pub fn buffer(&self) -> &str {
        &self.buf
    }
    /// Clear command buffer (leave inactive state).
    pub fn clear(&mut self) {
        self.buf.clear();
    }
    /// Begin a new command (resets existing content) – ensures leading ':'.
    pub fn begin(&mut self) {
        self.buf.clear();
        self.buf.push(':');
    }
    /// Push a character (assumes already active or will auto-activate if empty and ch not ':').
    pub fn push_char(&mut self, ch: char) {
        if self.buf.is_empty() && ch != ':' {
            self.buf.push(':');
        }
        self.buf.push(ch);
    }
    /// Backspace behavior inside command line (keeps ':' sentinel until removing last char resets activity).
    pub fn backspace(&mut self) {
        if self.buf.len() > 1 {
            self.buf.pop();
        } else {
            self.buf.clear();
        }
    }
}

impl EditorState {
    /// Create a new state with a single active buffer.
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffers: vec![buffer],
            active: 0,
            mode: Mode::Normal,
            position: Position::origin(),
            file_name: None,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            insert_run: InsertRun::Inactive,
            command_line: CommandLineState::default(),
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
    pub fn push_snapshot(&mut self, kind: SnapshotKind) {
        let snap = EditSnapshot {
            kind,
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
    /// Begin (or continue) an Insert-mode coalescing run.
    ///
    /// On the *first* call in a run a snapshot of the pre-edit state is pushed. Subsequent
    /// calls while `insert_run_active` is true are no-ops. Call this immediately before applying
    /// an Insert mutation (insert grapheme, backspace, newline) so the pre-edit state is captured
    /// exactly once.
    pub fn begin_insert_coalescing(&mut self) {
        match self.insert_run {
            InsertRun::Inactive => {
                self.push_snapshot(SnapshotKind::Edit);
                self.insert_run = InsertRun::Active {
                    started_at: std::time::Instant::now(),
                    edits: 0,
                };
            }
            InsertRun::Active { .. } => { /* already active */ }
        }
    }

    /// Ends the current Insert-mode coalescing run. Boundary triggers:
    /// * Leaving Insert mode (Esc)
    /// * Inserting a newline (treated as a boundary in 5b)
    ///   End the current Insert run (called on Esc or newline).
    ///
    /// The next Insert mutation will start a new run and thus push a new snapshot via
    /// `begin_insert_coalescing`.
    pub fn end_insert_coalescing(&mut self) {
        self.insert_run = InsertRun::Inactive;
    }

    /// Push a discrete edit snapshot (used for Normal mode edits like `x` or
    /// other non-coalesced operations). Always pushes regardless of coalescing flag.
    /// Push a discrete (non-coalesced) edit snapshot. Used for Normal mode edits like `x` where
    /// each action should undo independently. Ignores any Insert coalescing state.
    pub fn push_discrete_edit_snapshot(&mut self) {
        self.push_snapshot(SnapshotKind::Edit);
    }

    /// Increment the edit counter for an active insert run (used for diagnostics / future heuristics).
    pub fn note_insert_edit(&mut self) {
        if let InsertRun::Active { edits, .. } = &mut self.insert_run {
            *edits += 1;
        }
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
                kind: last.kind,
                buffer: self.active_buffer().clone(),
                position: self.position,
                mode: self.mode,
            };
            self.redo_stack.push(current);
            trace!(redo_depth = self.redo_stack.len(), "redo_push_from_undo");
            self.buffers[self.active] = last.buffer;
            self.position = last.position;
            if !matches!(last.kind, SnapshotKind::Edit) {
                self.mode = last.mode;
            }
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
                kind: next.kind,
                buffer: self.active_buffer().clone(),
                position: self.position,
                mode: self.mode,
            };
            self.undo_stack.push(current);
            trace!(undo_depth = self.undo_stack.len(), "undo_push_from_redo");
            self.buffers[self.active] = next.buffer;
            self.position = next.position;
            if !matches!(next.kind, SnapshotKind::Edit) {
                self.mode = next.mode;
            }
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
        st.push_snapshot(SnapshotKind::Edit);
        // mutate buffer by inserting (simulate via direct buffer clone replace for now)
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, "X");
            st.buffers[st.active] = modified;
            st.position = pos;
        }
        st.push_snapshot(SnapshotKind::Edit);
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
        assert!(matches!(st.insert_run, InsertRun::Active { .. }));

        // Simulate multiple inserts inside same run (no further snapshots expected)
        for ch in ["a", "b", "c"] {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, ch);
            st.buffers[st.active] = modified;
            st.position = pos;
            st.begin_insert_coalescing(); // no-op after first
            st.note_insert_edit();
        }
        assert_eq!(
            st.undo_stack.len(),
            1,
            "coalescing inserted multiple snapshots"
        );

        // End run and start new one -> pushes again
        st.end_insert_coalescing();
        st.begin_insert_coalescing();
        st.note_insert_edit();
        assert_eq!(
            st.undo_stack.len(),
            2,
            "second run did not create new snapshot"
        );
        if let InsertRun::Active { edits, .. } = st.insert_run {
            assert_eq!(edits, 1);
        }
    }

    #[test]
    fn redo_cleared_after_new_coalesced_edit() {
        let buf = Buffer::from_str("t", "Hello").unwrap();
        let mut st = EditorState::new(buf);
        st.push_snapshot(SnapshotKind::Edit); // baseline snapshot
        // Apply an edit by direct mutation and push snapshot to simulate earlier approach
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, "!");
            st.buffers[st.active] = modified;
            st.position = pos;
        }
        st.push_snapshot(SnapshotKind::Edit);
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
            st.push_snapshot(SnapshotKind::Edit);
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
            st.note_insert_edit();
        }
        if let InsertRun::Active { edits, .. } = st.insert_run {
            assert_eq!(edits, 2);
        }
        // End run via Esc boundary semantics
        st.end_insert_coalescing();
        // Simulate Esc leave insert for refined undo semantics
        st.mode = Mode::Normal;
        assert_eq!(st.undo_stack.len(), 1, "expected single snapshot for run");
        // Perform undo: buffer should become empty again
        assert!(st.undo());
        assert_eq!(st.active_buffer().line(0).unwrap_or_default(), "");
        assert!(
            matches!(st.mode, Mode::Normal),
            "mode restored unexpectedly to Insert"
        );
        // Redo should restore 'abc'
        assert!(st.redo());
        assert!(
            st.active_buffer()
                .line(0)
                .unwrap_or_default()
                .starts_with("abc")
        );
        assert!(
            matches!(st.mode, Mode::Normal),
            "redo changed mode unexpectedly"
        );
    }

    #[test]
    fn undo_does_not_restore_insert_mode() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        st.mode = Mode::Insert;
        st.begin_insert_coalescing();
        for ch in ["a", "b"] {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, ch);
            st.buffers[st.active] = modified;
            st.position = pos;
            st.begin_insert_coalescing();
        }
        st.end_insert_coalescing();
        st.mode = Mode::Normal; // simulate Esc
        assert!(st.undo());
        assert!(matches!(st.mode, Mode::Normal));
        assert!(st.redo());
        assert!(matches!(st.mode, Mode::Normal));
    }

    #[test]
    fn newline_is_coalescing_boundary() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        st.mode = Mode::Insert;
        // Insert 'a' then newline then 'b'; expect two snapshots (one for first run before 'a', one for second run after newline)
        st.begin_insert_coalescing();
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, "a");
            st.buffers[st.active] = modified;
            st.position = pos;
        }
        // Newline ends run
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_newline(&mut pos);
            st.buffers[st.active] = modified;
            st.position = pos;
        }
        st.end_insert_coalescing();
        // Start second run after boundary
        st.begin_insert_coalescing();
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, "b");
            st.buffers[st.active] = modified;
            st.position = pos;
        }
        assert_eq!(
            st.undo_stack.len(),
            2,
            "expected two snapshots across newline boundary"
        );
    }

    #[test]
    fn backspace_stays_in_run() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        st.mode = Mode::Insert;
        st.begin_insert_coalescing();
        // Insert two characters
        for ch in ["a", "b"] {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, ch);
            st.buffers[st.active] = modified;
            st.position = pos;
            st.begin_insert_coalescing();
            st.note_insert_edit();
        }
        // Backspace one character (should not end run or create new snapshot)
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.delete_grapheme_before(&mut pos);
            st.buffers[st.active] = modified;
            st.position = pos;
            st.begin_insert_coalescing(); // still active
            st.note_insert_edit();
        }
        assert_eq!(
            st.undo_stack.len(),
            1,
            "backspace created unexpected snapshot"
        );
        if let InsertRun::Active { edits, .. } = st.insert_run {
            assert_eq!(edits, 3);
        }
        // End run and undo should revert to empty buffer
        st.end_insert_coalescing();
        st.mode = Mode::Normal;
        assert!(st.undo());
        assert_eq!(st.active_buffer().line(0).unwrap_or_default(), "");
    }

    #[test]
    fn insert_run_newline_resets_counter() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        st.mode = Mode::Insert;
        st.begin_insert_coalescing();
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, "a");
            st.buffers[st.active] = modified;
            st.position = pos;
        }
        st.note_insert_edit();
        st.end_insert_coalescing();
        st.begin_insert_coalescing();
        {
            let mut modified = st.active_buffer().clone();
            let mut pos = st.position;
            modified.insert_grapheme(&mut pos, "b");
            st.buffers[st.active] = modified;
            st.position = pos;
        }
        st.note_insert_edit();
        if let InsertRun::Active { edits, .. } = st.insert_run {
            assert_eq!(edits, 1);
        }
    }
}
