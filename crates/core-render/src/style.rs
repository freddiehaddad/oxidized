//! Style layer scaffold (Refactor R4 / Step 8).
//!
//! Breadth-first goal: introduce a minimal abstraction for styling separate
//! from `CellFlags` so later phases (syntax highlighting, multi-span cursor or
//! selection overlays, diagnostics) can compose style spans without rewriting
//! emission logic. At this step only the software cursor is represented.
//!
//! Design invariants:
//! * A `StyleSpan` never splits a grapheme cluster; callers must compute
//!   visual columns using the authoritative width engine before constructing
//!   spans.
//! * Spans are line-local (identified by `line`). Horizontal ranges use
//!   half-open `[start_col, end_col)` semantics in visual columns.
//! * Overlap semantics are undefined for now (later phases will reconcile by
//!   z-order or layering rules). Step 8 guarantees at most one span (cursor).
//! * No allocation churn: a single `StyleLayer` is reused per frame via
//!   `clear()`; later we may pool or smallvec optimize if profiling warrants.
//!
//! Future extensions (documented up front to avoid ad hoc growth):
//! * Syntax(u16) span classes mapped to theme index.
//! * Selection / Visual mode multi-spans.
//! * Overlay / diagnostics categories.
//! * Per-span attribute bitflags (bold, italic, underline) if needed.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StyleAttr {
    InvertCursor,
    Syntax(u16),
    Selection,
    Overlay,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleSpan {
    pub line: usize,
    pub start_col: u16, // inclusive
    pub end_col: u16,   // exclusive
    pub attr: StyleAttr,
}

impl StyleSpan {
    pub fn width(&self) -> u16 {
        self.end_col.saturating_sub(self.start_col)
    }
}

#[derive(Default, Debug)]
pub struct StyleLayer {
    pub spans: Vec<StyleSpan>,
}

impl StyleLayer {
    pub fn new() -> Self {
        Self { spans: Vec::new() }
    }
    pub fn clear(&mut self) {
        self.spans.clear();
    }
    pub fn push(&mut self, span: StyleSpan) {
        self.spans.push(span);
    }
    pub fn cursor_span(&self) -> Option<&StyleSpan> {
        self.spans
            .iter()
            .find(|s| matches!(s.attr, StyleAttr::InvertCursor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_span_basic() {
        let mut layer = StyleLayer::new();
        layer.push(StyleSpan {
            line: 0,
            start_col: 1,
            end_col: 3,
            attr: StyleAttr::InvertCursor,
        });
        let c = layer.cursor_span().expect("cursor span");
        assert_eq!(c.start_col, 1);
        assert_eq!(c.end_col, 3);
        assert_eq!(c.width(), 2);
    }
}
