//! Editor state (Phase 0 minimal)

use core_text::Buffer;

/// Current editor mode. Only `Normal` exists in Phase 0.
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    /// Normal command/navigation mode.
    Normal,
}

/// Top-level editor state container (single-buffer in Phase 0).
pub struct EditorState {
    /// All loaded buffers (Phase 0: exactly one).
    pub buffers: Vec<Buffer>,
    /// Index into `buffers` of the active buffer.
    pub active: usize,
    /// Current editor mode.
    pub mode: Mode,
}

impl EditorState {
    /// Create a new state with a single active buffer.
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffers: vec![buffer],
            active: 0,
            mode: Mode::Normal,
        }
    }

    /// Borrow the currently active buffer.
    pub fn active_buffer(&self) -> &Buffer {
        &self.buffers[self.active]
    }
}
