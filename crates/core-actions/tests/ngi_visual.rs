use core_actions::{dispatch, translate_key, translate_ngi};
use core_config::Config;
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
fn ngi_visual_basic_ops() {
    let buf = Buffer::from_str("t", "hello world\n").unwrap();
    let state = EditorState::new(buf);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    let cfg = Config::default();

    // Enter visual via legacy path
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('v'),
    )
    .unwrap();
    dispatch(act, &mut model, &mut sticky, &[]);

    // Extend selection with 'w' via NGI adapter motion
    if let Some(act) = translate_ngi(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('w'),
        &cfg,
    )
    .action
    {
        dispatch(act, &mut model, &mut sticky, &[]);
    }
    // Delete selection via NGI adapter
    if let Some(act) = translate_ngi(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('d'),
        &cfg,
    )
    .action
    {
        let res = dispatch(act, &mut model, &mut sticky, &[]);
        assert!(res.dirty);
    }
    // After deleting inclusive span, we should have left Visual mode
    assert_eq!(model.state().mode, core_state::Mode::Normal);
}
