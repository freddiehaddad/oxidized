//! Rope-based text buffer abstraction.

use anyhow::Result;
use ropey::Rope;

/// A text buffer backed by a `ropey::Rope`.
pub struct Buffer {
    rope: Rope,
    pub name: String,
}

/// A position inside a buffer expressed as (line index, byte offset within that line).
/// Lines and byte offsets are guaranteed (when clamped) to be on UTF-8 code unit boundaries; grapheme
/// safety is enforced by higher-level navigation (Phase 1 motions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub byte: usize,
}

impl Position {
    pub fn new(line: usize, byte: usize) -> Self {
        Self { line, byte }
    }
    pub fn origin() -> Self {
        Self { line: 0, byte: 0 }
    }

    /// Clamp this position to the provided line count and byte length accessor for the current line.
    pub fn clamp_to<F>(&mut self, line_count: usize, mut line_len_fn: F)
    where
        F: FnMut(usize) -> usize,
    {
        if line_count == 0 {
            self.line = 0;
            self.byte = 0;
            return;
        }
        if self.line >= line_count {
            self.line = line_count - 1;
        }
        let max_len = line_len_fn(self.line);
        if self.byte > max_len {
            self.byte = max_len;
        }
    }
}

impl Buffer {
    /// Construct a buffer from an in-memory string slice.
    pub fn from_str(name: impl Into<String>, content: &str) -> Result<Self> {
        Ok(Self {
            rope: Rope::from_str(content),
            name: name.into(),
        })
    }

    /// Total number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    /// Return the requested line as an owned `String` (including trailing newline if present).
    pub fn line(&self, idx: usize) -> Option<String> {
        if idx < self.rope.len_lines() {
            Some(self.rope.line(idx).to_string())
        } else {
            None
        }
    }

    /// Byte length of a line (excluding any newline) for clamping purposes.
    pub fn line_byte_len(&self, idx: usize) -> usize {
        if idx >= self.rope.len_lines() {
            return 0;
        }
        let line = self.rope.line(idx);
        // ropey lines include the trailing newline except possibly the last line.
        let s = line.to_string();
        if s.ends_with('\n') {
            s.len() - 1
        } else {
            s.len()
        }
    }
}

/// Grapheme and width utilities (Phase 1). These are pure helpers operating on a single line.
pub mod grapheme {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    /// Iterate grapheme clusters in a line.
    pub fn iter(line: &str) -> impl Iterator<Item = &str> {
        line.graphemes(true)
    }

    /// Previous grapheme boundary (returns 0 if already at or below 1st boundary).
    pub fn prev_boundary(line: &str, byte: usize) -> usize {
        if byte == 0 || byte > line.len() {
            return 0;
        }
        let mut last = 0;
        for (idx, _) in line.grapheme_indices(true) {
            if idx >= byte {
                break;
            }
            last = idx;
        }
        last
    }

    /// Next grapheme boundary (returns line.len() if at or beyond end).
    pub fn next_boundary(line: &str, byte: usize) -> usize {
        if byte >= line.len() {
            return line.len();
        }
        for (idx, _) in line.grapheme_indices(true) {
            if idx > byte {
                return idx;
            }
        }
        line.len()
    }

    /// Compute visual column (terminal cells) up to (but not including) byte offset.
    pub fn visual_col(line: &str, byte: usize) -> usize {
        let mut col = 0;
        for (idx, g) in line.grapheme_indices(true) {
            if idx >= byte {
                break;
            }
            col += g.width();
        }
        col
    }

    /// Width in terminal cells of this grapheme cluster.
    pub fn cluster_width(g: &str) -> usize {
        g.width()
    }

    /// Naive word classification: alphanumeric or underscore start.
    pub fn is_word(g: &str) -> bool {
        g.chars()
            .next()
            .map(|c| c == '_' || c.is_alphanumeric())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::grapheme;
    use super::*;
    #[test]
    fn create_buffer_and_read_line() {
        let b = Buffer::from_str("test", "hello\nworld").unwrap();
        assert_eq!(b.line_count(), 2);
        assert_eq!(b.line(0).unwrap(), "hello\n");
        assert_eq!(b.line(1).unwrap(), "world");
    }

    #[test]
    fn grapheme_basic_emoji() {
        let s = "a😀b"; // 😀 is single cluster width 2 usually
        let bytes_a = 0;
        let bytes_emoji = grapheme::next_boundary(s, grapheme::next_boundary(s, bytes_a));
        // Ensure prev/next boundaries align
        assert_eq!(
            grapheme::prev_boundary(s, bytes_emoji),
            grapheme::next_boundary(s, bytes_a)
        );
        let vcol_emoji = grapheme::visual_col(s, bytes_emoji);
        assert!(vcol_emoji >= 1); // At least after 'a'
    }

    #[test]
    fn grapheme_family_emoji() {
        let s = "👨‍👩‍👧‍👦X"; // family emoji + ASCII
        // family emoji should count width >= 2 (exact width may vary by terminal, accept >=1)
        let next = grapheme::next_boundary(s, 0);
        assert!(next <= s.len());
        let col = grapheme::visual_col(s, next);
        assert!(col >= 1);
    }

    #[test]
    fn grapheme_combining_mark() {
        let s = "é"; // 'e' + U+0301 combining acute
        let nb = grapheme::next_boundary(s, 0);
        assert_eq!(nb, s.len()); // should be single cluster
        assert_eq!(grapheme::prev_boundary(s, nb), 0);
    }

    #[test]
    fn grapheme_cjk() {
        let s = "漢字"; // two CJK characters
        let first = grapheme::next_boundary(s, 0);
        let second = grapheme::next_boundary(s, first);
        assert!(second <= s.len());
        assert_eq!(grapheme::prev_boundary(s, second), first);
    }
}
