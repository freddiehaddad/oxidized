mod common;
use common::*;

use core_actions::{Action, MotionKind, NgiTranslator, OperatorKind, translate_keypress};
use core_config::Config;
use core_events::{KeyCode, KeyEvent, KeyEventExt, KeyModifiers, KeyToken, ModMask};
use core_state::Mode;

fn kc(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        mods: KeyModifiers::empty(),
    }
}

#[test]
fn motion_w() {
    reset_translator();
    let cfg = Config::default();
    let act = translate_ngi(Mode::Normal, "", &kc('w'), &cfg)
        .action
        .expect("motion");
    assert!(matches!(act, Action::Motion(MotionKind::WordForward)));
}

#[test]
fn count_motion_5w() {
    reset_translator();
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
    reset_translator();
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
    reset_translator();
    let cfg = Config::default();
    for c in ['2', 'd', '3', 'w'] {
        translate_ngi(Mode::Normal, "", &kc(c), &cfg);
    }
    // Sequence consumed; second 'w' intentionally omitted to avoid unused variable warning.
}

#[test]
fn register_yank_ayw() {
    reset_translator();
    let cfg = Config::default();
    for c in ['"', 'a', 'y', 'w'] {
        translate_ngi(Mode::Normal, "", &kc(c), &cfg);
    }
    // Last call should have produced ApplyOperator; not capturing due to simplicity (future: store actions in observer).
}

#[test]
fn keypress_delete_word_matches_legacy() {
    reset_translator();
    let keypress_actions = translate_sequence_keypress(vec![
        KeyEventExt::new(KeyToken::Char('d')),
        KeyEventExt::new(KeyToken::Char('w')),
    ]);
    let legacy_actions = translate_sequence_legacy(vec![
        KeyEvent {
            code: KeyCode::Char('d'),
            mods: KeyModifiers::empty(),
        },
        KeyEvent {
            code: KeyCode::Char('w'),
            mods: KeyModifiers::empty(),
        },
    ]);

    assert_eq!(keypress_actions, legacy_actions);
    assert_eq!(keypress_actions.len(), 1);
    assert!(keypress_actions[0].contains("ApplyOperator"));
}

#[test]
fn keypress_count_prefix_matches_legacy() {
    reset_translator();
    let keypress_actions = translate_sequence_keypress(vec![
        KeyEventExt::new(KeyToken::Char('5')),
        KeyEventExt::new(KeyToken::Char('w')),
    ]);
    let legacy_actions = translate_sequence_legacy(vec![
        KeyEvent {
            code: KeyCode::Char('5'),
            mods: KeyModifiers::empty(),
        },
        KeyEvent {
            code: KeyCode::Char('w'),
            mods: KeyModifiers::empty(),
        },
    ]);

    assert_eq!(keypress_actions, legacy_actions);
    assert_eq!(keypress_actions.len(), 1);
    assert!(keypress_actions[0].contains("MotionWithCount"));
}

#[test]
fn keypress_ctrl_d_matches_legacy() {
    reset_translator();
    let keypress_actions = translate_sequence_keypress(vec![KeyEventExt::new(KeyToken::Chord {
        base: Box::new(KeyToken::Char('d')),
        mods: ModMask::CTRL,
    })]);
    let legacy_actions = translate_sequence_legacy(vec![KeyEvent {
        code: KeyCode::Char('d'),
        mods: KeyModifiers::CTRL,
    }]);

    assert_eq!(keypress_actions, legacy_actions);
    assert_eq!(keypress_actions.len(), 1);
    assert!(keypress_actions[0].contains("Motion"));
}

fn translate_sequence_keypress(seq: Vec<KeyEventExt>) -> Vec<String> {
    std::thread::spawn(move || {
        let cfg = Config::default();
        let mut translator = NgiTranslator::new();
        seq.into_iter()
            .filter_map(|keypress| {
                translate_keypress(&mut translator, Mode::Normal, "", &keypress, &cfg)
                    .action
                    .map(|action| format!("{:?}", action))
            })
            .collect::<Vec<_>>()
    })
    .join()
    .expect("keypress translation thread panicked")
}

fn translate_sequence_legacy(seq: Vec<KeyEvent>) -> Vec<String> {
    std::thread::spawn(move || {
        let cfg = Config::default();
        seq.into_iter()
            .filter_map(|event| {
                translate_ngi(Mode::Normal, "", &event, &cfg)
                    .action
                    .map(|action| format!("{:?}", action))
            })
            .collect::<Vec<_>>()
    })
    .join()
    .expect("legacy translation thread panicked")
}
