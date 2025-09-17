//! High-level editor model wrapper (Refactor R2 Step 6 -> Phase 3 multi-view scaffolding).
//!
//! Breadth-first: this is a thin newtype over `EditorState` providing a
//! stable surface to hang multi-view/split abstractions in Phase 3 without
//! rewriting existing call sites again. It intentionally exposes only the
//! minimal API currently exercised by `ox-bin`.
//!
//! Invariants:
//! * No behavioral changes vs direct `EditorState` usage.
//! * Methods are simple passthroughs; zero additional allocations.
//! * Future: manage a collection of views, active buffer routing, focus.

use core_state::EditorState;
use core_text::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewId(pub usize);

#[derive(Debug, Clone)]
pub struct View {
    pub id: ViewId,
    pub buffer_index: usize,
    pub cursor: Position,
    pub viewport_first_line: usize,
}

impl View {
    pub fn new(
        id: ViewId,
        buffer_index: usize,
        cursor: Position,
        viewport_first_line: usize,
    ) -> Self {
        Self {
            id,
            buffer_index,
            cursor,
            viewport_first_line,
        }
    }
}

pub struct EditorModel {
    state: EditorState,
    views: Vec<View>,
    active_view_index: usize,
}

impl EditorModel {
    pub fn new(state: EditorState) -> Self {
        // Seed single view from initial state values (will migrate ownership in later sub-steps).
        let v = View::new(
            ViewId(0),
            state.active,
            state.position,
            state.viewport_first_line,
        );
        Self {
            state,
            views: vec![v],
            active_view_index: 0,
        }
    }
    pub fn state(&self) -> &EditorState {
        &self.state
    }
    pub fn state_mut(&mut self) -> &mut EditorState {
        &mut self.state
    }

    pub fn active_view(&self) -> &View {
        &self.views[self.active_view_index]
    }
    pub fn active_view_mut(&mut self) -> &mut View {
        &mut self.views[self.active_view_index]
    }
    pub fn views(&self) -> &[View] {
        &self.views
    }
}

// Re-export selected types for caller convenience (temporary breadth-first convenience)
pub use core_state::{LineEnding, Mode};

#[cfg(test)]
mod tests {
    use super::*;
    use core_state::EditorState;
    use core_text::Buffer;

    #[test]
    fn single_view_initialized() {
        let st = EditorState::new(Buffer::from_str("test", "hello\n").unwrap());
        let model = EditorModel::new(st);
        let v = model.active_view();
        assert_eq!(v.id.0, 0);
        assert_eq!(v.buffer_index, 0);
        assert_eq!(v.viewport_first_line, 0);
    }
}
