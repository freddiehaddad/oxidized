mod common;
use common::*;

use core_actions::dispatcher::dispatch;
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_model::EditorModel;
use core_state::{EditorState, Mode, SelectionKind};
use core_text::Buffer;

fn key(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        mods: KeyModifiers::empty(),
    }
}

#[test]
fn enter_visual_char_sets_selection_anchor() {
    reset_translator();
    let buffer = Buffer::from_str("t", "hello world\n").unwrap();
    let state = EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    let esc = KeyEvent {
        code: KeyCode::Esc,
        mods: KeyModifiers::empty(),
    };
    // Ensure starting in Normal
    assert!(matches!(model.state().mode, Mode::Normal));
    // Translate 'v'
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &key('v'),
    )
    .expect("v action");
    let mut sticky = None;
    dispatch(act, &mut model, &mut sticky, &[]);
    assert!(matches!(model.state().mode, Mode::VisualChar));
    let sel = model.state().selection.active.expect("selection active");
    assert!(matches!(sel.kind, SelectionKind::Characterwise));
    assert_eq!(sel.start, sel.end);
    assert_eq!(sel.start.line, 0);
    assert_eq!(sel.start.byte, 0);
    // Esc leaves visual
    let leave = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &esc,
    )
    .expect("esc leave");
    dispatch(leave, &mut model, &mut sticky, &[]);
    assert!(matches!(model.state().mode, Mode::Normal));
    assert!(model.state().selection.active.is_none());
}
