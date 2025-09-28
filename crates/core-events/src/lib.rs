//! Core event types and channel helpers for Oxidized.
//! Phase 0 scope: minimal input + control events.

use std::fmt;
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use tokio::sync::mpsc::Sender;
use tokio::task::JoinHandle;

// -------------------------------------------------------------------------------------------------
// Channel Policy (Phase 2 Step 16 – Activated)
// -------------------------------------------------------------------------------------------------
// The event loop now uses a bounded mpsc channel sized by `EVENT_CHANNEL_CAP` to provide memory
// safety and natural producer backpressure. Initial policy: the blocking input thread uses
// `blocking_send` which will park the thread until space is available rather than dropping events.
// Rationale: with a single producer (input) and single consumer (main loop) latency remains low and
// preserving motion / edit fidelity is preferred over lossy drop strategies. Future multi‑producer
// scenarios (timers, LSP, watchers) may introduce a priority control channel + selective drop of
// low‑value motion bursts. Telemetry counters record send failures (closed channel) and will later
// record explicit backpressure timings once multiple producers exist.
// -------------------------------------------------------------------------------------------------
pub const EVENT_CHANNEL_CAP: usize = 8192;

// -------------------------------------------------------------------------------------------------
// Telemetry (Phase 2 Step 16)
// -------------------------------------------------------------------------------------------------
// Simple atomic counters (no locking, fetch_add relaxed). These are intentionally minimal; a future
// metrics crate integration can export them via structured events. For now they can be inspected in
// unit tests or periodically logged.
// -------------------------------------------------------------------------------------------------
pub static CHANNEL_SEND_FAILURES: AtomicU64 = AtomicU64::new(0);
pub static CHANNEL_BLOCKING_SENDS: AtomicU64 = AtomicU64::new(0); // increments for each successful blocking_send (backpressure aware later)
// NGI Step 8: paste streaming telemetry
pub static PASTE_SESSIONS: AtomicU64 = AtomicU64::new(0); // number of PasteStart events
pub static PASTE_CHUNKS: AtomicU64 = AtomicU64::new(0); // number of PasteChunk events
pub static PASTE_BYTES: AtomicU64 = AtomicU64::new(0); // total bytes across all chunks
pub static KEYPRESS_TOTAL: AtomicU64 = AtomicU64::new(0); // total keypress events emitted
pub static KEYPRESS_REPEAT: AtomicU64 = AtomicU64::new(0); // keypress events flagged as repeat
// Async input task lifecycle telemetry (Refactor R4 Step 15)
pub static ASYNC_INPUT_STARTS: AtomicU64 = AtomicU64::new(0);
pub static ASYNC_INPUT_STOP_SIGNAL: AtomicU64 = AtomicU64::new(0);
pub static ASYNC_INPUT_STOP_CHANNEL: AtomicU64 = AtomicU64::new(0);
pub static ASYNC_INPUT_STOP_STREAM: AtomicU64 = AtomicU64::new(0);
pub static ASYNC_INPUT_STOP_ERROR: AtomicU64 = AtomicU64::new(0);

/// Top-level event enum consumed by the central event loop.
#[derive(Debug, Clone)]
pub enum Event {
    Input(InputEvent),
    Command(CommandEvent),
    RenderRequested,
    /// Periodic monotonic tick (Phase 4 Step 14) used to drive ephemeral expiry
    /// and future lightweight refresh tasks without busy polling.
    Tick,
    Shutdown,
}

// -------------------------------------------------------------------------------------------------
// Event Transform Hooks (no-op scaffolding)
// -------------------------------------------------------------------------------------------------
/// Optional hooks that can observe or transform events at the loop boundary.
///
/// Initial implementation is a no-op; consumers can provide their own impls in
/// higher layers. Kept minimal to avoid cross-crate coupling and to align with
/// breadth-first development. These hooks should not block.
pub trait EventHooks: Send + Sync + 'static {
    fn pre_handle(&self, _event: &Event) {}
    fn post_handle(&self, _event: &Event) {}
}

/// Default no-op hooks implementation.
pub struct NoopEventHooks;

impl EventHooks for NoopEventHooks {}

// -------------------------------------------------------------------------------------------------
// Async Event Sources (Refactor R4 Step 14)
// -------------------------------------------------------------------------------------------------
// Rationale: generalize the ad-hoc tick tokio::spawn task into a unified trait so future providers
// (LSP notifications, file watchers, plugin hosts, diagnostics) register uniformly. Each source is
// responsible for its own async task lifecycle; on channel send failure (consumer dropped) it must
// terminate promptly. Backpressure: bounded channel already provides flow control; higher-level
// prioritization (e.g. dropping low value motion bursts) can layer later without changing this API.
// Simplicity: minimal surface (spawn + name) to avoid premature abstraction; restart policies and
// dynamic enable/disable can be layered on top by keeping the registry handle.

/// Trait implemented by any async event producer. Implementors usually hold configuration and
/// spawn one background task that pushes `Event`s into the shared channel.
///
/// Design (Refactor R4 Step 14): minimal surface (name + spawn) to keep early
/// integration friction low. The `core-plugin` crate will later supply a
/// `PluginHost` whose discovered plugins can contribute additional sources (e.g.
/// LSP notifications, file watchers, diagnostics). Each source remains
/// independent and failure-isolated; higher-level supervision / restart policy
/// can wrap the registry without altering this contract.
pub trait AsyncEventSource: Send + 'static {
    /// Human-readable stable identifier (used for logging / diagnostics).
    fn name(&self) -> &'static str;
    /// Consume self and spawn the background task, returning a JoinHandle. Implementors should
    /// stop when `tx.send(..).await` returns Err (channel closed) or on their own internal stop
    /// condition. They should avoid busy loops by awaiting timers or external IO futures.
    fn spawn(self: Box<Self>, tx: Sender<Event>) -> JoinHandle<()>;
}

/// Registry of event sources. In this initial scaffold it stores boxed trait objects and can spawn
/// them all at startup. Later we can add dynamic add/remove and fine grained control.
pub struct EventSourceRegistry {
    sources: Vec<Box<dyn AsyncEventSource>>,
}

impl Default for EventSourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in monotonic tick source (replaces ad-hoc spawn in main.rs). Emits `Event::Tick` every
/// configured interval.
pub struct TickEventSource {
    interval: std::time::Duration,
}

impl TickEventSource {
    pub fn new(interval: std::time::Duration) -> Self {
        Self { interval }
    }
}

#[cfg(test)]
mod tests_async_sources {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use tokio::sync::mpsc;

    struct MockOnceSource {
        emitted: bool,
    }
    impl MockOnceSource {
        fn new() -> Self {
            Self { emitted: false }
        }
    }
    impl AsyncEventSource for MockOnceSource {
        fn name(&self) -> &'static str {
            "mock_once"
        }
        fn spawn(mut self: Box<Self>, tx: Sender<Event>) -> JoinHandle<()> {
            tokio::spawn(async move {
                if !self.emitted {
                    let _ = tx.send(Event::RenderRequested).await;
                    self.emitted = true;
                }
            })
        }
    }

    #[tokio::test]
    async fn registry_spawns_and_emits() {
        let (tx, mut rx) = mpsc::channel::<Event>(8);
        let mut reg = EventSourceRegistry::new();
        reg.register(MockOnceSource::new());
        reg.register(TickEventSource::new(std::time::Duration::from_millis(10)));
        let handles = reg.spawn_all(&tx);
        // Expect at least one event from each source quickly.
        let mut got_render = false;
        let mut got_tick = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_millis(100) && (!got_render || !got_tick)
        {
            if let Ok(Some(ev)) =
                tokio::time::timeout(std::time::Duration::from_millis(5), rx.recv()).await
            {
                match ev {
                    Event::RenderRequested => got_render = true,
                    Event::Tick => got_tick = true,
                    _ => {}
                }
            }
        }
        assert!(
            got_render,
            "expected mock source to produce a render request"
        );
        assert!(got_tick, "expected tick source to emit tick events");

        drop(tx);
        drop(rx);
        for handle in handles {
            let _ = tokio::time::timeout(Duration::from_millis(20), handle).await;
        }
    }

    struct MockCloseSource {
        flag: Arc<AtomicBool>,
    }

    impl MockCloseSource {
        fn new(flag: Arc<AtomicBool>) -> Self {
            Self { flag }
        }
    }

    impl AsyncEventSource for MockCloseSource {
        fn name(&self) -> &'static str {
            "mock_close"
        }

        fn spawn(self: Box<Self>, tx: Sender<Event>) -> JoinHandle<()> {
            let flag = self.flag;
            tokio::spawn(async move {
                tx.closed().await;
                flag.store(true, Ordering::SeqCst);
            })
        }
    }

    #[tokio::test]
    async fn registry_sources_exit_on_channel_drop() {
        let (tx, rx) = mpsc::channel::<Event>(8);
        let mut reg = EventSourceRegistry::new();
        let flag = Arc::new(AtomicBool::new(false));
        reg.register(MockCloseSource::new(flag.clone()));
        let handles = reg.spawn_all(&tx);

        drop(tx);
        drop(rx);

        for handle in handles {
            match tokio::time::timeout(Duration::from_millis(50), handle).await {
                Ok(join_res) => join_res.expect("source task should exit cleanly"),
                Err(_) => panic!("source task did not observe channel closure"),
            }
        }

        assert!(flag.load(Ordering::SeqCst));
    }
}

impl AsyncEventSource for TickEventSource {
    fn name(&self) -> &'static str {
        "tick"
    }
    fn spawn(self: Box<Self>, tx: Sender<Event>) -> JoinHandle<()> {
        let dur = self.interval;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(dur);
            loop {
                interval.tick().await;
                if tx.send(Event::Tick).await.is_err() {
                    break;
                }
            }
        })
    }
}

impl EventSourceRegistry {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }
    pub fn register<S: AsyncEventSource>(&mut self, src: S) {
        self.sources.push(Box::new(src));
    }
    /// Spawn all registered sources, returning their JoinHandles. Caller owns the handles (may
    /// choose to detach or await during shutdown sequence).
    /// Spawn all registered sources, returning their JoinHandles. The supplied `Sender`
    /// reference stays owned by the caller; each source receives its own clone so no
    /// additional strong references linger inside the registry once this call returns.
    ///
    /// Ordering guarantee: call this after constructing the primary runtime channel and
    /// before the event loop begins consuming events. During shutdown the caller should
    /// drop its final `Sender` clone before awaiting the returned handles so the sources
    /// observe the closed channel and exit cooperatively.
    pub fn spawn_all(&mut self, tx: &Sender<Event>) -> Vec<JoinHandle<()>> {
        // Take ownership so duplicate spawns are prevented if called twice.
        let mut out = Vec::with_capacity(self.sources.len());
        for src in self.sources.drain(..) {
            let name = src.name();
            tracing::info!(target: "runtime.events", source = name, "spawning event source");
            out.push(src.spawn(tx.clone()));
        }
        out
    }
}

#[derive(Debug, Clone)]
pub enum CommandEvent {
    Quit,
}

/// Normalized input events.
///
/// When the `ngi-input` feature is enabled this enum is expanded with additional
/// variants required by the Next-Gen Input (NGI) design. The legacy variants remain
/// unchanged so existing translation & dispatcher code compiles without modification
/// until the full Phase A–D migration completes.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Legacy key event (pre-NGI). Still emitted by the current blocking input thread.
    Key(KeyEvent),
    /// Terminal resize (columns, rows).
    Resize(u16, u16),
    /// Synthetic interrupt (Ctrl-C) surfaced distinctly for future job control.
    CtrlC,
    // --- NGI variants (feature gated) ---------------------------------------------------------
    /// Logical key press with richer token model, timestamp, and repeat flag.
    ///
    /// Invariants:
    /// * `KeyEventExt::timestamp` must be monotonic per input task (each event
    ///   carries the instant observed from the async task).
    /// * `KeyEventExt::repeat` is `true` only for auto-repeat events emitted by
    ///   the terminal (it **must not** be synthesized downstream).
    /// * The associated `KeyToken` never contains raw payloads that should be
    ///   redacted from logs; consumers log only discriminants or lengths.
    KeyPress(KeyEventExt),
    /// One or more extended grapheme clusters ready for insertion (already NFC normalized).
    TextCommit(String),
    /// Start of a bracketed paste sequence (size unknown until end). Mapping layer can choose
    /// to treat the entire paste as a single atomic insertion for undo grouping.
    PasteStart,
    /// A chunk within a bracketed paste (never logged verbatim per logging.md guidance; callers
    /// must only log length / size_bytes if instrumenting).
    PasteChunk(String),
    /// End of a bracketed paste sequence.
    PasteEnd,
    /// Mouse event (position + kind + modifiers). Initial scope: logging + potential selection
    /// experimentation; mapping layer may ignore until explicit mouse support phase.
    Mouse(MouseEvent),
    /// Focus gained (terminal window became active).
    FocusGained,
    /// Focus lost (terminal window deactivated). Future use: auto-blur modes, UI dimming.
    FocusLost,
    /// Raw uninterpreted bytes (escape sequences or unknown terminal reports) surfaced to allow
    /// incremental support without blocking the input thread.
    RawBytes(Vec<u8>),
    /// In-progress IME / composition preedit update. UI layers can surface the preedit string;
    /// final committed text will arrive as `TextCommit`.
    CompositionUpdate { preedit: String },
}

// -------------------------------------------------------------------------------------------------
// NGI Supporting Types
// -------------------------------------------------------------------------------------------------
/// Rich keypress metadata emitted by the async input task.
///
/// Fields:
/// * `token`: Logical key identity (character, named key, or chord).
/// * `repeat`: Whether this event was reported as an auto-repeat by the
///   terminal. Downstream consumers may use this to avoid resetting NGI timeout
///   state while still applying motions.
/// * `timestamp`: Instant captured when the input task observed the event.
///
/// Constructors ensure timestamps are monotonically increasing when called in
/// event order, but callers may also provide explicit instants (useful for
/// tests or deserialization paths).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyEventExt {
    pub token: KeyToken,
    pub repeat: bool,
    pub timestamp: Instant,
}

impl KeyEventExt {
    /// Create a `KeyEventExt` using the current instant and `repeat = false`.
    pub fn new(token: KeyToken) -> Self {
        Self::from_parts(token, false, Instant::now())
    }

    /// Create a `KeyEventExt` using the current instant and explicit repeat bit.
    pub fn with_repeat(token: KeyToken, repeat: bool) -> Self {
        Self::from_parts(token, repeat, Instant::now())
    }

    /// Create a `KeyEventExt` with caller supplied timestamp (primarily for tests).
    pub fn from_parts(token: KeyToken, repeat: bool, timestamp: Instant) -> Self {
        Self {
            token,
            repeat,
            timestamp,
        }
    }
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ModMask: u16 { const CTRL=1; const ALT=2; const SHIFT=4; const META=8; const SUPER=16; }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamedKey {
    Enter,
    Esc,
    Backspace,
    Tab,
    F(u8),
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
}

/// Canonical logical key tokens surfaced by NGI.
///
/// `KeyToken::Chord` wraps a base token plus modifier mask, ensuring consumers
/// can faithfully reconstruct combinations such as `<C-d>` without relying on
/// legacy translation shortcuts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyToken {
    Char(char),
    Named(NamedKey),
    Chord { base: Box<KeyToken>, mods: ModMask },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub column: u16,
    pub row: u16,
    pub mods: ModMask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseEventKind {
    Down(MouseButton),
    Up(MouseButton),
    Drag(MouseButton),
    ScrollUp,
    ScrollDown,
    Moved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// KeyCode enumerates normalized logical key representations consumed by higher layers.
/// (Historic note) Earlier phases briefly carried a dedicated `Colon` variant; Refactor R2
/// Step 8 removed it in favor of a normalization shim to ensure a single printable path.
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Tab,
    Up,
    Down,
    Left,
    Right,
}

/// Normalize a raw KeyCode that may have historically used dedicated printable variants
/// (Refactor R2 Step 8). After this step, callers should construct only standard forms
/// (e.g., ':' becomes `KeyCode::Char(':')`). Retained as a future extension point if
/// additional raw platform translations are introduced.
pub fn normalize_keycode(code: KeyCode) -> KeyCode {
    // Currently identity; future raw variants can map here.
    code
}

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct KeyModifiers: u8 {
        const CTRL = 0b0000_0001;
        const ALT  = 0b0000_0010;
        const SHIFT= 0b0000_0100;
    }
}

impl fmt::Display for KeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}{:?}", self.code, self.mods)
    }
}

/// Helper result type for channel creation (future phases may add bounded channels here).
pub type EventResult<T> = anyhow::Result<T>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    #[test]
    fn key_event_display() {
        let k = KeyEvent {
            code: KeyCode::Char('x'),
            mods: KeyModifiers::CTRL,
        };
        let s = format!("{}", k);
        assert!(s.contains("Char"));
    }

    #[test]
    fn key_event_ext_new_defaults() {
        let token = KeyToken::Char('a');
        let evt = KeyEventExt::new(token.clone());
        assert_eq!(evt.token, token);
        assert!(!evt.repeat, "new() must default repeat to false");
        assert!(evt.timestamp <= Instant::now());
    }

    #[test]
    fn key_event_ext_with_repeat_and_from_parts() {
        let token = KeyToken::Named(NamedKey::Enter);
        let ts = Instant::now();
        let evt = KeyEventExt::from_parts(token.clone(), true, ts);
        assert_eq!(evt.token, token);
        assert!(evt.repeat);
        assert_eq!(evt.timestamp, ts);

        let repeat_evt = KeyEventExt::with_repeat(token.clone(), false);
        assert_eq!(repeat_evt.token, token);
        assert!(!repeat_evt.repeat);
        assert!(repeat_evt.timestamp >= ts);
    }

    #[test]
    fn key_token_chord_round_trip() {
        let mods = ModMask::CTRL | ModMask::ALT;
        let base = KeyToken::Named(NamedKey::Down);
        let chord = KeyToken::Chord {
            base: Box::new(base.clone()),
            mods,
        };
        let evt = KeyEventExt::with_repeat(chord.clone(), true);
        match evt.token {
            KeyToken::Chord {
                base: boxed_base,
                mods: observed_mods,
            } => {
                assert_eq!(*boxed_base, base);
                assert_eq!(observed_mods, mods);
            }
            other => panic!("expected chord token, got {:?}", other),
        }
        assert!(evt.repeat);
    }
}
