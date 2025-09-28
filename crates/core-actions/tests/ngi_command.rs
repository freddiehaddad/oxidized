mod common;
use common::*;

use core_actions::dispatch;
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
fn ngi_command_line_basic_flow() {
    reset_translator();
    let buf = Buffer::from_str("t", "").unwrap();
    let state = EditorState::new(buf);
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    let cfg = Config::default();

    // Start ':' via NGI adapter in Normal mode
    if let Some(act) = translate_ngi(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc(':'),
        &cfg,
    )
    .action
    {
        dispatch(act, &mut model, &mut sticky, &[]);
    } else {
        panic!("NGI adapter did not start command line on ':'");
    }
    assert!(model.state().command_line.is_active());

    // Type 'q' via NGI adapter (command buffer becomes ":q")
    if let Some(act) = translate_ngi(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('q'),
        &cfg,
    )
    .action
    {
        dispatch(act, &mut model, &mut sticky, &[]);
    }
    assert_eq!(model.state().command_line.buffer(), ":q");

    // Backspace removes 'q'
    let bs = KeyEvent {
        code: KeyCode::Backspace,
        mods: KeyModifiers::empty(),
    };
    if let Some(act) = translate_ngi(
        model.state().mode,
        model.state().command_line.buffer(),
        &bs,
        &cfg,
    )
    .action
    {
        dispatch(act, &mut model, &mut sticky, &[]);
    }
    assert_eq!(model.state().command_line.buffer(), ":");

    // Cancel with Esc
    let esc = KeyEvent {
        code: KeyCode::Esc,
        mods: KeyModifiers::empty(),
    };
    if let Some(act) = translate_ngi(
        model.state().mode,
        model.state().command_line.buffer(),
        &esc,
        &cfg,
    )
    .action
    {
        dispatch(act, &mut model, &mut sticky, &[]);
    }
    assert!(!model.state().command_line.is_active());

    // Restart ':' and execute with Enter
    if let Some(act) = translate_ngi(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc(':'),
        &cfg,
    )
    .action
    {
        dispatch(act, &mut model, &mut sticky, &[]);
    }
    if let Some(act) = translate_ngi(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('q'),
        &cfg,
    )
    .action
    {
        dispatch(act, &mut model, &mut sticky, &[]);
    }
    let enter = KeyEvent {
        code: KeyCode::Enter,
        mods: KeyModifiers::empty(),
    };
    if let Some(_act) = translate_ngi(
        model.state().mode,
        model.state().command_line.buffer(),
        &enter,
        &cfg,
    )
    .action
    {
        // This will issue CommandExecute(":q"); in runtime this would trigger a Quit CommandEvent,
        // but dispatch() ignores command execution here; we only validate translation path.
        // Ensure command buffer remains as ":q" prior to side effects.
        assert_eq!(model.state().command_line.buffer(), ":q");
    }
}
