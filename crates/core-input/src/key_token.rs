use core_events::{KeyToken, ModMask, NamedKey};
use crossterm::event::{
    KeyCode as CKeyCode, KeyEvent as CKeyEvent, KeyEventKind as CKeyEventKind,
    KeyModifiers as CKeyModifiers,
};

/// Result of translating a terminal key event into NGI token components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeyPressParts {
    pub token: KeyToken,
    pub mods: ModMask,
    pub repeat: bool,
}

/// Map a crossterm key event into a NGI token + modifiers.
///
/// Returns `None` for key codes we do not yet support (e.g. media keys).
pub(crate) fn map_key_event(event: &CKeyEvent) -> Option<KeyPressParts> {
    let token = map_key_token(&event.code)?;
    let mods = map_mod_mask(event.modifiers);
    let repeat = matches!(event.kind, CKeyEventKind::Repeat);
    Some(KeyPressParts {
        token,
        mods,
        repeat,
    })
}

/// Convert a crossterm key code into a NGI key token.
pub(crate) fn map_key_token(code: &CKeyCode) -> Option<KeyToken> {
    let token = match code {
        CKeyCode::Char(c) => KeyToken::Char(*c),
        CKeyCode::Enter => KeyToken::Named(NamedKey::Enter),
        CKeyCode::Esc => KeyToken::Named(NamedKey::Esc),
        CKeyCode::Backspace => KeyToken::Named(NamedKey::Backspace),
        CKeyCode::Tab | CKeyCode::BackTab => KeyToken::Named(NamedKey::Tab),
        CKeyCode::Up => KeyToken::Named(NamedKey::Up),
        CKeyCode::Down => KeyToken::Named(NamedKey::Down),
        CKeyCode::Left => KeyToken::Named(NamedKey::Left),
        CKeyCode::Right => KeyToken::Named(NamedKey::Right),
        CKeyCode::Home => KeyToken::Named(NamedKey::Home),
        CKeyCode::End => KeyToken::Named(NamedKey::End),
        CKeyCode::PageUp => KeyToken::Named(NamedKey::PageUp),
        CKeyCode::PageDown => KeyToken::Named(NamedKey::PageDown),
        CKeyCode::Insert => KeyToken::Named(NamedKey::Insert),
        CKeyCode::Delete => KeyToken::Named(NamedKey::Delete),
        CKeyCode::F(n) => {
            let ordinal = u16::from(*n);
            let clamped = if ordinal > u16::from(u8::MAX) {
                u8::MAX
            } else {
                ordinal as u8
            };
            KeyToken::Named(NamedKey::F(clamped))
        }
        CKeyCode::Null
        | CKeyCode::CapsLock
        | CKeyCode::ScrollLock
        | CKeyCode::NumLock
        | CKeyCode::PrintScreen
        | CKeyCode::Pause
        | CKeyCode::Menu
        | CKeyCode::KeypadBegin
        | CKeyCode::Media(_)
        | CKeyCode::Modifier(_) => return None,
    };
    Some(token)
}

/// Convert crossterm modifier flags into the NGI `ModMask` bits.
pub(crate) fn map_mod_mask(mods: CKeyModifiers) -> ModMask {
    let mut out = ModMask::empty();
    if mods.contains(CKeyModifiers::CONTROL) {
        out |= ModMask::CTRL;
    }
    if mods.contains(CKeyModifiers::ALT) {
        out |= ModMask::ALT;
    }
    if mods.contains(CKeyModifiers::SHIFT) {
        out |= ModMask::SHIFT;
    }
    if mods.contains(CKeyModifiers::SUPER) {
        out |= ModMask::SUPER;
    }
    if mods.contains(CKeyModifiers::META) {
        out |= ModMask::META;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState as CKeyEventState;

    fn key_event(code: CKeyCode, modifiers: CKeyModifiers, kind: CKeyEventKind) -> CKeyEvent {
        CKeyEvent {
            code,
            modifiers,
            kind,
            state: CKeyEventState::empty(),
        }
    }

    #[test]
    fn maps_basic_char() {
        let ev = key_event(
            CKeyCode::Char('a'),
            CKeyModifiers::NONE,
            CKeyEventKind::Press,
        );
        let parts = map_key_event(&ev).expect("char should map");
        assert!(matches!(parts.token, KeyToken::Char('a')));
        assert!(parts.mods.is_empty());
        assert!(!parts.repeat);
    }

    #[test]
    fn maps_named_key() {
        let ev = key_event(CKeyCode::Enter, CKeyModifiers::NONE, CKeyEventKind::Press);
        let parts = map_key_event(&ev).expect("enter should map");
        assert!(matches!(parts.token, KeyToken::Named(NamedKey::Enter)));
        assert!(parts.mods.is_empty());
    }

    #[test]
    fn maps_function_key() {
        let ev = key_event(CKeyCode::F(5), CKeyModifiers::NONE, CKeyEventKind::Press);
        let parts = map_key_event(&ev).expect("F5 should map");
        assert!(matches!(parts.token, KeyToken::Named(NamedKey::F(5))));
    }

    #[test]
    fn maps_modifiers_into_chord_mask() {
        let ev = key_event(
            CKeyCode::Char('d'),
            CKeyModifiers::CONTROL | CKeyModifiers::SHIFT,
            CKeyEventKind::Press,
        );
        let parts = map_key_event(&ev).expect("ctrl-shift-d should map");
        assert!(matches!(parts.token, KeyToken::Char('d')));
        assert!(parts.mods.contains(ModMask::CTRL));
        assert!(parts.mods.contains(ModMask::SHIFT));
    }

    #[test]
    fn detects_repeat_kind() {
        let ev = key_event(
            CKeyCode::Char('j'),
            CKeyModifiers::NONE,
            CKeyEventKind::Repeat,
        );
        let parts = map_key_event(&ev).expect("repeat should map");
        assert!(parts.repeat, "repeat flag should be true");
    }

    #[test]
    fn unsupported_keys_return_none() {
        let ev = key_event(
            CKeyCode::CapsLock,
            CKeyModifiers::NONE,
            CKeyEventKind::Press,
        );
        assert!(map_key_event(&ev).is_none());
    }
}
