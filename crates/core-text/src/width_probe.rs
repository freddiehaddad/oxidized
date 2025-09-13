//! Runtime terminal width probe scaffold (Refactor R4 Step 4.4).
//!
//! Feature gated behind `term-probe` so normal builds pay zero cost.
//! Currently this is a no-op placeholder returning no overrides.
//! Future implementation plan (documented here so the commit captures intent):
//! 1. Emit CSI 6n to request cursor position, establishing round-trip timing.
//! 2. Print a curated set of candidate grapheme clusters (flags, ZWJ family,
//!    keycap, skin-tone sequences) bracketed by known ASCII markers and query
//!    resulting cursor column via additional CSI 6n.
//! 3. Derive observed terminal width differences vs static `egc_width` and
//!    populate a runtime override map.
//! 4. Cache by terminal signature (TERM + COLORTERM + optional OSC 0 title hash).
//!
//! This scaffold intentionally keeps API surface minimal: a single query
//! function returning an optional width override for an EGC. The width module
//! will check this before falling back to static classification logic.

/// Attempt to look up a runtime discovered width override for the given EGC.
///
/// Safety & concurrency: placeholder implementation uses a single unsafe
/// mutable static behind the feature flag; future iteration will replace this
/// with a lock-free once cell or immutable map after initialization.
pub fn runtime_override_width(_egc: &str) -> Option<u16> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_override_none() {
        // With feature disabled in CI/test matrix, always returns None.
        assert_eq!(runtime_override_width("😀"), None);
    }
}
