//! Rope-based text buffer abstraction.

use anyhow::Result;
use ropey::Rope;

/// A text buffer backed by a `ropey::Rope`.
#[derive(Clone)]
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

pub mod motion;
pub mod segment;
pub mod width; // Step 4.1: unified grapheme width indirection
#[cfg(feature = "term-probe")]
pub mod width_probe; // Step 4.4: runtime terminal probe scaffold // Step 4: centralized normalization + segmentation adapter

// Re-export primary width function for convenience in callers that already depend on core-text.
pub use width::egc_width;

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

    fn line_content_string(&self, idx: usize) -> String {
        let mut s = self.rope.line(idx).to_string();
        if s.ends_with('\n') {
            s.pop();
        }
        s
    }

    fn absolute_byte_index(&self, pos: &Position) -> usize {
        // Sum bytes of prior lines + byte within line. For early phase simplicity we derive via ropey APIs.
        // ropey lacks direct line_to_byte historically; compute using char indices.
        let line_start_char = self.rope.line_to_char(pos.line);
        let line_start_byte = self.rope.char_to_byte(line_start_char);
        line_start_byte + pos.byte
    }

    fn byte_to_char_index(&self, line: usize, byte_in_line: usize) -> usize {
        let line_start_char = self.rope.line_to_char(line);
        let line_str = self.rope.line(line).to_string();
        let mut trimmed = line_str.as_str();
        if trimmed.ends_with('\n') {
            trimmed = &trimmed[..trimmed.len() - 1];
        }
        // Slice up to byte_in_line (assumed valid grapheme boundary / char boundary) and count chars.
        let within = &trimmed[..byte_in_line];
        line_start_char + within.chars().count()
    }

    /// Insert a grapheme cluster string (may be multi-byte) at the given position; advances position by its byte length.
    pub fn insert_grapheme(&mut self, pos: &mut Position, g: &str) {
        let char_index = self.byte_to_char_index(pos.line, pos.byte);
        self.rope.insert(char_index, g);
        pos.byte += g.len();
    }

    /// Insert a newline at the given position, splitting the current line. Cursor moves to start of new line.
    pub fn insert_newline(&mut self, pos: &mut Position) {
        let char_index = self.byte_to_char_index(pos.line, pos.byte);
        self.rope.insert(char_index, "\n");
        pos.line += 1;
        pos.byte = 0;
    }

    /// Delete the grapheme cluster before the position (like backspace). If at start of line and not first line, joins with previous.
    pub fn delete_grapheme_before(&mut self, pos: &mut Position) {
        if pos.line == 0 && pos.byte == 0 {
            return;
        }
        if pos.byte == 0 {
            // join with previous line: remove the newline at end of previous line
            let prev_line = pos.line - 1;
            let prev_len = self.line_byte_len(prev_line);
            // absolute byte of newline char (after prev_len)
            let line_start_char_prev = self.rope.line_to_char(prev_line);
            let prev_line_start_byte = self.rope.char_to_byte(line_start_char_prev);
            let newline_byte = prev_line_start_byte + prev_len; // the '\n'
            let newline_char_index = self.rope.byte_to_char(newline_byte);
            self.rope.remove(newline_char_index..newline_char_index + 1);
            pos.line = prev_line;
            pos.byte = prev_len;
            return;
        }
        let line_str = self.line_content_string(pos.line);
        let prev = grapheme::prev_boundary(&line_str, pos.byte);
        if prev == pos.byte {
            return;
        }
        let abs_start = self.absolute_byte_index(&Position {
            line: pos.line,
            byte: prev,
        });
        let abs_end = self.absolute_byte_index(pos);
        let start_char = self.rope.byte_to_char(abs_start);
        let end_char = self.rope.byte_to_char(abs_end);
        self.rope.remove(start_char..end_char);
        pos.byte = prev;
    }

    /// Delete the grapheme cluster at the position (like Normal mode 'x'). No-op if at line end.
    pub fn delete_grapheme_at(&mut self, pos: &mut Position) {
        let line_len = self.line_byte_len(pos.line);
        if pos.byte >= line_len {
            return;
        }
        let line_str = self.line_content_string(pos.line);
        let next = grapheme::next_boundary(&line_str, pos.byte);
        if next == pos.byte {
            return;
        }
        let abs_start = self.absolute_byte_index(pos);
        let abs_end = self.absolute_byte_index(&Position {
            line: pos.line,
            byte: next,
        });
        let start_char = self.rope.byte_to_char(abs_start);
        let end_char = self.rope.byte_to_char(abs_end);
        self.rope.remove(start_char..end_char);
        // Position stays at same byte (now pointing at next cluster or EOL)
    }

    /// Return the UTF-8 slice in the absolute byte range `[start,end)`.
    /// Caller guarantees `start <= end` and both on character boundaries.
    /// (Motion span resolver ensures grapheme boundaries which imply char boundaries.)
    pub fn slice_bytes(&self, start: usize, end: usize) -> String {
        if start >= end {
            return String::new();
        }
        let total = self.rope.len_bytes();
        let s = start.min(total);
        let e = end.min(total);
        if s >= e {
            return String::new();
        }
        // Translate byte offsets to char indices (rope.slice expects char range)
        let start_char = self.rope.byte_to_char(s);
        let end_char = self.rope.byte_to_char(e);
        debug_assert_eq!(self.rope.char_to_byte(start_char), s);
        debug_assert_eq!(self.rope.char_to_byte(end_char), e);
        self.rope.slice(start_char..end_char).to_string()
    }

    /// Delete the UTF-8 slice in absolute byte range `[start,end)` (clamped).
    /// Returns the removed text for register / undo integration.
    pub fn delete_bytes(&mut self, start: usize, end: usize) -> String {
        if start >= end {
            return String::new();
        }
        let total = self.rope.len_bytes();
        let s = start.min(total);
        let e = end.min(total);
        if s >= e {
            return String::new();
        }
        let start_char = self.rope.byte_to_char(s);
        let end_char = self.rope.byte_to_char(e);
        debug_assert_eq!(self.rope.char_to_byte(start_char), s);
        debug_assert_eq!(self.rope.char_to_byte(end_char), e);
        let removed = self.rope.slice(start_char..end_char).to_string();
        self.rope.remove(start_char..end_char);
        removed
    }
}

/// Grapheme and width utilities (Phase 1). These are pure helpers operating on a single line.
pub mod grapheme {
    use crate::egc_width;
    use unicode_segmentation::UnicodeSegmentation; // unified width function

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
            col += egc_width(g) as usize;
        }
        col
    }

    /// Width in terminal cells of this grapheme cluster.
    pub fn cluster_width(g: &str) -> usize {
        egc_width(g) as usize
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
    use unicode_segmentation::UnicodeSegmentation;
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

    #[test]
    fn visual_col_mixed_sequences() {
        // Sequence: ASCII, emoji, combining mark, CJK, family emoji, ASCII
        let s = "a😀é漢字👨‍👩‍👧‍👦Z"; // note combining sequence e + ◌́
        // Walk boundaries and ensure visual_col is non-decreasing and >= byte index of prior ASCII assumption
        let mut b = 0;
        let mut last_col = 0;
        while b < s.len() {
            let next = grapheme::next_boundary(s, b);
            let col = grapheme::visual_col(s, next);
            assert!(col >= last_col, "visual column must be non-decreasing");
            last_col = col;
            b = next;
        }
        // Final column should be at least number of distinct clusters
        // (Width may exceed cluster count due to wide glyphs; just assert lower bound)
        // Count clusters
        let clusters = s.graphemes(true).count();
        assert!(last_col >= clusters - 1);
    }

    #[test]
    fn visual_col_gear_variation_selector() {
        // Ensure gear emoji (with VS16) currently treated as width 1 under temporary override.
        let s = "a⚙️b"; // a, gear+VS16, b
        // byte offsets
        let after_a = grapheme::next_boundary(s, 0);
        let after_gear = grapheme::next_boundary(s, after_a);
        assert_eq!(grapheme::visual_col(s, after_a), 1);
        // Expect gear adds 1 column (temporary narrowing override)
        assert_eq!(grapheme::visual_col(s, after_gear), 2);
    }

    #[test]
    fn insert_grapheme_middle() {
        let mut b = Buffer::from_str("t", "abc").unwrap();
        let mut pos = Position::new(0, 1); // after 'a'
        b.insert_grapheme(&mut pos, "😀");
        let line = b.line(0).unwrap();
        assert!(line.starts_with("a"));
        assert!(line.contains("😀"));
        assert_eq!(pos.byte, 1 + "😀".len());
    }

    #[test]
    fn insert_newline_split() {
        let mut b = Buffer::from_str("t", "abcd").unwrap();
        let mut pos = Position::new(0, 2);
        b.insert_newline(&mut pos);
        assert_eq!(b.line_count(), 2);
        assert_eq!(b.line(0).unwrap(), "ab\n");
        assert_eq!(b.line(1).unwrap(), "cd");
        assert_eq!(pos.line, 1);
        assert_eq!(pos.byte, 0);
    }

    #[test]
    fn delete_grapheme_before_simple() {
        let mut b = Buffer::from_str("t", "ab😀c").unwrap();
        let mut pos = Position::new(0, b.line_byte_len(0));
        b.delete_grapheme_before(&mut pos); // remove 'c'
        b.delete_grapheme_before(&mut pos); // remove emoji cluster
        let line = b.line(0).unwrap();
        assert_eq!(line, "ab");
        assert_eq!(pos.byte, 2);
    }

    #[test]
    fn delete_grapheme_before_join_lines() {
        let mut b = Buffer::from_str("t", "ab\ncd").unwrap();
        let mut pos = Position::new(1, 0); // start of second line
        b.delete_grapheme_before(&mut pos); // should join lines
        assert_eq!(b.line_count(), 1);
        let line = b.line(0).unwrap();
        assert_eq!(line, "abcd");
        assert_eq!(pos.line, 0);
        assert_eq!(pos.byte, 2); // end of original first line
    }

    #[test]
    fn delete_grapheme_at_end_noop() {
        let mut b = Buffer::from_str("t", "hi").unwrap();
        let mut pos = Position::new(0, 2); // at end
        b.delete_grapheme_at(&mut pos); // no-op
        assert_eq!(b.line(0).unwrap(), "hi");
        assert_eq!(pos.byte, 2);
    }
}
