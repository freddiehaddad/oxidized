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

use crate::{Action, ActionObserver, MotionKind};
use core_model::EditorModel;
use core_state::PasteSource;

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
    // Safe split borrow (encapsulated unsafety lives in `EditorModel::split_state_and_active_view`).
    let (state, view) = model.split_state_and_active_view();

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
        Action::ModeChange(mc) => mode::handle_mode_change(mc, state, view),
        Action::CommandStart
        | Action::CommandChar(_)
        | Action::CommandBackspace
        | Action::CommandCancel
        | Action::CommandExecute(_) => command::handle_command_action(action, state, view),
        Action::Edit(kind) => edit::handle_edit(kind, state, view),
        Action::Undo => undo::handle_undo(state, view),
        Action::Redo => undo::handle_redo(state, view),
        Action::PasteAfter { register } => {
            // Step 7: allow explicit named (a–z/A–Z) and numbered (0–9) registers.
            let source = register
                .and_then(|c| {
                    if c.is_ascii_alphabetic() {
                        Some(PasteSource::Named(c))
                    } else if c.is_ascii_digit() {
                        // Map ASCII digit to ring index; '0' -> 0 newest, '9' -> 9 oldest (if present)
                        Some(PasteSource::Numbered((c as u8 - b'0') as usize))
                    } else {
                        None
                    }
                })
                .unwrap_or(PasteSource::Unnamed);
            match state.paste(source, false, &mut view.cursor) {
                Ok(structural) => {
                    if structural {
                        DispatchResult::buffer_replaced()
                    } else {
                        DispatchResult::dirty()
                    }
                }
                Err(_) => DispatchResult::clean(),
            }
        }
        Action::PasteBefore { register } => {
            let source = register
                .and_then(|c| {
                    if c.is_ascii_alphabetic() {
                        Some(PasteSource::Named(c))
                    } else if c.is_ascii_digit() {
                        Some(PasteSource::Numbered((c as u8 - b'0') as usize))
                    } else {
                        None
                    }
                })
                .unwrap_or(PasteSource::Unnamed);
            match state.paste(source, true, &mut view.cursor) {
                Ok(structural) => {
                    if structural {
                        DispatchResult::buffer_replaced()
                    } else {
                        DispatchResult::dirty()
                    }
                }
                Err(_) => DispatchResult::clean(),
            }
        }
        Action::Quit => DispatchResult::quit(),
        Action::BeginOperator(_) => DispatchResult::clean(),
        Action::ApplyOperator {
            op,
            motion,
            count,
            register,
        } => {
            use crate::OperatorKind;
            use crate::span_resolver::resolve_selection;
            match op {
                OperatorKind::Delete => {
                    let start_pos = view.cursor;
                    let sel = resolve_selection(state, start_pos, motion, count);
                    if sel.start == sel.end {
                        return DispatchResult::clean();
                    }
                    // Convert selection start/end positions to absolute byte indices.
                    let (abs_start, abs_end) = selection_abs_byte_range(state, sel.start, sel.end);
                    if abs_start == abs_end {
                        return DispatchResult::clean();
                    }
                    let mut cursor = view.cursor;
                    let removed = state.delete_span_with_snapshot(&mut cursor, abs_start, abs_end);
                    let structural = removed.contains('\n')
                        || matches!(sel.kind, core_state::SelectionKind::Linewise);
                    {
                        let mut regs = state.registers_facade();
                        regs.write_delete(removed.clone(), register);
                    }
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
                OperatorKind::Yank => {
                    let start_pos = view.cursor;
                    let sel = resolve_selection(state, start_pos, motion, count);
                    if sel.start == sel.end {
                        return DispatchResult::clean();
                    }
                    let buffer = state.active_buffer();
                    let collected = if matches!(sel.kind, core_state::SelectionKind::Linewise) {
                        // Linewise selection encodes end as exclusive (end points to start of line after last included line).
                        let mut s = String::new();
                        let line_start = sel.start.line.min(sel.end.line);
                        let line_end_exclusive = sel.start.line.max(sel.end.line);
                        for l in line_start..line_end_exclusive {
                            if let Some(line) = buffer.line(l) {
                                s.push_str(&line);
                            }
                        }
                        s
                    } else {
                        let (abs_start, abs_end) =
                            selection_abs_byte_range(state, sel.start, sel.end);
                        // Iterate lines overlapping range to collect substring (existing logic simplified)
                        let mut collected = String::new();
                        let mut abs = 0usize;
                        for l in 0..buffer.line_count() {
                            let line = buffer.line(l).unwrap();
                            let len = line.len();
                            let end_abs = abs + len;
                            if end_abs <= abs_start {
                                abs = end_abs;
                                continue;
                            }
                            if abs >= abs_end {
                                break;
                            }
                            let local_start = abs_start.saturating_sub(abs);
                            let local_end = (abs_end - abs).min(len);
                            collected.push_str(&line[local_start..local_end]);
                            abs = end_abs;
                        }
                        collected
                    };
                    {
                        let mut regs = state.registers_facade();
                        regs.write_yank(collected.clone(), register);
                    }
                    DispatchResult::dirty()
                }
                OperatorKind::Change => {
                    let start_pos = view.cursor;
                    let sel = resolve_selection(state, start_pos, motion, count);
                    if sel.start == sel.end {
                        return DispatchResult::clean();
                    }
                    let (abs_start, mut abs_end) =
                        selection_abs_byte_range(state, sel.start, sel.end);
                    if abs_start == abs_end {
                        return DispatchResult::clean();
                    }
                    abs_end =
                        adjust_change_range(state.active_buffer(), motion, abs_start, abs_end);
                    if abs_start == abs_end {
                        return DispatchResult::clean();
                    }
                    let mut cursor = view.cursor;
                    let removed = state.delete_span_with_snapshot(&mut cursor, abs_start, abs_end);
                    let structural = removed.contains('\n')
                        || matches!(sel.kind, core_state::SelectionKind::Linewise);
                    {
                        let mut regs = state.registers_facade();
                        regs.write_change(removed.clone(), register);
                    }
                    // Change enters insert at beginning of span (linewise: first line start; charwise: absolute start)
                    view.cursor = sel.start; // sel.start already normalized
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
        Action::VisualOperator { op, register } => {
            use crate::OperatorKind;
            use core_state::SelectionKind;
            if !matches!(state.mode, core_state::Mode::VisualChar) {
                return DispatchResult::clean();
            }
            let Some(span) = state.selection.active else {
                return DispatchResult::clean();
            };
            if span.start == span.end {
                return DispatchResult::clean();
            }
            // Map selection to absolute byte indices. For characterwise selections we
            // must treat the visual representation as inclusive of the last grapheme.
            let (abs_start, abs_end) =
                if matches!(span.kind, core_state::SelectionKind::Characterwise) {
                    span.inclusive_byte_range(state.active_buffer())
                } else {
                    selection_abs_byte_range(state, span.start, span.end)
                };
            if abs_start == abs_end {
                return DispatchResult::clean();
            }
            match op {
                OperatorKind::Delete => {
                    let mut cursor = view.cursor;
                    let removed = state.delete_span_with_snapshot(&mut cursor, abs_start, abs_end);
                    let structural =
                        removed.contains('\n') || matches!(span.kind, SelectionKind::Linewise);
                    {
                        let mut regs = state.registers_facade();
                        regs.write_delete(removed.clone(), register);
                    }
                    // Cursor placement: start of resulting span (normalized span.start)
                    view.cursor = span.start;
                    state.clear_selection();
                    state.mode = core_state::Mode::Normal;
                    if !state.dirty {
                        state.dirty = true;
                    }
                    if structural {
                        DispatchResult::buffer_replaced()
                    } else {
                        DispatchResult::dirty()
                    }
                }
                OperatorKind::Yank => {
                    // Collect text similar to yank path in ApplyOperator
                    let buffer = state.active_buffer();
                    let collected = if matches!(span.kind, SelectionKind::Linewise) {
                        let mut s = String::new();
                        let line_start = span.start.line.min(span.end.line);
                        let line_end_exclusive = span.start.line.max(span.end.line);
                        for l in line_start..line_end_exclusive {
                            if let Some(line) = buffer.line(l) {
                                s.push_str(&line);
                            }
                        }
                        s
                    } else {
                        // Gather substring across lines.
                        let mut collected = String::new();
                        let mut abs = 0usize;
                        for l in 0..buffer.line_count() {
                            let line = buffer.line(l).unwrap();
                            let len = line.len();
                            let end_abs = abs + len;
                            if end_abs <= abs_start {
                                abs = end_abs;
                                continue;
                            }
                            if abs >= abs_end {
                                break;
                            }
                            let local_start = abs_start.saturating_sub(abs);
                            let local_end = (abs_end - abs).min(len);
                            collected.push_str(&line[local_start..local_end]);
                            abs = end_abs;
                        }
                        collected
                    };
                    {
                        let mut regs = state.registers_facade();
                        regs.write_yank(collected.clone(), register);
                    }
                    // Cursor stays at active end? Vim leaves at start for charwise.
                    view.cursor = span.start;
                    state.clear_selection();
                    state.mode = core_state::Mode::Normal;
                    DispatchResult::dirty()
                }
                OperatorKind::Change => {
                    let mut cursor = view.cursor;
                    let removed = state.delete_span_with_snapshot(&mut cursor, abs_start, abs_end);
                    let structural =
                        removed.contains('\n') || matches!(span.kind, SelectionKind::Linewise);
                    {
                        let mut regs = state.registers_facade();
                        regs.write_change(removed.clone(), register);
                    }
                    view.cursor = span.start; // enter insert at start
                    state.clear_selection();
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

// Helper: map selection positions to absolute byte indices (inclusive start, exclusive end) via scan.
fn selection_abs_byte_range(
    state: &core_state::EditorState,
    start: core_text::Position,
    end: core_text::Position,
) -> (usize, usize) {
    let buffer = state.active_buffer();
    // Reuse logic similar to span_resolver absolute_index but for both endpoints.
    let to_abs = |pos: core_text::Position| {
        let mut total = 0usize;
        for line in 0..pos.line {
            total += buffer.line_byte_len(line);
            if let Some(l) = buffer.line(line)
                && l.ends_with('\n')
            {
                total += 1;
            }
        }
        total + pos.byte
    };
    let a = to_abs(start);
    let b = to_abs(end);
    if a <= b { (a, b) } else { (b, a) }
}

fn adjust_change_range(
    buffer: &core_text::Buffer,
    motion: MotionKind,
    abs_start: usize,
    abs_end: usize,
) -> usize {
    if abs_start >= abs_end {
        return abs_end;
    }
    match motion {
        MotionKind::WordForward => {
            let slice = buffer.slice_bytes(abs_start, abs_end);
            if slice.is_empty() || slice.chars().all(|c| c.is_whitespace()) {
                return abs_end;
            }
            let trimmed = slice.trim_end_matches(|c: char| c.is_whitespace());
            if trimmed.len() == slice.len() {
                abs_end
            } else {
                abs_start + trimmed.len()
            }
        }
        _ => abs_end,
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
    fn visual_char_delete_forward_inclusive() {
        // Buffer: abcdef\n cursor at 'a' enter Visual, move right 3 times selects a..d then delete
        let buffer = Buffer::from_str("t", "abcdef\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // Enter visual
        dispatch(
            Action::ModeChange(ModeChange::EnterVisualChar),
            &mut model,
            &mut sticky,
            &[],
        );
        // Move right 3 times (selecting a,b,c,d visually)
        for _ in 0..3 {
            dispatch(
                Action::Motion(MotionKind::Right),
                &mut model,
                &mut sticky,
                &[],
            );
        }
        // Apply delete
        let res = dispatch(
            Action::VisualOperator {
                op: OperatorKind::Delete,
                register: None,
            },
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(res.dirty);
        let line = model.state().active_buffer().line(0).unwrap();
        assert_eq!(
            line, "ef\n",
            "expected first four chars removed inclusively (a-d)"
        );
    }

    #[test]
    fn visual_char_delete_reverse_inclusive() {
        // Start cursor at end, move left to build reverse selection then delete; inclusive must remove endpoints.
        let buffer = Buffer::from_str("t", "abcdef\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        // Place cursor on 'f' (before newline) by motioning to line end then left once (simulate user)
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // Move to line end
        dispatch(
            Action::Motion(MotionKind::LineEnd),
            &mut model,
            &mut sticky,
            &[],
        );
        // Enter visual with cursor on 'f'
        dispatch(
            Action::ModeChange(ModeChange::EnterVisualChar),
            &mut model,
            &mut sticky,
            &[],
        );
        // Move left 3 times to extend selection backward over c,d,e,f (order anchor at f)
        for _ in 0..3 {
            dispatch(
                Action::Motion(MotionKind::Left),
                &mut model,
                &mut sticky,
                &[],
            );
        }
        // Delete
        let res = dispatch(
            Action::VisualOperator {
                op: OperatorKind::Delete,
                register: None,
            },
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(res.dirty);
        let line = model.state().active_buffer().line(0).unwrap();
        assert_eq!(line, "ab\n", "expected inclusive removal of c-f");
    }

    #[test]
    fn visual_char_delete_single_grapheme_inclusive() {
        // Selecting a single character then deleting should remove it.
        let buffer = Buffer::from_str("t", "xYz\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // Move to 'Y'
        dispatch(
            Action::Motion(MotionKind::Right),
            &mut model,
            &mut sticky,
            &[],
        );
        dispatch(
            Action::ModeChange(ModeChange::EnterVisualChar),
            &mut model,
            &mut sticky,
            &[],
        );
        // Without moving, selection length 0 -> expand one side by moving right then left to force span? Instead move right once.
        dispatch(
            Action::Motion(MotionKind::Right),
            &mut model,
            &mut sticky,
            &[],
        ); // selection covers Y
        let res = dispatch(
            Action::VisualOperator {
                op: OperatorKind::Delete,
                register: None,
            },
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(res.dirty);
        let line = model.state().active_buffer().line(0).unwrap();
        assert_eq!(line, "x\n", "expected 'Y' and 'z' removed (inclusive span)");
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
    fn leave_insert_backs_up_cursor_one_grapheme() {
        let buffer = Buffer::from_str("t", "").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        // Enter insert
        dispatch(
            Action::ModeChange(ModeChange::EnterInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        // Insert abc
        for ch in ['a', 'b', 'c'] {
            dispatch(
                Action::Edit(EditKind::InsertGrapheme(ch.to_string())),
                &mut model,
                &mut sticky,
                &[],
            );
        }
        // Leave insert
        dispatch(
            Action::ModeChange(ModeChange::LeaveInsert),
            &mut model,
            &mut sticky,
            &[],
        );
        let (state_ref, view) = model.split_state_and_active_view();
        assert!(matches!(state_ref.mode, core_state::Mode::Normal));
        let line_owned = state_ref.active_buffer().line(0).unwrap();
        let line = line_owned.as_str();
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        assert!(
            view.cursor.byte < trimmed.len(),
            "cursor expected on last grapheme"
        );
        assert_eq!(&trimmed[view.cursor.byte..view.cursor.byte + 1], "c");
    }

    #[test]
    fn visual_enter_dirty_and_anchor_set() {
        let buffer = Buffer::from_str("t", "alpha").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        let res = dispatch(
            Action::ModeChange(ModeChange::EnterVisualChar),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(res.dirty, "entering visual should be dirty");
        let (state_ref, _view) = model.split_state_and_active_view();
        assert!(state_ref.selection.anchor.is_some());
        assert!(state_ref.selection.active.is_some());
        assert!(matches!(state_ref.mode, core_state::Mode::VisualChar));
        let res2 = dispatch(
            Action::ModeChange(ModeChange::LeaveVisualChar),
            &mut model,
            &mut sticky,
            &[],
        );
        assert!(res2.dirty, "leaving visual should be dirty");
        let (state_ref2, _) = model.split_state_and_active_view();
        assert!(matches!(state_ref2.mode, core_state::Mode::Normal));
        assert!(state_ref2.selection.active.is_none());
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
        if let Action::ApplyOperator {
            op,
            motion,
            count,
            register: _,
        } = apply
        {
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
        if let Action::ApplyOperator {
            op,
            motion,
            count,
            register: _,
        } = act
        {
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
        // Yank should leave buffer unchanged; dirty flag may remain false.
        assert!(!res.buffer_replaced, "yank must not be structural");
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
        assert_eq!(model.state().registers.unnamed, "one");
        // Vim parity: cw changes word but preserves following whitespace.
        let after_line = model.state().active_buffer().line(0).unwrap();
        assert_eq!(after_line, " two three\n");
    }

    #[test]
    fn operator_change_cw_unicode_word() {
        let buffer = Buffer::from_str("t", "éclair 😀 space\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let act = change_sequence(&mut model, "cw");
        let mut sticky = None;
        dispatch(act, &mut model, &mut sticky, &[]);
        assert_eq!(model.state().mode, core_state::Mode::Insert);
        assert_eq!(model.state().registers.unnamed, "éclair");
        let line = model.state().active_buffer().line(0).unwrap();
        assert_eq!(line, " 😀 space\n");
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
        assert_eq!(model.state().registers.unnamed, "one two");
        // two words removed while preserving trailing whitespace before third word
        assert_eq!(after_line, " three four\n");
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
        assert_eq!(model.state().registers.unnamed, "one two");
        assert_eq!(after_line, " three four\n");
    }

    #[test]
    fn operator_change_line_end_c_dollar() {
        let buffer = Buffer::from_str("t", "alpha beta\nsecond\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let act = change_sequence(&mut model, "c$");
        let mut sticky = None;
        dispatch(act, &mut model, &mut sticky, &[]);
        assert_eq!(model.state().mode, core_state::Mode::Insert);
        let first_line = model.state().active_buffer().line(0).unwrap();
        assert_eq!(first_line, "\n");
        assert_eq!(model.state().registers.unnamed, "alpha beta");
        let second_line = model.state().active_buffer().line(1).unwrap();
        assert_eq!(second_line, "second\n");
    }

    #[test]
    fn operator_change_line_start_c0() {
        let buffer = Buffer::from_str("t", "alpha beta gamma\n").unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let mut sticky = None;
        for _ in 0..6 {
            let act = translate_key(
                model.state().mode,
                model.state().command_line.buffer(),
                &KeyEvent {
                    code: KeyCode::Char('l'),
                    mods: KeyModifiers::empty(),
                },
            )
            .unwrap();
            dispatch(act, &mut model, &mut sticky, &[]);
        }
        let act = Action::ApplyOperator {
            op: OperatorKind::Change,
            motion: MotionKind::LineStart,
            count: 1,
            register: None,
        };
        dispatch(act, &mut model, &mut sticky, &[]);
        assert_eq!(model.state().mode, core_state::Mode::Insert);
        let line = model.state().active_buffer().line(0).unwrap();
        assert_eq!(line, "beta gamma\n");
        assert_eq!(model.state().registers.unnamed, "alpha ");
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
