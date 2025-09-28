use crate::key_token::{KeyPressParts, map_key_event};
use crate::log_paste_chunk_flush;
use core_events::{
    ASYNC_INPUT_STARTS, ASYNC_INPUT_STOP_CHANNEL, ASYNC_INPUT_STOP_ERROR, ASYNC_INPUT_STOP_SIGNAL,
    ASYNC_INPUT_STOP_STREAM, CHANNEL_BLOCKING_SENDS, CHANNEL_SEND_FAILURES, Event, InputEvent,
    KEYPRESS_REPEAT, KEYPRESS_TOTAL, KeyEventExt, KeyToken, ModMask, NamedKey, PASTE_BYTES,
    PASTE_CHUNKS, PASTE_SESSIONS,
};
use crossterm::event::{
    Event as CEvent, EventStream, KeyCode as CKeyCode, KeyEvent as CKeyEvent, KeyEventKind as CKind,
};
use std::io::{self, Write};
use std::sync::Arc;
use tokio::sync::{Notify, mpsc::Sender};
use tokio::task;
use tokio_stream::StreamExt;
use tracing::{debug, info, trace, warn};

const PASTE_START: &[u8] = b"\x1b[200~";
const PASTE_END: &[u8] = b"\x1b[201~";
const DEFAULT_PASTE_CAPACITY: usize = 4_096;

#[derive(Clone, Debug)]
pub struct AsyncInputShutdown {
    notify: Arc<Notify>,
}

impl AsyncInputShutdown {
    pub fn signal(&self) {
        self.notify.notify_one();
    }
}

#[derive(Clone, Debug)]
struct ShutdownListener {
    notify: Arc<Notify>,
}

impl ShutdownListener {
    fn new_pair() -> (AsyncInputShutdown, Self) {
        let notify = Arc::new(Notify::new());
        (
            AsyncInputShutdown {
                notify: notify.clone(),
            },
            ShutdownListener { notify },
        )
    }

    async fn wait(&self) {
        self.notify.notified().await;
    }
}

/// Spawn a Tokio task that mirrors the blocking input pipeline using `EventStream`.
pub(crate) fn spawn_async_event_task(
    sender: Sender<Event>,
) -> (task::JoinHandle<()>, AsyncInputShutdown) {
    let (shutdown, listener) = ShutdownListener::new_pair();
    let handle = task::spawn(async move {
        let span = tracing::debug_span!(target: "input.thread", "input_async_task");
        let _enter = span.enter();

        if let Err(join_err) = task::spawn_blocking(enable_bracketed_paste).await {
            debug!(target: "input.paste", ?join_err, "enable_failed_join");
        }

        let stream = EventStream::new();
        AsyncEventStreamTask::new(sender, stream, listener)
            .run()
            .await;

        if let Err(join_err) = task::spawn_blocking(disable_bracketed_paste).await {
            debug!(target: "input.paste", ?join_err, "disable_failed_join");
        }
    });

    (handle, shutdown)
}

fn enable_bracketed_paste() {
    if let Err(e) = write!(io::stdout(), "\x1b[?2004h") {
        debug!(target: "input.paste", ?e, "enable_failed");
    }
    let _ = io::stdout().flush();
}

fn disable_bracketed_paste() {
    if let Err(e) = write!(io::stdout(), "\x1b[?2004l") {
        debug!(target: "input.paste", ?e, "disable_failed");
    }
    let _ = io::stdout().flush();
}

#[derive(Debug, Default)]
enum PasteFsm {
    #[default]
    Idle,
    MaybeStart(Vec<u8>),
    Active {
        buf: Vec<u8>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExitReason {
    Running,
    ShutdownSignal,
    ChannelClosed,
    StreamEnded,
    StreamError,
}

impl ExitReason {
    fn as_str(&self) -> &'static str {
        match self {
            ExitReason::Running => "running",
            ExitReason::ShutdownSignal => "shutdown_signal",
            ExitReason::ChannelClosed => "channel_closed",
            ExitReason::StreamEnded => "stream_ended",
            ExitReason::StreamError => "stream_error",
        }
    }
}

struct AsyncEventStreamTask<S>
where
    S: tokio_stream::Stream<Item = io::Result<CEvent>> + Send + Unpin + 'static,
{
    sender: Sender<Event>,
    stream: S,
    paste_fsm: PasteFsm,
    shutdown: ShutdownListener,
    exit_reason: ExitReason,
    stream_error: Option<io::ErrorKind>,
}

impl<S> AsyncEventStreamTask<S>
where
    S: tokio_stream::Stream<Item = io::Result<CEvent>> + Send + Unpin + 'static,
{
    fn new(sender: Sender<Event>, stream: S, shutdown: ShutdownListener) -> Self {
        Self {
            sender,
            stream,
            paste_fsm: PasteFsm::default(),
            shutdown,
            exit_reason: ExitReason::Running,
            stream_error: None,
        }
    }

    pub async fn run(mut self) {
        info!(target: "input.thread", "async_input_task_started");
        ASYNC_INPUT_STARTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.exit_reason = ExitReason::StreamEnded;
        loop {
            let maybe_result = tokio::select! {
                biased;
                _ = self.shutdown.wait() => {
                    self.exit_reason = ExitReason::ShutdownSignal;
                    break;
                }
                result = self.stream.next() => result,
            };

            let Some(result) = maybe_result else {
                break;
            };

            match result {
                Ok(CEvent::Key(key)) => {
                    if !self.handle_key_event(key).await {
                        break;
                    }
                }
                Ok(CEvent::Resize(w, h)) => {
                    trace!(target: "input.event", w, h, "resize");
                    if !self
                        .send_event(Event::Input(InputEvent::Resize(w, h)))
                        .await
                    {
                        break;
                    }
                }
                Ok(CEvent::Paste(data)) => {
                    if !self.handle_clipboard_paste(data).await {
                        break;
                    }
                }
                Ok(other) => {
                    if matches!(other, CEvent::Key(_)) {
                        // already handled via Key arm
                    }
                }
                Err(err) => {
                    self.exit_reason = ExitReason::StreamError;
                    self.stream_error = Some(err.kind());
                    break;
                }
            }
        }

        let reason = match self.exit_reason {
            ExitReason::Running => ExitReason::StreamEnded,
            other => other,
        };

        match reason {
            ExitReason::ShutdownSignal => {
                ASYNC_INPUT_STOP_SIGNAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            ExitReason::ChannelClosed => {
                ASYNC_INPUT_STOP_CHANNEL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            ExitReason::StreamEnded => {
                ASYNC_INPUT_STOP_STREAM.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            ExitReason::StreamError => {
                ASYNC_INPUT_STOP_ERROR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            ExitReason::Running => {}
        }

        if matches!(reason, ExitReason::StreamError) {
            if let Some(kind) = self.stream_error {
                warn!(target: "input.thread", error_kind = ?kind, "async_input_task_stream_error");
            } else {
                warn!(target: "input.thread", "async_input_task_stream_error");
            }
        }

        info!(target: "input.thread", reason = reason.as_str(), "async_input_task_stopped");
    }

    async fn handle_key_event(&mut self, key: CKeyEvent) -> bool {
        if !matches!(key.kind, CKind::Press | CKind::Repeat) {
            return true;
        }

        // Paste detection path mirrors blocking implementation.
        match (&mut self.paste_fsm, &key.code) {
            (PasteFsm::Idle, CKeyCode::Esc) => {
                self.paste_fsm = PasteFsm::MaybeStart(Vec::with_capacity(8));
                return true;
            }
            (PasteFsm::MaybeStart(acc), CKeyCode::Char(ch)) => {
                acc.push(*ch as u8);
                let slice = acc.as_slice();
                let target_with_bracket = &PASTE_START[1..];
                let target_without_bracket = &PASTE_START[2..];
                let is_full_match = slice == target_with_bracket || slice == target_without_bracket;
                let is_valid_prefix = target_with_bracket.starts_with(slice)
                    || target_without_bracket.starts_with(slice);

                if is_full_match {
                    trace!(target: "input.paste", "start");
                    if !self.send_event(Event::Input(InputEvent::PasteStart)).await {
                        return false;
                    }
                    PASTE_SESSIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    self.paste_fsm = PasteFsm::Active {
                        buf: Vec::with_capacity(DEFAULT_PASTE_CAPACITY),
                    };
                } else if !is_valid_prefix {
                    let replay = acc.clone();
                    if !self
                        .emit_replayed_keypress(KeyToken::Named(NamedKey::Esc))
                        .await
                    {
                        return false;
                    }
                    for b in replay {
                        if !self.emit_replayed_keypress(KeyToken::Char(b as char)).await {
                            return false;
                        }
                    }
                    self.paste_fsm = PasteFsm::Idle;
                }
                return true;
            }
            (PasteFsm::Active { buf }, CKeyCode::Esc) => {
                buf.extend_from_slice(b"\x1b");
                return true;
            }
            (PasteFsm::Active { buf }, CKeyCode::Char(ch)) => {
                buf.push(*ch as u8);
                if buf.ends_with(PASTE_END) {
                    let end_len = PASTE_END.len();
                    let content_len = buf.len() - end_len;
                    let content = buf[..content_len].to_vec();
                    if !content.is_empty()
                        && let Ok(s) = String::from_utf8(content)
                    {
                        let slen = s.len();
                        log_paste_chunk_flush(&s);
                        if !self
                            .send_event(Event::Input(InputEvent::PasteChunk(s)))
                            .await
                        {
                            return false;
                        }
                        PASTE_CHUNKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        PASTE_BYTES.fetch_add(slen as u64, std::sync::atomic::Ordering::Relaxed);
                    }
                    trace!(target: "input.paste", "end");
                    if !self.send_event(Event::Input(InputEvent::PasteEnd)).await {
                        return false;
                    }
                    self.paste_fsm = PasteFsm::Idle;
                } else if buf.len() >= DEFAULT_PASTE_CAPACITY {
                    let mut flush = Vec::new();
                    std::mem::swap(&mut flush, buf);
                    if let Ok(s) = String::from_utf8(flush) {
                        let slen = s.len();
                        log_paste_chunk_flush(&s);
                        if !self
                            .send_event(Event::Input(InputEvent::PasteChunk(s)))
                            .await
                        {
                            return false;
                        }
                        PASTE_CHUNKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        PASTE_BYTES.fetch_add(slen as u64, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                return true;
            }
            _ => {}
        }

        if matches!(key.code, CKeyCode::Char('c'))
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            return self.send_event(Event::Input(InputEvent::CtrlC)).await;
        }

        if let Some(parts) = map_key_event(&key) {
            return self.emit_keypress(parts).await;
        }

        true
    }

    async fn handle_clipboard_paste(&mut self, data: String) -> bool {
        trace!(target: "input.paste", len = data.len(), "paste_event");
        if !self.send_event(Event::Input(InputEvent::PasteStart)).await {
            return false;
        }
        PASTE_SESSIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let mut remaining = data.as_str();
        while !remaining.is_empty() {
            let (chunk, rest) = split_utf8_chunk(remaining);
            if chunk.is_empty() {
                break;
            }
            log_paste_chunk_flush(chunk);
            if !self
                .send_event(Event::Input(InputEvent::PasteChunk(chunk.to_string())))
                .await
            {
                return false;
            }
            PASTE_CHUNKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            PASTE_BYTES.fetch_add(chunk.len() as u64, std::sync::atomic::Ordering::Relaxed);
            remaining = rest;
        }

        trace!(target: "input.paste", "paste_event_end");
        self.send_event(Event::Input(InputEvent::PasteEnd)).await
    }

    async fn emit_keypress(&mut self, parts: KeyPressParts) -> bool {
        let KeyPressParts {
            token,
            mods,
            repeat,
        } = parts;
        let token = if mods.is_empty() {
            token
        } else {
            KeyToken::Chord {
                base: Box::new(token),
                mods,
            }
        };

        trace!(
            target: "input.event",
            kind = "keypress",
            repeat,
            mods = ?mods,
            token_kind = token_kind_label(&token)
        );

        let event = Event::Input(InputEvent::KeyPress(KeyEventExt::with_repeat(
            token, repeat,
        )));
        let sent = self.send_event(event).await;
        if sent {
            KEYPRESS_TOTAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if repeat {
                KEYPRESS_REPEAT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
        sent
    }

    async fn emit_replayed_keypress(&mut self, token: KeyToken) -> bool {
        self.emit_keypress(KeyPressParts {
            token,
            mods: ModMask::empty(),
            repeat: false,
        })
        .await
    }

    async fn send_event(&mut self, event: Event) -> bool {
        match self.sender.send(event).await {
            Ok(_) => {
                CHANNEL_BLOCKING_SENDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                true
            }
            Err(_) => {
                CHANNEL_SEND_FAILURES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if !matches!(self.exit_reason, ExitReason::ShutdownSignal) {
                    self.exit_reason = ExitReason::ChannelClosed;
                }
                false
            }
        }
    }
}

fn token_kind_label(token: &KeyToken) -> &'static str {
    match token {
        KeyToken::Char(_) => "char",
        KeyToken::Named(_) => "named",
        KeyToken::Chord { .. } => "chord",
    }
}

fn split_utf8_chunk(input: &str) -> (&str, &str) {
    if input.len() <= DEFAULT_PASTE_CAPACITY {
        return (input, "");
    }

    let mut idx = DEFAULT_PASTE_CAPACITY;
    while idx > 0 && !input.is_char_boundary(idx) {
        idx -= 1;
    }

    if idx == 0 {
        return (input, "");
    }

    input.split_at(idx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_events::{
        ASYNC_INPUT_STARTS, ASYNC_INPUT_STOP_CHANNEL, ASYNC_INPUT_STOP_SIGNAL, Event, InputEvent,
    };
    use std::io;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, Mutex};
    use tokio::sync::{Mutex as TokioMutex, mpsc};
    use tokio::time::{Duration, timeout};
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use tracing::{Metadata, Subscriber, subscriber::Interest};

    use tracing::field::{Field, Visit};
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
    use tracing_subscriber::registry::Registry;

    static LOG_CAPTURE_GUARD: TokioMutex<()> = TokioMutex::const_new(());

    #[derive(Clone, Default)]
    struct LogCapture {
        events: Arc<Mutex<Vec<CapturedLog>>>,
    }

    #[derive(Clone, Debug)]
    struct CapturedLog {
        target: String,
        fields: Vec<(String, String)>,
    }

    #[derive(Default)]
    struct LogVisitor {
        fields: Vec<(String, String)>,
    }

    impl Visit for LogVisitor {
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            self.fields
                .push((field.name().to_string(), format!("{:?}", value)));
        }
    }

    impl<S> Layer<S> for LogCapture
    where
        S: Subscriber,
    {
        fn register_callsite(
            &self,
            _metadata: &'static tracing::Metadata<'static>,
        ) -> tracing::subscriber::Interest {
            Interest::always()
        }

        fn enabled(&self, metadata: &Metadata<'_>, _ctx: Context<'_, S>) -> bool {
            metadata.target().starts_with("input.")
        }

        fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
            let mut visitor = LogVisitor::default();
            event.record(&mut visitor);
            let meta = event.metadata();
            self.events.lock().unwrap().push(CapturedLog {
                target: meta.target().to_string(),
                fields: visitor.fields,
            });
        }
    }

    #[tokio::test]
    async fn forwards_basic_key_events() {
        let base_total = KEYPRESS_TOTAL.fetch_add(0, Ordering::Relaxed);
        let base_repeat = KEYPRESS_REPEAT.fetch_add(0, Ordering::Relaxed);

        let outputs = run_scenario(vec![CEvent::Key(CKeyEvent::new(
            CKeyCode::Char('a'),
            crossterm::event::KeyModifiers::NONE,
        ))])
        .await;

        match outputs.as_slice() {
            [Event::Input(InputEvent::KeyPress(keypress))] => {
                assert!(matches!(keypress.token, KeyToken::Char('a')));
                assert!(!keypress.repeat);
            }
            other => panic!("unexpected output sequence: {other:?}"),
        }

        let after_total = KEYPRESS_TOTAL.fetch_add(0, Ordering::Relaxed);
        let after_repeat = KEYPRESS_REPEAT.fetch_add(0, Ordering::Relaxed);
        assert_eq!(after_total.saturating_sub(base_total), 1);
        assert_eq!(after_repeat.saturating_sub(base_repeat), 0);
    }

    #[tokio::test]
    async fn repeat_key_events_set_repeat_flag() {
        let base_total = KEYPRESS_TOTAL.fetch_add(0, Ordering::Relaxed);
        let base_repeat = KEYPRESS_REPEAT.fetch_add(0, Ordering::Relaxed);

        let mut c_event = CKeyEvent::new(CKeyCode::Char('j'), crossterm::event::KeyModifiers::NONE);
        c_event.kind = CKind::Repeat;

        let outputs = run_scenario(vec![CEvent::Key(c_event)]).await;

        match outputs.as_slice() {
            [Event::Input(InputEvent::KeyPress(keypress))] => {
                assert!(matches!(keypress.token, KeyToken::Char('j')));
                assert!(keypress.repeat, "repeat flag should propagate");
            }
            other => panic!("unexpected output sequence: {other:?}"),
        }

        let after_total = KEYPRESS_TOTAL.fetch_add(0, Ordering::Relaxed);
        let after_repeat = KEYPRESS_REPEAT.fetch_add(0, Ordering::Relaxed);
        assert_eq!(after_total.saturating_sub(base_total), 1);
        assert_eq!(after_repeat.saturating_sub(base_repeat), 1);
    }

    #[tokio::test]
    async fn keypress_logging_and_counters() {
        let _log_guard = LOG_CAPTURE_GUARD.lock().await;
        let base_total = KEYPRESS_TOTAL.fetch_add(0, Ordering::Relaxed);
        let base_repeat = KEYPRESS_REPEAT.fetch_add(0, Ordering::Relaxed);

        let capture = LogCapture::default();
        let events_handle = capture.events.clone();
        let subscriber = Registry::default().with(capture.with_filter(LevelFilter::TRACE));
        let dispatch = tracing::Dispatch::new(subscriber);
        let _guard = tracing::dispatcher::set_default(&dispatch);

        let outputs = run_scenario(vec![CEvent::Key(CKeyEvent::new(
            CKeyCode::Char('l'),
            crossterm::event::KeyModifiers::NONE,
        ))])
        .await;

        assert!(matches!(
            outputs.as_slice(),
            [Event::Input(InputEvent::KeyPress(_))]
        ));

        let after_total = KEYPRESS_TOTAL.fetch_add(0, Ordering::Relaxed);
        let after_repeat = KEYPRESS_REPEAT.fetch_add(0, Ordering::Relaxed);
        assert_eq!(after_total.saturating_sub(base_total), 1);
        assert_eq!(after_repeat.saturating_sub(base_repeat), 0);

        let logs = events_handle.lock().unwrap();
        let keypress_log = logs
            .iter()
            .find(|entry| entry.target == "input.event")
            .unwrap_or_else(|| panic!("missing input.event log, captured: {logs:?}"));
        assert!(
            keypress_log
                .fields
                .iter()
                .any(|(k, v)| k == "kind" && v == "\"keypress\"")
        );
        assert!(
            keypress_log
                .fields
                .iter()
                .any(|(k, v)| k == "repeat" && v == "false")
        );
        assert!(
            keypress_log
                .fields
                .iter()
                .any(|(k, v)| k == "token_kind" && v == "\"char\"")
        );
    }

    #[tokio::test]
    async fn forwards_ctrl_c() {
        let outputs = run_scenario(vec![CEvent::Key(CKeyEvent::new(
            CKeyCode::Char('c'),
            crossterm::event::KeyModifiers::CONTROL,
        ))])
        .await;

        assert!(matches!(
            outputs.as_slice(),
            [Event::Input(InputEvent::CtrlC)]
        ));
    }

    #[tokio::test]
    async fn forwards_resize_event() {
        let outputs = run_scenario(vec![CEvent::Resize(120, 48)]).await;

        assert!(matches!(
            outputs.as_slice(),
            [Event::Input(InputEvent::Resize(120, 48))]
        ));
    }

    #[tokio::test]
    async fn handles_bracketed_paste_sequence() {
        let outputs = run_scenario(vec![
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Esc,
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('2'),
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('0'),
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('0'),
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('~'),
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('h'),
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('i'),
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Esc,
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('['),
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('2'),
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('0'),
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('1'),
                crossterm::event::KeyModifiers::NONE,
            )),
            CEvent::Key(CKeyEvent::new(
                CKeyCode::Char('~'),
                crossterm::event::KeyModifiers::NONE,
            )),
        ])
        .await;

        assert_eq!(outputs.len(), 3);
        assert!(matches!(outputs[0], Event::Input(InputEvent::PasteStart)));
        match &outputs[1] {
            Event::Input(InputEvent::PasteChunk(chunk)) => assert_eq!(chunk, "hi"),
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(matches!(outputs[2], Event::Input(InputEvent::PasteEnd)));
    }

    #[tokio::test]
    async fn paste_event_emits_single_chunk() {
        let base_sessions = PASTE_SESSIONS.fetch_add(0, std::sync::atomic::Ordering::Relaxed);
        let base_chunks = PASTE_CHUNKS.fetch_add(0, std::sync::atomic::Ordering::Relaxed);
        let base_bytes = PASTE_BYTES.fetch_add(0, std::sync::atomic::Ordering::Relaxed);

        let outputs = run_scenario(vec![CEvent::Paste("hello paste".to_string())]).await;

        assert_eq!(outputs.len(), 3);
        assert!(matches!(outputs[0], Event::Input(InputEvent::PasteStart)));
        match &outputs[1] {
            Event::Input(InputEvent::PasteChunk(chunk)) => assert_eq!(chunk, "hello paste"),
            other => panic!("unexpected event: {other:?}"),
        }
        assert!(matches!(outputs[2], Event::Input(InputEvent::PasteEnd)));

        let sessions = PASTE_SESSIONS.fetch_add(0, std::sync::atomic::Ordering::Relaxed);
        let chunks = PASTE_CHUNKS.fetch_add(0, std::sync::atomic::Ordering::Relaxed);
        let bytes = PASTE_BYTES.fetch_add(0, std::sync::atomic::Ordering::Relaxed);
        assert!(
            sessions - base_sessions >= 1,
            "paste sessions counter did not advance"
        );
        assert!(
            chunks - base_chunks >= 1,
            "paste chunks counter did not advance"
        );
        assert!(bytes - base_bytes >= "hello paste".len() as u64);
    }

    #[tokio::test]
    async fn bracketed_paste_large_payload_splits_chunks() {
        let payload_len = DEFAULT_PASTE_CAPACITY + 32;
        let payload: String = std::iter::repeat_n('a', payload_len).collect();

        let mut events = Vec::new();
        events.push(CEvent::Key(CKeyEvent::new(
            CKeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        )));
        for ch in ['[', '2', '0', '0', '~'] {
            events.push(CEvent::Key(CKeyEvent::new(
                CKeyCode::Char(ch),
                crossterm::event::KeyModifiers::NONE,
            )));
        }
        for ch in payload.chars() {
            events.push(CEvent::Key(CKeyEvent::new(
                CKeyCode::Char(ch),
                crossterm::event::KeyModifiers::NONE,
            )));
        }
        events.push(CEvent::Key(CKeyEvent::new(
            CKeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        )));
        for ch in ['[', '2', '0', '1', '~'] {
            events.push(CEvent::Key(CKeyEvent::new(
                CKeyCode::Char(ch),
                crossterm::event::KeyModifiers::NONE,
            )));
        }

        let outputs = run_scenario(events).await;
        assert_eq!(outputs.len(), 4, "expected start, two chunks, and end");
        assert!(matches!(outputs[0], Event::Input(InputEvent::PasteStart)));
        assert!(matches!(outputs[3], Event::Input(InputEvent::PasteEnd)));

        let mut chunks = Vec::new();
        for event in outputs.iter() {
            if let Event::Input(InputEvent::PasteChunk(chunk)) = event {
                chunks.push(chunk.clone());
            }
        }

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), DEFAULT_PASTE_CAPACITY);
        assert_eq!(chunks.concat(), payload);
    }

    #[tokio::test]
    async fn logs_startup_and_shutdown_reason_on_signal() {
        let _log_guard = LOG_CAPTURE_GUARD.lock().await;
        let capture = LogCapture::default();
        let events_handle = capture.events.clone();
        let subscriber = Registry::default().with(capture.with_filter(LevelFilter::TRACE));
        let dispatch = tracing::Dispatch::new(subscriber);
        let _guard = tracing::dispatcher::set_default(&dispatch);

        let base_start = ASYNC_INPUT_STARTS.fetch_add(0, Ordering::Relaxed);
        let base_signal = ASYNC_INPUT_STOP_SIGNAL.fetch_add(0, Ordering::Relaxed);

        let (tx, rx) = mpsc::channel(1);
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<io::Result<CEvent>>();
        let stream = UnboundedReceiverStream::new(event_rx);
        let (shutdown, listener) = ShutdownListener::new_pair();

        let notifier = shutdown.clone();
        let signal_task = tokio::spawn(async move {
            tokio::task::yield_now().await;
            notifier.signal();
        });

        let _keep_alive = event_tx;
        AsyncEventStreamTask::new(tx, stream, listener).run().await;
        signal_task.await.unwrap();
        drop(rx);

        let logged = events_handle.lock().unwrap();
        assert!(
            logged.iter().any(|entry| {
                entry.target == "input.thread"
                    && entry
                        .fields
                        .iter()
                        .any(|(k, v)| k == "message" && v == "async_input_task_started")
            }),
            "missing async_input_task_started log, captured events: {:?}",
            *logged
        );

        let stop_event = logged.iter().find(|entry| {
            entry.target == "input.thread"
                && entry
                    .fields
                    .iter()
                    .any(|(k, v)| k == "message" && v == "async_input_task_stopped")
        });
        let stop_event = stop_event.unwrap_or_else(|| {
            panic!(
                "missing async_input_task_stopped log, captured events: {:?}",
                *logged
            )
        });
        let reason_field = stop_event
            .fields
            .iter()
            .find(|(k, _)| k == "reason")
            .map(|(_, v)| v.trim_matches('"'))
            .unwrap_or_default();
        assert_eq!(reason_field, "shutdown_signal");

        let after_start = ASYNC_INPUT_STARTS.fetch_add(0, Ordering::Relaxed);
        let after_signal = ASYNC_INPUT_STOP_SIGNAL.fetch_add(0, Ordering::Relaxed);
        assert!(
            after_start > base_start,
            "async input starts counter did not advance"
        );
        assert!(
            after_signal > base_signal,
            "shutdown signal counter did not advance"
        );
    }

    #[tokio::test]
    async fn channel_closed_increments_telemetry() {
        let base_channel = ASYNC_INPUT_STOP_CHANNEL.fetch_add(0, Ordering::Relaxed);

        let (tx, rx) = mpsc::channel(1);
        drop(rx);

        let stream = tokio_stream::iter(vec![Ok(CEvent::Resize(10, 10))]);
        let (_shutdown, listener) = ShutdownListener::new_pair();

        AsyncEventStreamTask::new(tx, stream, listener).run().await;

        let after_channel = ASYNC_INPUT_STOP_CHANNEL.fetch_add(0, Ordering::Relaxed);
        assert!(
            after_channel > base_channel,
            "channel closed counter did not advance"
        );
    }

    #[tokio::test]
    async fn shutdown_signal_exits_immediately() {
        let (tx, mut rx) = mpsc::channel(1);
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<io::Result<CEvent>>();
        let stream = UnboundedReceiverStream::new(event_rx);
        let (shutdown, listener) = ShutdownListener::new_pair();

        let task = tokio::spawn(async move {
            let _keep_alive = event_tx;
            AsyncEventStreamTask::new(tx, stream, listener).run().await;
        });

        shutdown.signal();

        timeout(Duration::from_millis(50), task)
            .await
            .expect("shutdown should resolve promptly")
            .expect("task join failed");

        assert!(rx.recv().await.is_none());
    }

    async fn run_scenario(events: Vec<CEvent>) -> Vec<Event> {
        let (tx, mut rx) = mpsc::channel(64);
        let stream = tokio_stream::iter(events.into_iter().map(Ok));
        let (_shutdown, listener) = ShutdownListener::new_pair();
        AsyncEventStreamTask::new(tx, stream, listener).run().await;

        let mut outputs = Vec::new();
        while let Some(evt) = rx.recv().await {
            outputs.push(evt);
        }
        outputs
    }
}
