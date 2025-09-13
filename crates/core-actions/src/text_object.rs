//! Text object scaffold (Refactor R4 Step 15).
//!
//! Breadth-first placeholder introducing the `TextObject` trait and a small
//! enumerated set of future Vim/Neovim-style text object kinds. No behavior is
//! integrated yet; operators and key translation do not reference these. This
//! establishes a stable trait surface so subsequent steps (paste / visual mode
//! groundwork, object-aware motions) can evolve without a disruptive refactor.
//!
//! Design notes:
//! * The trait is intentionally minimal: `name()` for diagnostics / logging and
//!   `resolve()` producing a `SelectionSpan` relative to the current cursor &
//!   editor state.
//! * Resolution today returns an empty characterwise span in all placeholder
//!   implementations to guarantee zero semantic change.
//! * Future expansions may add an `is_linewise()` helper or expose inclusive /
//!   exclusive semantics flags.
//!
//! Out of scope for this scaffold:
//! * Parsing of text object commands (e.g. `diw`, `ci"`).
//! * Inner vs. around delimiter resolution rules.
//! * Paragraph / sentence boundary detection heuristics.
//! * Integration with operator dispatcher.
//!
//! Subsequent work will introduce concrete implementors and translation paths.
//! The dispatcher and key translator intentionally omit references until the
//! initial operator + motion parity is fully preserved under selection model
//! integration (visual mode enablement).

use core_state::{EditorState, SelectionKind, SelectionSpan};
use core_text::Position;

/// Future text object kinds (placeholder). Variants are *not* constructed by
/// any public API yet. They indicate intended coverage and may be reordered or
/// renamed prior to first semantic integration without breaking changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjectKind {
    /// Inner word (excluding surrounding whitespace) – planned `iw`.
    WordInner,
    /// A word including contiguous surrounding whitespace – planned `aw`.
    WordA,
    /// Paragraph – planned `ip` / `ap` (blank-line delimited).
    Paragraph,
    /// Sentence – planned `is` / `as` (simple punctuation heuristic first pass).
    Sentence,
}

/// Core trait every text object implementation will satisfy. Implementations
/// MUST be cheap to construct; heavy precomputation should be deferred until
/// `resolve()` is invoked and may be cached externally later if needed.
pub trait TextObject: Send + Sync {
    /// Stable identifier used for logging / tracing (human readable, kebab-case preferred).
    fn name(&self) -> &'static str;
    /// Resolve the selection represented by this object relative to the
    /// current cursor position. Breadth-first stub returns an empty span.
    fn resolve(&self, state: &EditorState, cursor: Position) -> SelectionSpan;
}

impl<T: TextObject + ?Sized> TextObject for &T {
    fn name(&self) -> &'static str {
        (**self).name()
    }
    fn resolve(&self, state: &EditorState, cursor: Position) -> SelectionSpan {
        (**self).resolve(state, cursor)
    }
}

/// Scaffold helper resolving a requested `TextObjectKind` using a trivial
/// placeholder implementation so call sites can be introduced ahead of real
/// semantics. Returns an empty characterwise span at the cursor.
pub fn resolve_text_object(
    _state: &EditorState,
    cursor: Position,
    _kind: TextObjectKind,
) -> SelectionSpan {
    SelectionSpan::new(cursor, cursor, SelectionKind::Characterwise)
}

// ---------------- Tests (scaffold only) ----------------
#[cfg(test)]
mod tests {
    use super::*;
    use core_text::Buffer;

    struct DummyWordInner;
    impl TextObject for DummyWordInner {
        fn name(&self) -> &'static str {
            "word-inner"
        }
        fn resolve(&self, _state: &EditorState, cursor: Position) -> SelectionSpan {
            // Stub: empty span at cursor for breadth-first safety.
            SelectionSpan::new(cursor, cursor, SelectionKind::Characterwise)
        }
    }

    #[test]
    fn dummy_impl_name_and_empty_selection() {
        let buf = Buffer::from_str("dummy", "alpha beta\n").unwrap();
        let state = EditorState::new(buf);
        let cursor = Position::origin();
        let obj = DummyWordInner;
        assert_eq!(obj.name(), "word-inner");
        let sel = obj.resolve(&state, cursor);
        assert_eq!(sel.start, sel.end); // empty
        assert!(sel.is_empty());
    }

    #[test]
    fn resolve_text_object_stub_empty() {
        let buf = Buffer::from_str("dummy", "alpha\n").unwrap();
        let state = EditorState::new(buf);
        let cursor = Position::origin();
        let sel = resolve_text_object(&state, cursor, TextObjectKind::WordInner);
        assert!(sel.is_empty());
    }
}
