//! Terminal capability probing (Refactor R3 Step 10).
//!
//! Breadth-first placeholder that records a minimal set of booleans the
//! renderer / scheduler can consult when deciding whether to attempt
//! scroll-region based optimizations or fall back to full line clears.
//!
//! Design considerations:
//! * Must be cheap: detection runs once at startup.
//! * Cross-platform: for now we optimistically enable scroll region support
//!   on all platforms where crossterm is used; later phases may refine by
//!   emitting a probe sequence and measuring terminal response.
//! * Extensible: struct is non-exhaustive (private field) so additional
//!   capabilities can be added without breaking downstream code.
//!
//! Future extensions (Phase 4+):
//! * Distinguish between absolute & relative scroll support.
//! * Detect truecolor vs 256-color fallbacks.
//! * Query bracketed paste / focus events / kitty keyboard protocols.
//! * Terminal width change debounce timings.
//!
//! Testing approach: current test asserts the optimistic defaults. Platform
//! divergence logic (when added) will come with targeted tests per branch.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct TerminalCapabilities {
    pub supports_scroll_region: bool,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        // Phase 3 policy: assume scroll region support. This unblocks early
        // integration of scroll optimization code paths gated by this flag
        // without prematurely implementing round-trip probing.
        Self {
            supports_scroll_region: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_sets_scroll_region_true() {
        let caps = TerminalCapabilities::detect();
        assert!(caps.supports_scroll_region);
    }
}
