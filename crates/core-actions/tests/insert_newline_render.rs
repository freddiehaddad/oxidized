mod common;
use common::*;

use core_actions::{Action, EditKind, ModeChange, dispatch};
use core_model::EditorModel;
use core_state::EditorState;
use core_text::Buffer;

#[test]
fn insert_newline_should_trigger_structural_render() {
    reset_translator();

    let buffer = Buffer::from_str("t", "hello world\n")
        .expect("buffer should initialize for newline regression test");
    let state = EditorState::new(buffer);
    let mut model = EditorModel::new(state);
    let mut sticky = None;

    dispatch(
        Action::ModeChange(ModeChange::EnterInsert),
        &mut model,
        &mut sticky,
        &[],
    );

    let result = dispatch(
        Action::Edit(EditKind::InsertNewline),
        &mut model,
        &mut sticky,
        &[],
    );

    assert!(
        result.buffer_replaced,
        "insert newline should trigger structural render to shift lower lines"
    );
}
