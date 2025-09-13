//! Rope-based text buffer abstraction.

use anyhow::Result;
use ropey::Rope;

/// A text buffer backed by a `ropey::Rope`.
pub struct Buffer {
    rope: Rope,
    pub name: String,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_buffer_and_read_line() {
        let b = Buffer::from_str("test", "hello\nworld").unwrap();
        assert_eq!(b.line_count(), 2);
        assert_eq!(b.line(0).unwrap(), "hello\n");
        assert_eq!(b.line(1).unwrap(), "world");
    }
}
