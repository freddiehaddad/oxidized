//! High-level editor model (Phase 3 multi-view scaffolding).
//!
//! Phase 3 established the first cut of *multi-view aware* architecture without
//! yet rendering more than one view. This file now carries the authoritative
//! rustdoc (Step 12) describing invariants and forward expansion points so that
//! later phases (true splits, per-view status bars, independent buffers) can
//! evolve with high confidence. Keeping this documentation co-located with the
//! code enforces the "single source of truth" design tenet.
//!
//! Why a `View` type?
//! ------------------
//! A `View` owns presentation state that was previously embedded in
//! `EditorState` (cursor, `viewport_first_line`). Extracting it:
//! * Decouples buffer editing semantics from viewport & focus concerns.
//! * Prevents a future retrofitting exercise when adding splits.
//! * Makes per-view policies (scroll margins, future horizontal offsets, fold
//!   state, local options) naturally local.
//!
//! Breadth-first constraints (Phase 3 / Step 12):
//! * Exactly one active view (index 0) is created; APIs intentionally *look* as
//!   if N views could exist (Vec storage) while mutators are private/minimal.
//! * No rendering fan-out yet: renderer receives only the active view; `Layout`
//!   currently wraps a single region but signatures accept a layout reference
//!   so multi-region traversal can remain a local change.
//! * Undo/redo still operate at buffer granularity; per-view granularity (e.g.
//!   independent unrelated buffers visible simultaneously) is deferred.
//!
//! Core invariants (must hold after every public call):
//! * `views` is never empty.
//! * `active_view_index < views.len()`.
//! * `views[i].buffer_index` always names an existing buffer inside
//!   `EditorState` (`EditorState` is the source of truth for buffer storage).
//! * Active view's cursor line is always a valid line index for its buffer
//!   except transiently inside mutation helpers before re-clamp.
//! * Auto-scroll never produces a negative / overflow first line; it clamps to
//!   valid range based on the last known text height.
//!
//! Forward roadmap (deferred beyond Phase 3):
//! * Multiple simultaneously rendered views (grid / stacked layout) with a
//!   layout manager component; render engine will iterate `LayoutRegion`s and
//!   schedule per-region partial decisions gated by terminal capabilities.
//! * Per-view status line (potentially condensed global + local segments).
//! * Buffer-focus changes as first-class events producing semantic `RenderDelta`.
//! * View close/open life-cycle with undo isolation (per-buffer or per-view
//!   stacks depending on chosen UX).
//! * Horizontal scrolling & pending fold state added to `View`.
//! * Persistent view identity for layout restoration across sessions.
//!
//! Safety notes:
//! * All mutation APIs remain breadth-first minimal; they will likely become a
//!   `ViewManager` facade once external commands can create/destroy splits.
//! * We deliberately avoid exposing interior `views: &mut Vec<View>` to keep
//!   invariants centralized.
//!
//! Testing strategy additions (Step 12):
//! * Existing tests assert auto-scroll behavior & single-view initialization.
//! * Future steps will add tests around view creation/focus once those APIs
//!   land; documenting intent now avoids speculative APIs.
//!
//! Non-goals for Phase 3:
//! * Rendering of more than one view.
//! * Per-view configuration overrides (options table).
//! * Cross-view diffing or synchronized scrolling.
//!
//! Updating this doc is REQUIRED when adding any new field to `View` or any
//! new invariant affecting view lifecycle. (Enforced by code review checklist.)

use core_state::EditorState;
use core_text::Position;
mod layout;
pub use layout::{Layout, LayoutRegion};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Stable identifier for a `View`.
///
/// Breadth-first placeholder: currently just wraps the index. In later phases
/// we may adopt a generational scheme (u32 generation + u32 slot) to allow
/// safe reuse after view closure without accidental stale references.
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
