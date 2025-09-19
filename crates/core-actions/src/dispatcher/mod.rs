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
                    // Step 6.2: Linewise vertical delete semantics.
                    // If motion is vertical (Up/Down/PageHalfUp/PageHalfDown) treat the
                    // operator as *linewise*: delete whole lines from the starting line
                    // through the target line inclusive. Otherwise use character span.
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
                        // Compute absolute byte start of first line and start of line after last.
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
                        state.registers_mut().record_delete(removed);
                        view.cursor = cursor; // positioned at start of first removed line
                        if !state.dirty {
                            state.dirty = true;
                        }
                        // Structural: multiple lines may have shifted; request full repaint via dirty flag only for now.
                        DispatchResult::dirty()
                    } else {
                        // Characterwise path (existing semantics)
                        let span = resolve_span(state, start_pos, motion, count);
                        if span.start == span.end {
                            return DispatchResult::clean();
                        }
                        let mut cursor = view.cursor; // copy for deletion API
                        let removed =
                            state.delete_span_with_snapshot(&mut cursor, span.start, span.end);
                        state.registers_mut().record_delete(removed);
                        view.cursor = cursor;
                        if !state.dirty {
                            state.dirty = true;
                        }
                        DispatchResult::dirty()
                    }
                }
                _ => DispatchResult::clean(), // Yank/Change later steps
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
}
