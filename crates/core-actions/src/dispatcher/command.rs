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
            use core_state::{METRICS_OVERLAY_DEFAULT_LINES, OverlayMode};
            let new_mode = state.toggle_metrics_overlay(METRICS_OVERLAY_DEFAULT_LINES);
            match new_mode {
                OverlayMode::Metrics { lines } => {
                    // Emit a concise one-line ephemeral so overlay rows remain the source of detail.
                    state.set_ephemeral(
                        format!("Metrics overlay ON ({} lines)", lines),
                        std::time::Duration::from_secs(2),
                    );
                }
                OverlayMode::None => {
                    state.set_ephemeral("Metrics overlay OFF", std::time::Duration::from_secs(2));
                }
            }
            // Structured log of toggle event for diagnostics.
            tracing::info!(target: "runtime.metrics", kind=":metrics_toggle", mode=?new_mode);
            DispatchResult::dirty()
        }
        ParsedCommand::Unknown(_) => DispatchResult::dirty(),
    };
    state.command_line.clear();
    result
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
                tracing::warn!(target: "io", "mixed_line_endings_detected");
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
            tracing::error!(target: "runtime.command", "write_no_filename");
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
        // New behavior: single-line toggle confirmation.
        assert!(
            eph.text.starts_with("Metrics overlay ON"),
            "expected overlay toggle confirmation, got: {}",
            eph.text
        );
    }
}
