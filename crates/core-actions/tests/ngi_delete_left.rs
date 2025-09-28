mod common;
use common::*;

use core_actions::{Action, EditKind};
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
fn ngi_normal_x_maps_delete_left() {
    reset_translator();
    let cfg = Config::default();
    let act = translate_ngi(Mode::Normal, "", &kc('X'), &cfg);
    match act.action {
        Some(Action::Edit(EditKind::DeleteLeft { count, register })) => {
            assert_eq!(count, 1);
            assert!(register.is_none());
        }
        other => panic!("unexpected: {:?}", other),
    }
}
