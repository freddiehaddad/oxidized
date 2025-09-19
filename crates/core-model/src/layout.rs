//! Layout scaffolding (Refactor R3 Step 9).
//!
//! Breadth-first single-region layout abstraction introduced to decouple
//! rendering surface geometry from render logic and prepare for future
//! multi-view splits (horizontal/vertical, grid, stacked, tabbed). For now
//! a `Layout` always contains exactly one `LayoutRegion` representing the
//! full terminal viewport. The public API *looks* multi-region capable but
//! enforces single-region invariants at runtime (debug assertions) so that
//! later expansion does not require signature churn across crates.
//!
//! Design Tenets Applied:
//! * Modularity: Geometry lives in `core-model` with other high level model
//!   concepts (views) rather than inside the renderer crate.
//! * Evolution Over Legacy: We intentionally start minimal instead of
//!   prematurely modeling split containers. Future phases can extend the
//!   `Layout` struct with region trees or constraints; existing call sites
//!   will remain source compatible.
//! * Unicode & Rendering Correctness: Region coordinates are expressed in
//!   terminal cell units (`u16`) aligning with existing rendering APIs.
//! * Documentation: Invariants and forward roadmap captured inline.
//!
//! Invariants (current phase):
//! * `regions.len() == 1`.
//! * Region 0 has origin (0,0).
//! * Width/height are the terminal reported dimensions.
//! * Width/height may be 0 (degenerate) but never exceed `u16::MAX`.
//!
//! Forward Roadmap (future phases, not yet implemented):
//! * Multiple regions addressing independent `View` instances.
//! * Region z-order & per-region status lines.
//! * Dynamic recompute on split create/close.
//! * Constraint solver or simple tiling strategy for evenly distributing
//!   space with minimum size hints.
//! * Persistent layout serialization in config file.
//!
//! Testing Strategy:
//! * Current test asserts single() constructor sets invariants.
//! * Future tests will cover split operations & region mapping.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutRegion {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl LayoutRegion {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Layout {
    regions: Vec<LayoutRegion>,
}

impl Layout {
    /// Create a layout representing a single full-screen region.
    pub fn single(width: u16, height: u16) -> Self {
        Self {
            regions: vec![LayoutRegion::new(0, 0, width, height)],
        }
    }

    /// Return the primary (currently only) region.
    pub fn primary(&self) -> &LayoutRegion {
        // Debug assert current single-region invariant.
        debug_assert!(self.regions.len() == 1, "multi-region not yet enabled");
        &self.regions[0]
    }

    pub fn regions(&self) -> &[LayoutRegion] {
        &self.regions
    }

    /// Internal (future) helper to push a region. Unused now; retained as a
    /// placeholder illustrating likely extension point.
    #[allow(dead_code)]
    fn push_region(&mut self, region: LayoutRegion) {
        self.regions.push(region);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_layout_invariants() {
        let l = Layout::single(80, 24);
        assert_eq!(l.regions().len(), 1);
        let r = l.primary();
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 0);
        assert_eq!(r.width, 80);
        assert_eq!(r.height, 24);
    }
}
