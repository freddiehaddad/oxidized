//! Overlay module (Refactor R4 Step 13)
//!
//! Provides fixed-line metrics overlay rendering above the status line.
//! Breadth-first implementation: always repaints overlay rows each frame; no
//! diffing or dynamic sizing yet. Future enhancement (Dynamic overlay auto-sizing)
//! will introduce width-aware wrapping and per-field prioritization.

use crate::batch_writer::BatchWriter;
use core_state::{EditorState, OverlayMode};

/// Build overlay lines based on the current overlay mode.
/// For Metrics mode we emit up to `max_lines` lines summarizing operator and
/// render path metrics. Width is currently unused (future wrapping).
pub fn build_overlay_lines(state: &EditorState, width: u16) -> Vec<String> {
    let mode = state.overlay_mode();
    match mode {
        OverlayMode::None => Vec::new(),
        OverlayMode::Metrics { lines } => build_metrics_lines(state, width, lines as usize),
    }
}

/// Return overlay line count (cheap) for geometry budgeting (recomputes lines; small N).
pub fn overlay_line_count(state: &EditorState, width: u16) -> u16 {
    build_overlay_lines(state, width).len() as u16
}

/// Paint overlay rows (always dirty) into a BatchWriter for partial render paths.
/// Assumes caller already ensured `h > 0` and will paint status line afterwards.
pub fn paint_overlay_rows_batch(writer: &mut BatchWriter, state: &EditorState, w: u16, h: u16) {
    if h == 0 {
        return;
    }
    let lines = build_overlay_lines(state, w);
    let count = lines.len() as u16;
    if count == 0 || count >= h {
        return;
    }
    let first_row = h - 1 - count; // top overlay row
    for (i, line) in lines.iter().enumerate() {
        let y = first_row + i as u16;
        writer.move_to(0, y);
        writer.clear_line(0, y);
        let mut byte = 0usize;
        let mut x: u16 = 0;
        while byte < line.len() && x < w {
            let next = core_text::grapheme::next_boundary(line, byte);
            let cluster = &line[byte..next];
            let width = core_text::grapheme::cluster_width(cluster).max(1) as u16;
            writer.print(cluster.to_string());
            x = x.saturating_add(width);
            byte = next;
        }
    }
}

fn build_metrics_lines(state: &EditorState, _width: u16, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    if max == 0 {
        return out;
    }
    let op = state.operator_metrics_snapshot();
    out.push(format!(
        "ops d:{} y:{} c:{} reg_w:{} rot:{}",
        op.operator_delete,
        op.operator_yank,
        op.operator_change,
        op.register_writes,
        op.numbered_ring_rotations
    ));
    if out.len() >= max {
        return out;
    }
    if let Some(rp) = state.last_render_path {
        out.push(format!(
            "rp full:{} part:{} cur:{} lines:{} dirty:{} cand:{} rep:{} cells:{} statSkip:{}",
            rp.full_frames,
            rp.partial_frames,
            rp.cursor_only_frames,
            rp.lines_frames,
            rp.dirty_lines_marked,
            rp.dirty_candidate_lines,
            rp.dirty_lines_repainted,
            rp.cells_printed,
            rp.status_skipped
        ));
    } else {
        out.push("rp <none>".to_string());
    }
    if out.len() >= max {
        return out;
    }
    if let Some(rd) = state.last_render_delta {
        out.push(format!(
            "delta f:{} l:{} sc:{} st:{} cur:{} sem:{}",
            rd.full, rd.lines, rd.scroll, rd.status_line, rd.cursor_only, rd.semantic_frames
        ));
    }
    if out.len() > max {
        out.truncate(max);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_text::Buffer;

    #[test]
    fn metrics_overlay_empty_when_none() {
        let st = core_state::EditorState::new(Buffer::from_str("t", "a\n").unwrap());
        assert!(build_overlay_lines(&st, 80).is_empty());
    }

    #[test]
    fn metrics_overlay_populates() {
        let mut st = core_state::EditorState::new(Buffer::from_str("t", "a\n").unwrap());
        st.toggle_metrics_overlay(core_state::METRICS_OVERLAY_DEFAULT_LINES);
        let lines = build_overlay_lines(&st, 80);
        assert!(!lines.is_empty());
        assert!(lines[0].starts_with("ops"));
    }
}
