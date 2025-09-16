//! Render scheduler (Phase 2 Steps 17–18).
//!
//! Breadth-first foundation for future partial rendering. Producers report
//! fine-grained invalidation intents (`RenderDelta`) via `mark`; on `consume`
//! we merge queued deltas into a single semantic shape and (for Phase 2)
//! always request a full-frame redraw. The semantic result is retained in the
//! returned `RenderDecision` and counted via lightweight atomic metrics so we
//! can later quantify optimization headroom before enabling partial paints.
//!
//! Merge semantics:
//! * Any `Full` present => `Full`.
//! * Multiple `Lines` coalesce into a single inclusive/exclusive range.
//! * Precedence order when heterogeneous: `Lines` > `StatusLine` > `CursorOnly`.
//! * `CursorOnly` + `StatusLine` collapses to `StatusLine`.
//!
//! Phase 2 policy: renderer still performs a full redraw (flicker-free, simple)
//! while instrumentation accumulates real semantic patterns. Phase 3+ may branch
//! on `decision.semantic` to drive incremental paint strategies.

/// Granular render invalidation intents produced by editor state changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderDelta {
    /// Entire frame must be repainted.
    Full,
    /// Text modifications confined to a (line) span. Range is half-open `[start, end)` using
    /// buffer line indices after internal normalization (LF-only lines).
    Lines(std::ops::Range<usize>),
    /// Only the status line contents changed (e.g. mode switch, filename, ephemeral message).
    StatusLine,
    /// Only the logical cursor moved within an otherwise unchanged line.
    CursorOnly,
}

#[derive(Debug, Default)]
pub struct RenderScheduler {
    /// Queue of deltas recorded since last `consume`.
    pending: Vec<RenderDelta>,
}

/// Result of a consume operation.
///
/// * `semantic`  – merged minimal damage (see merge semantics above).
/// * `effective` – delta the renderer should act on immediately (Phase 2 hard-coded `Full`).
///   Future phases may let this differ from `Full` for partial paints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderDecision {
    pub semantic: RenderDelta,
    pub effective: RenderDelta,
}

/// Simple atomic metrics for delta kind frequency (Phase 2 instrumentation).
pub mod metrics {
    use std::sync::atomic::{AtomicU64, Ordering};
    pub static DELTA_FULL: AtomicU64 = AtomicU64::new(0);
    pub static DELTA_LINES: AtomicU64 = AtomicU64::new(0);
    pub static DELTA_STATUS: AtomicU64 = AtomicU64::new(0);
    pub static DELTA_CURSOR: AtomicU64 = AtomicU64::new(0);

    pub fn incr(delta: &crate::scheduler::RenderDelta) {
        match delta {
            crate::scheduler::RenderDelta::Full => {
                DELTA_FULL.fetch_add(1, Ordering::Relaxed);
            }
            crate::scheduler::RenderDelta::Lines(_) => {
                DELTA_LINES.fetch_add(1, Ordering::Relaxed);
            }
            crate::scheduler::RenderDelta::StatusLine => {
                DELTA_STATUS.fetch_add(1, Ordering::Relaxed);
            }
            crate::scheduler::RenderDelta::CursorOnly => {
                DELTA_CURSOR.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

impl RenderScheduler {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Record a new delta. Multiple calls accumulate until `consume()`.
    pub fn mark(&mut self, delta: RenderDelta) {
        tracing::trace!(?delta, "render_mark");
        self.pending.push(delta);
    }

    /// Collapse queued deltas and return a `RenderDecision`.
    ///
    /// Phase 2 behavior: always sets `effective = RenderDelta::Full` while still reporting the
    /// merged semantic delta for telemetry and future incremental render logic.
    pub fn consume(&mut self) -> Option<RenderDecision> {
        if self.pending.is_empty() {
            return None;
        }
        let merged = self.collapse();
        tracing::trace!(?merged, "render_delta_collapse");
        self.pending.clear();
        metrics::incr(&merged);
        Some(RenderDecision {
            semantic: merged,
            effective: RenderDelta::Full,
        })
    }

    fn collapse(&self) -> RenderDelta {
        // Start pessimistic: if any Full present -> Full.
        if self.pending.iter().any(|d| matches!(d, RenderDelta::Full)) {
            return RenderDelta::Full;
        }
        let mut have_status = false;
        let mut have_cursor = false;
        let mut line_range: Option<std::ops::Range<usize>> = None;
        for d in &self.pending {
            match d {
                RenderDelta::Full => return RenderDelta::Full, // already handled
                RenderDelta::StatusLine => have_status = true,
                RenderDelta::CursorOnly => have_cursor = true,
                RenderDelta::Lines(r) => {
                    line_range = Some(match line_range.take() {
                        None => r.clone(),
                        Some(existing) => std::ops::Range {
                            start: existing.start.min(r.start),
                            end: existing.end.max(r.end),
                        },
                    });
                }
            }
        }
        // Precedence: lines outrank status/cursor because text change implies repaint there.
        if let Some(r) = line_range {
            return RenderDelta::Lines(r);
        }
        if have_status {
            return RenderDelta::StatusLine;
        }
        if have_cursor {
            return RenderDelta::CursorOnly;
        }
        // Should not reach (pending non-empty). Fallback Full.
        RenderDelta::Full
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_line_spans_merge() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::Lines(10..11));
        s.mark(RenderDelta::Lines(11..13));
        let merged = s.collapse();
        assert_eq!(merged, RenderDelta::Lines(10..13));
    }

    #[test]
    fn full_overrides_all() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::Lines(0..1));
        s.mark(RenderDelta::Full);
        s.mark(RenderDelta::CursorOnly);
        let merged = s.collapse();
        assert_eq!(merged, RenderDelta::Full);
    }

    #[test]
    fn status_plus_cursor_prefers_status() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::CursorOnly);
        s.mark(RenderDelta::StatusLine);
        let merged = s.collapse();
        assert_eq!(merged, RenderDelta::StatusLine);
    }

    #[test]
    fn consume_decision_semantic_and_full_effective() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::CursorOnly);
        let out = s.consume();
        let decision = out.expect("decision");
        assert_eq!(decision.semantic, RenderDelta::CursorOnly);
        assert_eq!(decision.effective, RenderDelta::Full);
        assert!(s.consume().is_none(), "second consume empty");
    }
}
