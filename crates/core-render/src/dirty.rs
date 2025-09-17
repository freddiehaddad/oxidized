//! Dirty line tracking (Phase 3 Step 1).
//!
//! Breadth-first minimal structure that records candidate buffer line indices
//! affected by edits. This is intentionally kept separate from semantic
//! `RenderDelta` to avoid widening the enum early; partial rendering will
//! intersect these with the active viewport to produce a concrete repaint set.
//!
//! Design constraints:
//! * Duplicate marks are deduped lazily when `take_in_viewport` is called.
//! * Internally stores raw `usize` line numbers; future optimization may store
//!   small ranges or a bitset if density patterns warrant.
//! * Not thread-safe (mutably borrowed in event loop single-thread context).
//!
//! Invariants:
//! * Returned vector from `take_in_viewport` is sorted ascending and unique.
//! * After `take_in_viewport`, internal storage is cleared (one-shot consumption).

#[derive(Debug, Default)]
pub struct DirtyLinesTracker {
    lines: Vec<usize>,
}

impl DirtyLinesTracker {
    /// Create an empty tracker.
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Mark a single line index as dirty.
    pub fn mark(&mut self, line: usize) {
        self.lines.push(line);
    }

    /// Mark an inclusive range of line indices as dirty.
    pub fn mark_range(&mut self, start: usize, end_inclusive: usize) {
        if start > end_inclusive {
            return;
        }
        // Breadth-first simple push; no attempt to collapse yet.
        for l in start..=end_inclusive {
            self.lines.push(l);
        }
    }

    /// Consume and return unique, sorted dirty lines that intersect the viewport
    /// defined by `[first, first+height)`.
    pub fn take_in_viewport(&mut self, first: usize, height: usize) -> Vec<usize> {
        if self.lines.is_empty() || height == 0 {
            self.lines.clear();
            return Vec::new();
        }
        let end = first + height;
        // Retain only those within the viewport span.
        let mut v: Vec<usize> = self
            .lines
            .drain(..)
            .filter(|l| *l >= first && *l < end)
            .collect();
        if v.is_empty() {
            return v;
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    /// True if no lines have been marked since last consumption.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Clear all tracked lines without returning them (reset state).
    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_and_take_basic() {
        let mut t = DirtyLinesTracker::new();
        t.mark(3);
        t.mark(1);
        t.mark(3);
        let out = t.take_in_viewport(0, 10);
        assert_eq!(out, vec![1, 3]);
        assert!(t.is_empty());
    }

    #[test]
    fn viewport_filter_and_sort() {
        let mut t = DirtyLinesTracker::new();
        t.mark_range(0, 5); // 0..=5
        t.mark(10);
        t.mark(7);
        let out = t.take_in_viewport(2, 3); // lines 2,3,4
        assert_eq!(out, vec![2, 3, 4]);
        assert!(t.is_empty());
    }

    #[test]
    fn empty_after_clear() {
        let mut t = DirtyLinesTracker::new();
        t.mark(42);
        t.clear();
        assert!(t.is_empty());
        let out = t.take_in_viewport(0, 100);
        assert!(out.is_empty());
    }
}
