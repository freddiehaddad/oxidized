//! RenderEngine abstraction (Refactor R2 Step 1): isolates frame building and renderer dispatch.
//!
//! For Phase 2 + Refactor R2 this still performs full-frame builds only. A `render_partial` stub
//! is provided so Phase 3 can introduce diff/segment application without touching call sites.

use crate::{CellFlags, Frame, Renderer};
use anyhow::Result;
use core_state::EditorState;
use core_text::grapheme;

/// Public facade used by the binary to produce a frame from state and flush it to the terminal.
pub struct RenderEngine;

impl RenderEngine {
    /// Build + render a full frame (current behavior; breadth-first guarantee).
    pub fn render_full(state: &EditorState, w: u16, h: u16) -> Result<()> {
        let frame = build_frame(state, w, h);
        Renderer::render(&frame)
    }

    /// Placeholder for future partial rendering path (Phase 3). For now this simply delegates
    /// to `render_full` so callers can unify invocation signatures later without behavioral change.
    #[allow(unused_variables)]
    pub fn render_partial(state: &EditorState, w: u16, h: u16) -> Result<()> {
        // Future: accept a diff structure and apply minimal updates.
        Self::render_full(state, w, h)
    }
}

/// Build a `Frame` representing the current editor state (pure, side-effect free).
/// Moved from `ox-bin` (Refactor R2 Step 1).
pub fn build_frame(state: &EditorState, w: u16, h: u16) -> Frame {
    let mut frame = Frame::new(w, h);
    // Viewport: use persistent first line from state.
    let text_height = if h > 0 { h - 1 } else { 0 };
    let buf = state.active_buffer();
    let start = state.viewport_first_line;
    let height = text_height as usize; // visible text rows
    let end = (start + height).min(buf.line_count());
    for (screen_y, line_idx) in (start..end).enumerate() {
        if (screen_y as u16) >= text_height {
            break;
        }
        if let Some(line) = buf.line(line_idx) {
            // Trim raw terminator for cluster iteration
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
                let width = grapheme::cluster_width(cluster) as u16; // 1 or 2 typical
                let mut chars = cluster.chars();
                if let Some(first) = chars.next() {
                    frame.set(vis_col, screen_y as u16, first);
                }
                if width > 1 {
                    // wide emoji fill
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
    // Software cursor overlay (reverse-video) for cluster under cursor, excluding status line.
    if h > 0 {
        let text_rows = text_height as usize;
        if state.position.line >= start
            && state.position.line < end
            && let Some(line_content) = buf.line(state.position.line)
        {
            let content_trim: &str = if line_content.ends_with('\n') {
                &line_content[..line_content.len() - 1]
            } else {
                &line_content
            };
            let vis_col = grapheme::visual_col(content_trim, state.position.byte);
            let next_byte = core_text::grapheme::next_boundary(content_trim, state.position.byte);
            let cluster = &content_trim[state.position.byte..next_byte];
            let width = grapheme::cluster_width(cluster);
            let rel_line = state.position.line - start;
            if rel_line < text_rows {
                let span_width = width.max(1);
                let mut chars = cluster.chars();
                let first_char = chars.next().unwrap_or(' ');
                if (vis_col as u16) < w {
                    frame.set_with_flags(
                        vis_col as u16,
                        rel_line as u16,
                        first_char,
                        CellFlags::REVERSE | CellFlags::CURSOR,
                    );
                }
                for fill_dx in 1..span_width {
                    let col = vis_col + fill_dx;
                    if col as u16 >= w {
                        break;
                    }
                    frame.set_with_flags(
                        col as u16,
                        rel_line as u16,
                        ' ',
                        CellFlags::REVERSE | CellFlags::CURSOR,
                    );
                }
            }
        }
    }
    // Status line (bottom row)
    if h > 0 {
        let y = h - 1;
        let buf = state.active_buffer();
        let line_content = buf.line(state.position.line).unwrap_or_default();
        let content_trim: &str = if line_content.ends_with("\r\n") {
            &line_content[..line_content.len() - 2]
        } else if line_content.ends_with('\n') || line_content.ends_with('\r') {
            &line_content[..line_content.len() - 1]
        } else {
            &line_content
        };
        let col = grapheme::visual_col(content_trim, state.position.byte);
        let status = crate::status::build_status(&crate::status::StatusContext {
            mode: state.mode,
            line: state.position.line,
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
                    let col = start_col + i as u16;
                    if col < w {
                        frame.set(col, y, ch);
                    }
                }
            }
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_text::Buffer;

    fn mk_state(initial: &str) -> EditorState {
        EditorState::new(Buffer::from_str("test", initial).unwrap())
    }

    #[test]
    fn cursor_ascii_single_width() {
        let mut state = mk_state("abc");
        state.position.line = 0;
        state.position.byte = 1; // 'b'
        let frame = build_frame(&state, 20, 4);
        let idx = 1;
        let cell = frame.cells[idx];
        assert_eq!(cell.ch, 'b');
        assert!(cell.flags.contains(CellFlags::CURSOR));
        assert!(cell.flags.contains(CellFlags::REVERSE));
    }
}
