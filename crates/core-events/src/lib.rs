//! Core event types and channel helpers for Oxidized.
//! Phase 0 scope: minimal input + control events.

use std::fmt;

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
