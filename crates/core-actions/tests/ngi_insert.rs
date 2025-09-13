use core_actions::{translate_key, translate_ngi};
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
fn ngi_insert_basic_text_newline_backspace() {
    // Prepare model and enter Insert mode
    let buf = Buffer::from_str("t", "").unwrap();
    let state = EditorState::new(buf);
    let mut model = EditorModel::new(state);

    // Enter insert via legacy key since translate_key wrapper routes legacy by default
    let act = translate_key(
        model.state().mode,
        model.state().command_line.buffer(),
        &kc('i'),
    )
    .unwrap();
    let mut sticky = None;
    core_actions::dispatcher::dispatch(act, &mut model, &mut sticky, &[]);

    // Now feed characters through NGI adapter
    let a = kc('a');
    let b = kc('b');
    let enter = KeyEvent {
        code: KeyCode::Enter,
        mods: KeyModifiers::empty(),
    };
    let bs = KeyEvent {
        code: KeyCode::Backspace,
        mods: KeyModifiers::empty(),
    };
    let cfg = Config::default();

    for ev in [a, b, enter, a, bs] {
        if let Some(act) = translate_ngi(
            model.state().mode,
            model.state().command_line.buffer(),
            &ev,
            &cfg,
        )
        .action
        {
            core_actions::dispatcher::dispatch(act, &mut model, &mut sticky, &[]);
        }
    }

    // After sequence: "ab\n" (enter) then insert 'a' then backspace removes it -> lines: ["ab\n", ""]
    assert_eq!(model.state().active_buffer().line_count(), 2);
    assert_eq!(model.state().active_buffer().line(0).unwrap(), "ab\n");
}
