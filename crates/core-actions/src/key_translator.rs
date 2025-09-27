//! KeyTranslator: stateful key->Action translation.
//!
//! Phase 4 Progress:
//! * Step 1: Count accumulation for motions (e.g. `5l`) -> `MotionWithCount`.
//! * Step 2: Operator pending state (`d`,`y`,`c`) + composite
//!   `ApplyOperator { op, motion, count }` emission with multiplicative count
//!   semantics (e.g. `2d3w` => count 6). Vim rule for `0` as a motion after an
//!   operator (e.g. `d0`) is preserved (treat as `LineStart` motion rather than
//!   starting a trailing count).
//!
//! State Machine (minimal):
//! * Idle: optional `pending_count` accumulating prefix digits.
//! * OperatorPending(op): operator captured; may accumulate a post-operator
//!   count (`post_op_count`) via digits 1-9 (leading 0 is NOT a count and is a
//!   motion `LineStart`).
//! * On motion while OperatorPending -> emit `ApplyOperator` with
//!   `count = prefix_count * post_op_count` (default 1). State resets.
//! * <Esc> while pending operator cancels and resets state silently.
//!
//! Breadth-First Guarantee: Dispatcher still treats operator actions as
//! inert (no buffer mutation yet). Later steps will implement span
//! resolution & actual delete/yank/change semantics.
//!
//! Design Tenets Applied:
//! * Modularity: confines complexity to this translator.
//! * Evolution: incremental activation per design plan.
//! * Safety: clamped counts (<= 999_999) prevent overflow.

use crate::{Action, EditKind, ModeChange, MotionKind, OperatorKind};
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_state::Mode;

#[derive(Debug, Default)]
pub struct KeyTranslator {
    /// Count prefix prior to an operator or motion (e.g. `12d` or `12w`).
    pending_count: Option<u32>,
    /// Pending operator kind (d,y,c) awaiting motion.
    pending_operator: Option<OperatorKind>,
    /// Count following an operator but before the motion (e.g. `d3w`).
    post_op_count: Option<u32>,
    /// Pending explicit register (after '"')
    pending_register: Option<char>,
    /// True if we just saw '"' and expect a register designator next.
    awaiting_register: bool,
}

impl KeyTranslator {
    pub fn new() -> Self {
        Self {
            pending_count: None,
            pending_operator: None,
            post_op_count: None,
            pending_register: None,
            awaiting_register: false,
        }
    }

    /// Reset transient state (counts/operators). Intentionally unused until
    /// counts/operator-pending logic (Phase 4) lands. Kept public so the
    /// runtime can invoke it on mode transitions without further signature
    /// churn.
    pub fn reset(&mut self) {
        self.pending_count = None;
        self.pending_operator = None;
        self.post_op_count = None;
        self.pending_register = None;
        self.awaiting_register = false;
    }

    /// Core translation entrypoint. Mirrors previous `translate_key` behavior.
    pub fn translate(
        &mut self,
        mode: Mode,
        pending_command: &str,
        key: &KeyEvent,
    ) -> Option<Action> {
        // Command-line active: delegate directly (counts/operators/registers do not apply inside ':')
        if pending_command.starts_with(':') {
            return legacy_map(mode, pending_command, key);
        }
        if !matches!(mode, Mode::Normal) {
            // Allow select VisualChar motions (half-page) to mirror Normal semantics.
            if matches!(mode, Mode::VisualChar) && key.mods.contains(KeyModifiers::CTRL) {
                if let KeyCode::Char('d') = key.code {
                    return Some(Action::Motion(MotionKind::PageHalfDown));
                }
                if let KeyCode::Char('u') = key.code {
                    return Some(Action::Motion(MotionKind::PageHalfUp));
                }
            }
            return legacy_map(mode, pending_command, key);
        }

        // Step 6.1: Ctrl-D / Ctrl-U precedence fix.
        // These half-page motions must be interpreted BEFORE any operator
        // pending or count accumulation logic (e.g. Vim's behavior: typing
        // `d` then <C-d> should scroll half a page down and NOT trigger a
        // delete). We therefore short-circuit here. Any pending operator or
        // counts are canceled (breadth-first safety: explicit over implicit).
        if key.mods.contains(KeyModifiers::CTRL) {
            if let KeyCode::Char('d') = key.code {
                self.reset();
                return Some(Action::Motion(MotionKind::PageHalfDown));
            }
            if let KeyCode::Char('u') = key.code {
                self.reset();
                return Some(Action::Motion(MotionKind::PageHalfUp));
            }
        }

        match key.code {
            KeyCode::Esc => {
                // Cancel any pending state.
                self.reset();
                return None;
            }
            KeyCode::Char(c) => {
                // Register prefix entry point (only in Normal / VisualChar like Vim). Occurs before counts/operators.
                if c == '"' {
                    self.awaiting_register = true;
                    self.pending_register = None; // reset previously captured register
                    return None;
                }
                if self.awaiting_register {
                    self.awaiting_register = false;
                    // Valid registers: a-z (named) or A-Z (append). Numbered 0-9 accepted (deferred semantics for Step 7) but stored.
                    if c.is_ascii_alphanumeric() {
                        self.pending_register = Some(c);
                        return None; // continue gathering operator/motion
                    } else {
                        // Invalid register specifier -> drop silently (breadth-first resilience)
                        self.pending_register = None;
                        return None;
                    }
                }
                // If we are currently waiting for a motion after an operator.
                if let Some(op) = self.pending_operator {
                    // Digits after operator may form a secondary count except leading '0'.
                    if c.is_ascii_digit() {
                        if c == '0' && self.post_op_count.is_none() {
                            // Treat as motion LineStart (d0 behavior)
                            let count_total = self
                                .pending_count
                                .unwrap_or(1)
                                .saturating_mul(self.post_op_count.unwrap_or(1))
                                .min(999_999);
                            self.pending_operator = None;
                            self.post_op_count = None;
                            self.pending_count = None; // counts consumed
                            return Some(Action::ApplyOperator {
                                op,
                                motion: MotionKind::LineStart,
                                count: count_total,
                                register: self.pending_register.take(),
                            });
                        }
                        // Accumulate post-op count (digits 1-9 start, 0 allowed once started)
                        let digit = (c as u8 - b'0') as u32;
                        let new_val = self
                            .post_op_count
                            .unwrap_or(0)
                            .saturating_mul(10)
                            .saturating_add(digit)
                            .min(999_999);
                        self.post_op_count = Some(new_val);
                        return None;
                    }
                    if c == operator_char(op) {
                        let prefix = self.pending_count.unwrap_or(1);
                        let post = self.post_op_count.unwrap_or(1);
                        let total = prefix.saturating_mul(post).min(999_999);
                        self.pending_operator = None;
                        self.post_op_count = None;
                        self.pending_count = None;
                        return Some(Action::LinewiseOperator {
                            op,
                            count: total.max(1),
                            register: self.pending_register.take(),
                        });
                    }
                    // Non-digit: attempt to map to a motion.
                    if let Some(Action::Motion(m)) = legacy_map(mode, pending_command, key) {
                        let prefix = self.pending_count.unwrap_or(1);
                        let post = self.post_op_count.unwrap_or(1);
                        let total = prefix.saturating_mul(post).min(999_999);
                        self.pending_operator = None;
                        self.post_op_count = None;
                        self.pending_count = None; // counts consumed
                        return Some(Action::ApplyOperator {
                            op,
                            motion: m,
                            count: total,
                            register: self.pending_register.take(),
                        });
                    } else {
                        // Not a motion; cancel operator and treat key normally.
                        self.pending_operator = None;
                        self.post_op_count = None;
                        // retain pending_register for subsequent operator if any
                        // pending_count intentionally retained: e.g. 2d<non-motion> should ignore operator but still allow count-l motion later.
                        return legacy_map(mode, pending_command, key);
                    }
                }

                // No operator pending: maybe digit (count) or operator key or ordinary motion.
                if c.is_ascii_digit() {
                    // Leading '0' with no current count -> motion LineStart
                    if c == '0' && self.pending_count.is_none() {
                        return Some(Action::Motion(MotionKind::LineStart));
                    }
                    let digit = (c as u8 - b'0') as u32;
                    let new_val = self
                        .pending_count
                        .unwrap_or(0)
                        .saturating_mul(10)
                        .saturating_add(digit)
                        .min(999_999);
                    self.pending_count = Some(new_val);
                    return None;
                }
                // Operator keys begin pending operator sequence.
                let op_kind = match c {
                    'd' => Some(OperatorKind::Delete),
                    'y' => Some(OperatorKind::Yank),
                    'c' => Some(OperatorKind::Change),
                    _ => None,
                };
                if let Some(kind) = op_kind {
                    self.pending_operator = Some(kind);
                    self.post_op_count = None;
                    return None; // no immediate action emitted (BeginOperator variant kept inert)
                }

                // If a count was accumulated and this is now a motion, emit MotionWithCount.
                if let Some(count) = self.pending_count.take() {
                    if let Some(Action::Motion(m)) = legacy_map(mode, pending_command, key) {
                        return Some(Action::MotionWithCount { motion: m, count });
                    } else {
                        // Non-motion after count (e.g. 12i) -> drop count breadth-first.
                        return legacy_map(mode, pending_command, key);
                    }
                }
            }
            _ => {}
        }
        // Fallback: legacy mapping.
        let mut act = legacy_map(mode, pending_command, key);
        // Attach pending register to paste actions if present.
        if let Some(reg) = self.pending_register.take() {
            if let Some(a) = act.take() {
                act = Some(match a {
                    Action::PasteAfter { .. } => Action::PasteAfter {
                        register: Some(reg),
                    },
                    Action::PasteBefore { .. } => Action::PasteBefore {
                        register: Some(reg),
                    },
                    Action::VisualOperator { op, register: _ } => Action::VisualOperator {
                        op,
                        register: Some(reg),
                    },
                    // For motions or others, we just store register and continue (in Vim register prefix must precede an operation).
                    other => other,
                });
            } else {
                // No action produced; keep register for next keypress.
                self.pending_register = Some(reg);
            }
        }
        act
    }
}

fn operator_char(op: OperatorKind) -> char {
    match op {
        OperatorKind::Delete => 'd',
        OperatorKind::Yank => 'y',
        OperatorKind::Change => 'c',
    }
}

fn legacy_map(mode: Mode, pending_command: &str, key: &KeyEvent) -> Option<Action> {
    // Lightweight translation event (trace) capturing mode + key + pending lengths.
    // Action classification decision happens at wrapper call site; here we log raw translation attempt.
    tracing::trace!(target: "actions.translate", mode=?mode, pending_len=pending_command.len(), key=?key.code, ctrl=key.mods.contains(KeyModifiers::CTRL), "translate_key_attempt");
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
                    'v' => Some(Action::ModeChange(ModeChange::EnterVisualChar)),
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
                    'p' => Some(Action::PasteAfter { register: None }),
                    'P' => Some(Action::PasteBefore { register: None }),
                    _ => None,
                },
                Mode::Insert => {
                    if !c.is_control() {
                        Some(Action::Edit(EditKind::InsertGrapheme(c.to_string())))
                    } else {
                        None
                    }
                }
                Mode::VisualChar => match c {
                    // Motions behave like Normal for now (Step 2 scope).
                    'h' => Some(Action::Motion(MotionKind::Left)),
                    'l' => Some(Action::Motion(MotionKind::Right)),
                    'j' => Some(Action::Motion(MotionKind::Down)),
                    'k' => Some(Action::Motion(MotionKind::Up)),
                    '0' => Some(Action::Motion(MotionKind::LineStart)),
                    '$' => Some(Action::Motion(MotionKind::LineEnd)),
                    'w' => Some(Action::Motion(MotionKind::WordForward)),
                    'b' => Some(Action::Motion(MotionKind::WordBackward)),
                    'd' => Some(Action::VisualOperator {
                        op: OperatorKind::Delete,
                        register: None,
                    }),
                    'y' => Some(Action::VisualOperator {
                        op: OperatorKind::Yank,
                        register: None,
                    }),
                    'c' => Some(Action::VisualOperator {
                        op: OperatorKind::Change,
                        register: None,
                    }),
                    'v' => Some(Action::ModeChange(ModeChange::LeaveVisualChar)), // toggle exit like Vim
                    'i' => None, // 'i' not active in VisualChar yet (text object placeholder)
                    _ => None,
                },
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
            } else if matches!(mode, Mode::VisualChar) {
                Some(Action::ModeChange(ModeChange::LeaveVisualChar))
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
    fn visual_char_enter_and_exit_translation() {
        let mut tr = KeyTranslator::new();
        // 'v' in Normal enters visual
        match tr.translate(Mode::Normal, "", &kc('v')) {
            Some(Action::ModeChange(ModeChange::EnterVisualChar)) => {}
            other => panic!("expected EnterVisualChar got {:?}", other),
        }
        // Esc in VisualChar leaves
        let esc = KeyEvent {
            code: KeyCode::Esc,
            mods: KeyModifiers::empty(),
        };
        match tr.translate(Mode::VisualChar, "", &esc) {
            Some(Action::ModeChange(ModeChange::LeaveVisualChar)) => {}
            other => panic!("expected LeaveVisualChar got {:?}", other),
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

    // --- Operator pending tests (Phase 4 Step 2) ---

    #[test]
    fn operator_simple_dw() {
        let mut tr = KeyTranslator::new();
        let d = kc('d');
        assert!(tr.translate(Mode::Normal, "", &d).is_none()); // pending
        let w = kc('w');
        match tr.translate(Mode::Normal, "", &w) {
            Some(Action::ApplyOperator {
                op,
                motion,
                count,
                register: _,
            }) => {
                assert!(matches!(op, OperatorKind::Delete));
                assert_eq!(motion, MotionKind::WordForward);
                assert_eq!(count, 1);
            }
            other => panic!("expected ApplyOperator(dw) got {:?}", other),
        }
    }

    #[test]
    fn operator_prefix_count_2dw() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('2')).is_none());
        assert!(tr.translate(Mode::Normal, "", &kc('d')).is_none());
        match tr.translate(Mode::Normal, "", &kc('w')) {
            Some(Action::ApplyOperator {
                op,
                motion,
                count,
                register: _,
            }) => {
                assert!(matches!(op, OperatorKind::Delete));
                assert_eq!(motion, MotionKind::WordForward);
                assert_eq!(count, 2);
            }
            other => panic!("expected ApplyOperator(2dw) got {:?}", other),
        }
    }

    #[test]
    fn operator_post_count_d2w() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('d')).is_none());
        assert!(tr.translate(Mode::Normal, "", &kc('2')).is_none());
        match tr.translate(Mode::Normal, "", &kc('w')) {
            Some(Action::ApplyOperator {
                op,
                motion,
                count,
                register: _,
            }) => {
                assert!(matches!(op, OperatorKind::Delete));
                assert_eq!(motion, MotionKind::WordForward);
                assert_eq!(count, 2);
            }
            other => panic!("expected ApplyOperator(d2w) got {:?}", other),
        }
    }

    #[test]
    fn operator_double_dd_linewise() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('d')).is_none());
        match tr.translate(Mode::Normal, "", &kc('d')) {
            Some(Action::LinewiseOperator {
                op,
                count,
                register,
            }) => {
                assert!(matches!(op, OperatorKind::Delete));
                assert_eq!(count, 1);
                assert!(register.is_none());
            }
            other => panic!("expected LinewiseOperator(dd) got {:?}", other),
        }
    }

    #[test]
    fn operator_double_prefix_count_3dd() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('3')).is_none());
        assert!(tr.translate(Mode::Normal, "", &kc('d')).is_none());
        match tr.translate(Mode::Normal, "", &kc('d')) {
            Some(Action::LinewiseOperator {
                op,
                count,
                register,
            }) => {
                assert!(matches!(op, OperatorKind::Delete));
                assert_eq!(count, 3);
                assert!(register.is_none());
            }
            other => panic!("expected LinewiseOperator(3dd) got {:?}", other),
        }
    }

    #[test]
    fn operator_double_post_count_d2d() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('d')).is_none());
        assert!(tr.translate(Mode::Normal, "", &kc('2')).is_none());
        match tr.translate(Mode::Normal, "", &kc('d')) {
            Some(Action::LinewiseOperator {
                op,
                count,
                register,
            }) => {
                assert!(matches!(op, OperatorKind::Delete));
                assert_eq!(count, 2);
                assert!(register.is_none());
            }
            other => panic!("expected LinewiseOperator(d2d) got {:?}", other),
        }
    }

    #[test]
    fn operator_double_yank_with_register() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('"')).is_none());
        assert!(tr.translate(Mode::Normal, "", &kc('a')).is_none());
        assert!(tr.translate(Mode::Normal, "", &kc('y')).is_none());
        match tr.translate(Mode::Normal, "", &kc('y')) {
            Some(Action::LinewiseOperator {
                op,
                count,
                register,
            }) => {
                assert!(matches!(op, OperatorKind::Yank));
                assert_eq!(count, 1);
                assert_eq!(register, Some('a'));
            }
            other => panic!("expected LinewiseOperator(\"ayy) got {:?}", other),
        }
    }

    #[test]
    fn operator_double_cc_change() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('c')).is_none());
        match tr.translate(Mode::Normal, "", &kc('c')) {
            Some(Action::LinewiseOperator {
                op,
                count,
                register,
            }) => {
                assert!(matches!(op, OperatorKind::Change));
                assert_eq!(count, 1);
                assert!(register.is_none());
            }
            other => panic!("expected LinewiseOperator(cc) got {:?}", other),
        }
    }

    #[test]
    fn operator_multiplicative_2d3w() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('2')).is_none());
        assert!(tr.translate(Mode::Normal, "", &kc('d')).is_none());
        assert!(tr.translate(Mode::Normal, "", &kc('3')).is_none());
        match tr.translate(Mode::Normal, "", &kc('w')) {
            Some(Action::ApplyOperator {
                op,
                motion,
                count,
                register: _,
            }) => {
                assert!(matches!(op, OperatorKind::Delete));
                assert_eq!(motion, MotionKind::WordForward);
                assert_eq!(count, 6);
            }
            other => panic!("expected ApplyOperator(2d3w) got {:?}", other),
        }
    }

    #[test]
    fn operator_d0() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('d')).is_none());
        match tr.translate(Mode::Normal, "", &kc('0')) {
            Some(Action::ApplyOperator {
                op,
                motion,
                count,
                register: _,
            }) => {
                assert!(matches!(op, OperatorKind::Delete));
                assert_eq!(motion, MotionKind::LineStart);
                assert_eq!(count, 1);
            }
            other => panic!("expected ApplyOperator(d0) got {:?}", other),
        }
    }

    #[test]
    fn operator_esc_cancels() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('d')).is_none());
        let esc = KeyEvent {
            code: KeyCode::Esc,
            mods: KeyModifiers::empty(),
        };
        assert!(tr.translate(Mode::Normal, "", &esc).is_none());
        // Subsequent motion should just be a plain motion (not operator)
        match tr.translate(Mode::Normal, "", &kc('w')) {
            Some(Action::Motion(MotionKind::WordForward)) => {}
            other => panic!("expected plain motion after cancel, got {:?}", other),
        }
    }

    // --- Step 6.1: Ctrl-D / Ctrl-U precedence tests ---

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            mods: KeyModifiers::CTRL,
        }
    }

    #[test]
    fn ctrl_d_half_page_down_basic() {
        let mut tr = KeyTranslator::new();
        match tr.translate(Mode::Normal, "", &ctrl('d')) {
            Some(Action::Motion(MotionKind::PageHalfDown)) => {}
            other => panic!("expected PageHalfDown, got {:?}", other),
        }
    }

    #[test]
    fn ctrl_u_half_page_up_basic() {
        let mut tr = KeyTranslator::new();
        match tr.translate(Mode::Normal, "", &ctrl('u')) {
            Some(Action::Motion(MotionKind::PageHalfUp)) => {}
            other => panic!("expected PageHalfUp, got {:?}", other),
        }
    }

    #[test]
    fn ctrl_d_cancels_pending_operator() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('d')).is_none()); // pending delete
        // Now ctrl-d should scroll and NOT apply operator.
        match tr.translate(Mode::Normal, "", &ctrl('d')) {
            Some(Action::Motion(MotionKind::PageHalfDown)) => {}
            other => panic!("expected PageHalfDown after pending op, got {:?}", other),
        }
        // Following motion should be plain (operator canceled)
        match tr.translate(Mode::Normal, "", &kc('w')) {
            Some(Action::Motion(MotionKind::WordForward)) => {}
            other => panic!("expected plain motion after ctrl-d cancel, got {:?}", other),
        }
    }

    #[test]
    fn ctrl_d_drops_prefix_count() {
        let mut tr = KeyTranslator::new();
        assert!(tr.translate(Mode::Normal, "", &kc('2')).is_none());
        // ctrl-d should ignore the accumulated count (like Vim: 2<C-d> scrolls one half page).
        match tr.translate(Mode::Normal, "", &ctrl('d')) {
            Some(Action::Motion(MotionKind::PageHalfDown)) => {}
            other => panic!("expected PageHalfDown with ignored count, got {:?}", other),
        }
        // New motion after should not inherit old count
        match tr.translate(Mode::Normal, "", &kc('l')) {
            Some(Action::Motion(MotionKind::Right)) => {}
            other => panic!("expected simple Right motion post ctrl-d, got {:?}", other),
        }
    }
}
