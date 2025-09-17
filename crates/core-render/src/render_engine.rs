//! RenderEngine abstraction (Refactor R2 Steps 1-2): isolates frame building and renderer
//! dispatch. Step 2 separates content assembly from cursor/status overlay and stores prior
//! cursor span metadata (no behavioral change yet).

use crate::{CellFlags, Frame, Renderer};
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
        }
    }

    /// Build + render a full frame (current behavior; breadth-first guarantee).
    pub fn render_full(&mut self, state: &EditorState, view: &View, w: u16, h: u16) -> Result<()> {
        let mut frame = build_content_frame(state, view, w, h);
        self.apply_cursor_overlay(state, view, &mut frame, w, h);
        apply_status_line(state, view, &mut frame, w, h);
        Renderer::render(&frame)
    }

    /// Placeholder for future partial rendering path (Phase 3).
    pub fn render_partial(
        &mut self,
        state: &EditorState,
        view: &View,
        w: u16,
        h: u16,
    ) -> Result<()> {
        self.render_full(state, view, w, h)
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
    use core_text::Buffer;

    fn mk_model(initial: &str) -> (EditorState, View) {
        let st = EditorState::new(Buffer::from_str("test", initial).unwrap());
        let view = core_model::View::new(
            core_model::ViewId(0),
            st.active,
            core_text::Position::origin(),
            0,
        );
        (st, view)
    }

    #[test]
    fn cursor_ascii_single_width() {
        let (state, mut view) = mk_model("abc");
        view.cursor.line = 0;
        view.cursor.byte = 1; // 'b'
        let frame = build_full_frame_for_test(&state, &view, 20, 4);
        let idx = 1;
        let cell = frame.cells[idx];
        assert_eq!(cell.ch, 'b');
        assert!(cell.flags.contains(CellFlags::CURSOR));
        assert!(cell.flags.contains(CellFlags::REVERSE));
    }
}
