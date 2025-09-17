//! Blocking input collection on a dedicated thread (no polling timeout).
//! Phase 2 Step 16: now targets a bounded `tokio::mpsc::Sender` (natural backpressure via
//! `blocking_send`).

use core_events::{
    CHANNEL_BLOCKING_SENDS, CHANNEL_SEND_FAILURES, Event, InputEvent, KeyCode, KeyEvent,
    KeyModifiers, normalize_keycode,
};
use crossterm::event::{
    self, Event as CEvent, KeyCode as CKeyCode, KeyEventKind as CKind, KeyModifiers as CMods,
};
use std::thread;
use tracing::trace;

/// Spawn a blocking input thread. The thread exits automatically when the
/// receiving side of the channel is dropped (send will return Err).
pub fn spawn_input_thread(sender: tokio::sync::mpsc::Sender<Event>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let span = tracing::info_span!("input_thread");
        let _e = span.enter();
        loop {
            match event::read() {
                Ok(CEvent::Key(k)) => {
                    if k.kind != CKind::Press {
                        continue; // skip repeats and releases
                    }
                    let mods = map_mods(k.modifiers);
                    let code = match k.code {
                        CKeyCode::Char(c) => KeyCode::Char(c),
                        CKeyCode::Enter => KeyCode::Enter,
                        CKeyCode::Esc => KeyCode::Esc,
                        CKeyCode::Backspace => KeyCode::Backspace,
                        CKeyCode::Tab => KeyCode::Tab,
                        CKeyCode::Up => KeyCode::Up,
                        CKeyCode::Down => KeyCode::Down,
                        CKeyCode::Left => KeyCode::Left,
                        CKeyCode::Right => KeyCode::Right,
                        _ => continue,
                    };
                    if code == KeyCode::Char('c') && mods.contains(KeyModifiers::CTRL) {
                        if sender
                            .blocking_send(Event::Input(InputEvent::CtrlC))
                            .is_err()
                        {
                            CHANNEL_SEND_FAILURES
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            break;
                        }
                        CHANNEL_BLOCKING_SENDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        continue;
                    }
                    let evt = Event::Input(InputEvent::Key(KeyEvent {
                        code: normalize_keycode(code),
                        mods,
                    }));
                    if sender.blocking_send(evt).is_err() {
                        CHANNEL_SEND_FAILURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    CHANNEL_BLOCKING_SENDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                Ok(CEvent::Resize(w, h)) => {
                    trace!(w, h, "resize");
                    if sender
                        .blocking_send(Event::Input(InputEvent::Resize(w, h)))
                        .is_err()
                    {
                        CHANNEL_SEND_FAILURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    CHANNEL_BLOCKING_SENDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                Ok(_other) => { /* ignore mouse/paste for now */ }
                Err(_) => {
                    /* read error; we can choose to break */
                    break;
                }
            }
        }
    })
}

fn map_mods(m: CMods) -> KeyModifiers {
    let mut out = KeyModifiers::empty();
    if m.contains(CMods::CONTROL) {
        out |= KeyModifiers::CTRL;
    }
    if m.contains(CMods::ALT) {
        out |= KeyModifiers::ALT;
    }
    if m.contains(CMods::SHIFT) {
        out |= KeyModifiers::SHIFT;
    }
    out
}
