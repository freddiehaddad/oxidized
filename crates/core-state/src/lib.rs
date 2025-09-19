//! Editor state: buffer collection, mode, undo engine, and core editor metadata.
//!
//! Refactor R3 Additions:
//! - Undo logic extracted into `undo::UndoEngine` (Step 8) isolating snapshot
//!   push/undo/redo & coalescing policy from higher-level dispatch.
//! - Ephemeral status messages & command line state surfaced here but rendered
//!   via `core-render::status` to keep presentation logic decoupled.
//! - View/model separation lives in `core-model`; this crate intentionally
//!   remains buffer-centric (single active buffer in Phase 3) while `EditorModel`
//!   orchestrates per-view cursors and scroll state.
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
pub mod undo;
use undo::UndoEngine;
pub use undo::{InsertRun, SnapshotKind, UNDO_HISTORY_MAX};

// (Undo snapshot types moved to undo module)

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
    pub buffers: Vec<Buffer>,
    pub active: usize,
    pub last_text_height: usize,
    pub mode: Mode,
    pub file_name: Option<std::path::PathBuf>,
    pub dirty: bool,
    undo: UndoEngine,
    pub command_line: CommandLineState,
    pub ephemeral_status: Option<EphemeralMessage>,
    pub original_line_ending: LineEnding,
    pub had_trailing_newline: bool,
    pub config_vertical_margin: usize,
}

/// Line ending style detected from source file (Phase 2 Step 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Cr,
    Crlf,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::Cr => "\r",
            LineEnding::Crlf => "\r\n",
        }
    }
}

/// Result of normalizing line endings (Phase 2 Step 9).
pub struct NormalizedText {
    pub normalized: String,         // LF-only content
    pub original: LineEnding,       // majority/origin style
    pub had_trailing_newline: bool, // original trailing newline presence
    pub mixed: bool,                // true if multiple styles encountered
}

/// Detect and normalize line endings of `input` to LF-only internal representation.
/// Counts CRLF, LF, and CR occurrences; picks the majority (ties resolved by precedence CRLF > LF > CR).
/// Mixed flag is true if more than one style observed and at least one count differs from majority.
pub fn normalize_line_endings(input: &str) -> NormalizedText {
    // Pass 1: count occurrences of CRLF, LF, and solitary CR
    let bytes = input.as_bytes();
    let mut i = 0usize;
    let mut crlf = 0usize;
    let mut lf = 0usize;
    let mut cr = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    crlf += 1;
                    i += 2;
                } else {
                    cr += 1;
                    i += 1;
                }
            }
            b'\n' => {
                lf += 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    let had_trailing_newline = if input.is_empty() {
        false
    } else {
        input.ends_with("\r\n") || input.ends_with('\n') || input.ends_with('\r')
    };
    // Majority with precedence CRLF > LF > CR for ties
    let mut original = LineEnding::Lf;
    let mut max = 0usize;
    for (style, count) in [
        (LineEnding::Crlf, crlf),
        (LineEnding::Lf, lf),
        (LineEnding::Cr, cr),
    ] {
        if count > max {
            max = count;
            original = style;
        }
    }
    let non_zero = [crlf, lf, cr].iter().filter(|c| **c > 0).count();
    let mixed = non_zero > 1 && [crlf, lf, cr].iter().any(|c| *c > 0 && *c != max);
    // Fast path: nothing to rewrite.
    if crlf == 0 && cr == 0 {
        return NormalizedText {
            normalized: input.to_string(),
            original,
            had_trailing_newline,
            mixed,
        };
    }
    // Slow path: span-copy. We only slice at '\r' boundaries so UTF-8 multi-byte sequences remain intact.
    let mut out = String::with_capacity(input.len());
    let mut seg_start = 0usize;
    let mut j = 0usize;
    while j < bytes.len() {
        if bytes[j] == b'\r' {
            if seg_start < j {
                out.push_str(&input[seg_start..j]);
            }
            if j + 1 < bytes.len() && bytes[j + 1] == b'\n' {
                out.push('\n');
                j += 2;
            } else {
                out.push('\n');
                j += 1;
            }
            seg_start = j;
        } else {
            j += 1;
        }
    }
    if seg_start < input.len() {
        out.push_str(&input[seg_start..]);
    }
    debug_assert!(!out.contains('\r'));
    NormalizedText {
        normalized: out,
        original,
        had_trailing_newline,
        mixed,
    }
}

// InsertRun moved to undo module

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

/// Ephemeral status message container (Phase 2 Step 6).
#[derive(Debug, Clone)]
pub struct EphemeralMessage {
    pub text: String,
    pub expires_at: std::time::Instant,
}

impl EditorState {
    /// Create a new state with a single active buffer.
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffers: vec![buffer],
            active: 0,
            last_text_height: 0,
            mode: Mode::Normal,
            file_name: None,
            dirty: false,
            undo: UndoEngine::new(),
            command_line: CommandLineState::default(),
            ephemeral_status: None,
            original_line_ending: LineEnding::Lf,
            had_trailing_newline: false,
            config_vertical_margin: 0,
        }
    }

    /// Set an ephemeral status message with a fixed timeout duration.
    pub fn set_ephemeral<S: Into<String>>(&mut self, msg: S, ttl: std::time::Duration) {
        self.ephemeral_status = Some(EphemeralMessage {
            text: msg.into(),
            expires_at: std::time::Instant::now() + ttl,
        });
    }

    /// Tick ephemeral status; returns true if message expired and was cleared.
    pub fn tick_ephemeral(&mut self) -> bool {
        if let Some(m) = &self.ephemeral_status
            && std::time::Instant::now() >= m.expires_at
        {
            self.ephemeral_status = None;
            return true;
        }
        false
    }

    /// Auto-scroll to keep cursor within the visible vertical viewport.
    ///
    /// `text_height` is the number of text rows (excludes status line). If zero, no-op.
    /// (Legacy note) Previously adjusted a per-state `viewport_first_line`; this responsibility
    /// now lives in `core-model::View` and the state no longer tracks a viewport origin.
    ///
    /// Returns true if the first line changed.
    // Auto-scroll logic moved to `core-model::View::auto_scroll` (Phase 3 Step 3.2).
    /// Test-only helper (and future external hook) to set cached viewport height explicitly.
    pub fn set_last_text_height(&mut self, h: usize) {
        self.last_text_height = h;
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
    pub fn push_snapshot(&mut self, kind: SnapshotKind, cursor: Position) {
        // Avoid double-borrow of &self by cloning buffer ref first
        let mode = self.mode;
        let buf_clone = self.active_buffer().clone();
        self.undo.push_snapshot(kind, cursor, &buf_clone, mode);
        // Replace active buffer clone to maintain identical behavior (original implementation cloned internally)
        self.buffers[self.active] = buf_clone;
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
    pub fn begin_insert_coalescing(&mut self, cursor: Position) {
        let mode = self.mode;
        let buf_clone = self.active_buffer().clone();
        self.undo.begin_insert_coalescing(cursor, &buf_clone, mode);
        self.buffers[self.active] = buf_clone;
    }

    /// Ends the current Insert-mode coalescing run. Boundary triggers:
    /// * Leaving Insert mode (Esc)
    /// * Inserting a newline (treated as a boundary in 5b)
    ///   End the current Insert run (called on Esc or newline).
    ///
    /// The next Insert mutation will start a new run and thus push a new snapshot via
    /// `begin_insert_coalescing`.
    pub fn end_insert_coalescing(&mut self) {
        self.undo.end_insert_coalescing();
    }

    /// Push a discrete edit snapshot (used for Normal mode edits like `x` or
    /// other non-coalesced operations). Always pushes regardless of coalescing flag.
    /// Push a discrete (non-coalesced) edit snapshot. Used for Normal mode edits like `x` where
    /// each action should undo independently. Ignores any Insert coalescing state.
    pub fn push_discrete_edit_snapshot(&mut self, cursor: Position) {
        self.push_snapshot(SnapshotKind::Edit, cursor);
    }

    /// Increment the edit counter for an active insert run (used for diagnostics / future heuristics).
    pub fn note_insert_edit(&mut self) {
        self.undo.note_insert_edit();
    }
    /// Restore previously captured snapshot (caller ensures existence). Returns true if restored.
    pub fn undo(&mut self, cursor: &mut Position) -> bool {
        let buffer = &mut self.buffers[self.active];
        self.undo.undo(cursor, buffer, &mut self.mode)
    }

    /// Redo previously undone snapshot. Returns true if applied.
    pub fn redo(&mut self, cursor: &mut Position) -> bool {
        let buffer = &mut self.buffers[self.active];
        self.undo.redo(cursor, buffer, &mut self.mode)
    }

    /// Number of successive identical snapshots skipped (Phase 3 Step 11).
    pub fn undo_snapshots_skipped(&self) -> u64 {
        self.undo.snapshots_skipped()
    }

    // Test/metrics helpers
    pub fn undo_depth(&self) -> usize {
        self.undo.undo_depth()
    }
    pub fn redo_depth(&self) -> usize {
        self.undo.redo_depth()
    }
    pub fn insert_run(&self) -> &InsertRun {
        self.undo.insert_run()
    }
}

// Compute a stable hash for the entire buffer content (Phase 3 Step 11).
// Simplicity-first implementation concatenates all lines (including newline characters)
// into the hasher. Future phases may adopt incremental diffing or rolling hashes.
// buffer_hash moved to undo module
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
        let cursor = Position { line: 0, byte: 0 };
        assert_eq!(cursor.line, 0);
        assert_eq!(cursor.byte, 0);
        assert!(matches!(st.mode, Mode::Normal));
    }

    #[test]
    fn cursor_clamp() {
        let buf = Buffer::from_str("test", "Hello\nWorld").unwrap();
        let st = EditorState::new(buf);
        let mut pos = Position {
            line: 10,
            byte: 999,
        }; // beyond
        let line_count = st.active_buffer().line_count();
        let last_len = st.active_buffer().line_byte_len(line_count - 1);
        // Provide a closure that does not borrow `st` to satisfy borrow checker.
        pos.clamp_to(line_count, |_| last_len);
        assert_eq!(pos.line, line_count - 1); // last valid line index
        assert_eq!(pos.byte, last_len);
    }

    #[test]
    fn snapshot_push_and_undo_redo() {
        let buf = Buffer::from_str("t", "one").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position { line: 0, byte: 0 };
        st.push_snapshot(SnapshotKind::Edit, cursor);
        // mutate buffer by inserting (simulate via direct buffer clone replace for now)
        {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, "X");
            st.buffers[st.active] = modified;
        }
        st.push_snapshot(SnapshotKind::Edit, cursor);
        assert!(st.undo(&mut cursor));
        // After undo we can redo
        assert!(st.redo(&mut cursor));
    }

    #[test]
    fn snapshot_dedupe_skips_identical() {
        let buf = Buffer::from_str("t", "abc").unwrap();
        let mut st = EditorState::new(buf);
        let cursor = Position { line: 0, byte: 0 };
        st.push_snapshot(SnapshotKind::Edit, cursor);
        let before = st.undo_depth();
        st.push_snapshot(SnapshotKind::Edit, cursor); // identical state -> skip
        assert_eq!(
            st.undo_depth(),
            before,
            "duplicate snapshot was not skipped"
        );
        assert_eq!(
            st.undo_snapshots_skipped(),
            1,
            "skip metric not incremented"
        );
    }

    #[test]
    fn snapshot_dedupe_allows_changed() {
        let buf = Buffer::from_str("t", "a").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position { line: 0, byte: 0 };
        st.push_snapshot(SnapshotKind::Edit, cursor);
        // mutate buffer
        {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, "b");
            st.buffers[st.active] = modified;
        }
        st.push_snapshot(SnapshotKind::Edit, cursor); // should NOT skip
        assert_eq!(st.undo_depth(), 2, "changed snapshot unexpectedly skipped");
        assert_eq!(
            st.undo_snapshots_skipped(),
            0,
            "skip metric incremented incorrectly"
        );
    }

    #[test]
    fn coalescing_run_only_pushes_once() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position { line: 0, byte: 0 };
        st.mode = Mode::Insert;
        st.begin_insert_coalescing(cursor); // should push snapshot
        let len_after_first = st.undo_depth();
        assert_eq!(len_after_first, 1);
        assert!(matches!(st.insert_run(), InsertRun::Active { .. }));

        // Simulate multiple inserts inside same run (no further snapshots expected)
        for ch in ["a", "b", "c"] {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, ch);
            st.buffers[st.active] = modified;
            st.begin_insert_coalescing(cursor); // no-op after first
            st.note_insert_edit();
        }
        assert_eq!(st.undo_depth(), 1, "coalescing inserted multiple snapshots");

        // End run and start new one -> pushes again
        st.end_insert_coalescing();
        st.begin_insert_coalescing(cursor);
        st.note_insert_edit();
        assert_eq!(st.undo_depth(), 2, "second run did not create new snapshot");
        if let InsertRun::Active { edits, .. } = st.insert_run() {
            assert_eq!(*edits, 1);
        }
    }

    #[test]
    fn redo_cleared_after_new_coalesced_edit() {
        let buf = Buffer::from_str("t", "Hello").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position { line: 0, byte: 0 };
        st.push_snapshot(SnapshotKind::Edit, cursor); // baseline snapshot
        // Apply an edit by direct mutation and push snapshot to simulate earlier approach
        {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, "!");
            st.buffers[st.active] = modified;
        }
        st.push_snapshot(SnapshotKind::Edit, cursor);
        assert!(st.undo(&mut cursor)); // creates one entry in redo
        assert_eq!(st.redo_depth(), 1);

        // New coalesced run should clear redo stack
        st.mode = Mode::Insert;
        st.begin_insert_coalescing(cursor);
        assert_eq!(st.redo_depth(), 0, "redo stack not cleared on new edit");
    }

    #[test]
    fn undo_stack_capped() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position { line: 0, byte: 0 };
        // Push more than max
        for i in 0..(UNDO_HISTORY_MAX + 10) {
            // Apply a trivial mutation to differentiate snapshots
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, "x");
            st.buffers[st.active] = modified;
            st.push_snapshot(SnapshotKind::Edit, cursor);
            // ensure length never exceeds cap + 1 transiently (since we remove after push)
            assert!(st.undo_depth() <= UNDO_HISTORY_MAX);
            // Reset to allow next iteration to mutate from previous base (simplistic)
            let _ = i; // silence unused warning if any
        }
        assert_eq!(st.undo_depth(), UNDO_HISTORY_MAX);
    }

    #[test]
    fn coalesced_insert_run_undo_redo() {
        // Simulate minimal insert mode run: enter Insert, type 'a','b','c', Esc boundary.
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position { line: 0, byte: 0 };
        st.mode = Mode::Insert;
        // First insert triggers snapshot
        st.begin_insert_coalescing(cursor);
        {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, "a");
            st.buffers[st.active] = modified;
        }
        // Additional inserts in same run (should not push extra snapshots)
        for ch in ["b", "c"] {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, ch);
            st.buffers[st.active] = modified;
            st.begin_insert_coalescing(cursor); // no-op while active
            st.note_insert_edit();
        }
        if let InsertRun::Active { edits, .. } = st.insert_run() {
            assert_eq!(*edits, 2);
        }
        // End run via Esc boundary semantics
        st.end_insert_coalescing();
        // Simulate Esc leave insert for refined undo semantics
        st.mode = Mode::Normal;
        assert_eq!(st.undo_depth(), 1, "expected single snapshot for run");
        // Perform undo: buffer should become empty again
        assert!(st.undo(&mut cursor));
        assert_eq!(st.active_buffer().line(0).unwrap_or_default(), "");
        assert!(
            matches!(st.mode, Mode::Normal),
            "mode restored unexpectedly to Insert"
        );
        // Redo should restore 'abc'
        assert!(st.redo(&mut cursor));
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
        let mut cursor = Position { line: 0, byte: 0 };
        st.mode = Mode::Insert;
        st.begin_insert_coalescing(cursor);
        for ch in ["a", "b"] {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, ch);
            st.buffers[st.active] = modified;
            st.begin_insert_coalescing(cursor);
        }
        st.end_insert_coalescing();
        st.mode = Mode::Normal; // simulate Esc
        assert!(st.undo(&mut cursor));
        assert!(matches!(st.mode, Mode::Normal));
        assert!(st.redo(&mut cursor));
        assert!(matches!(st.mode, Mode::Normal));
    }

    #[test]
    fn newline_is_coalescing_boundary() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position { line: 0, byte: 0 };
        st.mode = Mode::Insert;
        // Insert 'a' then newline then 'b'; expect two snapshots (one for first run before 'a', one for second run after newline)
        st.begin_insert_coalescing(cursor);
        {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, "a");
            st.buffers[st.active] = modified;
        }
        // Newline ends run
        {
            let mut modified = st.active_buffer().clone();
            modified.insert_newline(&mut cursor);
            st.buffers[st.active] = modified;
        }
        st.end_insert_coalescing();
        // Start second run after boundary
        st.begin_insert_coalescing(cursor);
        {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, "b");
            st.buffers[st.active] = modified;
        }
        assert_eq!(
            st.undo_depth(),
            2,
            "expected two snapshots across newline boundary"
        );
    }

    #[test]
    fn backspace_stays_in_run() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position { line: 0, byte: 0 };
        st.mode = Mode::Insert;
        st.begin_insert_coalescing(cursor);
        // Insert two characters
        for ch in ["a", "b"] {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, ch);
            st.buffers[st.active] = modified;
            st.begin_insert_coalescing(cursor);
            st.note_insert_edit();
        }
        // Backspace one character (should not end run or create new snapshot)
        {
            let mut modified = st.active_buffer().clone();
            modified.delete_grapheme_before(&mut cursor);
            st.buffers[st.active] = modified;
            st.begin_insert_coalescing(cursor); // still active
            st.note_insert_edit();
        }
        assert_eq!(st.undo_depth(), 1, "backspace created unexpected snapshot");
        if let InsertRun::Active { edits, .. } = st.insert_run() {
            assert_eq!(*edits, 3);
        }
        // End run and undo should revert to empty buffer
        st.end_insert_coalescing();
        st.mode = Mode::Normal;
        assert!(st.undo(&mut cursor));
        assert_eq!(st.active_buffer().line(0).unwrap_or_default(), "");
    }

    #[test]
    fn insert_run_newline_resets_counter() {
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position { line: 0, byte: 0 };
        st.mode = Mode::Insert;
        st.begin_insert_coalescing(cursor);
        {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, "a");
            st.buffers[st.active] = modified;
        }
        st.note_insert_edit();
        st.end_insert_coalescing();
        st.begin_insert_coalescing(cursor);
        {
            let mut modified = st.active_buffer().clone();
            modified.insert_grapheme(&mut cursor, "b");
            st.buffers[st.active] = modified;
        }
        st.note_insert_edit();
        if let InsertRun::Active { edits, .. } = st.insert_run() {
            assert_eq!(*edits, 1);
        }
    }

    #[test]
    fn ephemeral_lifecycle() {
        let buf = Buffer::from_str("t", "Hello").unwrap();
        let mut st = EditorState::new(buf);
        st.set_ephemeral("Message", std::time::Duration::from_millis(50));
        assert!(st.ephemeral_status.is_some());
        // Should not expire immediately
        assert!(!st.tick_ephemeral());
        // Fast-forward by manually adjusting expires_at (avoid sleeping in test)
        if let Some(m) = &mut st.ephemeral_status {
            m.expires_at = std::time::Instant::now() - std::time::Duration::from_millis(1);
        }
        assert!(st.tick_ephemeral(), "expected expiration");
        assert!(st.ephemeral_status.is_none());
    }

    #[test]
    fn normalize_crlf() {
        let src = "a\r\nb\r\n"; // ends with newline
        let n = normalize_line_endings(src);
        assert_eq!(n.normalized, "a\nb\n");
        assert_eq!(n.original, LineEnding::Crlf);
        assert!(n.had_trailing_newline);
        assert!(!n.mixed);
    }

    #[test]
    fn normalize_cr() {
        let src = "a\rb\r";
        let n = normalize_line_endings(src);
        assert_eq!(n.normalized, "a\nb\n");
        assert_eq!(n.original, LineEnding::Cr);
        assert!(n.had_trailing_newline);
        assert!(!n.mixed);
    }

    #[test]
    fn normalize_mixed_majority() {
        let src = "a\r\nb\nc\r\n"; // 2x CRLF, 1x LF
        let n = normalize_line_endings(src);
        assert_eq!(n.normalized, "a\nb\nc\n");
        assert_eq!(n.original, LineEnding::Crlf);
        assert!(n.mixed);
    }

    #[test]
    fn normalize_trailing_newline_absent() {
        let src = "a\r\nb"; // no final newline
        let n = normalize_line_endings(src);
        assert_eq!(n.normalized, "a\nb");
        assert!(!n.had_trailing_newline);
    }
    #[test]
    fn normalize_unicode_crlf_preserves_multibyte() {
        let src = "⚙️ Gear\r\nNext\r\n"; // multi-byte emoji + VS16 + ASCII + CRLF
        let n = normalize_line_endings(src);
        assert_eq!(n.normalized, "⚙️ Gear\nNext\n");
        assert!(n.normalized.starts_with("⚙️"));
    }
    #[test]
    fn normalize_unicode_mixed_endings() {
        let src = "α\r\nβ\nγ\r\n"; // Greek letters mixed CRLF + LF + CRLF
        let n = normalize_line_endings(src);
        assert_eq!(n.normalized, "α\nβ\nγ\n");
        assert!(n.mixed);
    }
    #[test]
    fn normalize_unicode_cr_only() {
        let src = "😀\r😀\r"; // CR only separators
        let n = normalize_line_endings(src);
        assert_eq!(n.normalized, "😀\n😀\n");
        assert_eq!(n.original, LineEnding::Cr);
    }
    #[test]
    fn normalize_round_trip_idempotent() {
        let samples = [
            "simple\nline",      // LF only
            "line\r\nline2\r\n", // CRLF only
            "a\rb\r",            // CR only
            "α\r\nβ\nγ\r\n",     // mixed majority CRLF
            "⚙️ Gear\r\nNext",   // unicode + CRLF no final newline
        ];
        for s in samples.iter() {
            let n1 = normalize_line_endings(s);
            // Rebuild using metadata.
            let mut rebuilt = String::new();
            let le = n1.original.as_str();
            let collected: Vec<&str> = n1.normalized.split('\n').collect();
            for (idx, line) in collected.iter().enumerate() {
                if idx + 1 == collected.len() && line.is_empty() {
                    break;
                } // final empty from trailing \n
                rebuilt.push_str(line);
                if idx + 1 < collected.len() || n1.had_trailing_newline {
                    rebuilt.push_str(le);
                }
            }
            let n2 = normalize_line_endings(&rebuilt);
            assert_eq!(
                n1.normalized, n2.normalized,
                "idempotence failed for sample: {s}"
            );
            assert!(!n2.normalized.contains('\r'));
        }
    }
}
