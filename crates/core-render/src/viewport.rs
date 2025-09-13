//! Viewport abstraction (Refactor R1 Step 9).
//!
//! Breadth-first stub: represents the visible vertical window into the buffer
//! without yet supporting scrolling or horizontal offsets. Future evolution:
//! - Scrolling logic (methods to scroll up/down keeping cursor visible)
//! - Horizontal offsets & horizontal scrolling
//! - Multiple viewports for split windows
//! - Damage-region intersection for diff rendering
//!
//! Current guarantees:
//! * `first_line` is the index of the topmost buffer line to render.
//! * `height` is the number of screen rows available for buffer text (excludes
//!   status line management left to caller).
//! * Provided helper `clamp_cursor_into_view` will adjust `first_line` to keep
//!   the cursor visible (no-op right now because we don't scroll yet; logic
//!   stubbed for future extension).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub first_line: usize,
    pub height: usize,
}

impl Viewport {
    pub fn new(first_line: usize, height: usize) -> Self {
        Self { first_line, height }
    }

    /// Adjust viewport to ensure `cursor_line` is visible. Breadth-first: we
    /// don't implement scrolling yet; placeholder for future logic.
    pub fn clamp_cursor_into_view(&mut self, _cursor_line: usize) {
        // Future: if cursor_line < first_line -> shift up; if >= first_line+height -> shift down.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_viewport() {
        let vp = Viewport::new(0, 10);
        assert_eq!(vp.first_line, 0);
        assert_eq!(vp.height, 10);
    }

    #[test]
    fn clamp_noop_for_now() {
        let mut vp = Viewport::new(5, 5);
        vp.clamp_cursor_into_view(20);
        assert_eq!(vp.first_line, 5, "no scrolling yet");
    }
}
