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
}

impl PartialCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset cache to represent a new viewport slice (caller supplies vector capacity hint).
    pub fn reset(&mut self, viewport_start: usize, width: u16, expected_lines: usize) {
        self.viewport_start = viewport_start;
        self.width = width;
        self.line_hashes.clear();
        if self.line_hashes.capacity() < expected_lines {
            self.line_hashes
                .reserve(expected_lines - self.line_hashes.capacity());
        }
    }

    /// Push a hash entry for a line (in viewport order).
    pub fn push_line(&mut self, entry: ViewportLineHash) {
        self.line_hashes.push(entry);
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
        assert!(c.get(1).is_some());
    }
}
