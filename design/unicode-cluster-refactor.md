# Unicode Cluster Refactor (Structural Fix)

## 1. Objective

Implement a structural, first-class representation of full grapheme clusters in the rendering pipeline so that every render path (full, partial, cursor-only, scroll, trimmed diff) emits and reasons about complete clusters (including variation selectors, combining marks, ZWJ sequences, skin tone modifiers). This replaces the prior implicit "first scalar + padding" cell model and eliminates Unicode correctness gaps on cold full renders.

## 2. Motivation

Previous approach stored only the leading Unicode scalar of each grapheme cluster inside a `Cell`, relying on writer logic to approximate width and emit padding. This caused:

- Loss of variation selectors & combining marks on initial full-frame renders.
- Inability to reverse-video entire cluster content uniformly without re-fetching raw lines.
- Fragile future extension path for styling/syntax (would require reworking cell model anyway).

A structural refactor now prevents repeated churn and ensures correctness before layering styling, syntax highlighting, or semantic tokens.

## 3. Scope

In Scope:

- Redefine `Cell` to hold: full cluster string, visual width (>=1 for leaders, 0 for continuation cells), and flags.
- Update `Frame` construction for content, cursor overlay, and status line.
- Rewrite full-frame emission to iterate only leading cells, printing cluster once.
- Harmonize partial paths (may continue to stream clusters directly; they remain correct). Ensure no inconsistency if future logic inspects `Frame` after a partial.
- Add comprehensive Unicode correctness tests.

Out of Scope (Deferred):

- Width caching and performance micro-optimizations.
- Styling spans / syntax tokens (future: style IDs or attribute layers referencing clusters).
- Ligature shaping beyond Unicode segmentation already handled by `unicode-segmentation`.

## 4. Data Model

```text
Cell (leader): { cluster: String, width: u8 (>=1), flags }
Cell (continuation): { cluster: "" (empty), width: 0, flags }
Frame: { width, height, cells: Vec<Cell> } length == width * height
```

Invariants:

- A leader cell begins a cluster spanning `width` columns (1..=MaxWidthAssumed[2 or more]).
- Continuation cells immediately follow leader horizontally; they never print `cluster` content.
- All continuation cells may inherit flags (e.g., reverse) but emission logic derives styling from leader to avoid duplication.

Helper Methods:

- `Cell::is_leader()` returns `width > 0`.
- `Cell::cluster()` returns `&str` for leaders else "".
- `Frame::set_cluster(x,y,&str,width,flags)` populates leader + continuation cells.

## 5. Algorithms

### Frame Construction

1. Determine cluster width (>=1).
2. Place leader at (x,y) with full cluster string & width.
3. For dx in 1..width place continuation cells (empty cluster, width=0).
4. Advance x by width.

### Cursor Overlay

- Identify cursor line & cluster boundaries.
- Set flags (REVERSE|CURSOR) on leader + continuation cells inside cursor span; do NOT alter stored cluster text.

### Full-Frame Emission

For each row:

- Move cursor to (0,y).
- Scan cells left→right:
  - If cell.is_leader():
    - If reverse flag set: emit ESC[7m + cluster + ESC[0m.
    - Else emit cluster.
    - Skip over `width-1` continuation cells.
  - Else (continuation): skip (already emitted by leader).

### Partial Paths

Remain cluster-streaming directly from buffer lines (existing logic). No change required for correctness. Optionally, a future consolidation can reuse frame logic for unified metrics.

## 6. Hashing & Diff

Line hashing currently works over raw line slices. No change needed. If any future logic inspects `Frame` for diff, iterate only leader cells to reconstruct text.

## 7. Tests

New test module (e.g., `core-render/tests/unicode_clusters.rs`):

- Variation Selector: `"⚙️X"` contains VS16 bytes; appears intact after initial full render.
- Combining Mark: `"e\u{0301}"` preserved.
- ZWJ Family Emoji: `"👨‍👩‍👧‍👦Z"` appears once; contains all ZWJ bytes.
- Skin Tone Modifier: `"👍🏽!"` preserved.
- Wide CJK + Combining Mix: cluster count == number of leader cells; emission alignment: following ASCII starts at expected column.
- Cursor Overlay: reverse-video encloses entire multi-scalar sequence (variation selector + base) with single ESC[7m .. ESC[0m pair.

Implementation Support: add helper `Frame::line_clusters(y)` returning Vec<&str> of leader clusters for assertions.

## 8. Performance Considerations

Memory: Additional per-leader `String` allocations; acceptable at current scale. Future optimization: arena or slice referencing original buffer line; width caching to avoid repeated width calls. Deferred until profiling indicates need.

Emission Complexity: Slight skip logic; amortized O(clusters) vs prior O(cells). Equivalent or faster for multi-column clusters.

## 9. Migration Strategy

- Introduce new Cell struct & helpers.
- Update module docs (remove “single-scalar temporary” text).
- Refactor builders & emission in one commit (compiles, tests still pass except new tests not added yet).
- Second commit adds tests.

## 10. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Continuation flag misuse causes double print | Emission explicitly checks `is_leader()` only. |
| Cursor overlay mismatch wide clusters | Apply flags across leader + continuation cells; emission sees leader flag only. |
| Tests referencing old `Cell.ch` break | Provide migration helper or adapt tests to new API. |
| Performance regression | Monitor test suite; add simple benchmark later if needed. |

## 11. Acceptance Criteria

- Initial full render shows all complex clusters intact (manual & automated tests).
- No duplication or truncation of multi-scalar clusters across any path.
- Existing non-Unicode tests remain green.
- Design doc + module docs updated; commit message references this design.

## 12. Follow-Up (Deferred Work Log)

- Style spans / syntax highlighting overlay referencing cluster indices.
- Grapheme width caching layer.
- Optional: unify partial paths to leverage Frame to reduce divergence.

## 13. Status

Planned (implementation in progress).

## 14. Implementation Steps / Progress Checklist

Each commit corresponds to one completed step (breadth-first, always building). This section will be updated and re-committed as steps complete.

- [x] Step 1 – Design plan authored (`unicode-cluster-refactor.md`) and agreed.
- [x] Step 2 – Data structure refactor: introduce cluster-aware `Cell` + `Frame::set_cluster` (remove legacy single-scalar setters).
- [x] Step 3 – Frame builders updated (`build_content_frame`, cursor overlay, status line) to populate leader + continuation cells.
- [x] Step 4 – Full-frame emission rewrite (`render_via_writer`) to emit clusters once, skipping continuation cells.
- [x] Step 5 – Partial path consistency sweep (helpers updated: partial overlay emits full cluster; wide cluster padding removed for parity).
- [x] Step 6 – Module & crate docs updated (remove obsolete notes; document invariants & future styling hooks).
- [x] Step 7 – Test helpers added (`Frame::line_clusters`, maybe `plain_text_line`).
- [x] Step 8 – Unicode cluster correctness test suite (`unicode_clusters.rs`).
- [x] Step 9 – Formatting & lint pass (`cargo fmt`, `clippy -D warnings`).
- [x] Step 10 – Full workspace test run (including new tests) green.
- [x] Step 11 – Commit: structures (refactor(render): cluster-aware cell model — unicode refactor step 1).
- [x] Step 12 – Commit: builders & emission (refactor(render): build+emit full clusters — unicode refactor step 2).
- [x] Step 13 – Commit: docs & helpers (docs(render): module invariants & helpers — unicode refactor step 3).
- [x] Step 14 – Commit: tests (test(render): unicode cluster correctness suite — unicode refactor step 4).
- [x] Step 15 – (Optional) Consolidation or squash per maintainer preference.

### Consolidation Note

All implementation steps (1–15) were ultimately consolidated into a **single squashed commit** to preserve atomic introduction of the cluster-aware model and its tests. Individual planned commit boundaries are documented above for historical rationale and future archaeology. Date of consolidation: 2025-09-20.

---
