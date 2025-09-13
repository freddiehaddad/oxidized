use core_actions::dispatch;
use core_actions::translate_key;
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_model::EditorModel;
use core_state::EditorState;
use core_text::Buffer;

fn kc(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        mods: KeyModifiers::empty(),
    }
}

#[test]
fn visual_char_delete_basic() {
    let buf = Buffer::from_str("t", "hello world\n").unwrap();
    let state = EditorState::new(buf);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    // enter visual mode
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('v'),
    )
    .unwrap();
    dispatch(act, &mut model, &mut sticky, &[]);
    // expand selection with 'w' (word forward)
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('w'),
    )
    .unwrap();
    dispatch(act, &mut model, &mut sticky, &[]);
    // delete selection
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('d'),
    )
    .unwrap();
    let res = dispatch(act, &mut model, &mut sticky, &[]);
    assert!(res.dirty);
    assert_eq!(model.state().mode, core_state::Mode::Normal);
    // After deleting inclusive span (anchor at 'h', motion 'w' advanced to start of next word and inclusive logic
    // consumed the first grapheme of that word), the remaining text should start with 'orld'.
    let line0 = model.state().active_buffer().line(0).unwrap();
    assert!(line0.starts_with("orld"));
}

#[test]
fn visual_char_yank_basic() {
    let buf = Buffer::from_str("t", "alpha beta gamma\n").unwrap();
    let state = EditorState::new(buf);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    // enter visual
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('v'),
    )
    .unwrap();
    dispatch(act, &mut model, &mut sticky, &[]);
    // expand one word
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('w'),
    )
    .unwrap();
    dispatch(act, &mut model, &mut sticky, &[]);
    // yank
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('y'),
    )
    .unwrap();
    let res = dispatch(act, &mut model, &mut sticky, &[]);
    assert!(res.dirty); // yank marks dirty? Current dispatcher returns dirty for mode change clearing selection.
    assert_eq!(model.state().mode, core_state::Mode::Normal);
    // buffer unchanged
    let line0 = model.state().active_buffer().line(0).unwrap();
    assert!(line0.starts_with("alpha"));
    // selection cleared
    assert!(model.state().selection().is_none());
}

#[test]
fn visual_char_change_enters_insert() {
    let buf = Buffer::from_str("t", "one two three\n").unwrap();
    let state = EditorState::new(buf);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    // enter visual
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('v'),
    )
    .unwrap();
    dispatch(act, &mut model, &mut sticky, &[]);
    // expand
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('w'),
    )
    .unwrap();
    dispatch(act, &mut model, &mut sticky, &[]);
    // change
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('c'),
    )
    .unwrap();
    let res = dispatch(act, &mut model, &mut sticky, &[]);
    assert!(res.dirty);
    assert_eq!(model.state().mode, core_state::Mode::Insert);
}
