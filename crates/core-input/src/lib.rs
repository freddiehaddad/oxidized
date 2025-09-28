//! Async input service helpers shared across the runtime.

mod async_service;
pub use async_service::AsyncInputShutdown;

use async_service::spawn_async_event_task;

use core_events::{Event, InputEvent, KeyCode, KeyEvent, KeyModifiers, normalize_keycode};
use crossterm::event::KeyModifiers as CMods;
use tokio::task::JoinHandle;

#[inline]
pub(crate) fn log_paste_chunk_flush(chunk: &str) {
    tracing::trace!(target: "input.paste", chunk_len = chunk.len(), "chunk_flush");
}

#[inline]
pub(crate) fn build_key_event(code: KeyCode, mods: KeyModifiers) -> Event {
    Event::Input(InputEvent::Key(KeyEvent {
        code: normalize_keycode(code),
        mods,
    }))
}

/// Spawn the async input service backed by `crossterm::EventStream`.
///
/// Returns the `JoinHandle` for the background task alongside a shutdown handle
/// that can be used to request immediate termination.
pub fn spawn_async_input(
    sender: tokio::sync::mpsc::Sender<Event>,
) -> (JoinHandle<()>, AsyncInputShutdown) {
    spawn_async_event_task(sender)
}

pub(crate) fn map_mods(m: CMods) -> KeyModifiers {
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
