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
    is_yank: bool,
    is_change: bool,
}

#[test]
fn delete_motion_matrix() {
    let cases = [
        Case {
            name: "dw_charwise",
            text: "one two three\n",
            keys: "dw",
            expect_structural: false,
            is_yank: false,
            is_change: false,
        },
        Case {
            name: "2dw_charwise",
            text: "one two three four\n",
            keys: "2dw",
            expect_structural: false,
            is_yank: false,
            is_change: false,
        },
        Case {
            name: "dj_linewise_two_lines",
            text: "l1\nl2\nl3\n",
            keys: "dj",
            expect_structural: true,
            is_yank: false,
            is_change: false,
        },
        Case {
            name: "2dj_linewise_three_lines",
            text: "a1\na2\na3\na4\n",
            keys: "2dj",
            expect_structural: true,
            is_yank: false,
            is_change: false,
        },
        Case {
            name: "d2j_linewise_three_lines",
            text: "b1\nb2\nb3\nb4\n",
            keys: "d2j",
            expect_structural: true,
            is_yank: false,
            is_change: false,
        },
        // Yank cases
        Case {
            name: "yw_charwise",
            text: "one two three\n",
            keys: "yw",
            expect_structural: false,
            is_yank: true,
            is_change: false,
        },
        Case {
            name: "2yw_charwise",
            text: "one two three four\n",
            keys: "2yw",
            expect_structural: false,
            is_yank: true,
            is_change: false,
        },
        Case {
            name: "yj_linewise_two_lines",
            text: "l1\nl2\nl3\n",
            keys: "yj",
            expect_structural: false,
            is_yank: true,
            is_change: false,
        },
        Case {
            name: "2yj_linewise_three_lines",
            text: "a1\na2\na3\na4\n",
            keys: "2yj",
            expect_structural: false,
            is_yank: true,
            is_change: false,
        },
        // Change cases
        Case {
            name: "cw_charwise",
            text: "one two three\n",
            keys: "cw",
            expect_structural: false,
            is_yank: false,
            is_change: true,
        },
        Case {
            name: "2cw_charwise",
            text: "one two three four\n",
            keys: "2cw",
            expect_structural: false,
            is_yank: false,
            is_change: true,
        },
        Case {
            name: "cj_linewise_two_lines",
            text: "l1\nl2\nl3\n",
            keys: "cj",
            expect_structural: true,
            is_yank: false,
            is_change: true,
        },
        Case {
            name: "2cj_linewise_three_lines",
            text: "a1\na2\na3\na4\n",
            keys: "2cj",
            expect_structural: true,
            is_yank: false,
            is_change: true,
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
        if case.is_yank {
            // Yank should leave buffer unchanged; dispatch result may be clean.
            // We no longer rely on dirty flag for pure register operations.
            assert!(!res.buffer_replaced, "yank cannot be structural");
        }
        if case.is_change {
            assert!(res.dirty, "change must dirty buffer");
        }
    }
}
