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

// Refactor R4 Step 2: Selection model scaffold
// Minimal persistent selection representation (visual mode placeholder).
// Stored in EditorState; initially unused by dispatcher/render paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Characterwise,
    Linewise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionSpan {
    pub start: Position,
    pub end: Position,
    pub kind: SelectionKind,
}

impl SelectionSpan {
    /// Construct a new span normalizing ordering so that start <= end (line, then byte).
    pub fn new(mut a: Position, mut b: Position, kind: SelectionKind) -> Self {
        if Self::greater(&a, &b) {
            std::mem::swap(&mut a, &mut b);
        }
        Self {
            start: a,
            end: b,
            kind,
        }
    }
    /// Construct a span preserving the supplied ordering (used when a persistent anchor
    /// must remain the `start` even if it sorts after the cursor). Caller guarantees invariants.
    pub fn anchored(anchor: Position, other: Position, kind: SelectionKind) -> Self {
        Self {
            start: anchor,
            end: other,
            kind,
        }
    }
    fn greater(a: &Position, b: &Position) -> bool {
        a.line > b.line || (a.line == b.line && a.byte > b.byte)
    }
    /// Returns true if span is empty (start == end).
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Compute an inclusive absolute byte range for this span within the active buffer.
    ///
    /// Characterwise selections in Visual mode conceptually include BOTH endpoint
    /// graphemes. The internal `SelectionSpan` stores a normalized half-open
    /// range `[start,end)` (cursor motions continually rebuild using `new` which
    /// sorts endpoints). For operator application we must expand a non-empty
    /// Characterwise selection so that the *last* grapheme boundary is included.
    ///
    /// Linewise selections are already naturally inclusive via whole-line copy
    /// semantics and remain unchanged.
    ///
    /// Returns `(abs_start, abs_end_exclusive)` suitable for deletion, yanking,
    /// or change operators. Empty selections always return identical indices.
    pub fn inclusive_byte_range(&self, buffer: &core_text::Buffer) -> (usize, usize) {
        // Map a position (line, byte) to absolute byte index accounting for newlines.
        let to_abs = |pos: Position| {
            let mut total = 0usize;
            for line in 0..pos.line {
                total += buffer.line_byte_len(line);
                if let Some(l) = buffer.line(line)
                    && l.ends_with('\n')
                {
                    total += 1;
                }
            }
            total + pos.byte
        };
        let mut a = to_abs(self.start);
        let mut b = to_abs(self.end);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        if a == b {
            return (a, b);
        }
        match self.kind {
            SelectionKind::Characterwise => {
                // Expand to include grapheme at logical end. We treat stored end
                // as *inclusive* visually; internal representation is half-open.
                // Determine absolute end of last grapheme starting at b-1 .. b.
                // Simplicity: scan the line containing end to find next boundary.
                let end_pos = self.end; // normalized
                if let Some(line) = buffer.line(end_pos.line) {
                    let line_str = line.as_str();
                    let trimmed = line_str.strip_suffix('\n').unwrap_or(line_str);
                    // Clamp byte within trimmed
                    let clamped = end_pos.byte.min(trimmed.len());
                    // If end is at line start and selection not empty, ensure we still include
                    // that grapheme by extending to its next boundary.
                    let next = core_text::grapheme::next_boundary(trimmed, clamped);
                    let line_prefix_abs = to_abs(Position {
                        line: end_pos.line,
                        byte: 0,
                    });
                    let expanded = line_prefix_abs + next;
                    if expanded > b {
                        b = expanded;
                    }
                }
                (a, b)
            }
            SelectionKind::Linewise => (a, b),
        }
    }
}

/// Persistent (yet optionally empty) selection model.
///
/// Refactor R4 Step 2 introduced a durable representation for a single active
/// selection span even though visual mode operations are deferred to a later
/// phase. Keeping an always-present model (instead of ad-hoc locals inside
/// operators) prevents future churn when visual highlighting and text object
/// motions arrive. The model is intentionally minimal: a single optional span.
/// Multi-span / blockwise expansions can layer on top by evolving this struct
/// (API breakage is acceptable under the project's evolution policy).
///
/// Invariants:
/// - If `active` is `Some(span)` then `span.start <= span.end` (enforced by
///   `SelectionSpan::new`).
/// - Empty selections (start==end) are permitted and treated as inactive for
///   highlighting purposes by future render logic.
#[derive(Debug, Default, Clone)]
pub struct SelectionModel {
    /// The currently active selection; None when no user selection exists.
    pub active: Option<SelectionSpan>,
    /// Persistent anchor position (where Visual mode was entered). Remains fixed until selection cleared.
    pub anchor: Option<Position>,
}

impl SelectionModel {
    pub fn clear(&mut self) {
        self.active = None;
        self.anchor = None;
    }
    pub fn set(&mut self, span: SelectionSpan) {
        self.active = Some(span);
    }
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }
}

// Registers & Operator Metrics (Phase 4 Steps 3,6–9):
// - Registers are now fully populated by delete/yank/change operators; unnamed
//   always mirrors the latest textual payload; numbered ring rotates with fixed
//   capacity (10) discarding oldest on overflow.
// - Operator metrics track counts & register writes enabling `:metrics` surface
//   correlation between editing patterns and repaint pipeline cost.
// - Paste & explicit register selection remain deferred (Phase 5) preserving
//   breadth-first progress while ensuring current operator semantics are testable.
#[derive(Debug, Default, Clone)]
pub struct Registers {
    pub unnamed: String,
    numbered: Vec<String>, // newest at index 0, length <= 10
    // Phase 5 Step 5: Named registers (a-z). Uppercase variants (A-Z) append.
    // Simpler than full Vim semantics (linewise nuances) for breadth-first path.
    named: [String; 26],
}

// Phase 4 Step 9: Operator & register metrics counters
// Breadth-first: simple non-atomic u64 fields mutated on dispatcher thread only.
// Future async multi-producer (multi-view) scenario can upgrade to atomics.
#[derive(Debug, Default, Clone, Copy)]
pub struct OperatorMetricsSnapshot {
    pub operator_delete: u64,
    pub operator_yank: u64,
    pub operator_change: u64,
    pub register_writes: u64,
    pub numbered_ring_rotations: u64,
}

// ---------------------------- Phase 5 Step 5 (Register/Paste Abstraction Scaffold) ----------------------------
/// Source of paste content (future implementation). For Step 5 we expose the enum so dispatcher & future
/// commands can select a source without yet performing modifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteSource {
    Unnamed,
    /// Numbered ring entry (0 newest). Bounds checked at call site.
    Numbered(usize),
    /// Named register (a–z, A–Z) – not yet populated (future macro/explicit yank targets).
    Named(char),
    /// System clipboard integration placeholder.
    System,
}

/// Paste operation error kinds (stub; enriched later once implementation lands).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteError {
    Unimplemented,
    OutOfRange,
    Empty,
}

#[derive(Debug, Default, Clone)]
pub struct OperatorMetrics {
    operator_delete: u64,
    operator_yank: u64,
    operator_change: u64,
    register_writes: u64,
    numbered_ring_rotations: u64,
}

impl OperatorMetrics {
    pub fn snapshot(&self) -> OperatorMetricsSnapshot {
        OperatorMetricsSnapshot {
            operator_delete: self.operator_delete,
            operator_yank: self.operator_yank,
            operator_change: self.operator_change,
            register_writes: self.register_writes,
            numbered_ring_rotations: self.numbered_ring_rotations,
        }
    }
    pub fn incr_delete(&mut self) {
        self.operator_delete += 1;
    }
    pub fn incr_yank(&mut self) {
        self.operator_yank += 1;
    }
    pub fn incr_change(&mut self) {
        self.operator_change += 1;
    }
    pub fn note_register_write(&mut self, rotated: bool) {
        self.register_writes += 1;
        if rotated {
            self.numbered_ring_rotations += 1;
        }
    }
}

/// Facade encapsulating register mutations and metrics updates.
///
/// Step 6 objective: concentrate register semantics so callers no longer reach into
/// `EditorState` for ad-hoc mutations. The facade accepts an optional register target
/// (alphabetic for named slots; `None` routes to unnamed + numbered ring) and applies
/// the correct write semantics while incrementing operator metrics.
pub struct RegistersFacade<'state> {
    registers: &'state mut Registers,
    metrics: &'state mut OperatorMetrics,
}

impl<'state> RegistersFacade<'state> {
    pub fn new(registers: &'state mut Registers, metrics: &'state mut OperatorMetrics) -> Self {
        Self { registers, metrics }
    }

    /// Record delete payload. Named targets honor uppercase append semantics.
    pub fn write_delete<S: Into<String>>(&mut self, payload: S, target: Option<char>) {
        self.metrics.incr_delete();
        let text = payload.into();
        if let Some(named) = target.filter(|c| c.is_ascii_alphabetic()) {
            self.registers
                .record_delete_named(named, text, self.metrics);
        } else {
            self.registers.record_delete(text, self.metrics);
        }
    }

    /// Record yank payload. Named targets honor uppercase append semantics.
    pub fn write_yank<S: Into<String>>(&mut self, payload: S, target: Option<char>) {
        self.metrics.incr_yank();
        let text = payload.into();
        if let Some(named) = target.filter(|c| c.is_ascii_alphabetic()) {
            self.registers.record_yank_named(named, text, self.metrics);
        } else {
            self.registers.record_yank(text, self.metrics);
        }
    }

    /// Record change payload (treated as delete for register semantics with a distinct metric).
    pub fn write_change<S: Into<String>>(&mut self, payload: S, target: Option<char>) {
        self.metrics.incr_change();
        let text = payload.into();
        if let Some(named) = target.filter(|c| c.is_ascii_alphabetic()) {
            self.registers
                .record_delete_named(named, text, self.metrics);
        } else {
            self.registers.record_delete(text, self.metrics);
        }
    }

    /// Retrieve paste payload for the given source (clone-on-read).
    pub fn read_paste(&self, source: PasteSource) -> Result<String, PasteError> {
        let registers: &Registers = &*self.registers;
        match source {
            PasteSource::Unnamed => {
                if registers.unnamed.is_empty() {
                    Err(PasteError::Empty)
                } else {
                    Ok(registers.unnamed.clone())
                }
            }
            PasteSource::Numbered(idx) => {
                if idx >= registers.numbered.len() {
                    return Err(PasteError::OutOfRange);
                }
                let entry = &registers.numbered[idx];
                if entry.is_empty() {
                    Err(PasteError::Empty)
                } else {
                    Ok(entry.clone())
                }
            }
            PasteSource::Named(c) => {
                let slot = c.to_ascii_lowercase();
                if !slot.is_ascii_lowercase() {
                    return Err(PasteError::OutOfRange);
                }
                let idx = (slot as u8 - b'a') as usize;
                let entry = &registers.named[idx];
                if entry.is_empty() {
                    Err(PasteError::Empty)
                } else {
                    Ok(entry.clone())
                }
            }
            PasteSource::System => Err(PasteError::Unimplemented),
        }
    }
}

impl Registers {
    pub const MAX: usize = 10; // ring capacity

    pub fn new() -> Self {
        Self {
            unnamed: String::new(),
            numbered: Vec::new(),
            named: std::array::from_fn(|_| String::new()),
        }
    }

    /// Push a yank (non-destructive copy). Mirrors into unnamed and ring[0].
    pub fn record_yank<S: Into<String>>(&mut self, text: S, metrics: &mut OperatorMetrics) {
        let s = text.into();
        self.unnamed = s.clone();
        let rotated = self.unshift_numbered(s);
        metrics.note_register_write(rotated);
    }

    /// Push a delete/change (destructive). Semantics identical for ring/unnamed at this stage.
    pub fn record_delete<S: Into<String>>(&mut self, text: S, metrics: &mut OperatorMetrics) {
        let s = text.into();
        self.unnamed = s.clone();
        let rotated = self.unshift_numbered(s);
        metrics.note_register_write(rotated);
    }

    /// Return immutable slice of numbered ring (newest first).
    pub fn numbered(&self) -> &[String] {
        &self.numbered
    }

    fn unshift_numbered(&mut self, s: String) -> bool {
        let rotated = self.numbered.len() == Self::MAX;
        if rotated {
            self.numbered.pop();
        }
        self.numbered.insert(0, s);
        rotated
    }

    // --- Phase 5 Step 5: Named register helpers ---
    fn named_index(c: char) -> Option<usize> {
        if c.is_ascii_alphabetic() {
            Some((c.to_ascii_lowercase() as u8 - b'a') as usize)
        } else {
            None
        }
    }

    /// Get named register content (lower/uppercase treated identically for lookup).
    pub fn get_named(&self, c: char) -> Option<&str> {
        Self::named_index(c).map(|i| self.named[i].as_str())
    }

    /// Snapshot non-empty named registers (a-z) for future :reg command.
    pub fn named_snapshot(&self) -> Vec<(char, &str)> {
        self.named
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                if s.is_empty() {
                    None
                } else {
                    Some(((b'a' + i as u8) as char, s.as_str()))
                }
            })
            .collect()
    }

    /// Record yank into named register `c` (lowercase replace, uppercase append). Updates
    /// unnamed + numbered ring identically to unnamed-only yanks (breadth-first simplification).
    pub fn record_yank_named<S: Into<String>>(
        &mut self,
        c: char,
        text: S,
        metrics: &mut OperatorMetrics,
    ) {
        if let Some(idx) = Self::named_index(c) {
            let mut payload = text.into();
            let append = c.is_ascii_uppercase();
            if append && !self.named[idx].is_empty() {
                self.named[idx].push_str(&payload);
                payload = self.named[idx].clone(); // full updated payload for unnamed/ring
            } else {
                self.named[idx] = payload.clone();
            }
            // Mirror to unnamed + ring
            self.unnamed = payload.clone();
            let rotated = self.unshift_numbered(payload);
            metrics.note_register_write(rotated);
        }
    }

    /// Record delete/change into named register `c` (same semantics as yank for now).
    pub fn record_delete_named<S: Into<String>>(
        &mut self,
        c: char,
        text: S,
        metrics: &mut OperatorMetrics,
    ) {
        self.record_yank_named(c, text, metrics);
    }
}

// (Undo snapshot types moved to undo module)

/// Current editor mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Normal command/navigation mode.
    Normal,
    /// Insert text mode (appends / inserts grapheme clusters into the active buffer).
    Insert,
    /// Visual character-wise selection mode (Phase 5 Step 2).
    VisualChar,
}

// Refactor R4 Step 13 (Metrics Overlay Scaffold)
// OverlayMode controls optional diagnostic overlay rows rendered above the status
// line. We begin with a fixed line allocation (breadth-first) to avoid destabilizing
// partial diff invariants; dynamic sizing & wrapping land in a follow-up step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayMode {
    None,
    Metrics { lines: u16 }, // always dirty; unconditional repaint each frame
}

impl Default for OverlayMode {
    fn default() -> Self {
        Self::None
    }
}

/// Default fixed line allocation for metrics overlay (follow-up will compute dynamically).
pub const METRICS_OVERLAY_DEFAULT_LINES: u16 = 2;

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
    pub registers: Registers, // Phase 4: populated by yank/delete/change
    pub operator_metrics: OperatorMetrics, // Phase 4: operator + register counters
    // Phase 4 Step 15: last render/scheduler metrics snapshots captured post-render.
    // To avoid a circular dependency (`core-render` depends on `core-state`), we store
    // lightweight copies of the snapshot data instead of the original types.
    pub last_render_path: Option<RenderPathSnapshotLite>,
    pub last_render_delta: Option<RenderDeltaSnapshotLite>,
    // Refactor R4 Step 2: persistent selection model scaffold (visual mode placeholder)
    pub selection: SelectionModel,
    // Refactor R4 Step 13: optional overlay (metrics) configuration
    pub overlay_mode: OverlayMode,
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

// Lightweight, non-atomic copies of render path metrics (subset mirror of
// `core_render::partial_metrics::RenderPathMetricsSnapshot`). Keeping this here
// avoids a circular dependency while letting higher layers (commands) expose
// snapshot data without reaching into the renderer crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderPathSnapshotLite {
    pub full_frames: u64,
    pub partial_frames: u64,
    pub cursor_only_frames: u64,
    pub lines_frames: u64,
    pub escalated_large_set: u64,
    pub resize_invalidations: u64,
    pub dirty_lines_marked: u64,
    pub dirty_candidate_lines: u64,
    pub dirty_lines_repainted: u64,
    pub last_full_render_ns: u64,
    pub last_partial_render_ns: u64,
    pub print_commands: u64,
    pub cells_printed: u64,
    pub scroll_region_shifts: u64,
    pub scroll_region_lines_saved: u64,
    pub scroll_shift_degraded_full: u64,
    pub trim_attempts: u64,
    pub trim_success: u64,
    pub cols_saved_total: u64,
    pub status_skipped: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderDeltaSnapshotLite {
    pub full: u64,
    pub lines: u64,
    pub scroll: u64,
    pub status_line: u64,
    pub cursor_only: u64,
    pub collapsed_scroll: u64,
    pub suppressed_scroll: u64,
    pub semantic_frames: u64,
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
            registers: Registers::new(),
            operator_metrics: OperatorMetrics::default(),
            last_render_path: None,  // Initialize last_render_path to None
            last_render_delta: None, // Initialize last_render_delta to None
            selection: SelectionModel::default(),
            overlay_mode: OverlayMode::default(),
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

    /// Delete a byte span `[start,end)` from the active buffer with an undo snapshot.
    /// Returns the removed text. The provided `cursor` represents the current cursor
    /// (start position of operator). After deletion, the cursor is clamped to the start
    /// of the removed region (matching typical Vim semantics for `d` operations).
    ///
    /// Breadth-first: This operation treats every span delete as a discrete edit snapshot.
    /// Future refinement (multi-operator coalescing) can add smarter grouping.
    pub fn delete_span_with_snapshot(
        &mut self,
        cursor: &mut Position,
        start: usize,
        end: usize,
    ) -> String {
        // Push snapshot of pre-delete state for undo. Use current cursor (before mutation).
        self.push_discrete_edit_snapshot(*cursor);
        let buf = self.active_buffer().clone(); // snapshot already cloned within push
        let mut working = buf.clone();
        let removed = working.delete_bytes(start, end);
        // Replace buffer
        self.buffers[self.active] = working;
        // Recompute cursor line/byte from absolute start (simple linear scan using public APIs).
        *cursor = absolute_position(self.active_buffer(), start);
        removed
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

    /// Mutable access to registers (operators populate these)
    pub fn registers_mut(&mut self) -> &mut Registers {
        &mut self.registers
    }

    /// Access operator/register metrics (mutable for instrumentation).
    pub fn operator_metrics_mut(&mut self) -> &mut OperatorMetrics {
        &mut self.operator_metrics
    }

    /// Snapshot current operator/register metrics.
    pub fn operator_metrics_snapshot(&self) -> OperatorMetricsSnapshot {
        self.operator_metrics.snapshot()
    }

    /// Borrow register operations + metrics as a cohesive facade.
    pub fn registers_facade(&mut self) -> RegistersFacade<'_> {
        RegistersFacade::new(&mut self.registers, &mut self.operator_metrics)
    }

    // --- Refactor R4 Step 2 helpers: selection model accessors ---
    pub fn selection(&self) -> Option<SelectionSpan> {
        self.selection.active
    }
    pub fn selection_mut(&mut self) -> &mut SelectionModel {
        &mut self.selection
    }
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    // ---------------- Phase 5 Step 5: Paste stub -----------------
    /// Paste API scaffold. Returns an error until paste is implemented in a later Phase 5 step.
    /// Parameters:
    /// - `view`: target view (cursor position determines insertion point)
    /// - `source`: which register source to pull from
    /// - `before`: if true, inserts before cursor (Normal-mode 'P' semantics), else after ('p').
    ///
    /// Behavior: For now does not mutate any buffer or register state; simply
    /// reports Unimplemented.
    pub fn paste_stub(&mut self, _source: PasteSource, _before: bool) -> Result<(), PasteError> {
        Err(PasteError::Unimplemented)
    }
    /// Perform a paste operation. Returns Ok(structural) where structural indicates multi-line insertion.
    /// Step 1: only supports PasteSource::Unnamed. Other sources return OutOfRange.
    /// Snapshot policy: single discrete snapshot before mutation (no coalescing).
    pub fn paste(
        &mut self,
        source: PasteSource,
        before: bool,
        cursor: &mut Position,
    ) -> Result<bool, PasteError> {
        if !matches!(self.mode, Mode::Normal) {
            return Err(PasteError::Unimplemented);
        }
        let text = self.registers_facade().read_paste(source)?;
        self.push_discrete_edit_snapshot(*cursor);
        let buffer = self.active_buffer_mut();
        let structural = text.contains('\n');
        // Fast path: single-line (charwise) paste
        if !structural {
            // Determine insertion point relative to current cursor grapheme.
            let mut insert_pos = *cursor;
            if !before {
                // After: advance to next grapheme boundary (Vim 'p')
                let line_len = buffer.line_byte_len(insert_pos.line);
                if insert_pos.byte < line_len
                    && let Some(line_owned) = buffer.line(insert_pos.line)
                {
                    let line_str = line_owned.as_str();
                    let trimmed = line_str.strip_suffix('\n').unwrap_or(line_str);
                    let next = core_text::grapheme::next_boundary(trimmed, insert_pos.byte);
                    insert_pos.byte = next.min(trimmed.len());
                }
            }
            // Insert grapheme-by-grapheme; final cursor should rest ON last inserted grapheme
            let mut idx = 0;
            let mut last_len = 0;
            while idx < text.len() {
                let next = core_text::grapheme::next_boundary(&text, idx);
                let g = &text[idx..next];
                buffer.insert_grapheme(&mut insert_pos, g);
                last_len = g.len();
                idx = next;
            }
            if last_len > 0 {
                insert_pos.byte = insert_pos.byte.saturating_sub(last_len);
            }
            *cursor = insert_pos;
            if !self.dirty {
                self.dirty = true;
            }
            return Ok(false);
        }

        // Multi-line path (retain tail extraction logic). Insert starting at cursor (before: same; after: move one grapheme first).
        let mut insert_pos = *cursor;
        if !before {
            let line_len = buffer.line_byte_len(insert_pos.line);
            if insert_pos.byte < line_len {
                let line_owned = buffer.line(insert_pos.line).unwrap();
                let line_str = line_owned.as_str();
                let trimmed = line_str.strip_suffix('\n').unwrap_or(line_str);
                let next = core_text::grapheme::next_boundary(trimmed, insert_pos.byte);
                insert_pos.byte = next.min(trimmed.len());
            }
        }
        let tail = {
            let line_len = buffer.line_byte_len(insert_pos.line);
            if insert_pos.byte < line_len {
                let line = buffer.line(insert_pos.line).unwrap_or_default();
                let trimmed = line.strip_suffix('\n').unwrap_or(&line);
                let suffix = trimmed[insert_pos.byte..].to_string();
                let mut abs = 0usize;
                for l in 0..insert_pos.line {
                    abs += buffer.line(l).unwrap().len();
                }
                let current_line = buffer.line(insert_pos.line).unwrap();
                let trimmed_current = current_line.strip_suffix('\n').unwrap_or(&current_line);
                let removal_start = abs + insert_pos.byte;
                let removal_end = abs + trimmed_current.len();
                buffer.delete_bytes(removal_start, removal_end);
                Some(suffix)
            } else {
                None
            }
        };
        // Insert multi-line content
        let mut last_insert_pos = insert_pos;
        for (i, segment) in text.split_inclusive('\n').enumerate() {
            if i > 0 {
                buffer.insert_newline(&mut last_insert_pos);
            }
            let frag = segment.strip_suffix('\n').unwrap_or(segment);
            if frag.is_empty() {
                continue;
            }
            let mut idx = 0;
            while idx < frag.len() {
                let next = core_text::grapheme::next_boundary(frag, idx);
                let g = &frag[idx..next];
                buffer.insert_grapheme(&mut last_insert_pos, g);
                idx = next;
            }
        }
        // Reattach tail
        if let Some(t) = tail
            && !t.is_empty()
        {
            let mut idx = 0;
            while idx < t.len() {
                let next = core_text::grapheme::next_boundary(&t, idx);
                let g = &t[idx..next];
                buffer.insert_grapheme(&mut last_insert_pos, g);
                idx = next;
            }
        }
        *cursor = last_insert_pos;
        if !self.dirty {
            self.dirty = true;
        }
        Ok(structural)
    }

    // ---------------- Overlay (Refactor R4 Step 13) ----------------
    /// Current overlay mode.
    pub fn overlay_mode(&self) -> OverlayMode {
        self.overlay_mode
    }
    /// Set overlay mode explicitly.
    pub fn set_overlay_mode(&mut self, mode: OverlayMode) {
        self.overlay_mode = mode;
    }
    /// Toggle metrics overlay. Returns the new mode.
    pub fn toggle_metrics_overlay(&mut self, default_lines: u16) -> OverlayMode {
        self.overlay_mode = match self.overlay_mode {
            OverlayMode::None => OverlayMode::Metrics {
                lines: default_lines,
            },
            OverlayMode::Metrics { .. } => OverlayMode::None,
        };
        self.overlay_mode
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
    fn paste_single_line_after() {
        let buf = Buffer::from_str("t", "abc\n").unwrap();
        let mut st = EditorState::new(buf);
        {
            let mut regs = st.registers_facade();
            regs.write_yank("X", None);
        }
        let mut cursor = Position { line: 0, byte: 1 }; // on 'b' boundary after 'a'
        let structural = st.paste(PasteSource::Unnamed, false, &mut cursor).unwrap();
        assert!(!structural);
        let line0 = st.active_buffer().line(0).unwrap();
        assert_eq!(line0, "abXc\n");
        assert_eq!(cursor.byte, 2, "cursor rests on inserted 'X'");
    }

    #[test]
    fn paste_single_line_before() {
        let buf = Buffer::from_str("t", "abc").unwrap();
        let mut st = EditorState::new(buf);
        {
            let mut regs = st.registers_facade();
            regs.write_yank("Z", None);
        }
        let mut cursor = Position { line: 0, byte: 2 }; // before 'c'
        let structural = st.paste(PasteSource::Unnamed, true, &mut cursor).unwrap();
        assert!(!structural);
        let line0 = st.active_buffer().line(0).unwrap();
        assert_eq!(line0, "abZc");
        assert_eq!(cursor.byte, 2, "cursor rests on inserted 'Z'");
    }

    #[test]
    fn delete_under_then_paste_after_swaps() {
        // Simulate: word "Ox" -> cursor at 'O', x then p should produce xO with cursor on O
        let buf = Buffer::from_str("t", "Oxidized\n").unwrap();
        let mut st = EditorState::new(buf);
        // Position at 'O'
        let mut cursor = Position { line: 0, byte: 0 };
        // DeleteUnder (x)
        st.push_discrete_edit_snapshot(cursor);
        {
            let mut regs = st.registers_facade();
            regs.write_delete("O".to_string(), None);
        }
        st.active_buffer_mut().delete_grapheme_at(&mut cursor); // removes 'O'; cursor stays at 0
        // Paste after (p semantics) should insert after current grapheme (which is now 'x')
        let structural = st.paste(PasteSource::Unnamed, false, &mut cursor).unwrap();
        assert!(!structural);
        let line0 = st.active_buffer().line(0).unwrap();
        assert_eq!(line0, "xOidized\n");
        // Cursor should rest on inserted 'O' (byte 1)
        assert_eq!(cursor.byte, 1);
    }

    #[test]
    fn delete_under_then_paste_before_restores() {
        let buf = Buffer::from_str("t", "Oxidized\n").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position { line: 0, byte: 0 };
        st.push_discrete_edit_snapshot(cursor);
        {
            let mut regs = st.registers_facade();
            regs.write_delete("O".to_string(), None);
        }
        st.active_buffer_mut().delete_grapheme_at(&mut cursor);
        // Paste before (P) inserts at cursor (before current 'x'), restoring original
        let structural = st.paste(PasteSource::Unnamed, true, &mut cursor).unwrap();
        assert!(!structural);
        let line0 = st.active_buffer().line(0).unwrap();
        assert_eq!(line0, "Oxidized\n");
        // Cursor should rest on inserted 'O' (byte 0)
        assert_eq!(cursor.byte, 0);
    }

    #[test]
    fn paste_multi_line_structural() {
        let buf = Buffer::from_str("t", "ab\ncd\n").unwrap();
        let mut st = EditorState::new(buf);
        {
            let mut regs = st.registers_facade();
            regs.write_yank("X\nY\n", None);
        }
        let mut cursor = Position { line: 0, byte: 1 }; // after 'a'
        let structural = st.paste(PasteSource::Unnamed, false, &mut cursor).unwrap();
        assert!(structural);
        // After semantics: advanced to end of first line, inserted multi-line payload
        let l0 = st.active_buffer().line(0).unwrap();
        let l1 = st.active_buffer().line(1).unwrap();
        let l2 = st.active_buffer().line(2).unwrap();
        assert_eq!(l0, "abX\n");
        assert_eq!(l1, "Y\n");
        assert_eq!(l2, "cd\n");
        // Cursor should be at end of last inserted line before original remainder merge (line index 2 start?) Here after attaching tail -> end of 'cd' line
        assert_eq!(cursor.line, 1);
    }

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

    // --- Selection model tests (Refactor R4 Step 2) ---
    #[test]
    fn selection_new_orders_positions() {
        let a = Position { line: 2, byte: 5 };
        let b = Position { line: 1, byte: 3 };
        let span = SelectionSpan::new(a, b, SelectionKind::Characterwise);
        assert!(span.start.line <= span.end.line);
        if span.start.line == span.end.line {
            assert!(span.start.byte <= span.end.byte);
        }
    }

    #[test]
    fn selection_empty_detection() {
        let p = Position { line: 0, byte: 0 };
        let span = SelectionSpan::new(p, p, SelectionKind::Characterwise);
        assert!(span.is_empty());
    }

    #[test]
    fn selection_model_default_empty() {
        let m = SelectionModel::default();
        assert!(!m.is_active());
        assert!(m.active.is_none());
    }

    #[test]
    fn selection_model_set_and_clear() {
        let mut m = SelectionModel::default();
        let a = Position { line: 0, byte: 0 };
        let b = Position { line: 0, byte: 5 };
        m.set(SelectionSpan::new(a, b, SelectionKind::Linewise));
        assert!(m.is_active());
        assert!(matches!(m.active.unwrap().kind, SelectionKind::Linewise));
        m.clear();
        assert!(!m.is_active());
    }
}

// Test module for span deletion API (Phase 4 Step 5)
#[cfg(test)]
mod span_delete_tests {
    use super::*;
    use core_text::{Buffer, Position};

    #[test]
    fn delete_span_single_line() {
        let buf = Buffer::from_str("t", "abcdef\n").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position::origin();
        // delete 'bcd' (bytes 1..4)
        let removed = st.delete_span_with_snapshot(&mut cursor, 1, 4);
        assert_eq!(removed, "bcd");
        let line = st.active_buffer().line(0).unwrap();
        assert!(line.starts_with("aef"), "line now: {line}");
        assert_eq!(cursor.line, 0);
        assert_eq!(cursor.byte, 1); // start of deleted span
        assert_eq!(st.undo_depth(), 1);
        assert!(st.undo(&mut cursor));
        let restored = st.active_buffer().line(0).unwrap();
        assert!(restored.starts_with("abcdef"));
    }

    #[test]
    fn delete_span_multi_line() {
        let buf = Buffer::from_str("t", "one\ntwo\nthree\n").unwrap();
        let mut st = EditorState::new(buf);
        let mut cursor = Position::origin();
        // Compute absolute bytes manually using slice_bytes helper after building resolver logic.
        // Lines:
        // one\n -> 4 bytes
        // two\n -> 4 bytes (cumulative 8)
        // three\n -> 6 bytes (cumulative 14)
        // Delete 'two\nthree\n' (bytes 4..14)
        let removed = st.delete_span_with_snapshot(&mut cursor, 4, 14);
        assert_eq!(removed, "two\nthree\n");
        let l0 = st.active_buffer().line(0).unwrap();
        assert_eq!(l0, "one\n");
        // After deletion we expect remaining text: "one\n" only.
        // Ropey will represent this as two lines: "one\n" and an empty last line (since original ended with newline).
        assert_eq!(st.active_buffer().line_count(), 2);
        // Cursor should land somewhere within remaining buffer (line 0 or 1) after deletion.
        assert!(cursor.line <= 1);
        assert!(st.undo(&mut cursor));
        // After undo we expect original prefix 'one\n' present.
        let restored_first = st.active_buffer().line(0).unwrap();
        assert!(restored_first.starts_with("one"));
    }
}

// Convert absolute byte index into Position using public Buffer APIs (linear scan).
fn absolute_position(buffer: &Buffer, abs: usize) -> Position {
    let mut remaining = abs;
    let mut line = 0usize;
    while line < buffer.line_count() {
        let line_len = buffer.line_byte_len(line);
        let has_newline = buffer
            .line(line)
            .map(|l| l.ends_with('\n'))
            .unwrap_or(false);
        if remaining <= line_len {
            return Position {
                line,
                byte: remaining,
            };
        }
        remaining -= line_len;
        if has_newline {
            if remaining == 0 {
                // exactly at newline boundary -> cursor stays at end of line
                return Position {
                    line,
                    byte: line_len,
                };
            }
            remaining -= 1; // consume newline byte
        }
        line += 1;
    }
    // Clamp to last line end
    if line == 0 {
        Position::origin()
    } else {
        let last = buffer.line_count().saturating_sub(1);
        let last_len = buffer.line_byte_len(last);
        Position {
            line: last,
            byte: last_len,
        }
    }
}

#[cfg(test)]
mod register_tests {
    use super::{OperatorMetrics, Registers};

    #[test]
    fn yank_populates_unnamed_and_ring() {
        let mut r = Registers::new();
        let mut m = OperatorMetrics::default();
        r.record_yank("alpha", &mut m);
        assert_eq!(r.unnamed, "alpha");
        assert_eq!(r.numbered(), &["alpha".to_string()]);
    }

    #[test]
    fn delete_rotates_ring_capped() {
        let mut r = Registers::new();
        let mut m = OperatorMetrics::default();
        for i in 0..12 {
            // exceed capacity intentionally
            r.record_delete(format!("d{i}"), &mut m);
        }
        assert_eq!(r.numbered().len(), Registers::MAX);
        // Newest at 0
        assert_eq!(r.numbered()[0], "d11");
        // Oldest retained should be d2 (d0,d1 dropped after overflow)
        assert_eq!(r.numbered().last().unwrap(), "d2");
        assert_eq!(r.unnamed, "d11");
    }

    #[test]
    fn interleave_yank_delete_ordering() {
        let mut r = Registers::new();
        let mut m = OperatorMetrics::default();
        r.record_yank("y1", &mut m);
        r.record_delete("d1", &mut m);
        r.record_yank("y2", &mut m);
        let ring: Vec<_> = r.numbered().iter().map(|s| s.as_str()).collect();
        assert_eq!(ring, vec!["y2", "d1", "y1"]);
        assert_eq!(r.unnamed, "y2");
    }

    // --- Additional invariant tests (Refactor R4 Step 12) ---

    #[test]
    fn mixed_overflow_sequence_invariants() {
        let mut r = Registers::new();
        let mut m = OperatorMetrics::default();
        // Sequence longer than capacity with interleaved yanks/deletes.
        for i in 0..(Registers::MAX + 5) {
            if i % 2 == 0 {
                r.record_yank(format!("y{i}"), &mut m);
            } else {
                r.record_delete(format!("d{i}"), &mut m);
            }
            // Head must always equal unnamed.
            assert_eq!(r.unnamed, r.numbered()[0]);
            assert!(r.numbered().len() <= Registers::MAX);
        }
        assert_eq!(r.unnamed, r.numbered()[0]);
        // Oldest entry is within capacity and is one of the expected prefixes.
        let oldest = r.numbered().last().unwrap();
        assert!(oldest.starts_with('y') || oldest.starts_with('d'));
    }

    #[test]
    fn unnamed_always_matches_head_across_ops() {
        let mut r = Registers::new();
        let mut m = OperatorMetrics::default();
        for i in 0..30 {
            // multiple overflows via deletes
            r.record_delete(format!("del{i}"), &mut m);
            assert_eq!(r.unnamed, r.numbered()[0]);
        }
        for i in 0..30 {
            // then yanks
            r.record_yank(format!("yank{i}"), &mut m);
            assert_eq!(r.unnamed, r.numbered()[0]);
        }
    }

    #[test]
    fn duplicate_payload_does_not_merge_order_preserved() {
        let mut r = Registers::new();
        let mut m = OperatorMetrics::default();
        r.record_yank("same", &mut m);
        r.record_delete("same", &mut m); // identical payload via different op
        r.record_yank("same", &mut m);
        let ring = r.numbered();
        assert_eq!(ring.len(), 3);
        assert_eq!(ring[0], "same");
        assert_eq!(ring[1], "same");
        assert_eq!(ring[2], "same");
    }

    #[test]
    fn metrics_rotation_and_writes_counts() {
        let mut r = Registers::new();
        let mut m = OperatorMetrics::default();
        for i in 0..(Registers::MAX + 3) {
            // overflow three times
            r.record_delete(format!("x{i}"), &mut m);
        }
        assert_eq!(m.register_writes, (Registers::MAX + 3) as u64);
        // First MAX fills without rotation; remaining 3 trigger rotation metric increments.
        assert_eq!(m.numbered_ring_rotations, 3);
    }

    // --- Phase 5 Step 5: Named register tests ---
    #[test]
    fn named_register_basic_write() {
        let mut r = Registers::new();
        let mut m = OperatorMetrics::default();
        r.record_yank_named('a', "alpha", &mut m);
        assert_eq!(r.get_named('a'), Some("alpha"));
        // unnamed mirrors named payload
        assert_eq!(r.unnamed, "alpha");
        assert_eq!(r.numbered()[0], "alpha");
    }

    #[test]
    fn named_register_uppercase_append() {
        let mut r = Registers::new();
        let mut m = OperatorMetrics::default();
        r.record_yank_named('a', "foo", &mut m);
        r.record_yank_named('A', "bar", &mut m); // append
        assert_eq!(r.get_named('a'), Some("foobar"));
        assert_eq!(r.get_named('A'), Some("foobar"));
        assert_eq!(r.unnamed, "foobar");
        assert_eq!(r.numbered()[0], "foobar");
    }

    #[test]
    fn named_snapshot_filters_empty() {
        let mut r = Registers::new();
        let mut m = OperatorMetrics::default();
        r.record_yank_named('b', "beta", &mut m);
        r.record_yank_named('d', "delta", &mut m);
        let snap = r.named_snapshot();
        assert!(snap.contains(&('b', "beta")));
        assert!(snap.contains(&('d', "delta")));
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn change_operation_behaves_like_delete() {
        use super::EditorState;
        use core_text::Buffer;
        let buf = Buffer::from_str("t", "abc\n").unwrap();
        let mut st = EditorState::new(buf);
        {
            let mut regs = st.registers_facade();
            regs.write_change("removed", None);
        }
        assert_eq!(st.registers.unnamed, "removed");
        assert_eq!(st.registers.numbered()[0], "removed");
        let metrics = st.operator_metrics_snapshot();
        assert_eq!(metrics.operator_change, 1);
        assert_eq!(metrics.register_writes, 1);
    }

    #[test]
    fn facade_delete_rotates_ring_and_metrics() {
        use super::EditorState;
        use core_text::Buffer;
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        for i in 0..(Registers::MAX + 2) {
            let mut regs = st.registers_facade();
            regs.write_delete(format!("d{i}"), None);
        }
        assert_eq!(st.registers.numbered().len(), Registers::MAX);
        assert_eq!(
            st.registers.numbered()[0],
            format!("d{}", Registers::MAX + 1)
        );
        assert_eq!(st.registers.unnamed, format!("d{}", Registers::MAX + 1));
        let metrics = st.operator_metrics_snapshot();
        assert_eq!(metrics.operator_delete, (Registers::MAX + 2) as u64);
        assert_eq!(metrics.register_writes, (Registers::MAX + 2) as u64);
        assert_eq!(metrics.numbered_ring_rotations, 2);
    }

    #[test]
    fn facade_named_register_append_across_ops() {
        use super::EditorState;
        use core_text::Buffer;
        let buf = Buffer::from_str("t", "").unwrap();
        let mut st = EditorState::new(buf);
        {
            let mut regs = st.registers_facade();
            regs.write_yank("foo", Some('a'));
        }
        {
            let mut regs = st.registers_facade();
            regs.write_change("bar", Some('A'));
        }
        assert_eq!(st.registers.get_named('a'), Some("foobar"));
        assert_eq!(st.registers.unnamed, "foobar");
        assert_eq!(st.registers.numbered()[0], "foobar");
        let metrics = st.operator_metrics_snapshot();
        assert_eq!(metrics.operator_yank, 1);
        assert_eq!(metrics.operator_change, 1);
        assert_eq!(metrics.register_writes, 2);
    }
}
