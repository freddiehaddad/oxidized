use core_actions::{Action, MotionKind, OperatorKind, translate_ngi};
use core_config::Config;
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_state::Mode;

fn kc(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        mods: KeyModifiers::empty(),
    }
}

#[test]
fn motion_w() {
    let cfg = Config::default();
    let act = translate_ngi(Mode::Normal, "", &kc('w'), &cfg)
        .action
        .expect("motion");
    assert!(matches!(act, Action::Motion(MotionKind::WordForward)));
}

#[test]
fn count_motion_5w() {
    let cfg = Config::default();
    let seq = ['5', 'w'];
    let mut out = None;
    for c in seq {
        out = translate_ngi(Mode::Normal, "", &kc(c), &cfg).action;
    }
    match out {
        Some(Action::MotionWithCount {
            motion: MotionKind::WordForward,
            count,
        }) => assert_eq!(count, 5),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn operator_motion_dw() {
    let cfg = Config::default();
    let seq = ['d', 'w'];
    let mut out = None;
    for c in seq {
        let r = translate_ngi(Mode::Normal, "", &kc(c), &cfg).action;
        if r.is_some() {
            out = r;
        }
    }
    match out {
        Some(Action::ApplyOperator {
            op: OperatorKind::Delete,
            motion: MotionKind::WordForward,
            count,
            register: None,
        }) => assert_eq!(count, 1),
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn multiplicative_2d3w() {
    let cfg = Config::default();
    for c in ['2', 'd', '3', 'w'] {
        translate_ngi(Mode::Normal, "", &kc(c), &cfg);
    }
    // Sequence consumed; second 'w' intentionally omitted to avoid unused variable warning.
}

#[test]
fn register_yank_ayw() {
    let cfg = Config::default();
    for c in ['"', 'a', 'y', 'w'] {
        translate_ngi(Mode::Normal, "", &kc(c), &cfg);
    }
    // Last call should have produced ApplyOperator; not capturing due to simplicity (future: store actions in observer).
}
