use core_actions::{Action, EditKind, translate_ngi};
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
fn ngi_normal_x_maps_delete_under() {
    let cfg = Config::default();
    let act = translate_ngi(Mode::Normal, "", &kc('x'), &cfg);
    match act.action {
        Some(Action::Edit(EditKind::DeleteUnder)) => {}
        other => panic!("unexpected: {:?}", other),
    }
}
