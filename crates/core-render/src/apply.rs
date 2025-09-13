use crate::dirty::DirtyLinesTracker;
use crate::render_engine::RenderEngine;
use anyhow::Result;
use core_model::{Layout, View};
use core_state::EditorState;

/// Shared render context describing the state needed to emit a frame.
///
/// The snapshot deliberately borrows the editor model/view instead of
/// cloning so `apply_*` entry points stay lightweight and cache-friendly.
#[derive(Clone, Copy)]
pub struct FrameSnapshot<'a> {
    pub state: &'a EditorState,
    pub view: &'a View,
    pub layout: &'a Layout,
    pub width: u16,
    pub height: u16,
    pub status_line: &'a str,
}

impl<'a> FrameSnapshot<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: &'a EditorState,
        view: &'a View,
        layout: &'a Layout,
        width: u16,
        height: u16,
        status_line: &'a str,
    ) -> Self {
        Self {
            state,
            view,
            layout,
            width,
            height,
            status_line,
        }
    }
}

/// Cursor-only repaint context.
pub struct CursorOnlyFrame<'a> {
    pub snapshot: FrameSnapshot<'a>,
}

impl<'a> CursorOnlyFrame<'a> {
    pub fn new(snapshot: FrameSnapshot<'a>) -> Self {
        Self { snapshot }
    }
}

/// Lines-partial repaint context with a dirty tracker snapshot.
pub struct LinesPartialFrame<'a> {
    pub snapshot: FrameSnapshot<'a>,
    pub dirty: &'a mut DirtyLinesTracker,
}

impl<'a> LinesPartialFrame<'a> {
    pub fn new(snapshot: FrameSnapshot<'a>, dirty: &'a mut DirtyLinesTracker) -> Self {
        Self { snapshot, dirty }
    }
}

/// Scroll-region shift repaint context.
pub struct ScrollShiftFrame<'a> {
    pub snapshot: FrameSnapshot<'a>,
    pub old_first: usize,
    pub new_first: usize,
}

impl<'a> ScrollShiftFrame<'a> {
    pub fn new(snapshot: FrameSnapshot<'a>, old_first: usize, new_first: usize) -> Self {
        Self {
            snapshot,
            old_first,
            new_first,
        }
    }
}

pub fn apply_cursor_only(engine: &mut RenderEngine, frame: CursorOnlyFrame<'_>) -> Result<()> {
    let CursorOnlyFrame { snapshot } = frame;
    let FrameSnapshot {
        state,
        view,
        layout,
        width,
        height,
        status_line,
    } = snapshot;
    engine.render_cursor_only(state, view, layout, width, height, status_line)
}

pub fn apply_lines_partial(engine: &mut RenderEngine, frame: LinesPartialFrame<'_>) -> Result<()> {
    let LinesPartialFrame { snapshot, dirty } = frame;
    let FrameSnapshot {
        state,
        view,
        layout,
        width,
        height,
        status_line,
    } = snapshot;
    engine.render_lines_partial(state, view, layout, width, height, dirty, status_line)
}

pub fn apply_scroll_shift(engine: &mut RenderEngine, frame: ScrollShiftFrame<'_>) -> Result<()> {
    let ScrollShiftFrame {
        snapshot,
        old_first,
        new_first,
    } = frame;
    let FrameSnapshot {
        state,
        view,
        layout,
        width,
        height,
        status_line,
    } = snapshot;
    engine.render_scroll_shift(
        state,
        view,
        layout,
        width,
        height,
        old_first,
        new_first,
        status_line,
    )
}

pub fn apply_full(engine: &mut RenderEngine, snapshot: FrameSnapshot<'_>) -> Result<()> {
    let FrameSnapshot {
        state,
        view,
        layout,
        width,
        height,
        status_line,
    } = snapshot;
    engine.render_full(state, view, layout, width, height, status_line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dirty::DirtyLinesTracker;
    use crate::render_engine::{RenderEngine, build_status_line_with_ephemeral};
    use core_model::EditorModel;
    use core_model::Layout;
    use core_text::{Buffer, Position};

    #[test]
    fn apply_entry_points_delegate_to_engine() {
        let buffer = Buffer::from_str("apply", "line0\nline1\nline2\nline3\n").expect("buffer");
        let mut model = EditorModel::new(EditorState::new(buffer));
        let layout = Layout::single(20, 6);
        let mut engine = RenderEngine::new();

        let mut status_line =
            build_status_line_with_ephemeral(model.state(), model.active_view(), 20);
        let snapshot = FrameSnapshot::new(
            model.state(),
            model.active_view(),
            &layout,
            20,
            6,
            &status_line,
        );
        apply_full(&mut engine, snapshot).expect("full render");
        let metrics = engine.metrics_snapshot();
        assert_eq!(metrics.full_frames, 1);

        status_line = build_status_line_with_ephemeral(model.state(), model.active_view(), 20);
        let cursor_snapshot = FrameSnapshot::new(
            model.state(),
            model.active_view(),
            &layout,
            20,
            6,
            &status_line,
        );
        apply_cursor_only(&mut engine, CursorOnlyFrame::new(cursor_snapshot)).expect("cursor-only");
        let metrics = engine.metrics_snapshot();
        assert_eq!(metrics.cursor_only_frames, 1);

        status_line = build_status_line_with_ephemeral(model.state(), model.active_view(), 20);
        let mut tracker = DirtyLinesTracker::new();
        tracker.mark(model.active_view().cursor.line);
        let lines_snapshot = FrameSnapshot::new(
            model.state(),
            model.active_view(),
            &layout,
            20,
            6,
            &status_line,
        );
        apply_lines_partial(
            &mut engine,
            LinesPartialFrame::new(lines_snapshot, &mut tracker),
        )
        .expect("lines partial");
        let metrics = engine.metrics_snapshot();
        assert_eq!(metrics.lines_frames, 1);

        {
            let view = model.active_view_mut();
            view.viewport_first_line = 1;
            view.cursor = Position::new(1, 0);
        }
        status_line = build_status_line_with_ephemeral(model.state(), model.active_view(), 20);
        let scroll_snapshot = FrameSnapshot::new(
            model.state(),
            model.active_view(),
            &layout,
            20,
            6,
            &status_line,
        );
        apply_scroll_shift(&mut engine, ScrollShiftFrame::new(scroll_snapshot, 0, 1))
            .expect("scroll shift");
        let metrics = engine.metrics_snapshot();
        assert_eq!(metrics.scroll_region_shifts, 1);
    }
}
