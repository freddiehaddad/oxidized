//! Command line editing & execution (:q, :e <file>, :w).
//!
//! Scope (R3 Step 1): behavior-neutral extraction. Parsing is still string
//! prefix matching; Step 2 introduces a structured `ParsedCommand`.
//!
//! Design Tenet Alignment:
//! * Modularity: isolates ephemeral command line buffer mutations and side
//!   effects (file IO) from the main dispatcher control flow.
//! * Evolution Over Legacy: intentionally simple now; replacing the ad-hoc
//!   parsing with a real parser will be a local change.
//! * Documentation: rustdoc describes future surface to avoid speculative
//!   premature abstractions.
//!
//! Forward Roadmap:
//! * Structured parser returning `ParsedCommand` variants (Step 2).
//! * Additional commands (`:metrics`, `:config-reload`) attach cleanly.
//! * Async file IO (potential) via background task + event completion; this
//!   module will convert completion events into editor state transitions.
//! * Error surfacing improvements (detailed messages, echo area reuse).

use super::DispatchResult;
use super::command_parser::{CommandParser, ParsedCommand};
use crate::Action;
use crate::io_ops::{OpenFileResult, WriteFileResult, open_file, write_file};
use core_model::View;
use core_state::EditorState;
use core_text::Position;

pub(crate) fn handle_command_action(
    action: Action,
    state: &mut EditorState,
    view: &mut View,
) -> DispatchResult {
    match action {
        Action::CommandStart => {
            state.command_line.begin();
            DispatchResult::dirty()
        }
        Action::CommandChar(ch) => {
            state.command_line.push_char(ch);
            DispatchResult::dirty()
        }
        Action::CommandBackspace => {
            state.command_line.backspace();
            DispatchResult::dirty()
        }
        Action::CommandCancel => {
            state.command_line.clear();
            DispatchResult::dirty()
        }
        Action::CommandExecute(cmd) => execute_command(cmd, state, view),
        _ => unreachable!("non-command action routed to command handler"),
    }
}

fn execute_command(raw: String, state: &mut EditorState, view: &mut View) -> DispatchResult {
    let parsed = CommandParser::parse(&raw);
    let result = match parsed {
        ParsedCommand::Quit => DispatchResult::quit(),
        ParsedCommand::Write => handle_write(state),
        ParsedCommand::Edit(path) => handle_edit(path, state, view),
        ParsedCommand::Metrics => {
            let text = build_metrics_snapshot(state);
            // Structured log emission (Phase 4 Step 15 augmentation): full metrics to log file.
            if let Some(rp) = state.last_render_path {
                tracing::info!(
                    target: "metrics",
                    kind = ":metrics",
                    op_del = state.operator_metrics_snapshot().operator_delete,
                    op_yank = state.operator_metrics_snapshot().operator_yank,
                    op_change = state.operator_metrics_snapshot().operator_change,
                    reg_writes = state.operator_metrics_snapshot().register_writes,
                    reg_ring_rot = state.operator_metrics_snapshot().numbered_ring_rotations,
                    rp_full = rp.full_frames,
                    rp_partial = rp.partial_frames,
                    rp_cursor = rp.cursor_only_frames,
                    rp_lines = rp.lines_frames,
                    rp_dirty_marked = rp.dirty_lines_marked,
                    rp_dirty_cand = rp.dirty_candidate_lines,
                    rp_dirty_repainted = rp.dirty_lines_repainted,
                    rp_last_full_ns = rp.last_full_render_ns,
                    rp_last_partial_ns = rp.last_partial_render_ns,
                    rp_print_cmds = rp.print_commands,
                    rp_cells = rp.cells_printed,
                    rp_scroll_shifts = rp.scroll_region_shifts,
                    rp_scroll_saved = rp.scroll_region_lines_saved,
                    rp_scroll_degraded = rp.scroll_shift_degraded_full,
                    rp_trim_attempts = rp.trim_attempts,
                    rp_trim_success = rp.trim_success,
                    rp_trim_cols_saved = rp.cols_saved_total,
                    rp_status_skipped = rp.status_skipped,
                    sem_full = state.last_render_delta.map(|d| d.full).unwrap_or(0),
                    sem_lines = state.last_render_delta.map(|d| d.lines).unwrap_or(0),
                    sem_scroll = state.last_render_delta.map(|d| d.scroll).unwrap_or(0),
                    sem_status = state.last_render_delta.map(|d| d.status_line).unwrap_or(0),
                    sem_cursor = state.last_render_delta.map(|d| d.cursor_only).unwrap_or(0),
                    sem_collapsed_scroll = state.last_render_delta.map(|d| d.collapsed_scroll).unwrap_or(0),
                    sem_suppressed_scroll = state.last_render_delta.map(|d| d.suppressed_scroll).unwrap_or(0),
                    sem_frames = state.last_render_delta.map(|d| d.semantic_frames).unwrap_or(0),
                    "metrics_snapshot"
                );
            } else {
                tracing::info!(target: "metrics", kind=":metrics", rp="none", "metrics_snapshot_no_render_path");
            }
            // Longer TTL (5s) to allow user to read.
            state.set_ephemeral(text, std::time::Duration::from_secs(5));
            DispatchResult::dirty()
        }
        ParsedCommand::Unknown(_) => DispatchResult::dirty(),
    };
    state.command_line.clear();
    result
}

// Phase 4 Step 15: Build multi-line metrics snapshot string. Conservative breadth-first: fixed
// ordering, simple formatting, truncate to ~8 lines if extremely long (future overlay can paginate).
fn build_metrics_snapshot(state: &EditorState) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let op = state.operator_metrics_snapshot();
    let render_path = state.last_render_path;
    let render_delta = state.last_render_delta;
    writeln!(
        out,
        "Operators: del={} yank={} chg={}",
        op.operator_delete, op.operator_yank, op.operator_change
    )
    .ok();
    writeln!(
        out,
        "Registers: writes={} ring_rot={}",
        op.register_writes, op.numbered_ring_rotations
    )
    .ok();
    if let Some(rp) = render_path {
        writeln!(
            out,
            "RenderPath: full={} partial={} cursor={} lines={}",
            rp.full_frames, rp.partial_frames, rp.cursor_only_frames, rp.lines_frames
        )
        .ok();
        writeln!(
            out,
            "DirtyFunnel: marked={} cand={} repainted={}",
            rp.dirty_lines_marked, rp.dirty_candidate_lines, rp.dirty_lines_repainted
        )
        .ok();
        writeln!(
            out,
            "Scroll: shifts={} lines_saved={} degraded={}",
            rp.scroll_region_shifts, rp.scroll_region_lines_saved, rp.scroll_shift_degraded_full
        )
        .ok();
        writeln!(
            out,
            "Trim: attempts={} success={} cols_saved={}",
            rp.trim_attempts, rp.trim_success, rp.cols_saved_total
        )
        .ok();
        writeln!(out, "Status: skipped={}", rp.status_skipped).ok();
    } else {
        writeln!(out, "RenderPath: <no snapshot yet>").ok();
    }
    if let Some(rd) = render_delta {
        writeln!(
            out,
            "Semantic: full={} lines={} scroll={} status={} cursor={}",
            rd.full, rd.lines, rd.scroll, rd.status_line, rd.cursor_only
        )
        .ok();
    }
    // Truncate to first 10 lines for safety (ephemeral area each line consumed by status builds).
    let mut lines: Vec<&str> = out.lines().collect();
    if lines.len() > 10 {
        lines.truncate(10);
    }
    lines.join("\n")
}

fn handle_edit(
    path: std::path::PathBuf,
    state: &mut EditorState,
    view: &mut View,
) -> DispatchResult {
    match open_file(&path) {
        OpenFileResult::Success(s) => {
            state.buffers[state.active] = s.buffer;
            view.cursor = Position::origin();
            state.file_name = Some(s.file_name);
            state.dirty = false;
            state.original_line_ending = s.original_line_ending;
            state.had_trailing_newline = s.had_trailing_newline;
            state.set_ephemeral("Opened", std::time::Duration::from_secs(3));
            if s.mixed_line_endings {
                tracing::warn!("mixed_line_endings_detected");
            }
            DispatchResult::buffer_replaced()
        }
        OpenFileResult::Error => {
            state.set_ephemeral("Open failed", std::time::Duration::from_secs(3));
            DispatchResult::dirty()
        }
    }
}

fn handle_write(state: &mut EditorState) -> DispatchResult {
    match write_file(state) {
        WriteFileResult::Success => {
            state.set_ephemeral("Wrote", std::time::Duration::from_secs(3));
        }
        WriteFileResult::NoFilename => {
            tracing::error!("write_no_filename");
            state.set_ephemeral("No filename", std::time::Duration::from_secs(3));
        }
        WriteFileResult::Error => {
            state.set_ephemeral("Write failed", std::time::Duration::from_secs(3));
        }
    }
    DispatchResult::dirty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Action;
    use core_text::Buffer;

    // Helper to construct minimal editor state + view for command tests
    fn mk_state() -> (EditorState, core_model::View) {
        let st = EditorState::new(Buffer::from_str("test", "abc\n").unwrap());
        let view = core_model::View::new(core_model::ViewId(0), st.active, Position::origin(), 0);
        (st, view)
    }

    #[test]
    fn metrics_command_sets_ephemeral() {
        let (mut st, mut view) = mk_state();
        // Simulate entering command mode then executing :metrics
        let _ = handle_command_action(Action::CommandStart, &mut st, &mut view);
        let res = handle_command_action(
            Action::CommandExecute(":metrics".to_string()),
            &mut st,
            &mut view,
        );
        assert!(
            res.dirty,
            "metrics command should mark dirty for status repaint"
        );
        let eph = st.ephemeral_status.as_ref().expect("ephemeral status set");
        // Should contain at least the Operators and RenderPath headings (RenderPath snapshot may be absent on first run)
        assert!(
            eph.text.contains("Operators:"),
            "expected Operators line in metrics output: {}",
            eph.text
        );
        // Render path may not yet exist (no render executed) so we only assert Registers present
        assert!(
            eph.text.contains("Registers:"),
            "expected Registers line in metrics output: {}",
            eph.text
        );
        // Ensure numeric counters formatted (simple regex-like contains a digit)
        assert!(
            eph.text.chars().any(|c| c.is_ascii_digit()),
            "expected some digits in metrics output: {}",
            eph.text
        );
    }
}
