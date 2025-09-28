mod common;
use common::*;

use core_actions::{Action, OperatorKind, dispatch};
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
    reset_translator();
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

#[test]
fn ngi_visual_operator_respects_count_and_register() {
    reset_translator();
    let buf = Buffer::from_str("t", "alpha beta\n").unwrap();
    let state = EditorState::new(buf);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    let cfg = Config::default();

    // Enter visual mode via legacy translator to seed selection context.
    let enter = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('v'),
    )
    .unwrap();
    dispatch(enter, &mut model, &mut sticky, &[]);

    // Register prefix '"a'
    assert!(
        translate_ngi(model.state().mode, "", &kc('"'), &cfg)
            .action
            .is_none()
    );
    assert!(
        translate_ngi(model.state().mode, "", &kc('a'), &cfg)
            .action
            .is_none()
    );
    // Count prefix '3'
    assert!(
        translate_ngi(model.state().mode, "", &kc('3'), &cfg)
            .action
            .is_none()
    );
    // Operator 'd' should now emit VisualOperator with register/count applied.
    let act = translate_ngi(model.state().mode, "", &kc('d'), &cfg).action;
    match act {
        Some(Action::VisualOperator {
            op,
            register,
            count,
        }) => {
            assert!(matches!(op, OperatorKind::Delete));
            assert_eq!(register, Some('a'));
            assert_eq!(count, 3);
        }
        other => panic!("unexpected action: {:?}", other),
    }
}

#[test]
fn ngi_visual_paste_supports_counts_and_registers() {
    reset_translator();
    let buf = Buffer::from_str("t", "gamma\n").unwrap();
    let state = EditorState::new(buf);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    let cfg = Config::default();

    // Enter visual mode and ensure selection exists (single char selection).
    let enter = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('v'),
    )
    .unwrap();
    dispatch(enter, &mut model, &mut sticky, &[]);

    // Prefix register '"b' and count '2', then paste with 'p'.
    assert!(
        translate_ngi(model.state().mode, "", &kc('"'), &cfg)
            .action
            .is_none()
    );
    assert!(
        translate_ngi(model.state().mode, "", &kc('b'), &cfg)
            .action
            .is_none()
    );
    assert!(
        translate_ngi(model.state().mode, "", &kc('2'), &cfg)
            .action
            .is_none()
    );
    let act = translate_ngi(model.state().mode, "", &kc('p'), &cfg).action;
    match act {
        Some(Action::VisualPaste {
            before,
            register,
            count,
        }) => {
            assert!(!before, "visual 'p' should paste after selection");
            assert_eq!(register, Some('b'));
            assert_eq!(count, 2);
        }
        other => panic!("unexpected action: {:?}", other),
    }
}
