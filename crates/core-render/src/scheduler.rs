//! Render scheduler (Phase 2 Steps 17–18, Refactor R2 Step 3 update).
//!
//! Breadth-first foundation for future partial rendering. Producers report
//! fine-grained invalidation intents (`RenderDelta`) via `mark`; on `consume`
//! we merge queued deltas into a single semantic shape and (for Phase 2 /
//! Refactor R2) always request a full-frame redraw. The semantic result is
//! retained in the returned `RenderDecision` and counted via lightweight
//! atomic metrics so we can later quantify optimization headroom before
//! enabling partial paints.
//!
//! Merge semantics:
//! * Any `Full` present => `Full`.
//! * Multiple `Lines` coalesce into a single inclusive/exclusive range.
//! * Multiple `Scroll` deltas coalesce (earliest `old_first`, latest `new_first`).
//! * Precedence when heterogeneous: `Lines` > `Scroll` > `StatusLine` > `CursorOnly`.
//! * `CursorOnly` + `StatusLine` collapses to `StatusLine` (unless `Lines`/`Scroll` present).
//!
//! Refactor R2 policy: renderer still performs a full redraw (flicker-free,
//! simple) while instrumentation accumulates real semantic patterns. Phase 3+
//! may branch on `decision.semantic` to drive incremental paint strategies.

/// Granular render invalidation intents produced by editor state changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderDelta {
    /// Entire frame must be repainted.
    Full,
    /// Text modifications confined to a (line) span. Range is half-open `[start, end)` using
    /// buffer line indices after internal normalization (LF-only lines).
    Lines(std::ops::Range<usize>),
    /// Viewport vertical scroll (semantic only in Refactor R2 – effective repaint still Full).
    /// `old_first` = previous first visible buffer line, `new_first` = new first visible line.
    Scroll { old_first: usize, new_first: usize },
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

/// Simple atomic metrics for delta kind frequency (Phase 2 / Refactor R2 instrumentation).
pub mod metrics {
    use std::sync::atomic::{AtomicU64, Ordering};
    pub static DELTA_FULL: AtomicU64 = AtomicU64::new(0);
    pub static DELTA_LINES: AtomicU64 = AtomicU64::new(0);
    pub static DELTA_SCROLL: AtomicU64 = AtomicU64::new(0);
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
            crate::scheduler::RenderDelta::Scroll { .. } => {
                DELTA_SCROLL.fetch_add(1, Ordering::Relaxed);
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
    /// Refactor R2 behavior: always sets `effective = RenderDelta::Full` while still reporting the
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
        // If any Full present -> Full.
        if self.pending.iter().any(|d| matches!(d, RenderDelta::Full)) {
            return RenderDelta::Full;
        }
        let mut have_status = false;
        let mut have_cursor = false;
        let mut line_range: Option<std::ops::Range<usize>> = None;
        let mut scroll_old_first: Option<usize> = None;
        let mut scroll_new_first: Option<usize> = None;
        for d in &self.pending {
            match d {
                RenderDelta::Full => return RenderDelta::Full, // already handled above
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
                RenderDelta::Scroll {
                    old_first,
                    new_first,
                } => {
                    if scroll_old_first.is_none() {
                        scroll_old_first = Some(*old_first);
                    }
                    scroll_new_first = Some(*new_first); // always update latest
                }
            }
        }
        // Precedence: Lines outrank all other semantic kinds.
        if let Some(r) = line_range {
            return RenderDelta::Lines(r);
        }
        // Next: Scroll (if any recorded and not superseded by Lines).
        if let (Some(of), Some(nf)) = (scroll_old_first, scroll_new_first) {
            return RenderDelta::Scroll {
                old_first: of,
                new_first: nf,
            };
        }
        if have_status {
            return RenderDelta::StatusLine;
        }
        if have_cursor {
            return RenderDelta::CursorOnly;
        }
        // Should not happen (pending non-empty) – fallback Full.
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
    fn scroll_exclusive_preserved() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::Scroll {
            old_first: 5,
            new_first: 7,
        });
        let merged = s.collapse();
        assert_eq!(
            merged,
            RenderDelta::Scroll {
                old_first: 5,
                new_first: 7
            }
        );
    }

    #[test]
    fn scroll_multiple_merge() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::Scroll {
            old_first: 5,
            new_first: 6,
        });
        s.mark(RenderDelta::Scroll {
            old_first: 6,
            new_first: 10,
        });
        s.mark(RenderDelta::Scroll {
            old_first: 10,
            new_first: 12,
        });
        let merged = s.collapse();
        assert_eq!(
            merged,
            RenderDelta::Scroll {
                old_first: 5,
                new_first: 12
            }
        );
    }

    #[test]
    fn lines_suppress_scroll() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::Scroll {
            old_first: 3,
            new_first: 5,
        });
        s.mark(RenderDelta::Lines(10..11));
        let merged = s.collapse();
        assert_eq!(merged, RenderDelta::Lines(10..11));
    }

    #[test]
    fn scroll_precedence_over_status_and_cursor() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::CursorOnly);
        s.mark(RenderDelta::StatusLine);
        s.mark(RenderDelta::Scroll {
            old_first: 0,
            new_first: 1,
        });
        let merged = s.collapse();
        assert_eq!(
            merged,
            RenderDelta::Scroll {
                old_first: 0,
                new_first: 1
            }
        );
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
