use core_actions::{Action, OperatorKind, translate_ngi};
use core_config::Config;
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_model::EditorModel;
use core_state::{EditorState, Mode};
use core_text::Buffer;

fn kc(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        mods: KeyModifiers::empty(),
    }
}

#[test]
fn ngi_visual_x_maps_to_visual_delete() {
    let buf = Buffer::from_str("t", "hello\n").unwrap();
    let state = EditorState::new(buf);
    let mut model = EditorModel::new(state);
    let mut sticky = None;

    // Enter Visual via legacy (: for now)
    core_actions::dispatch(
        core_actions::translate_key(Mode::Normal, "", &kc('v')).unwrap(),
        &mut model,
        &mut sticky,
        &[],
    );

    // Map via NGI adapter: 'x' -> VisualOperator(Delete)
    let cfg = Config::default();
    let act = translate_ngi(Mode::VisualChar, "", &kc('x'), &cfg).action;
    match act {
        Some(Action::VisualOperator { op, count, .. }) => {
            assert_eq!(count, 1, "visual x should default count to 1");
            assert!(matches!(op, OperatorKind::Delete));
        }
        other => panic!("unexpected: {:?}", other),
    }
}
