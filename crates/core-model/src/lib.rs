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
        // Seed single view at origin (cursor + viewport) since state no longer owns them.
        let v = View::new(ViewId(0), state.active, Position::origin(), 0);
        Self {
            state,
            views: vec![v],
            active_view_index: 0,
        }
    }
    /// Test/helper constructor allowing an already prepared view (cursor/viewport) to be injected.
    pub fn with_view(mut view: View, state: EditorState) -> Self {
        // Ensure view buffer index aligns with state's active buffer for Phase 3 single-buffer assumption.
        view.buffer_index = state.active; // enforce consistency
        Self {
            state,
            views: vec![view],
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

impl View {
    /// Auto-scroll this view to keep the cursor within the vertical viewport.
    /// Returns true if the first visible line changed. Updates state's last_text_height.
    pub fn auto_scroll(&mut self, state: &mut EditorState, text_height: usize) -> bool {
        if text_height == 0 {
            return false;
        }
        state.last_text_height = text_height; // record for page motions
        let cursor_line = self.cursor.line;
        let m = state.config_vertical_margin.min(text_height / 2); // defensive clamp
        let mut changed = false;
        let top = self.viewport_first_line;
        let bottom = self.viewport_first_line + text_height;
        if cursor_line < top + m {
            let new_first = cursor_line.saturating_sub(m);
            if new_first != self.viewport_first_line {
                self.viewport_first_line = new_first;
                changed = true;
            }
        } else if cursor_line + m >= bottom {
            let new_first = cursor_line + m + 1 - text_height;
            if new_first != self.viewport_first_line {
                self.viewport_first_line = new_first;
                changed = true;
            }
        }
        changed
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

    fn mk(text: &str) -> (EditorState, View) {
        let st = EditorState::new(Buffer::from_str("test", text).unwrap());
        let view = View::new(ViewId(0), st.active, core_text::Position::origin(), 0);
        (st, view)
    }

    #[test]
    fn auto_scroll_down_and_up() {
        let (mut st, mut v) = mk("0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n");
        let h = 5usize;
        // line 0 already visible, no scroll
        assert!(!v.auto_scroll(&mut st, h));
        v.cursor.line = 4; // still inside 0..5
        assert!(!v.auto_scroll(&mut st, h));
        v.cursor.line = 5; // triggers scroll to first=1
        assert!(v.auto_scroll(&mut st, h));
        assert_eq!(v.viewport_first_line, 1);
        v.cursor.line = 9; // bottom -> new_first = 9 +1 -5 =5
        assert!(v.auto_scroll(&mut st, h));
        assert_eq!(v.viewport_first_line, 5);
        v.cursor.line = 3; // above first -> clamp to 3
        assert!(v.auto_scroll(&mut st, h));
        assert_eq!(v.viewport_first_line, 3);
    }

    #[test]
    fn auto_scroll_with_zero_margin_matches_baseline() {
        let (mut st, mut v) = mk("0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n");
        st.config_vertical_margin = 0;
        v.cursor.line = 5;
        v.auto_scroll(&mut st, 5);
        assert_eq!(v.viewport_first_line, 1);
    }

    #[test]
    fn auto_scroll_with_margin_scrolls_earlier() {
        let (mut st, mut v) = mk("0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n");
        st.config_vertical_margin = 2;
        let h = 6usize;
        v.cursor.line = 4; // triggers early scroll because bottom margin violated
        v.auto_scroll(&mut st, h);
        assert_eq!(v.viewport_first_line, 1);
        v.cursor.line = 5; // subsequent scroll maintains margin -> new_first = 5 +2 +1 -6 =2
        v.auto_scroll(&mut st, h);
        assert_eq!(v.viewport_first_line, 2);
    }

    #[test]
    fn auto_scroll_margin_bottom_boundary() {
        let (mut st, mut v) = mk("0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n");
        st.config_vertical_margin = 2;
        v.cursor.line = 9;
        v.auto_scroll(&mut st, 5); // m = min(2, 2) =2 => 9+2+1-5 =7
        assert_eq!(v.viewport_first_line, 7);
    }

    #[test]
    fn auto_scroll_margin_small_viewport_disables_excess_margin() {
        let (mut st, mut v) = mk("0\n1\n2\n3\n4\n");
        st.config_vertical_margin = 10; // will clamp to h/2
        v.cursor.line = 2;
        v.auto_scroll(&mut st, 3); // h/2=1 -> 2+1+1-3=1
        assert_eq!(v.viewport_first_line, 1);
    }
}
