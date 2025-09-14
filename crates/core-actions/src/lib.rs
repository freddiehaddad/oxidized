//! High-level semantic editor actions and translation from raw input.
//! Phase 1 Task 9.4–9.5: Introduce Action abstraction.

use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_state::Mode;

/// Observer hook (Refactor R1 Step 8): allows external components (macro recorder, analytics,
/// future plugin host) to observe Actions as they are translated and/or dispatched without
/// mutating editor state. Breadth-first: only pre-dispatch hook provided now.
///
/// Observers MUST be cheap and non-blocking; heavy work should be offloaded asynchronously.
pub trait ActionObserver: Send + Sync {
    /// Called immediately before an Action is dispatched (state not yet mutated).
    fn on_action(&self, action: &Action);
}

impl<T: ActionObserver + ?Sized> ActionObserver for &T {
    fn on_action(&self, action: &Action) {
        (**self).on_action(action)
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    Motion(MotionKind),
    Edit(EditKind),
    ModeChange(ModeChange),
    Undo,
    Redo,
    // Command line actions (Task 7.1 Action Enum Refinement)
    CommandStart,           // begin command line (inserts leading ':')
    CommandChar(char),      // insert character into command buffer
    CommandBackspace,       // remove last character or cancel if only ':'
    CommandCancel,          // abort command (Esc)
    CommandExecute(String), // execute full buffer (still includes leading ':')
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionKind {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    WordForward,
    WordBackward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditKind {
    InsertGrapheme(String),
    InsertNewline,
    Backspace,
    DeleteUnder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeChange {
    EnterInsert,
    LeaveInsert,
}

/// Pure translation from a key event + editor mode + current command buffer into an Action.
/// This does NOT mutate state and is safe to unit test in isolation.
pub fn translate_key(mode: Mode, pending_command: &str, key: &KeyEvent) -> Option<Action> {
    let span = tracing::trace_span!(
        "translate_key",
        mode = ?mode,
        pending_len = pending_command.len(),
        key = ?key.code,
        ctrl = key.mods.contains(KeyModifiers::CTRL),
    );
    let _e = span.enter();
    match key.code {
        // Command start only if not already active. Runtime input thread currently emits
        // `KeyCode::Colon` (distinct variant) while some tests previously constructed
        // `KeyCode::Char(':')`. Support BOTH to avoid a silent divergence like the regression
        // where ':' stopped opening the command line. Treat them identically.
        KeyCode::Char(':') | KeyCode::Colon => {
            if pending_command.is_empty() {
                Some(Action::CommandStart)
            } else if pending_command.starts_with(':') {
                // Treat additional ':' as just another char in command line
                Some(Action::CommandChar(':'))
            } else {
                None
            }
        }
        KeyCode::Char(c) => {
            if pending_command.starts_with(':') {
                return Some(Action::CommandChar(c));
            }
            match mode {
                Mode::Normal => match c {
                    // Ctrl-R redo mapping: treat only control-modified 'r' as redo (reserve plain 'r' for future replace semantics)
                    'r' if key.mods.contains(KeyModifiers::CTRL) => Some(Action::Redo),
                    'h' => Some(Action::Motion(MotionKind::Left)),
                    'l' => Some(Action::Motion(MotionKind::Right)),
                    'j' => Some(Action::Motion(MotionKind::Down)),
                    'k' => Some(Action::Motion(MotionKind::Up)),
                    '0' => Some(Action::Motion(MotionKind::LineStart)),
                    '$' => Some(Action::Motion(MotionKind::LineEnd)),
                    'w' => Some(Action::Motion(MotionKind::WordForward)),
                    'b' => Some(Action::Motion(MotionKind::WordBackward)),
                    'i' => Some(Action::ModeChange(ModeChange::EnterInsert)),
                    'u' if !key.mods.contains(KeyModifiers::CTRL) => Some(Action::Undo),
                    'x' => Some(Action::Edit(EditKind::DeleteUnder)),
                    _ => None,
                },
                Mode::Insert => {
                    if !c.is_control() {
                        Some(Action::Edit(EditKind::InsertGrapheme(c.to_string())))
                    } else {
                        None
                    }
                }
            }
        }
        KeyCode::Enter => {
            if pending_command.starts_with(':') {
                Some(Action::CommandExecute(pending_command.to_string()))
            } else if matches!(mode, Mode::Insert) {
                Some(Action::Edit(EditKind::InsertNewline))
            } else {
                None
            }
        }
        KeyCode::Backspace => {
            if pending_command.starts_with(':') {
                Some(Action::CommandBackspace)
            } else if matches!(mode, Mode::Insert) {
                Some(Action::Edit(EditKind::Backspace))
            } else {
                None
            }
        }
        KeyCode::Esc => {
            if pending_command.starts_with(':') {
                Some(Action::CommandCancel)
            } else if matches!(mode, Mode::Insert) {
                Some(Action::ModeChange(ModeChange::LeaveInsert))
            } else {
                None
            }
        }
        KeyCode::Left => Some(Action::Motion(MotionKind::Left)),
        KeyCode::Right => Some(Action::Motion(MotionKind::Right)),
        KeyCode::Up => Some(Action::Motion(MotionKind::Up)),
        KeyCode::Down => Some(Action::Motion(MotionKind::Down)),
        _ => None,
    }
}

pub mod dispatcher;

#[cfg(test)]
mod tests {
    use super::*;
    use core_events::{KeyEvent, KeyModifiers};
    fn kc(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            mods: KeyModifiers::empty(),
        }
    }

    #[test]
    fn normal_mode_motion() {
        assert!(matches!(
            translate_key(Mode::Normal, "", &kc('h')),
            Some(Action::Motion(MotionKind::Left))
        ));
        assert!(translate_key(Mode::Normal, "", &kc('z')).is_none());
    }

    #[test]
    fn insert_mode_inserts() {
        assert!(
            matches!(translate_key(Mode::Insert, "", &kc('a')), Some(Action::Edit(EditKind::InsertGrapheme(ref s))) if s=="a")
        );
    }

    #[test]
    fn command_sequence_translation() {
        // start
        let start = translate_key(Mode::Normal, "", &kc(':'));
        assert!(matches!(start, Some(Action::CommandStart)));
        // after ':' pending buffer would be ':'; simulate adding 'q'
        let q = translate_key(Mode::Normal, ":", &kc('q'));
        assert!(matches!(q, Some(Action::CommandChar('q'))));
        // Enter executes
        let enter = translate_key(
            Mode::Normal,
            ":q",
            &KeyEvent {
                code: KeyCode::Enter,
                mods: KeyModifiers::empty(),
            },
        );
        assert!(matches!(enter, Some(Action::CommandExecute(ref s)) if s==":q"));
        // Esc cancels when active
        let esc = translate_key(
            Mode::Normal,
            ":q",
            &KeyEvent {
                code: KeyCode::Esc,
                mods: KeyModifiers::empty(),
            },
        );
        assert!(matches!(esc, Some(Action::CommandCancel)));
        // Backspace
        let bs = translate_key(
            Mode::Normal,
            ":q",
            &KeyEvent {
                code: KeyCode::Backspace,
                mods: KeyModifiers::empty(),
            },
        );
        assert!(matches!(bs, Some(Action::CommandBackspace)));
    }

    #[test]
    fn ctrl_r_maps_to_redo() {
        use core_events::{KeyCode, KeyEvent, KeyModifiers};
        let evt = KeyEvent {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::CTRL,
        };
        let act = translate_key(Mode::Normal, "", &evt);
        assert!(
            matches!(act, Some(Action::Redo)),
            "Ctrl-R should map to Redo action"
        );
        // Ensure plain 'r' (no ctrl) is currently unbound (reserved for future replace semantics)
        let plain = KeyEvent {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::empty(),
        };
        assert!(translate_key(Mode::Normal, "", &plain).is_none());
    }

    #[test]
    fn colon_variant_translation() {
        // Explicitly construct a KeyEvent using the Colon variant (emitted by input thread)
        let colon_event = KeyEvent {
            code: KeyCode::Colon,
            mods: KeyModifiers::empty(),
        };
        let start = translate_key(Mode::Normal, "", &colon_event);
        assert!(
            matches!(start, Some(Action::CommandStart)),
            "Colon variant should start command mode"
        );
    }
}
