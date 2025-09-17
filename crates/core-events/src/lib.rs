//! Core event types and channel helpers for Oxidized.
//! Phase 0 scope: minimal input + control events.

use std::fmt;
use std::sync::atomic::AtomicU64;

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

/// Top-level event enum consumed by the central event loop.
#[derive(Debug, Clone)]
pub enum Event {
    Input(InputEvent),
    Command(CommandEvent),
    RenderRequested,
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum CommandEvent {
    Quit,
}

/// Normalized input events.
#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyEvent),
    Resize(u16, u16),
    CtrlC,
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
    #[test]
    fn key_event_display() {
        let k = KeyEvent {
            code: KeyCode::Char('x'),
            mods: KeyModifiers::CTRL,
        };
        let s = format!("{}", k);
        assert!(s.contains("Char"));
    }
}
