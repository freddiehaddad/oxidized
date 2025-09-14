//! Core event types and channel helpers for Oxidized.
//! Phase 0 scope: minimal input + control events.

use std::fmt;

// -------------------------------------------------------------------------------------------------
// Channel Policy (Refactor R1 Step 10)
// -------------------------------------------------------------------------------------------------
// We intentionally start Phase 1 with an *unbounded* mpsc channel (`tokio::sync::mpsc::unbounded_channel`)
// feeding the single consumer event loop. This keeps early refactors simple (no backpressure /
// error handling churn) while we only have a single producer (the blocking input thread).
//
// Future phases (first moment we introduce a second asynchronous producer: config watcher, timer,
// LSP client, plugin host, etc.) will migrate to a *bounded* channel to provide memory safety and
// natural backpressure. That migration must be a single, tightly‑scoped change. To enable that we:
//   1. Define a central capacity constant here (`EVENT_CHANNEL_CAP`).
//   2. Reference it (in comments + TODO) at the creation site (currently `ox-bin/src/main.rs`).
//   3. Document the drop / overflow policy ahead of time.
//
// Proposed bounded semantics (documented now, implemented later):
//   * Capacity: power‑of‑two sized ring large enough for bursts but finite (8192 chosen: 2^13).
//   * On send when full: drop *oldest* non-critical events (likely motion spam) OR block the
//     producer task briefly using a bounded async channel with `send().await` (decision deferred
//     until concrete multi-producer workload exists). Critical events (Shutdown, Quit) will be
//     prioritized by sending through a small separate control channel if necessary.
//   * Telemetry: a counter increments each time backpressure engages or an event is dropped.
//
// Rationale for 8192: Empirically generous for human typing / motion bursts (thousands of keys) yet
// trivially bounded in memory (~ a few hundred KB given small enum size). Power‑of‑two simplifies
// potential future custom ring buffer optimizations.
//
// IMPORTANT: Until the migration occurs the constant is *unused*; keeping it here ensures no code
// search is needed later and prevents scattering magic numbers. The refactor design doc marks this
// step complete once the constant + commentary exist and `main.rs` references the planned swap.
// -------------------------------------------------------------------------------------------------
#[allow(dead_code)]
pub const EVENT_CHANNEL_CAP: usize = 8192; // Future bounded channel capacity (see comment block).

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
/// NOTE (Phase 1 breadth-first): `Colon` exists as a distinct variant because the initial
/// input thread mapping introduced it to signal potential command mode activation. A regression
/// occurred when translator logic only matched `Char(':')`. We now handle both forms.
/// Future consolidation: likely remove `Colon` and emit `Char(':')` exclusively, or add a
/// normalization shim mapping special printable variants into `Char` to guarantee a single
/// printable pipeline. Tracking note lives in `design/phase-1-tasks.md` (Task 7 postmortem).
pub enum KeyCode {
    Char(char),
    Enter,
    Esc,
    Colon,
    Backspace,
    Tab,
    Up,
    Down,
    Left,
    Right,
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
