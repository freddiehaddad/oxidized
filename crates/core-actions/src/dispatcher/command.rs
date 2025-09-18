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

fn execute_command(cmd: String, state: &mut EditorState, view: &mut View) -> DispatchResult {
    if cmd == ":q" {
        return DispatchResult::quit();
    }
    if let Some(rest) = cmd.strip_prefix(":e ") {
        return handle_edit_command(rest, state, view);
    }
    if cmd == ":w" {
        return handle_write_command(state);
    }
    state.command_line.clear();
    DispatchResult::dirty()
}

fn handle_edit_command(rest: &str, state: &mut EditorState, view: &mut View) -> DispatchResult {
    let path_str = rest.trim();
    if !path_str.is_empty() {
        let path = std::path::PathBuf::from(path_str);
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
                state.command_line.clear();
                return DispatchResult::buffer_replaced();
            }
            OpenFileResult::Error => {
                state.set_ephemeral("Open failed", std::time::Duration::from_secs(3));
            }
        }
    }
    state.command_line.clear();
    DispatchResult::dirty()
}

fn handle_write_command(state: &mut EditorState) -> DispatchResult {
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
    state.command_line.clear();
    DispatchResult::dirty()
}
