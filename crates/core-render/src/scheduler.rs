//! Render scheduler (Phase 2 Step 17 – RenderDelta scaffold).
//!
//! Evolves the initial dirty-flag-only scheduler into a semantic delta
//! accumulator. Producers call `mark(RenderDelta)`; on `consume()` we collapse
//! all queued deltas into a single merged representation. Phase 2 still forces
//! full-frame redraws for simplicity: `consume()` always returns
//! `RenderDelta::Full` but logs the merged pre-collapse shape for future
//! optimization.
//!
//! Future phases will interpret non-Full variants to perform partial paints
//! (e.g. line range diff, status-only, cursor-only). By landing this scaffold
//! early we avoid later refactors across numerous call sites.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderDelta {
    Full,
    /// Inclusive start, exclusive end line range (buffer line indices)
    Lines(std::ops::Range<usize>),
    StatusLine,
    CursorOnly,
}

#[derive(Debug, Default)]
pub struct RenderScheduler {
    pending: Vec<RenderDelta>,
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

    /// Collapse queued deltas and return (Phase 2: always `Full`).
    pub fn consume(&mut self) -> Option<RenderDelta> {
        if self.pending.is_empty() {
            return None;
        }
        let merged = self.collapse();
        tracing::trace!(?merged, "render_delta_collapse");
        self.pending.clear();
        Some(RenderDelta::Full) // Phase 2 policy
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
    fn consume_always_returns_full_phase2() {
        let mut s = RenderScheduler::new();
        s.mark(RenderDelta::CursorOnly);
        let out = s.consume();
        assert!(matches!(out, Some(RenderDelta::Full)));
        assert!(s.consume().is_none(), "second consume empty");
    }
}
