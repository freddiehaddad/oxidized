//! Partial rendering cache skeleton (Phase 3 Step 2).
//!
//! Stores lightweight hash metadata for each line in the currently rendered
//! viewport. Allows quick detection of unchanged lines to skip repaint in
//! subsequent partial frames. Activated in later steps (hash compare logic
//! and partial path activation). For now, a minimal API plus tests.
//!
//! Hashing strategy: (len, ahash64) on raw UTF-8 line content with trailing
//! newline removed. Length included to further reduce collision probability
//! and allow short-circuit mismatch detection.

use ahash::AHasher;
use std::hash::{Hash, Hasher};

/// Snapshot hash metadata for a single visible line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportLineHash {
    pub hash: u64,
    pub len: usize,
}

impl ViewportLineHash {
    pub fn new(hash: u64, len: usize) -> Self {
        Self { hash, len }
    }
}

/// Cache of line hashes for the active viewport.
#[derive(Debug, Default)]
pub struct PartialCache {
    /// First buffer line index represented by `line_hashes[0]`.
    pub viewport_start: usize,
    /// Terminal width (used later for padding decisions / truncation heuristics).
    pub width: u16,
    /// Hash entries per visible buffer line (excluding status line).
    pub line_hashes: Vec<ViewportLineHash>,
    /// Previous frame's exact UTF-8 text for each visible line (no trailing newline).
    /// Invariant: `prev_text.len() == line_hashes.len()` whenever cache is warm.
    /// Entry is `None` when content is unknown (cold start, newly entered via scroll, or after clear).
    pub prev_text: Vec<Option<String>>,
    /// Previous frame's cursor line (for repaint of old cursor span). None if unknown or no prior frame.
    pub last_cursor_line: Option<usize>,
}

impl PartialCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fully clear all cached metadata (used on resize invalidation). This resets
    /// viewport origin/width to sentinel values and drops line hashes and last cursor
    /// line so that the next render is forced to treat the cache as cold.
    pub fn clear(&mut self) {
        self.viewport_start = 0;
        self.width = 0;
        self.line_hashes.clear();
        self.prev_text.clear();
        self.last_cursor_line = None;
    }

    /// Reset cache to represent a new viewport slice (caller supplies vector capacity hint).
    pub fn reset(&mut self, viewport_start: usize, width: u16, expected_lines: usize) {
        self.viewport_start = viewport_start;
        self.width = width;
        self.line_hashes.clear();
        self.prev_text.clear();
        // Preserve last_cursor_line across resets; it is invalidated only explicitly (resize clears in later step).
        if self.line_hashes.capacity() < expected_lines {
            self.line_hashes
                .reserve(expected_lines - self.line_hashes.capacity());
        }
        if self.prev_text.capacity() < expected_lines {
            self.prev_text
                .reserve(expected_lines - self.prev_text.capacity());
        }
    }

    /// Push a hash entry for a line (in viewport order).
    pub fn push_line(&mut self, entry: ViewportLineHash) {
        self.line_hashes.push(entry);
        self.prev_text.push(None); // unknown until actually painted
    }

    /// Compute hash for a line (content without trailing newline). Public for tests; later used by partial renderer.
    pub fn compute_hash(line: &str) -> ViewportLineHash {
        let mut hasher = AHasher::default();
        line.hash(&mut hasher);
        ViewportLineHash {
            hash: hasher.finish(),
            len: line.len(),
        }
    }

    /// Access an entry by relative viewport row.
    pub fn get(&self, row: usize) -> Option<ViewportLineHash> {
        self.line_hashes.get(row).copied()
    }

    /// Access previously painted line text for a relative viewport row, if known.
    pub fn get_prev_text(&self, row: usize) -> Option<&str> {
        self.prev_text.get(row).and_then(|o| o.as_deref())
    }

    /// Update (or set) previously painted text for a relative viewport row.
    pub fn set_prev_text(&mut self, row: usize, text: String) {
        if row >= self.prev_text.len() {
            return; // defensive: ignore out-of-bounds silently (caller logic bug)
        }
        self.prev_text[row] = Some(text);
    }

    /// Phase 4 Step 11: shift cache in-place for a scroll-region shift.
    ///
    /// Arguments:
    /// * `delta` – positive when viewport moved down (content scrolled up / new lines enter at bottom),
    ///   negative when viewport moved up (new lines enter at top).
    /// * `new_first` – new viewport starting buffer line.
    /// * `visible_rows` – number of text rows (excluding status line) represented in the cache.
    /// * `buf_line_provider` – closure returning an optional raw buffer line by absolute line index.
    ///
    /// Behavior:
    /// * Reuses existing hash entries for lines that remain visible by shifting them in-place.
    /// * Recomputes hashes only for entering lines (top or bottom segment depending on delta).
    /// * If `delta` magnitude >= visible_rows, caller should have degraded to full render
    ///   before invoking (we assert to catch misuse in tests / debug builds).
    pub fn shift_for_scroll<F>(
        &mut self,
        delta: i32,
        new_first: usize,
        visible_rows: usize,
        mut buf_line_provider: F,
    ) where
        F: FnMut(usize) -> Option<String>,
    {
        debug_assert!(
            visible_rows == self.line_hashes.len(),
            "cache size mismatch"
        );
        debug_assert!(
            visible_rows == self.prev_text.len(),
            "prev_text size mismatch"
        );
        debug_assert!(delta != 0, "no-op delta passed to shift_for_scroll");
        let abs = delta.unsigned_abs() as usize;
        debug_assert!(
            abs < visible_rows,
            "degenerate shift should have escalated to full"
        );

        if delta > 0 {
            // Scroll down: viewport moved down, content moved up, new lines at bottom.
            let entering = abs;
            // Shift existing reused lines up.
            for i in 0..(visible_rows - entering) {
                let src = i + entering;
                self.line_hashes[i] = self.line_hashes[src];
                self.prev_text[i] = self.prev_text[src].take();
            }
            // Recompute hashes for entering lines (bottom segment).
            for i in 0..entering {
                let row = visible_rows - entering + i;
                let buf_line_index = new_first + row;
                let vh = if let Some(raw_line) = buf_line_provider(buf_line_index) {
                    let content_trim: &str = raw_line.trim_end_matches(['\n', '\r']);
                    PartialCache::compute_hash(content_trim)
                } else {
                    PartialCache::compute_hash("")
                };
                self.line_hashes[row] = vh;
                self.prev_text[row] = None; // unknown until painted
            }
        } else {
            // Scroll up: new lines entering at top.
            let entering = abs;
            // Shift existing reused lines down (iterate from bottom to avoid overwrite).
            for i in (0..(visible_rows - entering)).rev() {
                let dst = i + entering;
                self.line_hashes[dst] = self.line_hashes[i];
                self.prev_text[dst] = self.prev_text[i].take();
            }
            // Recompute top entering line hashes.
            for i in 0..entering {
                let buf_line_index = new_first + i;
                let vh = if let Some(raw_line) = buf_line_provider(buf_line_index) {
                    let content_trim: &str = raw_line.trim_end_matches(['\n', '\r']);
                    PartialCache::compute_hash(content_trim)
                } else {
                    PartialCache::compute_hash("")
                };
                self.line_hashes[i] = vh;
                self.prev_text[i] = None;
            }
        }
        self.viewport_start = new_first;
        // last_cursor_line left unchanged; render path will update after overlay.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_hash_changes_on_content() {
        let a = PartialCache::compute_hash("hello");
        let b = PartialCache::compute_hash("hello world");
        assert_ne!(a, b, "different content must produce different (hash,len)");
    }

    #[test]
    fn reset_and_push_sequence() {
        let mut c = PartialCache::new();
        c.reset(10, 120, 5);
        c.push_line(PartialCache::compute_hash("alpha"));
        c.push_line(PartialCache::compute_hash("beta"));
        assert_eq!(c.viewport_start, 10);
        assert_eq!(c.width, 120);
        assert_eq!(c.line_hashes.len(), 2);
        assert_eq!(c.prev_text.len(), 2);
        assert!(c.get(1).is_some());
    }
}
