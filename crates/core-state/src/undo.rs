use core_text::{Buffer, Position};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::trace;

use crate::Mode;

/// Maximum number of snapshots retained in undo history.
pub const UNDO_HISTORY_MAX: usize = 200;

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
    /// Content hash of the buffer at snapshot capture (Phase 3 Step 11).
    pub hash: u64,
}

/// Insert run state tracking (Refactor R1 Step 6).
#[derive(Debug, Clone)]
pub enum InsertRun {
    Inactive,
    Active {
        started_at: std::time::Instant,
        edits: u32,
    },
}

pub struct UndoEngine {
    undo_stack: Vec<EditSnapshot>,
    redo_stack: Vec<EditSnapshot>,
    insert_run: InsertRun,
    /// Count of snapshots skipped due to identical successive state (Phase 3 Step 11).
    undo_snapshots_skipped: AtomicU64,
}

impl Default for UndoEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoEngine {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            insert_run: InsertRun::Inactive,
            undo_snapshots_skipped: AtomicU64::new(0),
        }
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }
    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }
    pub fn insert_run(&self) -> &InsertRun {
        &self.insert_run
    }
    pub fn snapshots_skipped(&self) -> u64 {
        self.undo_snapshots_skipped.load(Ordering::Relaxed)
    }

    pub fn push_snapshot(
        &mut self,
        kind: SnapshotKind,
        cursor: Position,
        buffer: &Buffer,
        mode: Mode,
    ) {
        let current_hash = buffer_hash(buffer);
        if let Some(last) = self.undo_stack.last()
            && last.hash == current_hash
        {
            self.undo_snapshots_skipped.fetch_add(1, Ordering::Relaxed);
            trace!(target: "state.undo", undo_depth = self.undo_stack.len(), redo_depth = self.redo_stack.len(), hash = current_hash, "snapshot_dedupe_skip");
            return;
        }
        let snap = EditSnapshot {
            kind,
            buffer: buffer.clone(),
            position: cursor,
            mode,
            hash: current_hash,
        };
        let rope_lines_before = buffer.line_count();
        self.undo_stack.push(snap);
        trace!(target: "state.undo", undo_depth = self.undo_stack.len(), redo_depth = self.redo_stack.len(), lines = rope_lines_before, hash = current_hash, "push_snapshot");
        if self.undo_stack.len() > UNDO_HISTORY_MAX {
            let _ = self.undo_stack.remove(0);
            trace!(target: "state.undo", "undo_stack_trimmed");
        }
        self.redo_stack.clear();
        trace!(target: "state.undo", "redo_stack_cleared_on_new_edit");
    }

    pub fn begin_insert_coalescing(&mut self, cursor: Position, buffer: &Buffer, mode: Mode) {
        match self.insert_run {
            InsertRun::Inactive => {
                self.push_snapshot(SnapshotKind::Edit, cursor, buffer, mode);
                self.insert_run = InsertRun::Active {
                    started_at: std::time::Instant::now(),
                    edits: 0,
                };
            }
            InsertRun::Active { .. } => {}
        }
    }

    pub fn end_insert_coalescing(&mut self) {
        self.insert_run = InsertRun::Inactive;
    }
    pub fn push_discrete_edit_snapshot(&mut self, cursor: Position, buffer: &Buffer, mode: Mode) {
        self.push_snapshot(SnapshotKind::Edit, cursor, buffer, mode);
    }
    pub fn note_insert_edit(&mut self) {
        if let InsertRun::Active { edits, .. } = &mut self.insert_run {
            *edits += 1;
        }
    }

    pub fn undo(&mut self, cursor: &mut Position, buffer: &mut Buffer, mode: &mut Mode) -> bool {
        if let Some(last) = self.undo_stack.pop() {
            trace!(target: "state.undo", undo_depth = self.undo_stack.len(), redo_depth = self.redo_stack.len(), "undo_pop");
            let current = EditSnapshot {
                kind: last.kind,
                buffer: buffer.clone(),
                position: *cursor,
                mode: *mode,
                hash: buffer_hash(buffer),
            };
            self.redo_stack.push(current);
            trace!(target: "state.undo", redo_depth = self.redo_stack.len(), "redo_push_from_undo");
            *buffer = last.buffer;
            *cursor = last.position;
            if !matches!(last.kind, SnapshotKind::Edit) {
                *mode = last.mode;
            }
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, cursor: &mut Position, buffer: &mut Buffer, mode: &mut Mode) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            trace!(target: "state.undo", redo_depth = self.redo_stack.len(), undo_depth = self.undo_stack.len(), "redo_pop");
            let current = EditSnapshot {
                kind: next.kind,
                buffer: buffer.clone(),
                position: *cursor,
                mode: *mode,
                hash: buffer_hash(buffer),
            };
            self.undo_stack.push(current);
            trace!(target: "state.undo", undo_depth = self.undo_stack.len(), "undo_push_from_redo");
            *buffer = next.buffer;
            *cursor = next.position;
            if !matches!(next.kind, SnapshotKind::Edit) {
                *mode = next.mode;
            }
            true
        } else {
            false
        }
    }
}

fn buffer_hash(buf: &Buffer) -> u64 {
    let mut h = DefaultHasher::new();
    for i in 0..buf.line_count() {
        if let Some(l) = buf.line(i) {
            h.write(l.as_bytes());
        }
    }
    h.finish()
}
