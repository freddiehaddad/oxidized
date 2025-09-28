#![allow(dead_code)] // Shared across many integration tests; each test binary uses a subset of helpers.

use core_actions::{
    Action, NgiResolution, NgiTranslator, flush_pending_literal as core_flush_pending_literal,
    translate_key as core_translate_key, translate_ngi as core_translate_ngi,
};
use core_config::Config;
use core_events::KeyEvent;
use core_state::Mode;
use std::{cell::RefCell, time::Instant};

thread_local! {
    static TRANSLATOR: RefCell<NgiTranslator> = RefCell::new(NgiTranslator::new());
}

pub fn reset_translator() {
    TRANSLATOR.with(|t| *t.borrow_mut() = NgiTranslator::new());
}

pub fn translate_key(mode: Mode, pending: &str, key: &KeyEvent) -> Option<Action> {
    TRANSLATOR.with(|t| core_translate_key(&mut t.borrow_mut(), mode, pending, key))
}

pub fn translate_ngi(mode: Mode, pending: &str, key: &KeyEvent, cfg: &Config) -> NgiResolution {
    translate_ngi_at(mode, pending, key, cfg, Instant::now())
}

pub fn translate_ngi_at(
    mode: Mode,
    pending: &str,
    key: &KeyEvent,
    cfg: &Config,
    timestamp: Instant,
) -> NgiResolution {
    TRANSLATOR.with(|t| core_translate_ngi(&mut t.borrow_mut(), mode, pending, key, cfg, timestamp))
}

pub fn flush_pending_literal(cfg: &Config) -> Option<NgiResolution> {
    flush_pending_literal_at(cfg, Instant::now())
}

pub fn flush_pending_literal_at(cfg: &Config, now: Instant) -> Option<NgiResolution> {
    TRANSLATOR.with(|t| core_flush_pending_literal(&mut t.borrow_mut(), cfg, now))
}
