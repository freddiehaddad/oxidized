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

/// Manager responsible for owning and manipulating the collection of `View`
/// instances. Step 7 introduces this indirection so later phases can extend
/// view lifecycle (create / close / focus) without re‑plumbing every call site
/// that today reaches directly into the `EditorModel` fields. Breadth‑first the
/// manager still enforces a single view invariant.
#[derive(Debug)]
pub struct ViewManager {
    views: Vec<View>,
    active: usize,
}

impl ViewManager {
    pub fn new_single(initial: View) -> Self {
        Self {
            views: vec![initial],
            active: 0,
        }
    }
    pub fn active_view(&self) -> &View {
        debug_assert!(!self.views.is_empty(), "at least one view must exist");
        debug_assert!(self.active < self.views.len(), "active index in range");
        &self.views[self.active]
    }
    pub fn active_view_mut(&mut self) -> &mut View {
        debug_assert!(!self.views.is_empty(), "at least one view must exist");
        debug_assert!(self.active < self.views.len(), "active index in range");
        &mut self.views[self.active]
    }
    pub fn views(&self) -> &[View] {
        &self.views
    }
    fn active_index(&self) -> usize {
        self.active
    }
}

pub struct EditorModel {
    state: EditorState,
    view_mgr: ViewManager,
}

impl EditorModel {
    pub fn new(state: EditorState) -> Self {
        // Seed single view at origin (cursor + viewport) since state no longer owns them.
        let v = View::new(ViewId(0), state.active, Position::origin(), 0);
        Self {
            state,
            view_mgr: ViewManager::new_single(v),
        }
    }
    /// Test/helper constructor allowing an already prepared view (cursor/viewport) to be injected.
    pub fn with_view(mut view: View, state: EditorState) -> Self {
        // Ensure view buffer index aligns with state's active buffer for Phase 3 single-buffer assumption.
        view.buffer_index = state.active; // enforce consistency
        Self {
            state,
            view_mgr: ViewManager::new_single(view),
        }
    }
    pub fn state(&self) -> &EditorState {
        &self.state
    }
    pub fn state_mut(&mut self) -> &mut EditorState {
        &mut self.state
    }

    pub fn active_view(&self) -> &View {
        debug_assert_eq!(
            self.view_mgr.views.len(),
            1,
            "single-view invariant (Phase 3)"
        );
        self.view_mgr.active_view()
    }
    pub fn active_view_mut(&mut self) -> &mut View {
        debug_assert_eq!(
            self.view_mgr.views.len(),
            1,
            "single-view invariant (Phase 3)"
        );
        self.view_mgr.active_view_mut()
    }
    pub fn views(&self) -> &[View] {
        self.view_mgr.views()
    }

    /// Safely obtain mutable references to the underlying `EditorState` and the
    /// currently active `View` in a single call without resorting to raw pointer
    /// casts. This replaces prior patterns that created a raw `*mut EditorState`
    /// then separately borrowed the active view, which relied on careful reasoning
    /// about non-aliasing. The layout (`EditorModel { state, views, .. }`) permits
    /// returning disjoint mutable references because `state` and any element of
    /// `views` occupy distinct, non-overlapping fields. Rust cannot express this
    /// directly today without unsafe, so we perform an internal split using raw
    /// parts but keep the unsafety encapsulated here.
    pub fn split_state_and_active_view(&mut self) -> (&mut EditorState, &mut View) {
        // SAFETY: `self.state` and `self.views` are distinct fields. We take a
        // raw pointer to self.state, then create a mutable borrow into the
        // views slice for the active view. No aliasing occurs because no other
        // &mut to state or the active view is alive when this function returns.
        // The returned references have identical lifetime tied to &mut self.
        debug_assert!(
            !self.view_mgr.views.is_empty(),
            "at least one view must exist"
        );
        debug_assert!(
            self.view_mgr.active < self.view_mgr.views.len(),
            "active index in range"
        );
        let state_ptr: *mut EditorState = &mut self.state;
        // Obtain raw pointer to start of views Vec then offset by active index to avoid borrow checker conflict.
        let base_ptr = self.view_mgr.views.as_mut_ptr();
        let idx = self.view_mgr.active_index();
        let view_ptr = unsafe { base_ptr.add(idx) };
        unsafe { (&mut *state_ptr, &mut *view_ptr) }
    }
}

/// Compute the desired new first visible line to keep the cursor within the
/// vertical viewport subject to a top/bottom margin.
///
/// Inputs:
/// - first: current first visible line (top of viewport)
/// - cursor_line: current cursor line (0-based)
/// - text_height: number of text rows available (excludes status/overlay)
/// - margin: desired margin in rows (will be conservatively clamped to at most text_height/2)
///
/// Returns Some(new_first) if a scroll is needed, else None when the cursor is
/// already within the permitted band. The computation never underflows and will
/// not produce negative values.
pub fn compute_scroll_intent(
    first: usize,
    cursor_line: usize,
    text_height: usize,
    margin: usize,
) -> Option<usize> {
    if text_height == 0 {
        return None;
    }
    let m = margin.min(text_height / 2);
    let top = first;
    let bottom = first + text_height;
    if cursor_line < top + m {
        let new_first = cursor_line.saturating_sub(m);
        if new_first != first {
            return Some(new_first);
        }
    } else if cursor_line + m >= bottom {
        let new_first = cursor_line + m + 1 - text_height;
        if new_first != first {
            return Some(new_first);
        }
    }
    None
}

impl View {
    /// Auto-scroll this view to keep the cursor within the vertical viewport.
    /// Returns true if the first visible line changed. Updates state's last_text_height.
    pub fn auto_scroll(&mut self, state: &mut EditorState, text_height: usize) -> bool {
        if text_height == 0 {
            return false;
        }
        // Invariants (debug-only): single-buffer Phase 3 assumption and cursor validity
        debug_assert_eq!(
            self.buffer_index, state.active,
            "active view must point at active buffer"
        );
        let buf = state.active_buffer();
        debug_assert!(
            self.cursor.line < buf.line_count(),
            "cursor line within buffer"
        );
        if let Some(line) = buf.line(self.cursor.line) {
            let trimmed = line.trim_end_matches(['\n', '\r']);
            debug_assert!(
                self.cursor.byte <= trimmed.len(),
                "cursor byte within line bounds"
            );
        }
        state.last_text_height = text_height; // record for page motions
        let maybe_new = compute_scroll_intent(
            self.viewport_first_line,
            self.cursor.line,
            text_height,
            state.config_vertical_margin,
        );
        if let Some(new_first) = maybe_new {
            self.viewport_first_line = new_first;
            true
        } else {
            false
        }
    }
}

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

    #[test]
    fn view_manager_parity_active_access() {
        let st = EditorState::new(Buffer::from_str("t", "abc\n").unwrap());
        let mut model = EditorModel::new(st);
        // mutate via active_view_mut and ensure underlying state updates
        {
            let v = model.active_view_mut();
            v.cursor.line = 0;
            v.cursor.byte = 1; // move to 'b'
        }
        assert_eq!(model.active_view().cursor.byte, 1);
        // split borrow still yields same view pointer semantics
        let (state, view) = model.split_state_and_active_view();
        assert_eq!(state.active, view.buffer_index);
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

    #[test]
    fn compute_scroll_intent_basic_noop_when_inside_band() {
        // first=0, cursor within [m, h-m) should not scroll
        let first = 0usize;
        let h = 10usize;
        let m = 2usize;
        let cursor_line = 5usize;
        assert_eq!(compute_scroll_intent(first, cursor_line, h, m), None);
    }

    #[test]
    fn compute_scroll_intent_scrolls_up_when_above_top_margin() {
        let first = 10usize;
        let h = 6usize;
        let m = 2usize;
        // top band is [first, first+m) => [10,12); inside band should scroll up to maintain margin
        assert_eq!(compute_scroll_intent(first, 11, h, m), Some(9)); // 11 - 2
        assert_eq!(compute_scroll_intent(first, 10, h, m), Some(8)); // 10 - m
        assert_eq!(compute_scroll_intent(first, 9, h, m), Some(7));
    }

    #[test]
    fn compute_scroll_intent_scrolls_down_when_below_bottom_margin() {
        let first = 0usize;
        let h = 5usize;
        let m = 1usize;
        // bottom bound is first+h=5 => band bottom starts at 4 (inclusive)
        assert_eq!(compute_scroll_intent(first, 3, h, m), None);
        // At 4 with m=1: 4+1 >= 5 => new_first = 4+1+1-5 = 1
        assert_eq!(compute_scroll_intent(first, 4, h, m), Some(1));
        assert_eq!(compute_scroll_intent(1, 4, h, m), None);
    }

    #[test]
    fn compute_scroll_intent_clamps_margin_to_half_height() {
        let first = 0usize;
        let h = 4usize;
        let m = 10usize; // will clamp to 2
        // With clamp m=2, bottom threshold is cursor+2 >= 4 -> at cursor 2 triggers scroll
        assert_eq!(compute_scroll_intent(first, 2, h, m), Some(1)); // 2+2+1-4=1
    }
}
