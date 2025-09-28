mod common;
use common::*;

use core_actions::PendingState;
use core_config::Config;
use core_events::{KeyCode, KeyEvent, KeyModifiers};
use core_state::Mode;
use std::time::{Duration, Instant};

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
    reset_translator();
    let cfg = Config::default();
    reset(&cfg);
    let resolution = translate_ngi(Mode::Normal, "", &kc('h'), &cfg);
    assert!(resolution.action.is_some());
    assert_eq!(resolution.pending_state, PendingState::Idle);
    assert!(resolution.timeout_deadline.is_none());
}

#[test]
fn resolution_idle_when_timeout_disabled() {
    reset_translator();
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
    reset_translator();
    let cfg = Config::default();
    reset(&cfg);
    assert!(flush_pending_literal(&cfg).is_none());
}

#[test]
fn awaiting_more_deadline_uses_keypress_timestamp() {
    reset_translator();
    let mut cfg = Config::default();
    cfg.file.input.timeout = true;
    cfg.file.input.timeoutlen = 1500;

    let start = Instant::now();
    let resolution = translate_ngi_at(Mode::Normal, "", &kc('z'), &cfg, start);
    match resolution.pending_state {
        PendingState::AwaitingMore { buffered_len } => assert_eq!(buffered_len, 1),
        other => panic!("expected AwaitingMore state, got {:?}", other),
    }
    let expected_deadline = start + Duration::from_millis(cfg.file.input.timeoutlen as u64);
    assert_eq!(resolution.timeout_deadline, Some(expected_deadline));

    // Queue a second fallback literal so the flush path returns another pending deadline.
    let second_at = start + Duration::from_millis(100);
    let second = translate_ngi_at(Mode::Normal, "", &kc('a'), &cfg, second_at);
    match second.pending_state {
        PendingState::AwaitingMore { buffered_len } => assert_eq!(buffered_len, 2),
        other => panic!("expected AwaitingMore state, got {:?}", other),
    }
    assert_eq!(second.timeout_deadline, Some(expected_deadline));

    let flush_at = start + Duration::from_millis(250);
    let flushed = flush_pending_literal_at(&cfg, flush_at).expect("flush should emit resolution");
    match flushed.pending_state {
        PendingState::AwaitingMore { buffered_len } => assert_eq!(buffered_len, 1),
        other => panic!("expected AwaitingMore state after flush, got {:?}", other),
    }
    let expected_flush_deadline =
        flush_at + Duration::from_millis(cfg.file.input.timeoutlen as u64);
    assert_eq!(flushed.timeout_deadline, Some(expected_flush_deadline));
}
