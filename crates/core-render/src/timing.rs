//! Render timing instrumentation (Refactor R2 Step 11).
//!
//! Captures duration of the last completed full render in nanoseconds. This is
//! intentionally minimal breadth-first telemetry; future phases may expand to
//! running averages or histograms.
use std::sync::atomic::{AtomicU64, Ordering};

static LAST_RENDER_NS: AtomicU64 = AtomicU64::new(0);

/// Record a render duration in nanoseconds.
pub fn record_last_render_ns(ns: u64) {
    LAST_RENDER_NS.store(ns, Ordering::Relaxed);
}

/// Fetch the last recorded render duration in nanoseconds.
pub fn last_render_ns() -> u64 {
    LAST_RENDER_NS.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn store_and_load_nonzero() {
        record_last_render_ns(1234);
        assert_eq!(last_render_ns(), 1234);
    }
}
