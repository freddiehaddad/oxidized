use core_actions::{Action, MotionKind, NgiTranslator, OperatorKind, PendingState};
use core_config::Config;
use core_events::{KeyEventExt, KeyToken, NamedKey};
use core_state::Mode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::dispatcher::{Dispatch, with_default};
use tracing::subscriber::Interest;
use tracing::{Metadata, Subscriber};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::Registry;

fn mk_key(token: KeyToken, repeat: bool, base: Instant, offset_ms: u64) -> KeyEventExt {
    KeyEventExt::from_parts(token, repeat, base + Duration::from_millis(offset_ms))
}

#[derive(Clone, Default)]
struct TargetCapture {
    events: Arc<Mutex<Vec<String>>>,
}

impl TargetCapture {
    fn targets(&self) -> Arc<Mutex<Vec<String>>> {
        self.events.clone()
    }
}

impl<S> Layer<S> for TargetCapture
where
    S: Subscriber,
{
    fn register_callsite(&self, _metadata: &'static Metadata<'static>) -> Interest {
        Interest::always()
    }

    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        self.events
            .lock()
            .unwrap()
            .push(event.metadata().target().to_string());
    }
}

#[test]
fn ingest_count_and_operator_sequence_emits_apply_operator() {
    let mut translator = NgiTranslator::new();
    let mut cfg = Config::default();
    cfg.file.input.timeout = true;
    cfg.file.input.timeoutlen = 750;

    let base = Instant::now();
    let first = mk_key(KeyToken::Char('2'), false, base, 0);
    let after_digit = translator.ingest_keypress(Mode::Normal, "", &first, &cfg);
    assert!(after_digit.action.is_none());

    let op = mk_key(KeyToken::Char('d'), false, base, 5);
    let after_operator = translator.ingest_keypress(Mode::Normal, "", &op, &cfg);
    assert!(after_operator.action.is_none());

    let motion = mk_key(KeyToken::Char('w'), false, base, 10);
    let resolved = translator.ingest_keypress(Mode::Normal, "", &motion, &cfg);
    match resolved.action {
        Some(Action::ApplyOperator {
            op,
            motion,
            count,
            register,
        }) => {
            assert_eq!(op, OperatorKind::Delete);
            assert_eq!(motion, MotionKind::WordForward);
            assert_eq!(count, 2);
            assert!(register.is_none());
        }
        other => panic!("unexpected action: {other:?}"),
    }
    assert!(matches!(resolved.pending_state, PendingState::Idle));
    assert_eq!(resolved.timeout_deadline, None);
}

#[test]
fn ingest_register_prefix_propagates_to_actions() {
    let mut translator = NgiTranslator::new();
    let cfg = Config::default();

    let base = Instant::now();
    let mark = mk_key(KeyToken::Char('"'), false, base, 0);
    let prefix = translator.ingest_keypress(Mode::Normal, "", &mark, &cfg);
    assert!(prefix.action.is_none());
    match prefix.pending_state {
        PendingState::AwaitingMore { buffered_len } => assert!(buffered_len <= 1),
        PendingState::Idle => {}
    }

    let register = mk_key(KeyToken::Char('a'), false, base, 2);
    let after_register = translator.ingest_keypress(Mode::Normal, "", &register, &cfg);
    assert!(after_register.action.is_none());
    assert!(matches!(
        after_register.pending_state,
        PendingState::Idle | PendingState::AwaitingMore { .. }
    ));

    let paste = mk_key(KeyToken::Char('p'), false, base, 4);
    let resolved = translator.ingest_keypress(Mode::Normal, "", &paste, &cfg);
    match resolved.action {
        Some(Action::PasteAfter { count, register }) => {
            assert_eq!(count, 1);
            assert_eq!(register, Some('a'));
        }
        other => panic!("unexpected action: {:?}", other),
    }
    assert!(matches!(resolved.pending_state, PendingState::Idle));
    assert_eq!(resolved.timeout_deadline, None);
}

#[test]
fn ingest_timeout_flush_emits_literal_with_deadline() {
    let mut translator = NgiTranslator::new();
    let mut cfg = Config::default();
    cfg.file.input.timeout = true;
    cfg.file.input.timeoutlen = 1200;

    let base = Instant::now();
    let pending = mk_key(KeyToken::Char('z'), false, base, 0);
    let resolution = translator.ingest_keypress(Mode::Normal, "", &pending, &cfg);
    assert!(resolution.action.is_none());
    match resolution.pending_state {
        PendingState::AwaitingMore { buffered_len } => assert_eq!(buffered_len, 1),
        other => panic!(
            "unexpected pending state after pending literal: {:?}",
            other
        ),
    }
    let expected_deadline =
        pending.timestamp + Duration::from_millis(cfg.file.input.timeoutlen as u64);
    assert_eq!(resolution.timeout_deadline, Some(expected_deadline));

    let flush_at = expected_deadline + Duration::from_millis(250);
    let flushed = translator
        .flush_pending_literal(&cfg, flush_at)
        .expect("flush should emit resolution");
    assert!(matches!(flushed.action, Some(Action::CommandChar('z'))));
    assert!(matches!(flushed.pending_state, PendingState::Idle));
    assert_eq!(flushed.timeout_deadline, None);
    assert!(translator.flush_pending_literal(&cfg, flush_at).is_none());
}

#[test]
fn ingest_keypress_emits_actions_translate_target() {
    let capture = TargetCapture::default();
    let targets = capture.targets();
    let subscriber = Registry::default().with(capture.with_filter(LevelFilter::TRACE));
    let dispatch = Dispatch::new(subscriber);

    with_default(&dispatch, || {
        let mut translator = NgiTranslator::new();
        let cfg = Config::default();
        let key = KeyEventExt::new(KeyToken::Named(NamedKey::Right));
        let _ = translator.ingest_keypress(Mode::Normal, "", &key, &cfg);
    });

    let recorded = targets.lock().unwrap();
    println!("captured targets: {:?}", *recorded);
    assert!(recorded.iter().any(|target| target == "actions.translate"));
}
