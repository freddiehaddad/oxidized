//! Dispatcher applying `Action` to mutable editor state (Refactor R1 Step 5).
//!
//! Breadth-first extraction from `ox-bin/src/main.rs`. Behavior intentionally
//! unchanged; future evolution will:
//! * Split motion/edit/command application into dedicated sub-modules.
//! * Emit structured render deltas instead of a boolean dirty flag.
//! * Integrate observer hooks (macro recorder, analytics) before mutation.
//!
//! Telemetry (Phase 1 final set):
//! * `motion` span around all motion kinds (`kind` field distinguishes variants).
//! * `edit_insert`, `edit_newline`, `edit_backspace`, `edit_delete_under` for edit paths.
//! * `undo`, `redo` spans around snapshot restoration.
//!   Snapshot lifecycle trace events (`push_snapshot`, `undo_pop`, `redo_pop`, stack trims)
//!   originate in `core-state`.

use crate::{Action, ActionObserver, EditKind, ModeChange, MotionKind};
use core_state::{EditorState, Mode, normalize_line_endings};
use core_text::{Buffer, Position, motion};

/// Result of dispatching a single `Action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchResult {
    pub dirty: bool,
    pub quit: bool,
}

impl DispatchResult {
    pub fn dirty() -> Self {
        Self {
            dirty: true,
            quit: false,
        }
    }
    pub fn clean() -> Self {
        Self {
            dirty: false,
            quit: false,
        }
    }
    pub fn quit() -> Self {
        Self {
            dirty: true,
            quit: true,
        }
    }
}

/// Apply an action to editor state. Returns `DispatchResult` describing whether
/// a render is needed (`dirty`) or the editor should exit (`quit`).
pub fn dispatch(
    action: Action,
    state: &mut EditorState,
    sticky_visual_col: &mut Option<usize>,
    observers: &[Box<dyn ActionObserver>],
) -> DispatchResult {
    // Notify observers (pre-dispatch). Failures inside observers should not crash the editor;
    // we rely on them being lightweight & infallible. Any panics propagate (deliberate) to avoid
    // silently masking logic errors in early development.
    for obs in observers {
        obs.on_action(&action);
    }
    match action {
        Action::Motion(kind) => {
            let span = tracing::trace_span!("motion", kind = ?kind, line = state.position.line, byte = state.position.byte);
            let _e = span.enter();
            let before = state.position;
            match kind {
                MotionKind::Left => {
                    apply_horizontal_motion(state, motion::left);
                    *sticky_visual_col = None;
                }
                MotionKind::Right => {
                    apply_horizontal_motion(state, motion::right);
                    *sticky_visual_col = None;
                }
                MotionKind::LineStart => {
                    apply_horizontal_motion(state, motion::line_start);
                    *sticky_visual_col = None;
                }
                MotionKind::LineEnd => {
                    apply_horizontal_motion(state, motion::line_end);
                    *sticky_visual_col = None;
                }
                MotionKind::Up => {
                    *sticky_visual_col =
                        apply_vertical_motion(state, *sticky_visual_col, motion::up);
                }
                MotionKind::Down => {
                    *sticky_visual_col =
                        apply_vertical_motion(state, *sticky_visual_col, motion::down);
                }
                MotionKind::WordForward => {
                    apply_horizontal_motion(state, motion::word_forward);
                    *sticky_visual_col = None;
                }
                MotionKind::WordBackward => {
                    apply_horizontal_motion(state, motion::word_backward);
                    *sticky_visual_col = None;
                }
                MotionKind::PageHalfDown => {
                    // Half page = max(1, last_text_height / 2). Fallback to 1 if unknown.
                    let h = state.last_text_height.max(1);
                    let jump = (h / 2).max(1);
                    let target = (state.position.line + jump)
                        .min(state.active_buffer().line_count().saturating_sub(1));
                    let mut moved = false;
                    while state.position.line < target {
                        *sticky_visual_col =
                            apply_vertical_motion(state, *sticky_visual_col, motion::down);
                        moved = true;
                    }
                    if !moved {
                        // Attempt single down if at end to allow clamp behavior test expectations
                        *sticky_visual_col =
                            apply_vertical_motion(state, *sticky_visual_col, motion::down);
                    }
                }
                MotionKind::PageHalfUp => {
                    let h = state.last_text_height.max(1);
                    let jump = (h / 2).max(1);
                    let target = state.position.line.saturating_sub(jump);
                    let mut moved = false;
                    while state.position.line > target {
                        *sticky_visual_col =
                            apply_vertical_motion(state, *sticky_visual_col, motion::up);
                        moved = true;
                    }
                    if !moved {
                        *sticky_visual_col =
                            apply_vertical_motion(state, *sticky_visual_col, motion::up);
                    }
                }
            }
            if before != state.position {
                DispatchResult::dirty()
            } else {
                DispatchResult::clean()
            }
        }
        Action::ModeChange(mc) => {
            match mc {
                ModeChange::EnterInsert => {
                    state.end_insert_coalescing();
                    state.mode = Mode::Insert;
                }
                ModeChange::LeaveInsert => {
                    state.end_insert_coalescing();
                    state.mode = Mode::Normal;
                }
            }
            DispatchResult::dirty()
        }
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
        Action::CommandExecute(cmd) => {
            if cmd == ":q" {
                return DispatchResult::quit();
            }
            // File edit command: :e <path>
            if let Some(rest) = cmd.strip_prefix(":e ") {
                let path_str = rest.trim();
                if !path_str.is_empty() {
                    let path = std::path::PathBuf::from(path_str);
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
                            // Normalize line endings (Phase 2 Step 9)
                            let norm = normalize_line_endings(&content);
                            match Buffer::from_str(name, &norm.normalized) {
                                Ok(new_buf) => {
                                    state.buffers[state.active] = new_buf;
                                    state.position = Position::origin();
                                    state.file_name = Some(path);
                                    state.dirty = false; // clean after load
                                    state.original_line_ending = norm.original;
                                    state.had_trailing_newline = norm.had_trailing_newline;
                                    state
                                        .set_ephemeral("Opened", std::time::Duration::from_secs(3));
                                    if norm.mixed {
                                        tracing::warn!("mixed_line_endings_detected");
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(?e, "buffer_create_failed");
                                    state.set_ephemeral(
                                        "Open failed",
                                        std::time::Duration::from_secs(3),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(?e, "file_open_error");
                            state.set_ephemeral("Open failed", std::time::Duration::from_secs(3));
                        }
                    }
                }
                state.command_line.clear();
                return DispatchResult::dirty();
            }
            // Write file: :w (Phase 2 Step 4 - only writes if file_name is Some; else logs)
            if cmd == ":w" {
                if let Some(path) = state.file_name.clone() {
                    // clone small PathBuf
                    // Re-expand line endings based on original metadata (Phase 2 Step 9)
                    let mut content = String::new();
                    let line_ending = state.original_line_ending.as_str();
                    let last_index = state.active_buffer().line_count();
                    for i in 0..last_index {
                        if let Some(mut l) = state.active_buffer().line(i) {
                            // Each buffer line currently includes a trailing \n except maybe last (internal LF-only).
                            let ends_nl = l.ends_with('\n');
                            if ends_nl {
                                l.pop();
                            }
                            content.push_str(&l);
                            // Add original line ending if (a) not last line, or (b) last line and original had final newline
                            if (i + 1 < last_index)
                                || (state.had_trailing_newline && i + 1 == last_index)
                            {
                                content.push_str(line_ending);
                            }
                        }
                    }
                    match std::fs::write(&path, content.as_bytes()) {
                        Ok(_) => {
                            state.dirty = false;
                            state.set_ephemeral("Wrote", std::time::Duration::from_secs(3));
                        }
                        Err(e) => {
                            tracing::error!(?e, "file_write_error");
                            state.set_ephemeral("Write failed", std::time::Duration::from_secs(3));
                        }
                    }
                } else {
                    tracing::error!("write_no_filename");
                    state.set_ephemeral("No filename", std::time::Duration::from_secs(3));
                }
                state.command_line.clear();
                return DispatchResult::dirty();
            }
            state.command_line.clear();
            DispatchResult::dirty()
        }
        Action::Edit(kind) => match kind {
            EditKind::InsertGrapheme(g) => {
                if matches!(state.mode, Mode::Insert) {
                    let span = tracing::trace_span!("edit_insert", grapheme = %g);
                    let _e = span.enter();
                    state.begin_insert_coalescing();
                    state.note_insert_edit();
                    let mut pos = state.position;
                    state.active_buffer_mut().insert_grapheme(&mut pos, &g);
                    state.position = pos;
                    if !state.dirty {
                        state.dirty = true;
                    }
                    DispatchResult::dirty()
                } else {
                    DispatchResult::clean()
                }
            }
            EditKind::InsertNewline => {
                if matches!(state.mode, Mode::Insert) {
                    let span = tracing::trace_span!("edit_newline");
                    let _e = span.enter();
                    state.begin_insert_coalescing();
                    state.note_insert_edit();
                    let mut pos = state.position;
                    state.active_buffer_mut().insert_newline(&mut pos);
                    state.position = pos;
                    state.end_insert_coalescing();
                    if !state.dirty {
                        state.dirty = true;
                    }
                    DispatchResult::dirty()
                } else {
                    DispatchResult::clean()
                }
            }
            EditKind::Backspace => {
                if matches!(state.mode, Mode::Insert) {
                    let span = tracing::trace_span!("edit_backspace");
                    let _e = span.enter();
                    state.begin_insert_coalescing(); // ensure pre-edit snapshot captured once per run
                    state.note_insert_edit();
                    let mut pos = state.position;
                    state.active_buffer_mut().delete_grapheme_before(&mut pos);
                    state.position = pos;
                    if !state.dirty {
                        state.dirty = true;
                    }
                    DispatchResult::dirty()
                } else {
                    DispatchResult::clean()
                }
            }
            EditKind::DeleteUnder => {
                if matches!(state.mode, Mode::Normal) {
                    let span = tracing::trace_span!("edit_delete_under");
                    let _e = span.enter();
                    state.push_discrete_edit_snapshot();
                    let mut pos = state.position;
                    state.active_buffer_mut().delete_grapheme_at(&mut pos);
                    state.position = pos;
                    if !state.dirty {
                        state.dirty = true;
                    }
                    DispatchResult::dirty()
                } else {
                    DispatchResult::clean()
                }
            }
        },
        Action::Undo => {
            let span = tracing::trace_span!("undo");
            let _e = span.enter();
            if state.undo() {
                DispatchResult::dirty()
            } else {
                DispatchResult::clean()
            }
        }
        Action::Redo => {
            let span = tracing::trace_span!("redo");
            let _e = span.enter();
            if state.redo() {
                DispatchResult::dirty()
            } else {
                DispatchResult::clean()
            }
        }
        Action::Quit => DispatchResult::quit(),
    }
}

// --- Local safe motion helpers (mirroring those in main until further extraction) ---
fn apply_horizontal_motion(state: &mut EditorState, f: fn(&Buffer, &mut Position)) {
    let buf = state.active_buffer();
    let mut pos = state.position;
    f(buf, &mut pos);
    state.position = pos;
}

fn apply_vertical_motion(
    state: &mut EditorState,
    sticky: Option<usize>,
    f: fn(&Buffer, &mut Position, Option<usize>) -> Option<usize>,
) -> Option<usize> {
    let buf = state.active_buffer();
    let mut pos = state.position;
    let new_sticky = f(buf, &mut pos, sticky);
    state.position = pos;
    new_sticky
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translate_key; // simple sanity around motions via existing translator
    use core_events::{KeyCode, KeyEvent, KeyModifiers};
    use core_text::Buffer;

    #[test]
    fn motion_left_right_dirty() {
        let buffer = Buffer::from_str("t", "ab\ncd").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        // Move right
        let act = translate_key(
            state.mode,
            state.command_line.buffer(),
            &KeyEvent {
                code: KeyCode::Char('l'),
                mods: KeyModifiers::empty(),
            },
        )
        .unwrap();
        assert!(dispatch(act, &mut state, &mut sticky, &[]).dirty);
        // Moving left should also be dirty (position changed)
        let act = translate_key(
            state.mode,
            state.command_line.buffer(),
            &KeyEvent {
                code: KeyCode::Char('h'),
                mods: KeyModifiers::empty(),
            },
        )
        .unwrap();
        assert!(dispatch(act, &mut state, &mut sticky, &[]).dirty);
    }

    #[test]
    fn quit_command_execute() {
        let buffer = Buffer::from_str("t", "abc").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        // Simulate entering :q
        dispatch(Action::CommandStart, &mut state, &mut sticky, &[]);
        dispatch(Action::CommandChar('q'), &mut state, &mut sticky, &[]);
        let res = dispatch(
            Action::CommandExecute(":q".into()),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(res.quit && res.dirty);
    }

    #[test]
    fn edit_command_opens_file() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("sample.txt");
        {
            let mut f = std::fs::File::create(&file_path).unwrap();
            writeln!(f, "Hello Edit Command").unwrap();
        }
        let buffer = Buffer::from_str("t", "initial").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        // Simulate entering :e <path>
        dispatch(Action::CommandStart, &mut state, &mut sticky, &[]);
        for ch in format!("e {}", file_path.display()).chars() {
            dispatch(Action::CommandChar(ch), &mut state, &mut sticky, &[]);
        }
        let res = dispatch(
            Action::CommandExecute(format!(":e {}", file_path.display())),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(res.dirty);
        assert!(state.file_name.as_ref().is_some());
        assert!(
            state
                .active_buffer()
                .line(0)
                .unwrap()
                .starts_with("Hello Edit Command")
        );
        assert!(!state.dirty, "buffer must be clean after load");
        assert!(state.ephemeral_status.as_ref().map(|m| m.text.as_str()) == Some("Opened"));
    }

    #[test]
    fn write_command_writes_file() {
        use std::io::Read;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("write_test.txt");
        // Start with named buffer by loading file via :e logic path to set file_name
        let initial = Buffer::from_str("t", "hello").unwrap();
        let mut state = EditorState::new(initial);
        let mut sticky = None;
        // Assign file_name manually to simulate earlier open (simpler than invoking :e again here)
        state.file_name = Some(file_path.clone());
        state.dirty = true; // pretend modified
        // Execute :w
        dispatch(Action::CommandStart, &mut state, &mut sticky, &[]);
        dispatch(Action::CommandChar('w'), &mut state, &mut sticky, &[]);
        let res = dispatch(
            Action::CommandExecute(":w".into()),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(res.dirty);
        assert!(!state.dirty, "dirty flag should clear after write");
        // Confirm file content
        let mut f = std::fs::File::open(&file_path).unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        assert!(s.starts_with("hello"));
    }

    #[test]
    fn write_command_without_filename_logs_and_keeps_dirty() {
        let buffer = Buffer::from_str("t", "scratch buffer").unwrap();
        let mut state = EditorState::new(buffer);
        state.dirty = true;
        let mut sticky = None;
        dispatch(Action::CommandStart, &mut state, &mut sticky, &[]);
        dispatch(Action::CommandChar('w'), &mut state, &mut sticky, &[]);
        let res = dispatch(
            Action::CommandExecute(":w".into()),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(res.dirty);
        assert!(state.dirty, "dirty flag should remain when no filename");
        assert!(state.ephemeral_status.as_ref().map(|m| m.text.as_str()) == Some("No filename"));
    }

    #[test]
    fn edit_command_open_failure_sets_ephemeral() {
        let buffer = Buffer::from_str("t", "initial").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        dispatch(Action::CommandStart, &mut state, &mut sticky, &[]);
        for ch in "e non_existent_file_12345".chars() {
            dispatch(Action::CommandChar(ch), &mut state, &mut sticky, &[]);
        }
        dispatch(
            Action::CommandExecute(":e non_existent_file_12345".into()),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(state.ephemeral_status.as_ref().map(|m| m.text.as_str()) == Some("Open failed"));
    }

    #[test]
    fn dirty_flag_sets_on_first_insert() {
        let buffer = Buffer::from_str("t", "").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        assert!(!state.dirty, "initial dirty should be false");
        // Enter insert and type a grapheme
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".into())),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(state.dirty, "dirty should be true after first mutation");
    }

    #[test]
    fn undo_does_not_clear_dirty() {
        let buffer = Buffer::from_str("t", "").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".into())),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(state.dirty);
        dispatch(Action::Undo, &mut state, &mut sticky, &[]);
        assert!(state.dirty, "dirty should remain true after undo");
    }

    #[test]
    fn write_clears_then_new_edit_sets_dirty_again() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("dirty_cycle.txt");
        // Start with named buffer
        let buffer = Buffer::from_str("t", "start").unwrap();
        let mut state = EditorState::new(buffer);
        state.file_name = Some(file_path.clone());
        let mut sticky = None;
        // Mutate
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("x".into())),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(state.dirty);
        // Write
        dispatch(Action::CommandStart, &mut state, &mut sticky, &[]);
        dispatch(Action::CommandChar('w'), &mut state, &mut sticky, &[]);
        dispatch(
            Action::CommandExecute(":w".into()),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(!state.dirty, "dirty should clear after write");
        // New edit sets it again
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("y".into())),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(state.dirty, "dirty should set again after new edit");
    }

    #[test]
    fn undo_redo_cycle() {
        let buffer = Buffer::from_str("t", "").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        // Enter insert and insert a char
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".into())),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut state,
            &mut sticky,
            &[],
        );
        // Undo
        assert!(dispatch(Action::Undo, &mut state, &mut sticky, &[]).dirty);
        assert_eq!(state.active_buffer().line(0).unwrap(), "");
        // Redo
        assert!(dispatch(Action::Redo, &mut state, &mut sticky, &[]).dirty);
        assert_eq!(state.active_buffer().line(0).unwrap(), "a");
    }

    #[test]
    fn observer_invoked() {
        use std::sync::{Arc, Mutex};
        struct CountObs(Arc<Mutex<usize>>);
        impl ActionObserver for CountObs {
            fn on_action(&self, _action: &Action) {
                *self.0.lock().unwrap() += 1;
            }
        }
        let counter = Arc::new(Mutex::new(0usize));
        let obs = CountObs(counter.clone());
        let observers: Vec<Box<dyn ActionObserver>> = vec![Box::new(obs)];
        let buffer = Buffer::from_str("t", "").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
            &observers,
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".into())),
            &mut state,
            &mut sticky,
            &observers,
        );
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut state,
            &mut sticky,
            &observers,
        );
        assert_eq!(
            *counter.lock().unwrap(),
            3,
            "observer should have seen three actions"
        );
    }

    #[test]
    fn empty_buffer_backspace_noop() {
        let buffer = Buffer::from_str("t", "").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        // Enter insert then hit backspace (should not panic or change)
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
            &[],
        );
        let before = state.position;
        let res = dispatch(
            Action::Edit(EditKind::Backspace),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(
            res.dirty,
            "still considered edit path (render) even if no change"
        );
        assert_eq!(state.position, before, "position unchanged");
        assert_eq!(state.active_buffer().line(0).unwrap(), "");
    }

    #[test]
    fn end_of_line_motion_clamp() {
        let buffer = Buffer::from_str("t", "short\nlonger line here").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        // Move to end of first line
        dispatch(
            Action::Motion(MotionKind::LineEnd),
            &mut state,
            &mut sticky,
            &[],
        );
        let end_byte = state.position.byte;
        // Move down; position.byte should clamp within second line (not exceed its len)
        dispatch(
            Action::Motion(MotionKind::Down),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(state.position.byte <= state.active_buffer().line_byte_len(state.position.line));
        // Move up; should restore original end byte of first line
        dispatch(Action::Motion(MotionKind::Up), &mut state, &mut sticky, &[]);
        assert_eq!(state.position.byte, end_byte);
    }

    #[test]
    fn page_half_down_and_up_basic() {
        // Build buffer with multiple lines to allow paging
        let mut content = String::new();
        for i in 0..40 {
            content.push_str(&format!("line{idx}\n", idx = i));
        }
        let buffer = Buffer::from_str("t", &content).unwrap();
        let mut state = EditorState::new(buffer);
        state.set_last_text_height(20); // pretend viewport shows 20 lines
        let mut sticky = None;
        let start_line = state.position.line;
        // Dispatch PageHalfDown (~10 lines)
        let res = dispatch(
            Action::Motion(MotionKind::PageHalfDown),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(res.dirty);
        assert!(
            state.position.line >= start_line + 9 && state.position.line <= start_line + 11,
            "expected roughly half page down"
        );
        let after_down = state.position.line;
        // Page up returns near original (clamped)
        let _ = dispatch(
            Action::Motion(MotionKind::PageHalfUp),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(
            state.position.line <= after_down && state.position.line <= start_line + 2,
            "expected upward half page"
        );
    }

    #[test]
    fn page_half_respects_buffer_end() {
        // Small buffer: ensure we clamp and do not move beyond last line
        let buffer = Buffer::from_str("t", "a\nb\nc\n").unwrap(); // 4 lines (last empty due to trailing newline)
        let mut state = EditorState::new(buffer);
        state.set_last_text_height(10); // large viewport
        let mut sticky = None;
        // Two half-page downs should land on last non-empty or final empty line (clamped)
        for _ in 0..2 {
            let _ = dispatch(
                Action::Motion(MotionKind::PageHalfDown),
                &mut state,
                &mut sticky,
                &[],
            );
        }
        let last_index = state.active_buffer().line_count() - 1;
        assert_eq!(
            state.position.line, last_index,
            "expected clamp to last line"
        );
        // Further down should not move
        let before = state.position.line;
        let _ = dispatch(
            Action::Motion(MotionKind::PageHalfDown),
            &mut state,
            &mut sticky,
            &[],
        );
        assert_eq!(state.position.line, before);
        // Page up returns toward top (line 0 or 1 depending on jump)
        let _ = dispatch(
            Action::Motion(MotionKind::PageHalfUp),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(
            state.position.line < last_index,
            "should have moved up from last line"
        );
    }

    #[test]
    fn ctrl_d_ctrl_u_translate() {
        use core_events::{KeyEvent, KeyModifiers};
        let buffer = Buffer::from_str("t", "x\n".repeat(50).as_str()).unwrap();
        let mut state = EditorState::new(buffer);
        state.set_last_text_height(24);
        let mut sticky = None;
        let ctrl_d = KeyEvent {
            code: KeyCode::Char('d'),
            mods: KeyModifiers::CTRL,
        };
        let act = crate::translate_key(state.mode, state.command_line.buffer(), &ctrl_d);
        assert!(matches!(
            act,
            Some(Action::Motion(MotionKind::PageHalfDown))
        ));
        let _ = dispatch(act.unwrap(), &mut state, &mut sticky, &[]);
        let ctrl_u = KeyEvent {
            code: KeyCode::Char('u'),
            mods: KeyModifiers::CTRL,
        };
        let act2 = crate::translate_key(state.mode, state.command_line.buffer(), &ctrl_u);
        assert!(matches!(act2, Some(Action::Motion(MotionKind::PageHalfUp))));
    }

    #[test]
    fn delete_under_at_eof_safe() {
        let buffer = Buffer::from_str("t", "abc").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        // Move to end and attempt delete_under (no-op)
        dispatch(
            Action::Motion(MotionKind::LineEnd),
            &mut state,
            &mut sticky,
            &[],
        );
        let end_byte = state.position.byte;
        let line_before = state.active_buffer().line(0).unwrap().to_string();
        let res = dispatch(
            Action::Edit(EditKind::DeleteUnder),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(!res.quit, "should not quit");
        assert_eq!(state.position.byte, end_byte);
        assert_eq!(state.active_buffer().line(0).unwrap(), line_before);
    }

    #[test]
    fn newline_undo_redo_at_file_end() {
        let buffer = Buffer::from_str("t", "abc").unwrap();
        let mut state = EditorState::new(buffer);
        let mut sticky = None;
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut state,
            &mut sticky,
            &[],
        );
        // Move to end and insert newline then a char
        dispatch(
            Action::Motion(MotionKind::LineEnd),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertNewline),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("x".into())),
            &mut state,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut state,
            &mut sticky,
            &[],
        ); // boundary
        // Collect buffer content lines for verification (single buffer)
        let mut collected = String::new();
        for i in 0..state.active_buffer().line_count() {
            if let Some(l) = state.active_buffer().line(i) {
                collected.push_str(&l);
            }
        }
        let after_insert = collected.clone();
        assert!(after_insert.starts_with("abc"));
        // Undo should remove entire run (newline + x)
        dispatch(Action::Undo, &mut state, &mut sticky, &[]);
        // After undo the buffer should match original single-line content (may or may not retain trailing newline; ensure content prefix matches and no second non-empty line)
        assert!(state.active_buffer().line(0).unwrap().starts_with("abc"));
        if state.active_buffer().line_count() > 1 {
            let l1 = state.active_buffer().line(1).unwrap();
            assert!(
                l1.is_empty(),
                "second line should be empty after undo if present"
            );
        }
        // Redo should restore
        dispatch(Action::Redo, &mut state, &mut sticky, &[]);
        let mut redo_collected = String::new();
        for i in 0..state.active_buffer().line_count() {
            if let Some(l) = state.active_buffer().line(i) {
                redo_collected.push_str(&l);
            }
        }
        assert_eq!(redo_collected, after_insert);
    }
}
