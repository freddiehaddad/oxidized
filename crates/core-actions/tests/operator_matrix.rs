//! Step 6.4 operator × motion matrix for delete semantics.
//! Focus: verifying linewise vs charwise classification, counts, and structural repaint flag.

use core_actions::{Action, dispatcher::dispatch, translate_key};
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_model::EditorModel;
use core_text::Buffer;

fn dispatch_keys(model: &mut EditorModel, seq: &str) -> Action {
    let mut last = None;
    for ch in seq.chars() {
        let evt = KeyEvent {
            code: KeyCode::Char(ch),
            mods: KeyModifiers::empty(),
        };
        last = translate_key(
            model.state().mode,
            model.state().command_line.buffer(),
            &evt,
        );
    }
    last.expect("sequence must produce final action")
}

#[derive(Debug)]
struct Case<'a> {
    name: &'a str,
    text: &'a str,
    keys: &'a str,
    expect_structural: bool,
}

#[test]
fn delete_motion_matrix() {
    let cases = [
        Case {
            name: "dw_charwise",
            text: "one two three\n",
            keys: "dw",
            expect_structural: false,
        },
        Case {
            name: "2dw_charwise",
            text: "one two three four\n",
            keys: "2dw",
            expect_structural: false,
        },
        Case {
            name: "dj_linewise_two_lines",
            text: "l1\nl2\nl3\n",
            keys: "dj",
            expect_structural: true,
        },
        Case {
            name: "2dj_linewise_three_lines",
            text: "a1\na2\na3\na4\n",
            keys: "2dj",
            expect_structural: true,
        },
        Case {
            name: "d2j_linewise_three_lines",
            text: "b1\nb2\nb3\nb4\n",
            keys: "d2j",
            expect_structural: true,
        },
    ];

    for case in cases {
        let buffer = Buffer::from_str("t", case.text).unwrap();
        let state = core_state::EditorState::new(buffer);
        let mut model = EditorModel::new(state);
        let act = dispatch_keys(&mut model, case.keys);
        let mut sticky = None;
        let res = dispatch(act, &mut model, &mut sticky, &[]);
        assert_eq!(
            res.buffer_replaced, case.expect_structural,
            "case '{}' structural flag mismatch",
            case.name
        );
    }
}
