use core_actions::{dispatch, translate_ngi};
use core_config::Config;
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_model::EditorModel;
use core_state::EditorState;
use core_text::Buffer;

#[derive(Clone, Copy, Debug)]
enum Stroke {
    Char(char),
    Esc,
    Ctrl(char),
}

impl Stroke {
    fn to_event(self) -> KeyEvent {
        match self {
            Stroke::Char(c) => KeyEvent {
                code: KeyCode::Char(c),
                mods: KeyModifiers::empty(),
            },
            Stroke::Esc => KeyEvent {
                code: KeyCode::Esc,
                mods: KeyModifiers::empty(),
            },
            Stroke::Ctrl(c) => KeyEvent {
                code: KeyCode::Char(c),
                mods: KeyModifiers::CTRL,
            },
        }
    }
}

fn run_sequence(initial: &str, keys: &[Stroke]) -> EditorModel {
    let buffer = Buffer::from_str("fixture", initial).unwrap();
    let state = EditorState::new(buffer);
    replay_with_state(state, keys)
}

fn replay_with_state(state: EditorState, keys: &[Stroke]) -> EditorModel {
    let mut model = EditorModel::new(state);
    let mut sticky = None;
    let cfg = Config::default();

    for stroke in keys {
        let event = stroke.to_event();
        let mode = model.state().mode;
        let pending = model.state().command_line.buffer().to_string();
        let resolution = translate_ngi(mode, &pending, &event, &cfg);
        if let Some(action) = resolution.action {
            let res = dispatch(action, &mut model, &mut sticky, &[]);
            if res.quit {
                break;
            }
        }
    }

    model
}

fn buffer_contents(state: &EditorState) -> String {
    let mut out = String::new();
    let buf = state.active_buffer();
    for line_idx in 0..buf.line_count() {
        if let Some(line) = buf.line(line_idx) {
            out.push_str(&line);
        }
    }
    out
}

#[test]
fn unicode_linewise_change_and_paste_regression() {
    let initial = "καλη μέρα\nemoji🙂 line\nalpha βeta\n";
    let keys = [
        Stroke::Char('0'),
        Stroke::Char('c'),
        Stroke::Char('w'),
        Stroke::Char('χ'),
        Stroke::Char('α'),
        Stroke::Char('ρ'),
        Stroke::Char('ά'),
        Stroke::Esc,
        Stroke::Char('0'),
        Stroke::Char('y'),
        Stroke::Char('y'),
        Stroke::Char('j'),
        Stroke::Char('p'),
    ];

    let model = run_sequence(initial, &keys);
    let state = model.state();
    let contents = buffer_contents(state);
    assert_eq!(contents, "χαρά μέρα\nemoji🙂 line\nχαρά μέρα\nalpha βeta\n");

    let cursor = model.active_view().cursor;
    assert_eq!(cursor.line, 2);
    assert_eq!(cursor.byte, 0);

    assert_eq!(state.registers.unnamed, "χαρά μέρα\n");
    assert_eq!(
        state.registers.numbered().first().map(String::as_str),
        Some("χαρά μέρα\n")
    );
    assert!(state.registers.get_named('a').unwrap_or("").is_empty());
    assert!(state.dirty);
    assert!(state.undo_depth() >= 2);
    assert_eq!(state.redo_depth(), 0);
}

#[test]
fn redo_and_named_register_snapshot_regression() {
    let initial = "emoji 🙂 test\nalpha\n";
    let keys = [
        Stroke::Char('0'),
        Stroke::Char('y'),
        Stroke::Char('y'),
        Stroke::Char('p'),
        Stroke::Char('u'),
        Stroke::Ctrl('r'),
        Stroke::Char('"'),
        Stroke::Char('a'),
        Stroke::Char('y'),
        Stroke::Char('y'),
        Stroke::Char('j'),
        Stroke::Char('"'),
        Stroke::Char('a'),
        Stroke::Char('p'),
    ];

    let model = run_sequence(initial, &keys);
    let state = model.state();
    let contents = buffer_contents(state);
    assert_eq!(
        contents,
        "emoji 🙂 test\nemoji 🙂 test\nalpha\nemoji 🙂 test\n"
    );

    let cursor = model.active_view().cursor;
    assert_eq!(cursor.line, 3);
    assert_eq!(cursor.byte, 0);

    assert_eq!(state.registers.unnamed, "emoji 🙂 test\n");
    assert_eq!(
        state.registers.numbered().first().map(String::as_str),
        Some("emoji 🙂 test\n")
    );
    assert_eq!(state.registers.get_named('a'), Some("emoji 🙂 test\n"));
    assert_eq!(state.undo_depth(), 2);
    assert_eq!(state.redo_depth(), 0);
}
