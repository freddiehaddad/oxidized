//! Blocking input collection on a dedicated thread (no polling timeout).
//! Phase 2 Step 16: now targets a bounded `tokio::mpsc::Sender` (natural backpressure via
//! `blocking_send`).

use core_events::{
    CHANNEL_BLOCKING_SENDS, CHANNEL_SEND_FAILURES, Event, InputEvent, KeyCode, KeyEvent,
    KeyModifiers, PASTE_BYTES, PASTE_CHUNKS, PASTE_SESSIONS, normalize_keycode,
};
use crossterm::event::{
    self, Event as CEvent, KeyCode as CKeyCode, KeyEventKind as CKind, KeyModifiers as CMods,
};
use std::io::{self, Write};
use std::thread;
use tracing::debug;
use tracing::trace;

#[inline]
fn log_paste_chunk_flush(chunk: &str) {
    tracing::trace!(target: "input.paste", chunk_len = chunk.len(), "chunk_flush");
}

/// Spawn a blocking input thread. The thread exits automatically when the
/// receiving side of the channel is dropped (send will return Err).
pub fn spawn_input_thread(sender: tokio::sync::mpsc::Sender<Event>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let span = tracing::debug_span!(target: "input.thread", "input_thread");
        let _e = span.enter();

        const PASTE_START: &[u8] = b"\x1b[200~"; // ESC [ 200 ~
        const PASTE_END: &[u8] = b"\x1b[201~"; // ESC [ 201 ~

        enum PasteFsm {
            Idle,
            MaybeStart(Vec<u8>),
            Active { buf: Vec<u8> },
        }
        let mut paste_fsm = PasteFsm::Idle;

        // Attempt to enable bracketed paste mode (best effort). Ignore errors.
        if let Err(e) = write!(io::stdout(), "\x1b[?2004h") {
            debug!(target = "input.paste", ?e, "enable_failed");
        }
        let _ = io::stdout().flush();

        loop {
            match event::read() {
                Ok(CEvent::Key(k)) => {
                    if k.kind != CKind::Press {
                        continue;
                    }
                    let mods = map_mods(k.modifiers);
                    // Fast path ordinary keys when not in paste active mode.
                    {
                        use crossterm::event::KeyCode as CK;
                        match (&mut paste_fsm, &k.code) {
                            (PasteFsm::Idle, CK::Esc) => {
                                // Potential start sequence; transition to MaybeStart collecting bytes through subsequent key events.
                                paste_fsm = PasteFsm::MaybeStart(Vec::with_capacity(8));
                                continue; // do not emit plain Esc yet
                            }
                            (PasteFsm::MaybeStart(acc), CK::Char(ch)) => {
                                acc.push(*ch as u8);
                                let slice = acc.as_slice();
                                if PASTE_START.ends_with(slice) && slice == &PASTE_START[2..] {
                                    // We have collected '[200~' after initial ESC.
                                    trace!(target = "input.paste", "start");
                                    if sender
                                        .blocking_send(Event::Input(InputEvent::PasteStart))
                                        .is_err()
                                    {
                                        CHANNEL_SEND_FAILURES
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        break;
                                    }
                                    CHANNEL_BLOCKING_SENDS
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    // Telemetry: count paste sessions on successful send
                                    PASTE_SESSIONS
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    paste_fsm = PasteFsm::Active {
                                        buf: Vec::with_capacity(4096),
                                    };
                                } else if !PASTE_START[2..].starts_with(slice) {
                                    // Not a valid start sequence; emit original ESC then replay collected chars as keys.
                                    if let Err(break_loop) = emit_key(KeyCode::Esc, mods, &sender)
                                        && break_loop
                                    {
                                        break;
                                    }
                                    for b in slice {
                                        if let Err(break_loop) =
                                            emit_key(KeyCode::Char(*b as char), mods, &sender)
                                            && break_loop
                                        {
                                            break;
                                        }
                                    }
                                    paste_fsm = PasteFsm::Idle;
                                }
                                continue;
                            }
                            (PasteFsm::Active { buf }, CK::Esc) => {
                                // Possible end marker; push a sentinel and wait for following chars.
                                // We'll treat subsequent chars as part of end probe; store current buffer length.
                                buf.extend_from_slice(b"\x1b");
                                continue;
                            }
                            (PasteFsm::Active { buf }, CK::Char(ch)) => {
                                buf.push(*ch as u8);
                                // Check if recent bytes end with full end marker.
                                if buf.ends_with(PASTE_END) {
                                    // Remove the end marker bytes from buffer prior to final chunk flush.
                                    let end_len = PASTE_END.len();
                                    let content_len = buf.len() - end_len;
                                    let content = buf[..content_len].to_vec();
                                    if !content.is_empty()
                                        && let Ok(s) = String::from_utf8(content)
                                    {
                                        let slen = s.len();
                                        log_paste_chunk_flush(&s);
                                        // chunk length logged via trace; counters added in Step 8
                                        if sender
                                            .blocking_send(Event::Input(InputEvent::PasteChunk(s)))
                                            .is_err()
                                        {
                                            CHANNEL_SEND_FAILURES
                                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                            break;
                                        }
                                        CHANNEL_BLOCKING_SENDS
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        // Telemetry: count chunks and bytes on successful send
                                        PASTE_CHUNKS
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        PASTE_BYTES.fetch_add(
                                            slen as u64,
                                            std::sync::atomic::Ordering::Relaxed,
                                        );
                                    }
                                    trace!(target = "input.paste", "end");
                                    if sender
                                        .blocking_send(Event::Input(InputEvent::PasteEnd))
                                        .is_err()
                                    {
                                        CHANNEL_SEND_FAILURES
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        break;
                                    }
                                    CHANNEL_BLOCKING_SENDS
                                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    paste_fsm = PasteFsm::Idle;
                                } else if buf.len() >= 4096 {
                                    // flush large chunk
                                    let mut flush = Vec::new();
                                    std::mem::swap(&mut flush, buf);
                                    if let Ok(s) = String::from_utf8(flush) {
                                        let slen = s.len();
                                        log_paste_chunk_flush(&s);
                                        // chunk length logged via trace; counters added in Step 8
                                        if sender
                                            .blocking_send(Event::Input(InputEvent::PasteChunk(s)))
                                            .is_err()
                                        {
                                            CHANNEL_SEND_FAILURES
                                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                            break;
                                        }
                                        CHANNEL_BLOCKING_SENDS
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        // Telemetry: count chunks and bytes on successful send
                                        PASTE_CHUNKS
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                        PASTE_BYTES.fetch_add(
                                            slen as u64,
                                            std::sync::atomic::Ordering::Relaxed,
                                        );
                                    }
                                }
                                continue;
                            }
                            _ => {}
                        }
                    }
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
                    trace!(target: "input.event", w, h, "resize");
                    if sender
                        .blocking_send(Event::Input(InputEvent::Resize(w, h)))
                        .is_err()
                    {
                        CHANNEL_SEND_FAILURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        break;
                    }
                    CHANNEL_BLOCKING_SENDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                // NOTE: crossterm does not surface bracketed paste chunks as dedicated events yet.
                // Some terminals emit them as a rapid sequence of Key events; to support true
                // streaming we would need raw stdin reads. As an incremental step (Phase C Step 7)
                // we peek into stdin for pending bytes when we see an ESC and attempt to match the
                // known bracketed paste control sequences. If not matched we fall back to treating
                // the ESC as a normal key (Esc). This keeps logic local and avoids altering the
                // channel protocol.
                Ok(other) => {
                    if let CEvent::Key(_k) = &other {
                        // already handled above
                    }
                    // Future: handle mouse / focus here.
                }
                Err(_) => {
                    break;
                }
            }
        }
        // Attempt to disable bracketed paste mode on exit.
        let _ = write!(io::stdout(), "\x1b[?2004l");
        let _ = io::stdout().flush();
    })
}

fn emit_key(
    code: KeyCode,
    mods: KeyModifiers,
    sender: &tokio::sync::mpsc::Sender<Event>,
) -> Result<(), bool> {
    let evt = Event::Input(InputEvent::Key(KeyEvent {
        code: normalize_keycode(code),
        mods,
    }));
    if sender.blocking_send(evt).is_err() {
        CHANNEL_SEND_FAILURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        return Err(true);
    }
    CHANNEL_BLOCKING_SENDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(())
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

#[cfg(test)]
mod tests {

    use std::fmt;
    use std::sync::{Arc, Mutex};
    use tracing::Subscriber;
    use tracing::dispatcher::Dispatch;
    use tracing::field::{Field, Visit};
    use tracing_subscriber::layer::Context;
    use tracing_subscriber::layer::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::registry::Registry;

    #[derive(Clone, Default)]
    struct Capture {
        events: Arc<Mutex<Vec<CapturedEvent>>>,
    }

    #[derive(Clone, Debug)]
    struct CapturedEvent {
        target: String,
        fields: Vec<(String, String)>,
    }

    #[derive(Default)]
    struct FieldCollector {
        fields: Vec<(String, String)>,
    }

    impl Visit for FieldCollector {
        fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
            self.fields
                .push((field.name().to_string(), format!("{:?}", value)));
        }
    }

    impl<S> Layer<S> for Capture
    where
        S: Subscriber,
    {
        fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
            let mut collector = FieldCollector::default();
            event.record(&mut collector);
            let meta = event.metadata();
            self.events.lock().unwrap().push(CapturedEvent {
                target: meta.target().to_string(),
                fields: collector.fields,
            });
        }
    }

    #[test]
    fn chunk_flush_log_redacts_content() {
        let capture = Capture::default();
        let events = capture.events.clone();
        let subscriber = Registry::default().with(capture);
        let dispatch = Dispatch::new(subscriber);

        tracing::dispatcher::with_default(&dispatch, || {
            let secret = "secret paste payload 💣";
            super::log_paste_chunk_flush(secret);
        });

        let events = events.lock().unwrap();
        assert!(
            !events.is_empty(),
            "expected at least one captured input.paste event"
        );
        let event = events
            .iter()
            .find(|e| e.target == "input.paste")
            .expect("missing input.paste event");
        assert!(
            event.fields.iter().any(|(name, _)| name == "chunk_len"),
            "chunk_len field missing from event"
        );
        for (_, value) in &event.fields {
            assert!(
                !value.contains("secret paste payload"),
                "event leaked raw paste content: {value}"
            );
            assert!(
                !value.contains("💣"),
                "event leaked emoji from paste content: {value}"
            );
        }
    }
}
