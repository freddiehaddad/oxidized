//! High-level editor model wrapper (Refactor R2 Step 6).
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

pub struct EditorModel {
    state: EditorState,
}

impl EditorModel {
    pub fn new(state: EditorState) -> Self {
        Self { state }
    }
    pub fn state(&self) -> &EditorState {
        &self.state
    }
    pub fn state_mut(&mut self) -> &mut EditorState {
        &mut self.state
    }
}

// Re-export selected types for caller convenience (temporary breadth-first convenience)
pub use core_state::{LineEnding, Mode};
