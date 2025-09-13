//! Editor state: buffer collection, mode, and caret position (Cursor moved to core-text as Position).

use core_text::{Buffer, Position};

/// Current editor mode.
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    /// Normal command/navigation mode.
    Normal,
    /// Insert text mode (appends / inserts grapheme clusters into the active buffer).
    Insert,
}

/// Top-level editor state container (single-buffer in Phase 0).
pub struct EditorState {
    /// All loaded buffers (Phase 0: exactly one).
    pub buffers: Vec<Buffer>,
    /// Index into `buffers` of the active buffer.
    pub active: usize,
    /// Current editor mode.
    pub mode: Mode,
    /// Primary caret position (grapheme boundary) within active buffer.
    pub position: Position,
}

impl EditorState {
    /// Create a new state with a single active buffer.
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffers: vec![buffer],
            active: 0,
            mode: Mode::Normal,
            position: Position::origin(),
        }
    }

    /// Borrow the currently active buffer.
    pub fn active_buffer(&self) -> &Buffer {
        &self.buffers[self.active]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_text::Buffer;

    #[test]
    fn cursor_initializes_at_origin() {
        let buf = Buffer::from_str("test", "Hello").unwrap();
        let st = EditorState::new(buf);
        assert_eq!(st.position.line, 0);
        assert_eq!(st.position.byte, 0);
        assert!(matches!(st.mode, Mode::Normal));
    }

    #[test]
    fn cursor_clamp() {
        let buf = Buffer::from_str("test", "Hello\nWorld").unwrap();
        let mut st = EditorState::new(buf);
        st.position.line = 10; // beyond
        st.position.byte = 999;
        let line_count = st.active_buffer().line_count();
        let last_len = st.active_buffer().line_byte_len(line_count - 1);
        // Provide a closure that does not borrow `st` to satisfy borrow checker.
        st.position.clamp_to(line_count, |_| last_len);
        assert_eq!(st.position.line, line_count - 1); // last valid line index
        assert_eq!(st.position.byte, last_len);
    }
}
