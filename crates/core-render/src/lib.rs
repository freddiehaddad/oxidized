//! Rendering primitives + frame assembly + partial repaint engine.
//!
//! Post Refactor R3 (Step 12) the legacy one-shot `Renderer` was removed; all
//! paths flow through `RenderEngine` which owns metrics, hashing, scheduling,
//! and writer emission. This concentrates optimization and future scroll-region
//! work in a single locus.
//!
//! Integration Points:
//! - `Layout` (core-model): currently single-region; future multi-view fan-out will
//!   iterate regions and invoke partial scheduling per viewport.
//! - `TerminalCapabilities` (core-terminal): stub exposes `supports_scroll_region` and
//!   will gate scroll-delta optimization & cache shifting in Phase 4.
//! - `UndoEngine` (core-state) indirectly influences dirty marking via dispatcher edits.
//! - `KeyTranslator` / command system drive semantic deltas feeding the scheduler.
//!
//! Exposed Components:
//! - `Cell` / `Frame`: logical grid backing full-frame composition.
//! - `render_engine`: orchestrates full + partial paths (cursor-only & lines) with
//!   hashing cache, overlay application, status line integration, and instrumentation.
//! - `scheduler`: merges fine‑grained semantic deltas (`RenderDelta`) into an effective
//!   per-frame decision (may escalate to Full).
//! - `partial_cache` / `partial_diff`: viewport line hashing & change classification.
//! - `writer`: terminal command abstraction (MoveTo, ClearLine, Print) used by partial
//!   paths and (currently) full path translation for consistency.
//! - `partial_metrics`: execution path counters & timing separate from semantic metrics.
//! - `status`: builds status line string (mode, file, position, ephemeral messages).
//! - `dirty`: dirty line tracker fed by dispatcher edit mutations.
//!
//! Partial Pipeline (Phase 3 MVP):
//! 1. Scheduler emits semantic delta (CursorOnly | Lines | Scroll | Full) after coalescing.
//! 2. Effective decision derived (may escalate to Full for scroll, resize, cold cache,
//!    large candidate set, or structural buffer replacement).
//! 3. Full path: classify hashes (warm vs cold), build `Frame`, apply cursor & status,
//!    emit via writer (row‑major MoveTo per row ensures wrap safety), refresh cache.
//! 4. Cursor-only path: repaint prior + new cursor lines (if distinct) and status line;
//!    skip hashing; minimal writer output.
//! 5. Lines path: gather dirty indices + old/new cursor lines, threshold check (>=60%
//!    visible rows escalates), compute hashes only for candidates, repaint changed or
//!    cursor-mandated lines, overlay cursor, leave untouched lines intact.
//!
//! Hash & Cache Lifecycle:
//! - Full renders always (re)build hash snapshot for the viewport making subsequent
//!   partial frames safe.
//! - Lines partial updates per repainted line; unchanged cached entries are retained.
//! - Cache invalidated explicitly on resize or buffer replacement and implicitly
//!   treated cold if viewport start / width mismatch.
//!
//! Metrics Taxonomy (`RenderPathMetrics`):
//! - Volume: `full_frames`, `partial_frames`, `cursor_only_frames`, `lines_frames`.
//! - Dirty Funnel: `dirty_lines_marked` (pre-filter), `dirty_candidate_lines` (post
//!   intersection + cursor injection), `dirty_lines_repainted` (actual repaints).
//! - Escalation & Env: `escalated_large_set`, `resize_invalidations`.
//! - Timing: `last_full_render_ns`, `last_partial_render_ns` (point samples; moving
//!   averages deferred).
//!
//!   Interpretation Signals:
//! - High candidate vs repainted delta => hashing avoiding redundant repaints.
//! - Frequent escalation events => tune threshold or implement scroll-region fast path.
//! - Large partial latency vs full => investigate hashing overhead or ClearLine volume.
//!
//! Invalidation & Escalation Policies:
//! - Resize / buffer replacement => unconditional cache clear; force next frame Full.
//! - Lines threshold (>= 60% of visible rows) => escalate to Full.
//! - Cold cache (viewport start or width change) => Full (caller or internal fallback).
//! - Cursor-only path relies on prior full frame correctness (no hashing each motion).
//!
//! Phase 4 Additions:
//! - Scroll region shift path (real S/T emission) with entering line repaints
//!   and reuse of partial cache via `shift_for_scroll` (lines saved metric).
//! - Trimmed line diff heuristic (prefix/suffix skip) storing prior line text
//!   (`prev_text`) and emitting only interior mutations when savings threshold met.
//! - Status line skip cache (`prev_status`) increments `status_skipped` when content
//!   unchanged across partial frames.
//! - Unified helpers (`paint_content_trim`, `overlay_cursor_cluster`,
//!   `maybe_repaint_status`) reduced duplicated ANSI emission logic across partial
//!   strategies.
//! - Performance parity tests (Step 17) assert structural metric invariants for the
//!   optimized paths without relying on timing.
//!
//! Deferred (Future Phases): multi-line segmented diff trimming, command batching,
//! moving average latency metrics, Unicode width caching, multi-viewport fan-out.
//!
//! Architectural Tenets Applied:
//! - Breadth-first: feature order prioritized correctness & instrumentation before micro
//!   optimizations (full -> cursor-only -> lines -> escalation heuristic).
//! - Modularity: hashing & metrics isolated; future LSP / syntax can compose without
//!   coupling to render internals.
//! - Unicode correctness: grapheme cluster boundaries & display width respected in all
//!   paths (status column fix hotfix Steps 8.1/8.2).
//!
//! See Phase 3 design document (Step 14) for extended narrative & rationale.

use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CellFlags: u8 {
        const REVERSE = 0b0000_0001; // reverse-video (software cursor)
        const CURSOR  = 0b0000_0010; // marks cell part of cursor span
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub flags: CellFlags,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            flags: CellFlags::empty(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<Cell>,
}

impl Frame {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); (width as usize) * (height as usize)],
        }
    }

    pub fn set(&mut self, x: u16, y: u16, ch: char) {
        if x < self.width && y < self.height {
            let idx = y as usize * self.width as usize + x as usize;
            self.cells[idx].ch = ch;
        }
    }

    pub fn set_with_flags(&mut self, x: u16, y: u16, ch: char, flags: CellFlags) {
        if x < self.width && y < self.height {
            let idx = y as usize * self.width as usize + x as usize;
            self.cells[idx].ch = ch;
            self.cells[idx].flags = flags;
        }
    }
}

// Legacy full-frame `Renderer` removed in Refactor R3 Step 12. All rendering paths
// now flow through `RenderEngine` (full + partial) and shared writer abstraction.

pub mod batch_writer; // Refactor R3 Step 7: batching writer wrapper
pub mod dirty; // Phase 3 Step 1: dirty line tracking (external to RenderDelta)
pub mod partial_cache; // Phase 3 Step 2: line hash + cache skeleton
pub mod partial_diff; // New module for partial differences
pub mod partial_metrics; // Phase 3 Step 4: metrics scaffold
pub mod render_engine;
pub mod scheduler;
pub mod status;
pub mod timing;
pub mod viewport; // (placeholder for future viewport helpers)
pub mod writer; // Phase 3 Step 6: terminal writer abstraction
