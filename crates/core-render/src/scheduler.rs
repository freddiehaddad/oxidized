//! Render scheduler (Refactor R1 Step 4).
//!
//! Breadth-first placeholder abstraction that currently tracks only a single
//! dirty bit and always triggers a full-frame redraw when consumed. Future
//! phases will evolve this module (without changing call-sites) to support:
//!
//! * Debounced / coalesced renders (e.g. within a small delay window)
//! * Damage tracking (line ranges, cursor-only, status-only)
//! * Integration with async producers (timers, background tasks)
//!
//! The intent is to keep the binary `main` free of ad-hoc scheduling logic and
//! local state so that adding more sophisticated render policies does not
//! require touching the event loop logic.

#[derive(Debug, Default)]
pub struct RenderScheduler {
    dirty: bool,
}

impl RenderScheduler {
    /// Create a new scheduler (initially clean).
    pub fn new() -> Self {
        Self { dirty: false }
    }

    /// Mark the frame as needing a redraw.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Consume the dirty flag, returning true if a redraw should occur.
    ///
    /// Current policy: any dirty state triggers a full-frame redraw, and the
    /// flag is reset. Future versions will return richer delta information.
    pub fn consume_dirty(&mut self) -> bool {
        if self.dirty {
            self.dirty = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_clean() {
        let mut s = RenderScheduler::new();
        assert!(!s.consume_dirty());
    }

    #[test]
    fn mark_and_consume() {
        let mut s = RenderScheduler::new();
        s.mark_dirty();
        assert!(s.consume_dirty());
        // Second consume should be clean again.
        assert!(!s.consume_dirty());
    }
}
