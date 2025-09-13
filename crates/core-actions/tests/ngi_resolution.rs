use core_actions::{PendingState, flush_pending_literal, translate_ngi};
use core_config::Config;
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_state::Mode;

fn kc(c: char) -> KeyEvent {
    KeyEvent {
        code: KeyCode::Char(c),
        mods: KeyModifiers::empty(),
    }
}

fn reset(cfg: &Config) {
    let esc = KeyEvent {
        code: KeyCode::Esc,
        mods: KeyModifiers::empty(),
    };
    let _ = translate_ngi(Mode::Normal, "", &esc, cfg);
    let _ = flush_pending_literal(cfg);
}

#[test]
fn resolution_idle_no_deadline_for_motion() {
    let cfg = Config::default();
    reset(&cfg);
    let resolution = translate_ngi(Mode::Normal, "", &kc('h'), &cfg);
    assert!(resolution.action.is_some());
    assert_eq!(resolution.pending_state, PendingState::Idle);
    assert!(resolution.timeout_deadline.is_none());
}

#[test]
fn resolution_idle_when_timeout_disabled() {
    let mut cfg = Config::default();
    cfg.file.input.timeout = false;
    reset(&cfg);
    let resolution = translate_ngi(Mode::Normal, "", &kc('l'), &cfg);
    assert!(resolution.action.is_some());
    assert_eq!(resolution.pending_state, PendingState::Idle);
    assert!(resolution.timeout_deadline.is_none());
}

#[test]
fn flush_pending_literal_noop_without_pending() {
    let cfg = Config::default();
    reset(&cfg);
    assert!(flush_pending_literal(&cfg).is_none());
}
