//! KeyTranslator: stateful key->Action translation (Refactor R3 Step 3).
//!
//! This struct encapsulates transient translation state (counts and pending
//! operator scaffolding), preparing for Phase 4 expansion. For now it behaves
//! identically to the historical stateless `translate_key` free function:
//! * No count accumulation yet (digits are passed through as literals unless
//!   part of a command buffer).
//! * No operator-pending state; related fields exist but remain `None`.
//!
//! Design Tenet Alignment:
//! * Modularity: isolates future complexity (counts/operators) from callers.
//! * Unicode correctness: defers grapheme decisions to edit paths (unchanged).
//! * Evolution over legacy: stateless function remains a thin wrapper for
//!   backward test compatibility and will be removed once counts/operators
//!   ship.
//!
//! Forward Roadmap (Phase 4):
//! * Accumulate numeric prefix before motion/operator (e.g. `5j`).
//! * Support operator capture (e.g. `d` then motion) producing composite
//!   action variants.
//! * Integrate register capture and dot-repeat state tracking.
//! * Provide reset semantics on mode switches and command entry.

use crate::{Action, EditKind, ModeChange, MotionKind};
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_state::Mode;

#[derive(Debug, Default)]
pub struct KeyTranslator {
    pending_count: Option<u32>,
    // Placeholder for operator kind once Step 4 introduces OperatorKind.
    pending_operator: Option<()>,
}

impl KeyTranslator {
    pub fn new() -> Self {
        Self {
            pending_count: None,
            pending_operator: None,
        }
    }

    /// Reset transient state (counts/operators). Intentionally unused until
    /// counts/operator-pending logic (Phase 4) lands. Kept public so the
    /// runtime can invoke it on mode transitions without further signature
    /// churn.
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.pending_count = None;
        self.pending_operator = None;
    }

    /// Core translation entrypoint. Mirrors previous `translate_key` behavior.
    pub fn translate(
        &mut self,
        mode: Mode,
        pending_command: &str,
        key: &KeyEvent,
    ) -> Option<Action> {
        // Command-line active: delegate directly (counts do not apply inside ':')
        if pending_command.starts_with(':') {
            return legacy_map(mode, pending_command, key);
        }
        // Only Normal mode supports count prefixes at this stage.
        if matches!(mode, Mode::Normal) {
            if let KeyCode::Char(c) = key.code {
                if c.is_ascii_digit() {
                    // Vim rule: a solitary leading '0' with no accumulated count maps to LineStart motion.
                    if c == '0' && self.pending_count.is_none() {
                        return Some(Action::Motion(MotionKind::LineStart));
                    }
                    let digit = (c as u8 - b'0') as u32;
                    let new_val = self
                        .pending_count
                        .unwrap_or(0)
                        .saturating_mul(10)
                        .saturating_add(digit)
                        .min(999_999); // clamp safety consistent with design constant
                    self.pending_count = Some(new_val);
                    return None; // wait for following motion
                }
                // Non-digit: if we have an accumulated count and this char is a motion, emit MotionWithCount.
                if let Some(count) = self.pending_count.take() {
                    let mapped = legacy_map(mode, pending_command, key);
                    if let Some(Action::Motion(m)) = mapped {
                        return Some(Action::MotionWithCount { motion: m, count });
                    } else {
                        // Not a motion -> restore count? Vim typically treats e.g. 12i as entering insert with count ignored.
                        // For simplicity breadth-first: fall back to mapped action; count discarded (will refine if needed).
                        return mapped;
                    }
                }
            }
            // Esc cancels pending count.
            if matches!(key.code, KeyCode::Esc) {
                self.pending_count = None;
            }
        }
        legacy_map(mode, pending_command, key)
    }
}

fn legacy_map(mode: Mode, pending_command: &str, key: &KeyEvent) -> Option<Action> {
    let span = tracing::trace_span!(
        "translate_key_stateful",
        mode = ?mode,
        pending_len = pending_command.len(),
        key = ?key.code,
        ctrl = key.mods.contains(KeyModifiers::CTRL),
    );
    let _e = span.enter();
    match key.code {
        KeyCode::Char('d')
            if key.mods.contains(KeyModifiers::CTRL) && matches!(mode, Mode::Normal) =>
        {
            Some(Action::Motion(MotionKind::PageHalfDown))
        }
        KeyCode::Char('u')
            if key.mods.contains(KeyModifiers::CTRL) && matches!(mode, Mode::Normal) =>
        {
            Some(Action::Motion(MotionKind::PageHalfUp))
        }
        KeyCode::Char(':') => {
            if pending_command.is_empty() {
                Some(Action::CommandStart)
            } else if pending_command.starts_with(':') {
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

#[cfg(test)]
mod tests {
    use super::*;
    use core_events::{KeyCode, KeyEvent, KeyModifiers};

    fn kc(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            mods: KeyModifiers::empty(),
        }
    }

    #[test]
    fn parity_basic_motions() {
        let mut tr = KeyTranslator::new();
        assert!(matches!(
            tr.translate(Mode::Normal, "", &kc('h')),
            Some(Action::Motion(MotionKind::Left))
        ));
        assert!(tr.translate(Mode::Normal, "", &kc('z')).is_none());
        // Exercise reset (currently a no-op state clear) to keep method live.
        tr.reset();
    }

    #[test]
    fn parity_insert() {
        let mut tr = KeyTranslator::new();
        assert!(
            matches!(tr.translate(Mode::Insert, "", &kc('a')), Some(Action::Edit(EditKind::InsertGrapheme(ref s))) if s=="a")
        );
    }

    #[test]
    fn parity_command_sequence() {
        let mut tr = KeyTranslator::new();
        assert!(matches!(
            tr.translate(Mode::Normal, "", &kc(':')),
            Some(Action::CommandStart)
        ));
        assert!(matches!(
            tr.translate(Mode::Normal, ":", &kc('q')),
            Some(Action::CommandChar('q'))
        ));
        let enter = KeyEvent {
            code: KeyCode::Enter,
            mods: KeyModifiers::empty(),
        };
        assert!(
            matches!(tr.translate(Mode::Normal, ":q", &enter), Some(Action::CommandExecute(ref s)) if s==":q")
        );
    }

    #[test]
    fn parity_ctrl_r_and_esc() {
        let mut tr = KeyTranslator::new();
        let ctrl_r = KeyEvent {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::CTRL,
        };
        assert!(matches!(
            tr.translate(Mode::Normal, "", &ctrl_r),
            Some(Action::Redo)
        ));
        let plain_r = KeyEvent {
            code: KeyCode::Char('r'),
            mods: KeyModifiers::empty(),
        };
        assert!(tr.translate(Mode::Normal, "", &plain_r).is_none());
        let esc = KeyEvent {
            code: KeyCode::Esc,
            mods: KeyModifiers::empty(),
        };
        assert!(matches!(
            tr.translate(Mode::Insert, "", &esc),
            Some(Action::ModeChange(ModeChange::LeaveInsert))
        ));
    }

    #[test]
    fn count_accumulation_basic() {
        let mut tr = KeyTranslator::new();
        // 5l -> move right 5 times => MotionWithCount
        let five = KeyEvent {
            code: KeyCode::Char('5'),
            mods: KeyModifiers::empty(),
        };
        assert!(tr.translate(Mode::Normal, "", &five).is_none());
        let ell = KeyEvent {
            code: KeyCode::Char('l'),
            mods: KeyModifiers::empty(),
        };
        match tr.translate(Mode::Normal, "", &ell) {
            Some(Action::MotionWithCount {
                motion: MotionKind::Right,
                count,
            }) => assert_eq!(count, 5),
            other => panic!("expected MotionWithCount, got {:?}", other),
        }
    }

    #[test]
    fn zero_rule_line_start() {
        let mut tr = KeyTranslator::new();
        let zero = KeyEvent {
            code: KeyCode::Char('0'),
            mods: KeyModifiers::empty(),
        };
        // Leading zero with no prior count -> LineStart motion
        assert!(matches!(
            tr.translate(Mode::Normal, "", &zero),
            Some(Action::Motion(MotionKind::LineStart))
        ));
        // Now accumulate 10 by pressing '1','0' then 'l'
        let one = KeyEvent {
            code: KeyCode::Char('1'),
            mods: KeyModifiers::empty(),
        };
        assert!(tr.translate(Mode::Normal, "", &one).is_none());
        assert!(tr.translate(Mode::Normal, "", &zero).is_none());
        let ell = KeyEvent {
            code: KeyCode::Char('l'),
            mods: KeyModifiers::empty(),
        };
        match tr.translate(Mode::Normal, "", &ell) {
            Some(Action::MotionWithCount {
                motion: MotionKind::Right,
                count,
            }) => assert_eq!(count, 10),
            other => panic!("expected MotionWithCount(10), got {:?}", other),
        }
    }
}
