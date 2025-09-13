//! Render path metrics scaffold (Phase 3 Step 4).
//!
//! Distinct from `RenderDeltaMetrics` (scheduler) which counts *semantic*
//! invalidation requests (what the editor asked for). This struct records
//! *execution* strategy outcomes and internal partial rendering pipeline
//! counters (what we actually did / will do once partial path activates).
//! Keeping them separate preserves diagnostic ability to correlate semantic
//! intent vs chosen render strategy.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct RenderPathMetrics {
    /// Count of full frame renders executed (includes escalations & scroll/resize).
    pub full_frames: AtomicU64,
    /// Count of frames that executed any partial strategy (cursor-only or lines).
    pub partial_frames: AtomicU64,
    /// Sub-count: cursor-only partial frames (old+new cursor lines + status repainted).
    pub cursor_only_frames: AtomicU64,
    /// Sub-count: lines partial frames (hash-driven selective line repaints).
    pub lines_frames: AtomicU64,
    /// Escalations from candidate threshold (large dirty set forced full path).
    pub escalated_large_set: AtomicU64,
    /// Number of explicit cache invalidations due to resize (and buffer replacement reuse).
    pub resize_invalidations: AtomicU64,
    /// Raw dirty lines marked prior to viewport intersection + cursor injection.
    pub dirty_lines_marked: AtomicU64,
    /// Candidate lines after viewport filtering + cursor line additions.
    pub dirty_candidate_lines: AtomicU64,
    /// Lines physically repainted (subset of candidates; includes forced cursor lines).
    pub dirty_lines_repainted: AtomicU64,
    /// Duration (ns) of the most recent full frame render.
    pub last_full_render_ns: AtomicU64,
    /// Duration (ns) of the most recent partial frame render (cursor-only or lines).
    pub last_partial_render_ns: AtomicU64,
    /// Number of terminal Print commands emitted after batching (Step 7 baseline).
    pub print_commands: AtomicU64,
    /// Logical cells printed (batched plain chars + styled/multi-char units).
    pub cells_printed: AtomicU64,
    /// Count of scroll-region shift renders executed (Step 10).
    pub scroll_region_shifts: AtomicU64,
    /// Total lines saved from repaint due to scroll shift optimization.
    pub scroll_region_lines_saved: AtomicU64,
    /// Scroll shift attempts that degraded to a full repaint (Step 10 interim safeguard).
    pub scroll_shift_degraded_full: AtomicU64,
    /// Number of line trim attempts (Step 12).
    pub trim_attempts: AtomicU64,
    /// Number of successful trimmed line emissions.
    pub trim_success: AtomicU64,
    /// Total columns saved across successful trims.
    pub cols_saved_total: AtomicU64,
    /// Status line repaints skipped because content unchanged (Phase 4 Step 13).
    pub status_skipped: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderPathMetricsSnapshot {
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

impl RenderPathMetrics {
    pub fn snapshot(&self) -> RenderPathMetricsSnapshot {
        RenderPathMetricsSnapshot {
            full_frames: self.full_frames.load(Ordering::Relaxed),
            partial_frames: self.partial_frames.load(Ordering::Relaxed),
            cursor_only_frames: self.cursor_only_frames.load(Ordering::Relaxed),
            lines_frames: self.lines_frames.load(Ordering::Relaxed),
            escalated_large_set: self.escalated_large_set.load(Ordering::Relaxed),
            resize_invalidations: self.resize_invalidations.load(Ordering::Relaxed),
            dirty_lines_marked: self.dirty_lines_marked.load(Ordering::Relaxed),
            dirty_candidate_lines: self.dirty_candidate_lines.load(Ordering::Relaxed),
            dirty_lines_repainted: self.dirty_lines_repainted.load(Ordering::Relaxed),
            last_full_render_ns: self.last_full_render_ns.load(Ordering::Relaxed),
            last_partial_render_ns: self.last_partial_render_ns.load(Ordering::Relaxed),
            print_commands: self.print_commands.load(Ordering::Relaxed),
            cells_printed: self.cells_printed.load(Ordering::Relaxed),
            scroll_region_shifts: self.scroll_region_shifts.load(Ordering::Relaxed),
            scroll_region_lines_saved: self.scroll_region_lines_saved.load(Ordering::Relaxed),
            scroll_shift_degraded_full: self.scroll_shift_degraded_full.load(Ordering::Relaxed),
            trim_attempts: self.trim_attempts.load(Ordering::Relaxed),
            trim_success: self.trim_success.load(Ordering::Relaxed),
            cols_saved_total: self.cols_saved_total.load(Ordering::Relaxed),
            status_skipped: self.status_skipped.load(Ordering::Relaxed),
        }
    }
}
