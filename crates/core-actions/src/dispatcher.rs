//! Dispatcher applying `Action` to mutable editor state (Refactor R1 Step 5).
//!
//! Breadth-first extraction from `ox-bin/src/main.rs`. Behavior intentionally
//! unchanged; future evolution will:
//! * Split motion/edit/command application into dedicated sub-modules.
//! * Emit structured render deltas instead of a boolean dirty flag.
//! * Integrate observer hooks (macro recorder, analytics) before mutation.

use crate::{Action, ActionObserver, EditKind, ModeChange, MotionKind};
use core_state::{EditorState, Mode};
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
        Action::CommandInput(ch) => {
            if ch == '\u{08}' {
                state.command_line.backspace();
            } else {
                state.command_line.push_char(ch);
            }
            DispatchResult::dirty()
        }
        Action::CommandExecute(cmd) => {
            if cmd == ":q" {
                return DispatchResult::quit();
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
                    DispatchResult::dirty()
                } else {
                    DispatchResult::clean()
                }
            }
            EditKind::DeleteUnder => {
                if matches!(state.mode, Mode::Normal) {
                    state.push_discrete_edit_snapshot();
                    let mut pos = state.position;
                    state.active_buffer_mut().delete_grapheme_at(&mut pos);
                    state.position = pos;
                    DispatchResult::dirty()
                } else {
                    DispatchResult::clean()
                }
            }
        },
        Action::Undo => {
            if state.undo() {
                DispatchResult::dirty()
            } else {
                DispatchResult::clean()
            }
        }
        Action::Redo => {
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
        dispatch(Action::CommandInput(':'), &mut state, &mut sticky, &[]);
        dispatch(Action::CommandInput('q'), &mut state, &mut sticky, &[]);
        let res = dispatch(
            Action::CommandExecute(":q".into()),
            &mut state,
            &mut sticky,
            &[],
        );
        assert!(res.quit && res.dirty);
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
}
