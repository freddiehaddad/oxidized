//! Dispatcher applying `Action` to mutable editor state.
//!
//! Refactor R3 / Step 1: Module decomposition.
//! -------------------------------------------------
//! This module was previously a single 1000+ line file. It is now
//! decomposed into focused sub-modules:
//! * `motion`  - cursor movement semantics
//! * `mode`    - mode transitions (Normal <-> Insert)
//! * `command` - command line editing & execution (:q, :e, :w)
//! * `edit`    - text mutation (insert/delete/backspace/newline)
//! * `undo`    - undo / redo dispatch
//!
//! The public surface (`dispatch`, `DispatchResult`) remains unchanged.
//! Borrow splitting (raw pointer for `EditorState` + mutable active view
//! borrow) is preserved exactly to avoid accidental semantic drift.
//!
//! Zero behavioral change is intended in this step; tests from the
//! original monolithic module are retained verbatim below to guarantee
//! parity. Subsequent refactor steps (command parser extraction, etc.)
//! will build on this structure.

use crate::{Action, ActionObserver};
use core_model::EditorModel;
use core_state::EditorState;

mod command;
mod command_parser;
mod edit;
mod mode;
mod motion;
mod undo;

/// Result of dispatching a single `Action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchResult {
    pub dirty: bool,
    pub quit: bool,
    /// Indicates a structural buffer replacement occurred (e.g. :e loaded a new file)
    /// and any partial render caches (line hashes, last cursor line) must be treated
    /// as invalid. The runtime should escalate to a Full render regardless of the
    /// semantic dirty heuristic chosen for ordinary edits/motions.
    pub buffer_replaced: bool,
}

impl DispatchResult {
    pub fn dirty() -> Self {
        Self {
            dirty: true,
            quit: false,
            buffer_replaced: false,
        }
    }
    pub fn clean() -> Self {
        Self {
            dirty: false,
            quit: false,
            buffer_replaced: false,
        }
    }
    pub fn quit() -> Self {
        Self {
            dirty: true,
            quit: true,
            buffer_replaced: false,
        }
    }
    pub fn buffer_replaced() -> Self {
        Self {
            dirty: true,
            quit: false,
            buffer_replaced: true,
        }
    }
}

/// Apply an action to editor state. Returns `DispatchResult` describing whether
/// a render is needed (`dirty`) or the editor should exit (`quit`).
pub fn dispatch(
    action: Action,
    model: &mut EditorModel,
    sticky_visual_col: &mut Option<usize>,
    observers: &[Box<dyn ActionObserver>],
) -> DispatchResult {
    // Borrow mutable state and view separately without overlapping mutable borrows of model.
    let state_ptr: *mut EditorState = model.state_mut();
    // SAFETY: We only use `state_ptr` for field/method access that does not move `state`.
    // We then take another mutable borrow for the active view. These do not alias because
    // `active_view_mut` only touches the views vector while state fields are accessed via raw pointer.
    let view = model.active_view_mut();
    let state: &mut EditorState = unsafe { &mut *state_ptr };

    // Notify observers (pre-dispatch).
    for obs in observers {
        obs.on_action(&action);
    }

    match action {
        Action::Motion(kind) => motion::handle_motion(kind, state, view, sticky_visual_col),
        Action::MotionWithCount {
            motion: kind,
            count,
        } => {
            let mut result = DispatchResult::clean();
            for _ in 0..count {
                // repeat motion count times
                let r = motion::handle_motion(kind, state, view, sticky_visual_col);
                if r.dirty {
                    result.dirty = true;
                }
            }
            result
        }
        Action::ModeChange(mc) => mode::handle_mode_change(mc, state),
        Action::CommandStart
        | Action::CommandChar(_)
        | Action::CommandBackspace
        | Action::CommandCancel
        | Action::CommandExecute(_) => command::handle_command_action(action, state, view),
        Action::Edit(kind) => edit::handle_edit(kind, state, view),
        Action::Undo => undo::handle_undo(state, view),
        Action::Redo => undo::handle_redo(state, view),
        Action::Quit => DispatchResult::quit(),
        Action::BeginOperator(_) => DispatchResult::clean(),
        Action::ApplyOperator { op, motion, count } => {
            use crate::OperatorKind;
            use crate::span_resolver::resolve_span;
            match op {
                OperatorKind::Delete => {
                    let start_pos = view.cursor;
                    let vertical = matches!(
                        motion,
                        crate::MotionKind::Up
                            | crate::MotionKind::Down
                            | crate::MotionKind::PageHalfUp
                            | crate::MotionKind::PageHalfDown
                    );
                    if vertical {
                        // Replay motion count times on a temp cursor to find target line.
                        let buf = state.active_buffer();
                        let mut tmp = start_pos;
                        for _ in 0..count.max(1) {
                            match motion {
                                crate::MotionKind::Up => {
                                    let _ = core_text::motion::up(buf, &mut tmp, None);
                                }
                                crate::MotionKind::Down => {
                                    let _ = core_text::motion::down(buf, &mut tmp, None);
                                }
                                crate::MotionKind::PageHalfUp => {
                                    let _ = core_text::motion::up(buf, &mut tmp, None);
                                }
                                crate::MotionKind::PageHalfDown => {
                                    let _ = core_text::motion::down(buf, &mut tmp, None);
                                }
                                _ => {}
                            }
                        }
                        let line_start = start_pos.line.min(tmp.line);
                        let line_end = start_pos.line.max(tmp.line);
                        let buffer = state.active_buffer();
                        let mut abs_start = 0usize;
                        for l in 0..line_start {
                            abs_start += buffer.line_byte_len(l);
                            if let Some(s) = buffer.line(l)
                                && s.ends_with('\n')
                            {
                                abs_start += 1;
                            }
                        }
                        let mut abs_after_last = abs_start;
                        for l in line_start..=line_end {
                            abs_after_last += buffer.line_byte_len(l);
                            if let Some(s) = buffer.line(l)
                                && s.ends_with('\n')
                            {
                                abs_after_last += 1;
                            }
                        }
                        if abs_start == abs_after_last {
                            return DispatchResult::clean();
                        }
                        let mut cursor = view.cursor;
                        let removed =
                            state.delete_span_with_snapshot(&mut cursor, abs_start, abs_after_last);
                        let structural = line_end > line_start || removed.contains('\n');
                        let metrics_ptr: *mut _ = state.operator_metrics_mut();
                        unsafe {
                            (*metrics_ptr).incr_delete();
                        }
                        // Safe because we have exclusive &mut state in scope; raw pointer used to avoid borrow overlap.
                        let metrics_ref = unsafe { &mut *metrics_ptr };
                        state.registers_mut().record_delete(removed, metrics_ref);
                        view.cursor = cursor;
                        if !state.dirty {
                            state.dirty = true;
                        }
                        if structural {
                            DispatchResult::buffer_replaced()
                        } else {
                            DispatchResult::dirty()
                        }
                    } else {
                        // Characterwise path (existing semantics)
                        let span = resolve_span(state, start_pos, motion, count);
                        if span.start == span.end {
                            return DispatchResult::clean();
                        }
                        let mut cursor = view.cursor; // copy for deletion API
                        let removed =
                            state.delete_span_with_snapshot(&mut cursor, span.start, span.end);
                        let structural = removed.contains('\n');
                        let metrics_ptr: *mut _ = state.operator_metrics_mut();
                        unsafe {
                            (*metrics_ptr).incr_delete();
                        }
                        let metrics_ref = unsafe { &mut *metrics_ptr };
                        state.registers_mut().record_delete(removed, metrics_ref);
                        view.cursor = cursor;
                        if !state.dirty {
                            state.dirty = true;
                        }
                        if structural {
                            DispatchResult::buffer_replaced()
                        } else {
                            DispatchResult::dirty()
                        }
                    }
                }
                OperatorKind::Yank => {
                    use crate::span_resolver::resolve_span;
                    let start_pos = view.cursor;
                    let vertical = matches!(
                        motion,
                        crate::MotionKind::Up
                            | crate::MotionKind::Down
                            | crate::MotionKind::PageHalfUp
                            | crate::MotionKind::PageHalfDown
                    );
                    if vertical {
                        let buf = state.active_buffer();
                        let mut tmp = start_pos;
                        for _ in 0..count.max(1) {
                            match motion {
                                crate::MotionKind::Up => {
                                    let _ = core_text::motion::up(buf, &mut tmp, None);
                                }
                                crate::MotionKind::Down => {
                                    let _ = core_text::motion::down(buf, &mut tmp, None);
                                }
                                crate::MotionKind::PageHalfUp => {
                                    let _ = core_text::motion::up(buf, &mut tmp, None);
                                }
                                crate::MotionKind::PageHalfDown => {
                                    let _ = core_text::motion::down(buf, &mut tmp, None);
                                }
                                _ => {}
                            }
                        }
                        let line_start = start_pos.line.min(tmp.line);
                        let line_end = start_pos.line.max(tmp.line);
                        if line_start != line_end {
                            let buffer = state.active_buffer();
                            let mut collected = String::new();
                            for l in line_start..=line_end {
                                if let Some(s) = buffer.line(l) {
                                    collected.push_str(&s);
                                } else {
                                    break;
                                }
                            }
                            let metrics_ptr: *mut _ = state.operator_metrics_mut();
                            unsafe {
                                (*metrics_ptr).incr_yank();
                            }
                            let metrics_ref = unsafe { &mut *metrics_ptr };
                            state.registers_mut().record_yank(collected, metrics_ref);
                            return DispatchResult::clean();
                        }
                    }
                    // Characterwise span resolution (non-mutating)
                    let span = resolve_span(state, start_pos, motion, count);
                    if span.start == span.end {
                        return DispatchResult::clean();
                    }
                    // Collect substring for span without mutating buffer.
                    let buffer = state.active_buffer();
                    // Reconstruct by iterating lines overlapping span (simple early impl):
                    let mut collected = String::new();
                    let remaining_start = span.start;
                    let remaining_end = span.end;
                    // Iterate all lines accumulating absolute byte offsets similarly to delete path.
                    let mut abs = 0usize;
                    for l in 0..buffer.line_count() {
                        let line = buffer.line(l).unwrap();
                        let line_len_bytes = line.len();
                        let line_end_abs = abs + line_len_bytes;
                        if line_end_abs <= remaining_start {
                            abs = line_end_abs;
                            continue;
                        }
                        if abs >= remaining_end {
                            break;
                        }
                        // Overlap region within this line
                        let local_start = remaining_start.saturating_sub(abs);
                        let local_end = (remaining_end - abs).min(line_len_bytes);
                        collected.push_str(&line[local_start..local_end]);
                        abs = line_end_abs;
                    }
                    let metrics_ptr: *mut _ = state.operator_metrics_mut();
                    unsafe {
                        (*metrics_ptr).incr_yank();
                    }
                    let metrics_ref = unsafe { &mut *metrics_ptr };
                    state.registers_mut().record_yank(collected, metrics_ref);
                    DispatchResult::clean()
                }
                OperatorKind::Change => {
                    // Change = Delete span then enter Insert at span start.
                    use crate::span_resolver::resolve_span;
                    let start_pos = view.cursor;
                    let vertical = matches!(
                        motion,
                        crate::MotionKind::Up
                            | crate::MotionKind::Down
                            | crate::MotionKind::PageHalfUp
                            | crate::MotionKind::PageHalfDown
                    );
                    if vertical {
                        // Replay vertical motion to compute inclusive line range identical to Delete.
                        let buf = state.active_buffer();
                        let mut tmp = start_pos;
                        for _ in 0..count.max(1) {
                            match motion {
                                crate::MotionKind::Up => {
                                    let _ = core_text::motion::up(buf, &mut tmp, None);
                                }
                                crate::MotionKind::Down => {
                                    let _ = core_text::motion::down(buf, &mut tmp, None);
                                }
                                crate::MotionKind::PageHalfUp => {
                                    let _ = core_text::motion::up(buf, &mut tmp, None);
                                }
                                crate::MotionKind::PageHalfDown => {
                                    let _ = core_text::motion::down(buf, &mut tmp, None);
                                }
                                _ => {}
                            }
                        }
                        let line_start = start_pos.line.min(tmp.line);
                        let line_end = start_pos.line.max(tmp.line);
                        let buffer = state.active_buffer();
                        let mut abs_start = 0usize;
                        for l in 0..line_start {
                            abs_start += buffer.line_byte_len(l);
                            if let Some(s) = buffer.line(l)
                                && s.ends_with('\n')
                            {
                                abs_start += 1;
                            }
                        }
                        let mut abs_after_last = abs_start;
                        for l in line_start..=line_end {
                            abs_after_last += buffer.line_byte_len(l);
                            if let Some(s) = buffer.line(l)
                                && s.ends_with('\n')
                            {
                                abs_after_last += 1;
                            }
                        }
                        if abs_start == abs_after_last {
                            return DispatchResult::clean();
                        }
                        let mut cursor = view.cursor;
                        let removed =
                            state.delete_span_with_snapshot(&mut cursor, abs_start, abs_after_last);
                        let structural = line_end > line_start || removed.contains('\n');
                        let metrics_ptr: *mut _ = state.operator_metrics_mut();
                        unsafe {
                            (*metrics_ptr).incr_change();
                        }
                        let metrics_ref = unsafe { &mut *metrics_ptr };
                        state.registers_mut().record_delete(removed, metrics_ref);
                        // For change we enter Insert at original start of span (line_start, col 0)
                        view.cursor.line = line_start;
                        view.cursor.byte = 0;
                        state.mode = core_state::Mode::Insert;
                        if !state.dirty {
                            state.dirty = true;
                        }
                        if structural {
                            DispatchResult::buffer_replaced()
                        } else {
                            DispatchResult::dirty()
                        }
                    } else {
                        let span = resolve_span(state, start_pos, motion, count);
                        if span.start == span.end {
                            return DispatchResult::clean();
                        }
                        let mut cursor = view.cursor;
                        let removed =
                            state.delete_span_with_snapshot(&mut cursor, span.start, span.end);
                        let structural = removed.contains('\n');
                        let metrics_ptr: *mut _ = state.operator_metrics_mut();
                        unsafe {
                            (*metrics_ptr).incr_change();
                        }
                        let metrics_ref = unsafe { &mut *metrics_ptr };
                        state.registers_mut().record_delete(removed, metrics_ref);
                        // Map absolute byte index span.start back to (line, byte)
                        let buffer = state.active_buffer();
                        let mut abs = 0usize;
                        let mut new_line = 0usize;
                        let mut new_byte = 0usize;
                        'outer: for l in 0..buffer.line_count() {
                            let mut line_total = buffer.line_byte_len(l);
                            if let Some(s) = buffer.line(l)
                                && s.ends_with('\n')
                            {
                                line_total += 1;
                            }
                            if abs + line_total > span.start {
                                new_line = l;
                                new_byte = span.start - abs;
                                break 'outer;
                            }
                            abs += line_total;
                        }
                        view.cursor.line = new_line;
                        view.cursor.byte = new_byte;
                        state.mode = core_state::Mode::Insert;
                        if !state.dirty {
                            state.dirty = true;
                        }
                        if structural {
                            DispatchResult::buffer_replaced()
                        } else {
                            DispatchResult::dirty()
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, EditKind, ModeChange, MotionKind, OperatorKind, translate_key}; // test-only imports
    use core_events::{KeyCode, KeyEvent, KeyModifiers};
    use core_model::EditorModel;
    use core_text::Buffer;

    #[test]
    fn motion_left_right_dirty() {
        let buffer = Buffer::from_str("t", "ab\ncd").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // Move right
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &KeyEvent {
                code: KeyCode::Char('l'),
                mods: KeyModifiers::empty(),
            },
        )
        .unwrap();
        assert!(dispatch(act, &mut model, &mut sticky, &[]).dirty);
        // Moving left should also be dirty (position changed)
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &KeyEvent {
                code: KeyCode::Char('h'),
                mods: KeyModifiers::empty(),
            },
        )
        .unwrap();
        assert!(dispatch(act, &mut model, &mut sticky, &[]).dirty);
    }

    #[test]
    fn quit_command_execute() {
        let buffer = Buffer::from_str("t", "abc").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // Simulate entering :q
        dispatch(Action::CommandStart, &mut model, &mut sticky, &[]);
        dispatch(Action::CommandChar('q'), &mut model, &mut sticky, &[]);
        let res = dispatch(
            Action::CommandExecute(":q".into()),
            &mut model,
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
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // Simulate entering :e <path>
        dispatch(Action::CommandStart, &mut model, &mut sticky, &[]);
        for ch in format!("e {}", file_path.display()).chars() {
            dispatch(Action::CommandChar(ch), &mut model, &mut sticky, &[]);
        }
        let res = dispatch(
            Action::CommandExecute(format!(":e {}", file_path.display())),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(res.dirty);
        assert!(model.state().file_name.as_ref().is_some());
        assert!(
            model
                .state()
                .active_buffer()
                .line(0)
                .unwrap()
                .starts_with("Hello Edit Command")
        );
        assert!(!model.state().dirty, "buffer must be clean after load");
        assert!(
            model
                .state()
                .ephemeral_status
                .as_ref()
                .map(|m| m.text.as_str())
                == Some("Opened")
        );
    }

    #[test]
    fn write_command_writes_file() {
        use std::io::Read;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("write_test.txt");
        let initial = Buffer::from_str("t", "hello").unwrap();
        let state = core_state::EditorState::new(initial);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        model.state_mut().file_name = Some(file_path.clone());
        model.state_mut().dirty = true; // pretend modified
        dispatch(Action::CommandStart, &mut model, &mut sticky, &[]);
        dispatch(Action::CommandChar('w'), &mut model, &mut sticky, &[]);
        let res = dispatch(
            Action::CommandExecute(":w".into()),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(res.dirty);
        assert!(!model.state().dirty, "dirty flag should clear after write");
        let mut f = std::fs::File::open(&file_path).unwrap();
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        assert!(s.starts_with("hello"));
    }

    #[test]
    fn write_command_without_filename_logs_and_keeps_dirty() {
        let buffer = Buffer::from_str("t", "scratch buffer").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        model.state_mut().dirty = true;
        let mut sticky = None;
        dispatch(Action::CommandStart, &mut model, &mut sticky, &[]);
        dispatch(Action::CommandChar('w'), &mut model, &mut sticky, &[]);
        let res = dispatch(
            Action::CommandExecute(":w".into()),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(res.dirty);
        assert!(
            model.state().dirty,
            "dirty flag should remain when no filename"
        );
        assert!(
            model
                .state()
                .ephemeral_status
                .as_ref()
                .map(|m| m.text.as_str())
                == Some("No filename")
        );
    }

    #[test]
    fn edit_command_open_failure_sets_ephemeral() {
        let buffer = Buffer::from_str("t", "initial").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        dispatch(Action::CommandStart, &mut model, &mut sticky, &[]);
        for ch in "e non_existent_file_12345".chars() {
            dispatch(Action::CommandChar(ch), &mut model, &mut sticky, &[]);
        }
        dispatch(
            Action::CommandExecute(":e non_existent_file_12345".into()),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(
            model
                .state()
                .ephemeral_status
                .as_ref()
                .map(|m| m.text.as_str())
                == Some("Open failed")
        );
    }

    #[test]
    fn dirty_flag_sets_on_first_insert() {
        let buffer = Buffer::from_str("t", "").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        assert!(!model.state().dirty, "initial dirty should be false");
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".into())),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(
            model.state().dirty,
            "dirty should be true after first mutation"
        );
    }

    #[test]
    fn undo_does_not_clear_dirty() {
        let buffer = Buffer::from_str("t", "").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".into())),
            &mut model,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(model.state().dirty);
        dispatch(Action::Undo, &mut model, &mut sticky, &[]);
        assert!(model.state().dirty, "dirty should remain true after undo");
    }

    #[test]
    fn write_clears_then_new_edit_sets_dirty_again() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("dirty_cycle.txt");
        let buffer = Buffer::from_str("t", "start").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        model.state_mut().file_name = Some(file_path.clone());
        let mut sticky = None;
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("x".into())),
            &mut model,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(model.state().dirty);
        dispatch(Action::CommandStart, &mut model, &mut sticky, &[]);
        dispatch(Action::CommandChar('w'), &mut model, &mut sticky, &[]);
        dispatch(
            Action::CommandExecute(":w".into()),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(!model.state().dirty, "dirty should clear after write");
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("y".into())),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(model.state().dirty, "dirty should set again after new edit");
    }

    #[test]
    fn undo_redo_cycle() {
        let buffer = Buffer::from_str("t", "").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".into())),
            &mut model,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(dispatch(Action::Undo, &mut model, &mut sticky, &[]).dirty);
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "");
        assert!(dispatch(Action::Redo, &mut model, &mut sticky, &[]).dirty);
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "a");
    }

    #[test]
    fn observer_invoked() {
        use std::sync::{Arc, Mutex};
        struct CountObs(Arc<Mutex<usize>>);
        impl crate::ActionObserver for CountObs {
            fn on_action(&self, _action: &crate::Action) {
                *self.0.lock().unwrap() += 1;
            }
        }
        let counter = Arc::new(Mutex::new(0usize));
        let obs = CountObs(counter.clone());
        let observers: Vec<Box<dyn crate::ActionObserver>> = vec![Box::new(obs)];
        let buffer = Buffer::from_str("t", "").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &observers,
        );
        dispatch(
            Action::Edit(EditKind::InsertGrapheme("a".into())),
            &mut model,
            &mut sticky,
            &observers,
        );
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut model,
            &mut sticky,
            &observers,
        );
        assert_eq!(*counter.lock().unwrap(), 3);
    }

    #[test]
    fn empty_buffer_backspace_noop() {
        let buffer = Buffer::from_str("t", "").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        let before = model.active_view().cursor;
        let res = dispatch(
            Action::Edit(EditKind::Backspace),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(
            res.dirty,
            "still considered edit path (render) even if no change"
        );
        assert_eq!(model.active_view().cursor, before, "cursor unchanged");
        assert_eq!(model.state().active_buffer().line(0).unwrap(), "");
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            mods: KeyModifiers::empty(),
        }
    }

    #[test]
    fn operator_delete_dw_basic() {
        let buffer = Buffer::from_str("t", "one two three\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // Simulate: d w
        // 'd'
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('d'),
        );
        // translator state is thread-local; call directly for second key
        let apply = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('w'),
        )
        .expect("apply op");
        if let Action::ApplyOperator { op, motion, count } = apply {
            assert!(matches!(op, OperatorKind::Delete));
            assert!(matches!(motion, MotionKind::WordForward));
            assert_eq!(count, 1);
            assert!(dispatch(apply, &mut model, &mut sticky, &[]).dirty);
        } else {
            panic!("expected ApplyOperator");
        }
        // Expect registers populated
        assert!(!model.state().registers.unnamed.is_empty());
    }

    #[test]
    fn operator_delete_count_prefix_2dw() {
        let buffer = Buffer::from_str("t", "one two three four five\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // 2 d w -> should delete two words starting at cursor ("one ")? Implementation: count applies to motion; starting at origin before 'one' deleting up to after second word.
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('2'),
        );
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('d'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('w'),
        )
        .unwrap();
        if let Action::ApplyOperator { count, .. } = act {
            assert_eq!(count, 2);
        } else {
            panic!();
        }
        dispatch(act, &mut model, &mut sticky, &[]);
        assert!(!model.state().registers.unnamed.is_empty());
    }

    #[test]
    fn operator_delete_multiplicative_d2w() {
        let buffer = Buffer::from_str("t", "one two three four five\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // d 2 w -> post-op count
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('d'),
        );
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('2'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('w'),
        )
        .unwrap();
        if let Action::ApplyOperator { count, .. } = act {
            assert_eq!(count, 2);
        } else {
            panic!();
        }
        dispatch(act, &mut model, &mut sticky, &[]);
        assert!(!model.state().registers.unnamed.is_empty());
    }

    // --- Step 6.2 tests: linewise vertical delete ---

    #[test]
    fn operator_delete_dj_linewise_two_lines() {
        let text = "l1\nl2\nl3\nl4\n"; // trailing newline
        let buffer = Buffer::from_str("t", text).unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // d
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('d'),
        );
        // j
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('j'),
        )
        .unwrap();
        if let Action::ApplyOperator { motion, .. } = act {
            assert!(matches!(motion, MotionKind::Down));
        }
        dispatch(act, &mut model, &mut sticky, &[]);
        // Expect lines l3,l4 remain
        let b = model.state().active_buffer();
        assert_eq!(b.line(0).unwrap(), "l3\n");
        assert_eq!(b.line(1).unwrap(), "l4\n");
        // ring contains deleted text (l1 + l2 + newline)
        assert!(model.state().registers.unnamed.contains("l1\nl2\n"));
    }

    #[test]
    fn operator_delete_2dj_linewise_three_lines() {
        let text = "a1\na2\na3\na4\na5\n";
        let buffer = Buffer::from_str("t", text).unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // 2 d j -> should delete three lines total (current + two down) since motion Down with count 2 reaches line index 2 inclusive (a1,a2,a3)
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('2'),
        );
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('d'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('j'),
        )
        .unwrap();
        dispatch(act, &mut model, &mut sticky, &[]);
        let b = model.state().active_buffer();
        assert_eq!(b.line(0).unwrap(), "a4\n");
        assert_eq!(b.line(1).unwrap(), "a5\n");
        assert!(model.state().registers.unnamed.starts_with("a1\na2\na3"));
    }

    #[test]
    fn operator_delete_d2j_linewise_three_lines() {
        let text = "b1\nb2\nb3\nb4\nb5\n";
        let buffer = Buffer::from_str("t", text).unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // d 2 j -> post operator count
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('d'),
        );
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('2'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('j'),
        )
        .unwrap();
        dispatch(act, &mut model, &mut sticky, &[]);
        let b = model.state().active_buffer();
        assert_eq!(b.line(0).unwrap(), "b4\n");
        assert_eq!(b.line(1).unwrap(), "b5\n");
        assert!(model.state().registers.unnamed.starts_with("b1\nb2\nb3"));
    }

    #[test]
    fn structural_multi_line_delete_sets_buffer_replaced() {
        let buffer = Buffer::from_str("t", "a1\na2\na3\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // d j (delete two lines)
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('d'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('j'),
        )
        .unwrap();
        let res = dispatch(act, &mut model, &mut sticky, &[]);
        assert!(
            res.buffer_replaced,
            "multi-line delete must mark structural"
        );
    }

    #[test]
    fn structural_multi_line_delete_then_undo_sets_buffer_replaced() {
        let buffer = Buffer::from_str("t", "b1\nb2\nb3\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // Perform dj
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('d'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('j'),
        )
        .unwrap();
        let res = dispatch(act, &mut model, &mut sticky, &[]);
        assert!(res.buffer_replaced);
        // Undo
        let undo_res = dispatch(Action::Undo, &mut model, &mut sticky, &[]);
        assert!(
            undo_res.buffer_replaced,
            "undo restoring lines must be structural"
        );
    }

    #[test]
    fn single_line_delete_not_structural() {
        let buffer = Buffer::from_str("t", "one two three\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // dw (delete one word inside single line)
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('d'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('w'),
        )
        .unwrap();
        let res = dispatch(act, &mut model, &mut sticky, &[]);
        assert!(res.dirty);
        assert!(
            !res.buffer_replaced,
            "single-line delete should not be structural"
        );
    }

    // --- Step 7 Yank operator tests ---

    fn key_evt(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            mods: KeyModifiers::empty(),
        }
    }

    #[test]
    fn operator_yank_basic_yw() {
        let buffer = Buffer::from_str("t", "one two three\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // y w
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('y'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('w'),
        )
        .unwrap();
        if let Action::ApplyOperator { op, motion, count } = act {
            assert!(matches!(op, OperatorKind::Yank));
            assert!(matches!(motion, MotionKind::WordForward));
            assert_eq!(count, 1);
        } else {
            panic!();
        }
        let pre_text = {
            let b = model.state().active_buffer();
            let mut s = String::new();
            for i in 0..b.line_count() {
                if let Some(l) = b.line(i) {
                    s.push_str(&l);
                }
            }
            s
        };
        let res = dispatch(act, &mut model, &mut sticky, &[]);
        assert!(!res.dirty, "yank should not mark dirty (buffer unchanged)");
        let after = {
            let b = model.state().active_buffer();
            let mut s = String::new();
            for i in 0..b.line_count() {
                if let Some(l) = b.line(i) {
                    s.push_str(&l);
                }
            }
            s
        };
        assert_eq!(after, pre_text);
        assert!(model.state().registers.unnamed.starts_with("one"));
    }

    #[test]
    fn operator_yank_prefix_count_2yw() {
        let buffer = Buffer::from_str("t", "one two three four\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // 2 y w
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('2'),
        );
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('y'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('w'),
        )
        .unwrap();
        if let Action::ApplyOperator { count, .. } = act {
            assert_eq!(count, 2);
        } else {
            panic!();
        }
        let pre = {
            let b = model.state().active_buffer();
            let mut s = String::new();
            for i in 0..b.line_count() {
                if let Some(l) = b.line(i) {
                    s.push_str(&l);
                }
            }
            s
        };
        dispatch(act, &mut model, &mut sticky, &[]);
        let after = {
            let b = model.state().active_buffer();
            let mut s = String::new();
            for i in 0..b.line_count() {
                if let Some(l) = b.line(i) {
                    s.push_str(&l);
                }
            }
            s
        };
        assert_eq!(after, pre);
        assert!(model.state().registers.unnamed.contains("one two"));
    }

    // Change operator tests (Step 8)
    fn change_sequence(model: &mut EditorModel, seq: &str) -> Action {
        let mut last = None;
        for ch in seq.chars() {
            let evt = KeyEvent {
                code: KeyCode::Char(ch),
                mods: KeyModifiers::empty(),
            };
            last = crate::translate_key(
                model.state().mode,
                model.state().command_line.buffer(),
                &evt,
            );
        }
        last.expect("sequence produced final action")
    }

    #[test]
    fn operator_change_basic_cw() {
        let buffer = Buffer::from_str("t", "one two three\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let act = change_sequence(&mut model, "cw");
        let mut sticky = None;
        let res = dispatch(act, &mut model, &mut sticky, &[]);
        assert!(res.dirty);
        assert_eq!(model.state().mode, core_state::Mode::Insert);
        assert!(model.state().registers.unnamed.contains("one"));
        // Buffer should have removed first word only (current resolver yields remaining without leading space)
        let after_line = model.state().active_buffer().line(0).unwrap();
        assert!(after_line.starts_with("two"));
    }

    #[test]
    fn operator_change_prefix_count_2cw() {
        let buffer = Buffer::from_str("t", "one two three four\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let act = change_sequence(&mut model, "2cw");
        let mut sticky = None;
        dispatch(act, &mut model, &mut sticky, &[]);
        assert_eq!(model.state().mode, core_state::Mode::Insert);
        let after_line = model.state().active_buffer().line(0).unwrap();
        // two words removed -> remaining starts with third word directly
        assert!(after_line.starts_with("three"));
    }

    #[test]
    fn operator_change_post_count_c2w() {
        let buffer = Buffer::from_str("t", "one two three four\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let act = change_sequence(&mut model, "c2w");
        let mut sticky = None;
        dispatch(act, &mut model, &mut sticky, &[]);
        assert_eq!(model.state().mode, core_state::Mode::Insert);
        let after_line = model.state().active_buffer().line(0).unwrap();
        assert!(after_line.starts_with("three"));
    }

    #[test]
    fn operator_change_linewise_cj() {
        let buffer = Buffer::from_str("t", "l1\nl2\nl3\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let act = change_sequence(&mut model, "cj");
        let mut sticky = None;
        let res = dispatch(act, &mut model, &mut sticky, &[]);
        assert!(res.buffer_replaced);
        assert_eq!(model.state().mode, core_state::Mode::Insert);
        // first two lines removed; resulting first line expected to be l3
        let after_line0 = model.state().active_buffer().line(0).unwrap();
        assert!(after_line0.starts_with("l3"));
    }

    #[test]
    fn operator_change_linewise_prefix_2cj() {
        let buffer = Buffer::from_str("t", "a1\na2\na3\na4\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let act = change_sequence(&mut model, "2cj");
        let mut sticky = None;
        let res = dispatch(act, &mut model, &mut sticky, &[]);
        assert!(res.buffer_replaced);
        assert_eq!(model.state().mode, core_state::Mode::Insert);
        let after_line0 = model.state().active_buffer().line(0).unwrap();
        // Inclusive vertical motion semantics: prefix count 2 with motion j deletes lines a1..a3, leaving a4
        assert!(after_line0.starts_with("a4"));
    }

    #[test]
    fn operator_metrics_delete_yank_change_counts() {
        let buffer = Buffer::from_str("t", "one two three\nalpha beta gamma\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // d w
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('d'),
        );
        let act_del = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('w'),
        )
        .unwrap();
        dispatch(act_del, &mut model, &mut sticky, &[]);
        // y w
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('y'),
        );
        let act_yank = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('w'),
        )
        .unwrap();
        dispatch(act_yank, &mut model, &mut sticky, &[]);
        // c w
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('c'),
        );
        let act_change = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key('w'),
        )
        .unwrap();
        dispatch(act_change, &mut model, &mut sticky, &[]);
        let snap = model.state().operator_metrics_snapshot();
        assert_eq!(snap.operator_delete, 1);
        assert_eq!(snap.operator_yank, 1);
        assert_eq!(snap.operator_change, 1);
        // At least three register writes (one per op) though change/delete may rotate.
        assert!(snap.register_writes >= 3);
    }

    #[test]
    fn operator_metrics_numbered_ring_rotation() {
        // Build buffer with many distinct words so each yank is unique
        let text = "w1 w2 w3 w4 w5 w6 w7 w8 w9 w10 w11 w12\n";
        let buffer = Buffer::from_str("t", text).unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // Perform more than ring capacity yanks (Registers::MAX == 10)
        for _ in 0..12 {
            translate_key(
                model.state().mode,
                model.state().command_line.buffer(),
                &key('y'),
            );
            let act = translate_key(
                model.state().mode,
                model.state().command_line.buffer(),
                &key('w'),
            )
            .unwrap();
            dispatch(act, &mut model, &mut sticky, &[]);
        }
        let snap = model.state().operator_metrics_snapshot();
        assert_eq!(snap.operator_yank, 12);
        // Rotations should be >= (yanks - capacity) i.e. at least 2
        assert!(snap.numbered_ring_rotations >= 2);
    }

    #[test]
    fn operator_yank_post_count_y2w() {
        let buffer = Buffer::from_str("t", "one two three four\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // y 2 w
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('y'),
        );
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('2'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('w'),
        )
        .unwrap();
        if let Action::ApplyOperator { count, .. } = act {
            assert_eq!(count, 2);
        } else {
            panic!();
        }
        let pre = {
            let b = model.state().active_buffer();
            let mut s = String::new();
            for i in 0..b.line_count() {
                if let Some(l) = b.line(i) {
                    s.push_str(&l);
                }
            }
            s
        };
        dispatch(act, &mut model, &mut sticky, &[]);
        let after = {
            let b = model.state().active_buffer();
            let mut s = String::new();
            for i in 0..b.line_count() {
                if let Some(l) = b.line(i) {
                    s.push_str(&l);
                }
            }
            s
        };
        assert_eq!(after, pre);
        assert!(model.state().registers.unnamed.contains("one two"));
    }

    #[test]
    fn operator_yank_linewise_yj() {
        let buffer = Buffer::from_str("t", "l1\nl2\nl3\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // y j
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('y'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('j'),
        )
        .unwrap();
        let pre = {
            let b = model.state().active_buffer();
            let mut s = String::new();
            for i in 0..b.line_count() {
                if let Some(l) = b.line(i) {
                    s.push_str(&l);
                }
            }
            s
        };
        dispatch(act, &mut model, &mut sticky, &[]);
        let after = {
            let b = model.state().active_buffer();
            let mut s = String::new();
            for i in 0..b.line_count() {
                if let Some(l) = b.line(i) {
                    s.push_str(&l);
                }
            }
            s
        };
        assert_eq!(after, pre);
        assert!(model.state().registers.unnamed.contains("l1"));
        assert!(model.state().registers.unnamed.contains("l2"));
    }

    #[test]
    fn operator_yank_linewise_count_2yj() {
        let buffer = Buffer::from_str("t", "a1\na2\na3\na4\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // 2 y j (captures three lines total like 2dj semantics for delete)
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('2'),
        );
        translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('y'),
        );
        let act = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &key_evt('j'),
        )
        .unwrap();
        dispatch(act, &mut model, &mut sticky, &[]);
        assert!(model.state().registers.unnamed.contains("a1"));
        assert!(model.state().registers.unnamed.contains("a2"));
        assert!(model.state().registers.unnamed.contains("a3"));
        assert!(!model.state().registers.unnamed.contains("a4"));
    }
}
