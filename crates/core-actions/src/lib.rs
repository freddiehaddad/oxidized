//! High-level semantic editor actions and translation from raw input.
//! Phase 1 Task 9.4–9.5: Introduce Action abstraction.

use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_state::Mode;

#[derive(Debug, Clone)]
pub enum Action {
    Motion(MotionKind),
    Edit(EditKind),
    ModeChange(ModeChange),
    Undo,
    Redo,
    CommandInput(char),
    CommandExecute(String),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionKind { Left, Right, Up, Down, LineStart, LineEnd, WordForward, WordBackward }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditKind { InsertGrapheme(String), InsertNewline, Backspace, DeleteUnder }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeChange { EnterInsert, LeaveInsert }

/// Pure translation from a key event + editor mode + current command buffer into an Action.
/// This does NOT mutate state and is safe to unit test in isolation.
pub fn translate_key(mode: Mode, pending_command: &str, key: &KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char(':') => {
            if pending_command.is_empty() { return Some(Action::CommandInput(':')); }
            None
        }
        KeyCode::Char(c) => {
            // Command-line accumulation
            if pending_command.starts_with(':') {
                return Some(Action::CommandInput(c));
            }
            match mode {
                Mode::Normal => match c {
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
                    _ => None,
                },
                Mode::Insert => {
                    // Printable insertion (grapheme boundaries handled upstream by terminal input)
                    if !c.is_control() { return Some(Action::Edit(EditKind::InsertGrapheme(c.to_string()))); }
                    None
                }
            }
        }
        KeyCode::Enter => {
            if pending_command.starts_with(':') {
                // Execution handled later; translator just signals command execute intent when Enter.
                Some(Action::CommandExecute(pending_command.to_string()))
            } else if matches!(mode, Mode::Insert) {
                Some(Action::Edit(EditKind::InsertNewline))
            } else { None }
        }
        KeyCode::Backspace => {
            if matches!(mode, Mode::Insert) {
                Some(Action::Edit(EditKind::Backspace))
            } else if pending_command.starts_with(':') && !pending_command.is_empty() {
                // Backspace inside command-line: treat as removing last char (will be handled by caller) -> signal input with sentinel? For now reuse CommandInput with '\u{08}'
                Some(Action::CommandInput('\u{08}'))
            } else { None }
        }
        KeyCode::Esc => {
            if pending_command.starts_with(':') {
                Some(Action::CommandExecute(String::new())) // special meaning: cancel
            } else if matches!(mode, Mode::Insert) {
                Some(Action::ModeChange(ModeChange::LeaveInsert))
            } else { None }
        }
        KeyCode::Left => Some(Action::Motion(MotionKind::Left)),
        KeyCode::Right => Some(Action::Motion(MotionKind::Right)),
        KeyCode::Up => Some(Action::Motion(MotionKind::Up)),
        KeyCode::Down => Some(Action::Motion(MotionKind::Down)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_events::{KeyEvent, KeyModifiers};
    fn kc(c: char) -> KeyEvent { KeyEvent { code: KeyCode::Char(c), mods: KeyModifiers::empty() } }

    #[test]
    fn normal_mode_motion() {
        assert!(matches!(translate_key(Mode::Normal, "", &kc('h')), Some(Action::Motion(MotionKind::Left))));
        assert!(translate_key(Mode::Normal, "", &kc('z')).is_none());
    }

    #[test]
    fn insert_mode_inserts() {
        assert!(matches!(translate_key(Mode::Insert, "", &kc('a')), Some(Action::Edit(EditKind::InsertGrapheme(ref s))) if s=="a"));
    }
}
