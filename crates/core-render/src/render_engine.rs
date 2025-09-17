//! RenderEngine abstraction (Refactor R2 Steps 1-2): isolates frame building and renderer
//! dispatch. Step 2 separates content assembly from cursor/status overlay and stores prior
//! cursor span metadata (no behavioral change yet).

use crate::partial_cache::PartialCache;
use crate::partial_diff::classify_viewport_changes;
use crate::partial_metrics::{RenderPathMetrics, RenderPathMetricsSnapshot};
use crate::writer::Writer;
use crate::{CellFlags, Frame};
use anyhow::Result;
use core_model::View;
use core_state::EditorState;
use core_text::grapheme;

/// Metadata describing the last cursor span painted (for future minimal invalidation logic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorSpanMeta {
    pub line: Option<usize>,
    pub start_col: Option<u16>,
    pub width: Option<u16>,
}

/// Public facade used by the binary to produce a frame from state and flush it to the terminal.
pub struct RenderEngine {
    last_cursor: CursorSpanMeta,
    cache: PartialCache,
    metrics: RenderPathMetrics,
}

impl Default for RenderEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderEngine {
    pub fn new() -> Self {
        Self {
            last_cursor: CursorSpanMeta::default(),
            cache: PartialCache::new(),
            metrics: RenderPathMetrics::default(),
        }
    }

    /// Build + render a full frame (current behavior; breadth-first guarantee).
    pub fn render_full(&mut self, state: &EditorState, view: &View, w: u16, h: u16) -> Result<()> {
        let start = std::time::Instant::now();
        // Step 5: classify hash differences (still full frame output). We run this
        // before building the frame so the hashing path always executes each frame.
        classify_viewport_changes(state, view, w, h, &mut self.cache, &self.metrics, None);

        let mut frame = build_content_frame(state, view, w, h);
        self.apply_cursor_overlay(state, view, &mut frame, w, h);
        apply_status_line(state, view, &mut frame, w, h);
        // Phase 3 Step 6: translate Frame into writer commands (still full repaint)
        let res = self.render_via_writer(&frame);
        // Update last cursor line in cache.
        self.cache.last_cursor_line = Some(view.cursor.line);
        let dur = start.elapsed().as_nanos() as u64;
        self.metrics
            .full_frames
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.metrics
            .last_full_render_ns
            .store(dur, std::sync::atomic::Ordering::Relaxed);
        res
    }

    /// Placeholder for future partial rendering path (Phase 3).
    pub fn render_partial(
        &mut self,
        state: &EditorState,
        view: &View,
        w: u16,
        h: u16,
    ) -> Result<()> {
        // Step 7 currently only activates for CursorOnly effective decisions.
        // Fallback to full for any other future kinds until Step 8 extends logic.
        self.render_full(state, view, w, h)
    }

    /// Phase 3 Step 7: cursor-only partial repaint. Repaints just the previous cursor line
    /// (if still visible and different) plus the new cursor line. Status line unchanged.
    pub fn render_cursor_only(
        &mut self,
        state: &EditorState,
        view: &View,
        w: u16,
        h: u16,
    ) -> Result<()> {
        let start_time = std::time::Instant::now();
        if h == 0 {
            return Ok(());
        }
        let text_height = h - 1; // reserve status line
        let buf = state.active_buffer();
        let viewport_first = view.viewport_first_line;
        let viewport_last_excl = viewport_first + text_height as usize;
        let mut writer = Writer::new();

        // Determine previous & current cursor lines.
        let prev_line_opt = self.cache.last_cursor_line;
        let curr_line = view.cursor.line;

        // Helper to paint a single buffer line at given buffer index if in viewport.
        let mut paint_line = |buf_line: usize| {
            if buf_line < viewport_first || buf_line >= viewport_last_excl {
                return; // not visible
            }
            let rel_y = (buf_line - viewport_first) as u16;
            writer.move_to(0, rel_y);
            writer.clear_line(0, rel_y);
            if let Some(raw_line) = buf.line(buf_line) {
                let content_trim: &str = if raw_line.ends_with(['\n', '\r']) {
                    &raw_line[..raw_line.len() - 1]
                } else {
                    raw_line.as_str()
                };
                // Walk grapheme clusters identical to build_content_frame (duplication acceptable for now; Step 8 may refactor).
                let mut byte = 0;
                let mut vis_col = 0u16;
                while byte < content_trim.len() && vis_col < w {
                    let next = grapheme::next_boundary(content_trim, byte);
                    let cluster = &content_trim[byte..next];
                    let width = grapheme::cluster_width(cluster) as u16;
                    let mut chars = cluster.chars();
                    if let Some(first) = chars.next() {
                        writer.print(first.to_string());
                    }
                    if width > 1 {
                        for _ in 1..width {
                            if vis_col + 1 >= w {
                                break;
                            }
                            writer.print(" ");
                            vis_col = vis_col.saturating_add(1);
                        }
                    }
                    vis_col = vis_col.saturating_add(width.max(1));
                    byte = next;
                }
            }
        };

        // Repaint previous cursor line first if distinct.
        if let Some(prev) = prev_line_opt
            && prev != curr_line
        {
            paint_line(prev);
        }
        paint_line(curr_line);

        // After printing lines, we must overlay cursor cluster (reverse video) on current line.
        // Reuse overlay logic by building a temporary mini Frame row representing current line
        // then emitting styled cells. Simpler: recompute cluster & append ANSI styled sequence.
        // Approach: MoveTo cursor visual start again and write styled cluster.
        if curr_line >= viewport_first
            && curr_line < viewport_last_excl
            && let Some(line_content) = buf.line(curr_line)
        {
            let content_trim: &str = if line_content.ends_with(['\n', '\r']) {
                &line_content[..line_content.len() - 1]
            } else {
                line_content.as_str()
            };
            let vis_col = grapheme::visual_col(content_trim, view.cursor.byte) as u16;
            if vis_col < w {
                let next_byte = grapheme::next_boundary(content_trim, view.cursor.byte);
                let cluster = &content_trim[view.cursor.byte..next_byte];
                let width = grapheme::cluster_width(cluster).max(1) as u16;
                writer.move_to(vis_col, (curr_line - viewport_first) as u16);
                // Styled first cell
                let mut chars = cluster.chars();
                if let Some(first) = chars.next() {
                    let mut s = String::from("\x1b[7m");
                    s.push(first);
                    s.push_str("\x1b[0m");
                    writer.print(s);
                } else {
                    let s = String::from("\x1b[7m \x1b[0m");
                    writer.print(s);
                }
                // Fill remainder cells (wide cluster) with reversed spaces
                for dx in 1..width {
                    if vis_col + dx >= w {
                        break;
                    }
                    writer.move_to(vis_col + dx, (curr_line - viewport_first) as u16);
                    writer.print("\x1b[7m \x1b[0m");
                }
            }
        }

        writer.flush()?;
        // Update metrics
        let dur = start_time.elapsed().as_nanos() as u64;
        use std::sync::atomic::Ordering::Relaxed;
        self.metrics.partial_frames.fetch_add(1, Relaxed);
        self.metrics.cursor_only_frames.fetch_add(1, Relaxed);
        self.metrics.last_partial_render_ns.store(dur, Relaxed);
        // Update last cursor line cache for next cursor-only diff.
        self.cache.last_cursor_line = Some(curr_line);
        Ok(())
    }

    /// Access a snapshot of current metrics (for tests and future status integration).
    pub fn metrics_snapshot(&self) -> RenderPathMetricsSnapshot {
        self.metrics.snapshot()
    }

    /// Expose last cursor line cached (Phase 3 Step 4 test hook).
    pub fn last_cursor_line(&self) -> Option<usize> {
        self.cache.last_cursor_line
    }

    fn apply_cursor_overlay(
        &mut self,
        state: &EditorState,
        view: &View,
        frame: &mut Frame,
        w: u16,
        h: u16,
    ) {
        if h == 0 {
            return;
        }
        let text_height = h - 1;
        let buf = state.active_buffer();
        let start = view.viewport_first_line;
        let text_rows = text_height as usize;
        let mut meta = CursorSpanMeta::default();
        if view.cursor.line >= start
            && view.cursor.line < buf.line_count()
            && let Some(line_content) = buf.line(view.cursor.line)
        {
            let visible_end = start + text_rows;
            if view.cursor.line < visible_end {
                let content_trim: &str = if line_content.ends_with('\n') {
                    &line_content[..line_content.len() - 1]
                } else {
                    &line_content
                };
                let vis_col = grapheme::visual_col(content_trim, view.cursor.byte);
                let next_byte = core_text::grapheme::next_boundary(content_trim, view.cursor.byte);
                let cluster = &content_trim[view.cursor.byte..next_byte];
                let width = grapheme::cluster_width(cluster).max(1) as u16;
                let rel_line = (view.cursor.line - start) as u16;
                meta.line = Some(view.cursor.line);
                meta.start_col = Some(vis_col as u16);
                meta.width = Some(width);
                if (vis_col as u16) < w {
                    let mut chars = cluster.chars();
                    let first_char = chars.next().unwrap_or(' ');
                    frame.set_with_flags(
                        vis_col as u16,
                        rel_line,
                        first_char,
                        CellFlags::REVERSE | CellFlags::CURSOR,
                    );
                    for fill_dx in 1..width {
                        let col = vis_col as u16 + fill_dx;
                        if col >= w {
                            break;
                        }
                        frame.set_with_flags(
                            col,
                            rel_line,
                            ' ',
                            CellFlags::REVERSE | CellFlags::CURSOR,
                        );
                    }
                }
            }
        }
        self.last_cursor = meta;
    }

    /// Temporary full-frame translation using Writer (Step 6). Keeps behavior identical
    /// to legacy Renderer::render while establishing the abstraction for later partial path.
    fn render_via_writer(&self, frame: &Frame) -> Result<()> {
        // Step 6.1 hotfix: row-major iteration with explicit MoveTo at each row start
        // eliminates reliance on terminal implicit wrap behavior (prevents content
        // bleeding into subsequent rows or the status line). Slightly more commands
        // (one MoveTo per row) but guarantees alignment correctness.
        let mut writer = Writer::new();
        for y in 0..frame.height {
            writer.move_to(0, y);
            let row_start = y as usize * frame.width as usize;
            let row_end = row_start + frame.width as usize;
            for cell in &frame.cells[row_start..row_end] {
                if cell.flags.contains(CellFlags::REVERSE) {
                    let mut s = String::from("\x1b[7m");
                    s.push(cell.ch);
                    s.push_str("\x1b[0m");
                    writer.print(s);
                } else {
                    writer.print(cell.ch.to_string());
                }
            }
        }
        writer.flush()
    }
}

/// Test-only helper: build full frame (content + cursor + status) without emitting to terminal.
#[cfg(test)]
pub fn build_full_frame_for_test(state: &EditorState, view: &View, w: u16, h: u16) -> Frame {
    let mut eng = RenderEngine::new();
    let mut frame = build_content_frame(state, view, w, h);
    eng.apply_cursor_overlay(state, view, &mut frame, w, h);
    apply_status_line(state, view, &mut frame, w, h);
    frame
}

/// Build only the content (text lines) portion of the frame; no cursor or status decorations.
pub fn build_content_frame(state: &EditorState, view: &View, w: u16, h: u16) -> Frame {
    let mut frame = Frame::new(w, h);
    let text_height = if h > 0 { h - 1 } else { 0 };
    let buf = state.active_buffer();
    let start = view.viewport_first_line;
    let height = text_height as usize;
    let end = (start + height).min(buf.line_count());
    for (screen_y, line_idx) in (start..end).enumerate() {
        if (screen_y as u16) >= text_height {
            break;
        }
        if let Some(line) = buf.line(line_idx) {
            let content_trim: &str = if line.ends_with('\n') || line.ends_with('\r') {
                &line[..line.len() - 1]
            } else {
                &line
            };
            let mut byte = 0;
            let mut vis_col = 0u16;
            while byte < content_trim.len() && vis_col < w {
                let next = core_text::grapheme::next_boundary(content_trim, byte);
                let cluster = &content_trim[byte..next];
                let width = grapheme::cluster_width(cluster) as u16;
                let mut chars = cluster.chars();
                if let Some(first) = chars.next() {
                    frame.set(vis_col, screen_y as u16, first);
                }
                if width > 1 {
                    for dx in 1..width {
                        if vis_col + dx < w {
                            frame.set(vis_col + dx, screen_y as u16, ' ');
                        }
                    }
                }
                vis_col = vis_col.saturating_add(width.max(1));
                byte = next;
            }
        }
    }
    frame
}

fn apply_status_line(state: &EditorState, view: &View, frame: &mut Frame, w: u16, h: u16) {
    if h == 0 {
        return;
    }
    let y = h - 1;
    let buf = state.active_buffer();
    let line_content = buf.line(view.cursor.line).unwrap_or_default();
    let content_trim: &str = if line_content.ends_with("\r\n") {
        &line_content[..line_content.len() - 2]
    } else if line_content.ends_with('\n') || line_content.ends_with('\r') {
        &line_content[..line_content.len() - 1]
    } else {
        &line_content
    };
    let col = grapheme::visual_col(content_trim, view.cursor.byte);
    let status = crate::status::build_status(&crate::status::StatusContext {
        mode: state.mode,
        line: view.cursor.line,
        col,
        command_active: state.command_line.is_active(),
        command_buffer: state.command_line.buffer(),
        file_name: state.file_name.as_deref(),
        dirty: state.dirty,
    });
    for (i, ch) in status.chars().enumerate() {
        if (i as u16) < w {
            frame.set(i as u16, y, ch);
        }
    }
    if !state.command_line.is_active()
        && let Some(msg) = &state.ephemeral_status
    {
        let text = &msg.text;
        let msg_len = text.chars().count() as u16;
        if msg_len < w {
            let start_col = w - msg_len;
            for (i, ch) in text.chars().enumerate() {
                let col2 = start_col + i as u16;
                if col2 < w {
                    frame.set(col2, y, ch);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::EditorModel;
    use core_state::EditorState;
    use core_text::Buffer;

    fn mk_state(text: &str) -> EditorModel {
        let st = EditorState::new(Buffer::from_str("test", text).unwrap());
        EditorModel::new(st)
    }

    #[test]
    fn last_cursor_line_updates_across_frames() {
        let mut model = mk_state("a\nb\nc\n");
        let mut eng = RenderEngine::new();
        let view = model.active_view().clone();
        eng.render_full(model.state(), &view, 80, 10).unwrap();
        assert_eq!(eng.last_cursor_line(), Some(0));
        // Move cursor
        {
            let view_mut = model.active_view_mut();
            view_mut.cursor.line = 2;
        }
        let view_after = model.active_view().clone();
        eng.render_full(model.state(), &view_after, 80, 10).unwrap();
        assert_eq!(eng.last_cursor_line(), Some(2));
    }

    #[test]
    fn metrics_full_frames_increment() {
        let model = mk_state("x\n");
        let mut eng = RenderEngine::new();
        let view = model.active_view().clone();
        eng.render_full(model.state(), &view, 40, 5).unwrap();
        eng.render_full(model.state(), &view, 40, 5).unwrap();
        let snap = eng.metrics_snapshot();
        assert_eq!(snap.full_frames, 2);
        assert!(snap.last_full_render_ns > 0);
    }

    #[test]
    fn cursor_only_partial_metrics_and_cache() {
        let model = mk_state("a\nb\nc\n");
        let mut eng = RenderEngine::new();
        let view0 = model.active_view().clone();
        eng.render_full(model.state(), &view0, 80, 6).unwrap();
        assert_eq!(eng.last_cursor_line(), Some(0));
        // Move cursor to line 2 and perform cursor-only partial render.
        let mut view_move = view0.clone();
        view_move.cursor.line = 2;
        eng.render_cursor_only(model.state(), &view_move, 80, 6)
            .unwrap();
        let snap = eng.metrics_snapshot();
        assert_eq!(snap.full_frames, 1, "only initial full frame counted");
        assert_eq!(snap.partial_frames, 1, "one partial frame executed");
        assert_eq!(snap.cursor_only_frames, 1, "cursor-only frame counted");
        assert!(snap.last_partial_render_ns > 0);
        assert_eq!(eng.last_cursor_line(), Some(2));
    }
}
