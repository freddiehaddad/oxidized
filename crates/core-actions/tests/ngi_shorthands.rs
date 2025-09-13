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
fn ngi_normal_d_maps_to_delete_to_eol() {
    let cfg = Config::default();
    let act = translate_ngi(Mode::Normal, "", &kc('D'), &cfg).action;
    match act {
        Some(Action::ApplyOperator {
            op,
            motion,
            count,
            register,
        }) => {
            assert_eq!(op, OperatorKind::Delete);
            assert_eq!(motion, MotionKind::LineEnd);
            assert_eq!(count, 1);
            assert!(register.is_none());
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn ngi_normal_c_maps_to_change_to_eol() {
    let cfg = Config::default();
    let act = translate_ngi(Mode::Normal, "", &kc('C'), &cfg).action;
    match act {
        Some(Action::ApplyOperator {
            op,
            motion,
            count,
            register,
        }) => {
            assert_eq!(op, OperatorKind::Change);
            assert_eq!(motion, MotionKind::LineEnd);
            assert_eq!(count, 1);
            assert!(register.is_none());
        }
        other => panic!("unexpected: {:?}", other),
    }
}
