//! Render scheduler (Refactor R2 Step 3).
//!
//! Breadth-first foundation for future partial rendering. Producers report
//! fine-grained invalidation intents (`RenderDelta`) via `mark`. On `consume`
//! we merge queued deltas into a single semantic shape while (during Refactor
//! R2) still requesting a full-frame redraw. The semantic decision and its
//! metrics provide empirical guidance for when partial strategies will pay
//! off without guessing.
//!
//! Merge semantics (documented contract for Step 2):
//! - If any `Full` is present in the queue, the semantic decision is `Full`.
//! - Multiple `Lines` deltas merge into a single half-open range covering the min start to the max end: `[min(start), max(end))`.
//! - Multiple `Scroll` deltas coalesce into a single scroll capturing the earliest observed `old_first` (from the first scroll recorded) and the latest `new_first` (from the last scroll recorded).
//! - Heterogeneous precedence (Refactor R3 Step 5): `Scroll` > `Lines` > `StatusLine` > `CursorOnly`.
//!   This change favors scroll semantics so a future scroll‑region fast path can avoid unnecessary line hashing work.
//! - `CursorOnly` with `StatusLine` collapses to `StatusLine` (unless `Lines`/`Scroll` are present).
//!
//! Notes and examples:
//! - Order sensitivity: coalescing of `Scroll` retains the first seen `old_first` rather than the absolute minimum. Producers should mark deltas in the order they occurred within a frame.
//! - Example: `Lines(5..6) + CursorOnly` => `Lines(5..6)`.
//! - Example: `StatusLine + CursorOnly` => `StatusLine`.
//! - Example: `Scroll{3->7} + Lines(10..11)` => `Scroll{3->7}` (lines suppressed by precedence).
//!
//! Refactor R2 policy: renderer still performs a full redraw (flicker-free
//! and simple) while instrumentation accumulates real semantic patterns.
//! Phase 3 will branch on `decision.semantic` to drive incremental paints.

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
    /// Metrics accumulator (Refactor R2 Step 9).
    metrics: RenderDeltaMetrics,
}

/// Stable decision DTO (Step 2): minimal shape exposed to consumers.
///
/// Contract:
/// - `semantic`: the merged minimal damage kind for this frame based on queued marks.
/// - `effective`: the strategy the engine should execute now. May differ from
///   `semantic` when heuristics escalate or optimize (e.g., large scroll -> Full).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub semantic: RenderDelta,
    pub effective: RenderDelta,
}

/// Simple atomic metrics for delta kind frequency (Phase 2 / Refactor R2 instrumentation).
#[derive(Debug, Default)]
pub struct RenderDeltaMetrics {
    full: std::sync::atomic::AtomicU64,
    lines: std::sync::atomic::AtomicU64,
    scroll: std::sync::atomic::AtomicU64,
    status_line: std::sync::atomic::AtomicU64,
    cursor_only: std::sync::atomic::AtomicU64,
    collapsed_scroll: std::sync::atomic::AtomicU64,
    suppressed_scroll: std::sync::atomic::AtomicU64, // Refactor R3 Step 5: now counts Lines suppressed by Scroll precedence.
    /// Number of semantic collapse cycles processed (may diverge from
    /// executed frame strategy counts in `RenderPathMetrics`).
    semantic_frames: std::sync::atomic::AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderDeltaMetricsSnapshot {
    pub full: u64,
    pub lines: u64,
    pub scroll: u64,
    pub status_line: u64,
    pub cursor_only: u64,
    pub collapsed_scroll: u64,
    pub suppressed_scroll: u64,
    pub semantic_frames: u64,
}

impl RenderDeltaMetrics {
    pub fn snapshot(&self) -> RenderDeltaMetricsSnapshot {
        use std::sync::atomic::Ordering::Relaxed;
        RenderDeltaMetricsSnapshot {
            full: self.full.load(Relaxed),
            lines: self.lines.load(Relaxed),
            scroll: self.scroll.load(Relaxed),
            status_line: self.status_line.load(Relaxed),
            cursor_only: self.cursor_only.load(Relaxed),
            collapsed_scroll: self.collapsed_scroll.load(Relaxed),
            suppressed_scroll: self.suppressed_scroll.load(Relaxed),
            semantic_frames: self.semantic_frames.load(Relaxed),
        }
    }
    fn incr_semantic(&self, delta: &RenderDelta) {
        use std::sync::atomic::Ordering::Relaxed;
        match delta {
            RenderDelta::Full => {
                self.full.fetch_add(1, Relaxed);
            }
            RenderDelta::Lines(_) => {
                self.lines.fetch_add(1, Relaxed);
            }
            RenderDelta::Scroll { .. } => {
                self.scroll.fetch_add(1, Relaxed);
            }
            RenderDelta::StatusLine => {
                self.status_line.fetch_add(1, Relaxed);
            }
            RenderDelta::CursorOnly => {
                self.cursor_only.fetch_add(1, Relaxed);
            }
        }
    }
    fn incr_collapsed_scroll(&self) {
        self.collapsed_scroll
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn incr_suppressed_scroll(&self) {
        self.suppressed_scroll
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn incr_frame(&self) {
        self.semantic_frames
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl RenderScheduler {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            metrics: RenderDeltaMetrics::default(),
        }
    }

    /// Threshold: maximum absolute scroll line delta eligible for the scroll-region
    /// shift fast path. Larger deltas fall back to a full repaint since repainting
    /// entering lines would approach full frame cost and terminal scroll region
    /// commands provide diminishing benefit. Tuned conservatively; future tuning
    /// can be driven by metrics once the path is exercised.
    pub const SCROLL_SHIFT_MAX: usize = 12;

    /// Obtain a snapshot of current metrics (Refactor R2 Step 9).
    pub fn metrics_snapshot(&self) -> RenderDeltaMetricsSnapshot {
        self.metrics.snapshot()
    }

    /// Record a new delta. Multiple calls accumulate until `consume()`.
    pub fn mark(&mut self, delta: RenderDelta) {
        tracing::trace!(target: "render.scheduler", ?delta, "render_mark");
        self.pending.push(delta);
    }

    /// Convenience API (Refactor R3 Step 5): mark status line dirty explicitly
    /// without the caller constructing the enum variant. Intended for mode /
    /// command buffer changes (Step 6 logic) while keeping breadth-first stub
    /// inert until that step wires detection.
    pub fn mark_status(&mut self) {
        self.mark(RenderDelta::StatusLine);
    }

    /// Collapse queued deltas and return a `Decision`.
    ///
    /// Refactor R2 behavior: always sets `effective = RenderDelta::Full` while still reporting the
    /// merged semantic delta for telemetry and future incremental render logic.
    pub fn consume(&mut self) -> Option<Decision> {
        if self.pending.is_empty() {
            return None;
        }
        let merged = self.collapse();
        tracing::trace!(target: "render.scheduler", ?merged, "render_delta_collapse");
        self.pending.clear();
        self.metrics.incr_semantic(&merged);
        self.metrics.incr_frame();
        // Phase 3 Step 7: allow CursorOnly semantic to execute as a partial effective path.
        let effective = match &merged {
            // Phase 3 Step 7: CursorOnly partial; Phase 3 Step 8: Lines partial path
            RenderDelta::CursorOnly => RenderDelta::CursorOnly,
            RenderDelta::Lines(r) => RenderDelta::Lines(r.clone()),
            // Phase 4 Step 10: small scrolls (<= SCROLL_SHIFT_MAX) become an
            // effective Scroll path. Larger scrolls still escalate to Full so
            // we do not pay for shifting cache state & selective repaints.
            RenderDelta::Scroll {
                old_first,
                new_first,
            } => {
                let diff = new_first.abs_diff(*old_first);
                if diff <= Self::SCROLL_SHIFT_MAX {
                    RenderDelta::Scroll {
                        old_first: *old_first,
                        new_first: *new_first,
                    }
                } else {
                    RenderDelta::Full
                }
            }
            _ => RenderDelta::Full,
        };
        Some(Decision {
            semantic: merged.clone(),
            effective,
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
        let mut scroll_events = 0usize;
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
                    scroll_events += 1;
                    if scroll_old_first.is_none() {
                        scroll_old_first = Some(*old_first);
                    }
                    scroll_new_first = Some(*new_first); // always update latest
                }
            }
        }
        // New precedence (Refactor R3 Step 5): Scroll outranks Lines.
        if let (Some(of), Some(nf)) = (scroll_old_first, scroll_new_first) {
            if scroll_events > 1 {
                for _ in 1..scroll_events {
                    self.metrics.incr_collapsed_scroll();
                }
            }
            if line_range.is_some() {
                // Lines suppressed by scroll precedence (repurpose metric name).
                self.metrics.incr_suppressed_scroll();
            }
            return RenderDelta::Scroll {
                old_first: of,
                new_first: nf,
            };
        }
        if let Some(r) = line_range {
            return RenderDelta::Lines(r);
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
    fn scroll_precedence_over_lines() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::Scroll {
            old_first: 3,
            new_first: 5,
        });
        s.mark(RenderDelta::Lines(10..11));
        let merged = s.collapse();
        assert!(matches!(
            merged,
            RenderDelta::Scroll {
                old_first: 3,
                new_first: 5
            }
        ));
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
        // Phase 3 Step 7: CursorOnly now executes as a partial effective path.
        assert_eq!(decision.effective, RenderDelta::CursorOnly);
        assert!(s.consume().is_none(), "second consume empty");
    }

    #[test]
    fn metrics_scroll_collapsed_and_suppressed() {
        let mut s = RenderScheduler::new();
        // Three scroll events before consume -> 2 collapsed increments
        s.mark(RenderDelta::Scroll {
            old_first: 0,
            new_first: 1,
        });
        s.mark(RenderDelta::Scroll {
            old_first: 1,
            new_first: 2,
        });
        s.mark(RenderDelta::Scroll {
            old_first: 2,
            new_first: 3,
        });
        let d = s.consume().unwrap();
        assert!(matches!(
            d.semantic,
            RenderDelta::Scroll {
                old_first: 0,
                new_first: 3
            }
        ));
        let snap = s.metrics_snapshot();
        assert_eq!(snap.scroll, 1);
        assert_eq!(
            snap.collapsed_scroll, 2,
            "expected two collapsed increments (three events -> two collapses)"
        );
        // Now add a scroll then a lines edit (lines suppressed by scroll precedence).
        s.mark(RenderDelta::Scroll {
            old_first: 3,
            new_first: 4,
        });
        s.mark(RenderDelta::Lines(10..11));
        let d2 = s.consume().unwrap();
        assert!(matches!(
            d2.semantic,
            RenderDelta::Scroll {
                old_first: 3,
                new_first: 4
            }
        ));
        let snap2 = s.metrics_snapshot();
        assert_eq!(snap2.scroll, 2, "two scroll semantic frames");
        assert_eq!(
            snap2.suppressed_scroll, 1,
            "one lines set suppressed by scroll"
        );
        // Frames rendered should equal number of consume decisions (2 here)
        assert_eq!(snap2.semantic_frames, 2);
    }

    #[test]
    fn effective_small_scroll_path() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::Scroll {
            old_first: 10,
            new_first: 15,
        });
        let d = s.consume().unwrap();
        assert_eq!(
            d.semantic,
            RenderDelta::Scroll {
                old_first: 10,
                new_first: 15
            }
        );
        assert_eq!(
            d.effective,
            RenderDelta::Scroll {
                old_first: 10,
                new_first: 15
            },
            "small scroll within threshold should be effective Scroll"
        );
    }

    #[test]
    fn effective_large_scroll_escalates_full() {
        let mut s = RenderScheduler::new();
        // Exceed threshold (SCROLL_SHIFT_MAX=12) with delta 20
        s.mark(RenderDelta::Scroll {
            old_first: 0,
            new_first: 20,
        });
        let d = s.consume().unwrap();
        assert_eq!(
            d.semantic,
            RenderDelta::Scroll {
                old_first: 0,
                new_first: 20
            }
        );
        assert_eq!(
            d.effective,
            RenderDelta::Full,
            "large scroll should escalate to full effective"
        );
    }
}
