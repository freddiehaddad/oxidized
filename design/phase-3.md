# Phase 3: Partial Rendering MVP & Multi-View Scaffolding

## 1. Objective

Activate a minimal, correct partial rendering pipeline that converts existing semantic render deltas into selective terminal updates (lines + cursor) while retaining safe fallbacks (full redraw for scroll, resize, or cache invalidation). Introduce always-on multi-view scaffolding (single active view) to prepare for future splits without feature flags. Instrument performance metrics (line hashes, repaint counts, scroll fallbacks) to guide later optimization phases.

## 2. Scope

In Scope:

- Dirty line tracking external to `RenderDelta` (no enum enrichment yet).
- Line hash snapshot cache for active viewport; change detection drives selective repaint.
- Partial render path handling `CursorOnly` and `Lines` semantic deltas.
- Forced full redraw fallback for `Scroll` and `Full` semantic/effective deltas.
- Previous cursor line repaint inclusion (erase old overlay) + new cursor line.
- Terminal writer abstraction (`MoveTo`, line clear/print) replacing monolithic full frame emission for partial path.
- Metrics: counts of semantic deltas, partial candidate lines, repainted lines, scroll fallbacks, full vs partial frame durations.
- Multi-view scaffolding: `View` struct + `EditorModel` updated to hold `views: Vec<View>` with single active view.
- Snapshot duplicate suppression in undo (skip storing identical successive snapshots) — low-risk memory win.
- Resize invalidation policy: always clear line hash cache + force full on next frame.
- Basic heuristic fallback: if dirty candidate lines > threshold (% of viewport), escalate to full.

Out of Scope (Deferred):

- True terminal scroll optimization (using ScrollUp/Down commands).
- Enriched `RenderDelta` carrying explicit line indices.
- Horizontal scrolling, gutters, or syntax highlighting.
- Multi-view rendering (only active view rendered) or split layout drawing.
- Diff-based undo granularity (still coarse snapshot, just dedupe identical).
- Timer/Tick events (may appear in Phase 4 unless schedule permits late add).
- Advanced line diff (prefix/suffix trim) and rope slice zero-copy.

## 3. Architectural Touchpoints

- `core-actions`: call dirty line tracker on text mutations (inserts, deletes, newline splits, joins).
- `core-render`: add `DirtyLinesTracker`, partial renderer logic, line hash cache structure, metrics extension, and a `RenderPath` selection in existing engine.
- `core-terminal`: add lightweight writer helper (batched crossterm commands) for partial updates.
- `core-model`: extend with `View` & view management; adapt existing code to reference active view cursor/viewport.
- `core-state`: helper to expose line content for hashing; add undo snapshot dedupe.
- `ox-bin`: integrate new partial branch using `RenderDecision` + tracker output; invalidate caches on resize.

## 4. Data Structures

```rust
// core-render
pub struct DirtyLinesTracker { /* small Vec or SmallVec<usize, N> plus optional ranges */ }

pub struct ViewportLineHash {
    pub hash: u64,
    pub len: usize, // raw byte length sans trailing newline
}

pub struct PartialCache {
    pub line_hashes: Vec<ViewportLineHash>,
    pub viewport_start: usize,
    pub width: u16,
}

pub struct PartialMetrics { /* atomic or interior mut counters */ }

// core-model
pub struct View {
    pub id: ViewId,
    pub buffer_index: usize,
    pub cursor: core_text::Position,
    pub viewport_first_line: usize,
}
```

Hashing: use `ahash` (add dep) over the raw line (without newline). Store length + hash; compare both.

## 5. Render Flow (Partial Path)

1. Event loop produces a `RenderDecision { semantic, effective }`.
2. If `effective` is Full OR semantic is Scroll OR resize flag set → full render fallback; rebuild cache afterward.
3. Else gather candidate dirty lines:
   - From `DirtyLinesTracker` intersected with current viewport.
   - Add old cursor line (from last frame) if different.
   - Add new cursor line.
4. Deduplicate + sort candidates.
5. If candidate count > threshold (e.g., 60% of viewport height) → escalate to full (metric: `escalated_large_set`).
6. For each candidate line:
   - Fetch content; compute hash.
   - Compare to cached entry (if in range). If changed or line newly enters range: repaint.
   - Always repaint new/old cursor lines even if unchanged (ensures overlay correctness).
7. Update cache entries for repainted lines; leave others untouched.
8. Emit terminal commands via writer: `MoveTo(0, line_row)` + `ClearLine` + content + newline elision (print only visible content) + status line unaffected unless semantic requested it.
9. Apply cursor overlay (reuse existing logic) using reverse style on cursor cluster; ensure old line repainted or overwritten.

## 6. Dirty Line Tracking Rules

Mark (add line index) when:

- Insert grapheme: affected line.
- Insert newline: original line (split before), new line index.
- Delete backspace at line start (join): previous line (now merged) and removed line index region (conservatively previous line only; join implies content change).
- Delete grapheme at: current line.
- Bulk future operations (paste) — Phase 3 still per-grapheme; range insert not yet optimized. If later added, mark range.

Tracker operations:

- `mark(line)`; ignores duplicates until `take()`.
- `take(viewport_start, viewport_end)` returns vector (clears internal storage).

## 7. Multi-View Scaffolding

- Introduce `ViewId(usize)` newtype.
- `EditorModel` holds `views: Vec<View>`; initial `View { id:0, buffer_index:0, cursor: old_cursor, viewport_first_line: old_viewport }`.
- Replace direct cursor / viewport references in main loop & renderer with active view reference.
- Add methods: `active_view()`, `active_view_mut()`. No user commands to switch yet.
- Tests: invariant that at least one view exists, active id valid, cursor operations modify active view only.

## 8. Undo Snapshot Dedupe

- Before pushing new snapshot, hash (fast 64-bit) the current buffer content (or incremental rolling hash if cheap) and compare with last snapshot stored hash; skip if identical.
- Store snapshot hash alongside snapshot in stack.
- Metrics: `undo_snapshots_skipped`.

## 9. Metrics

Counters (atomic):

- `partial_frames`
- `full_frames`
- `dirty_lines_marked` (raw sum pre-dedupe)
- `dirty_lines_candidates` (after intersect & add cursor lines)
- `dirty_lines_repainted`
- `scroll_full_fallbacks`
- `escalated_large_set`
- `resize_invalidations`
- `cursor_only_short_circuit` (frames where only cursor lines repainted)

Timing:

- Reuse existing last render ns; extend with `last_partial_render_ns` (latest partial path duration).
- Potential future: moving average (deferred).

## 10. Steps (Ordered, One Commit Each)

1. feat(render): add DirtyLinesTracker and integrate with dispatcher markings.
2. feat(render): add ahash dependency & line hash structs + PartialCache skeleton (unused).
3. feat(model): introduce View struct & migrate cursor/viewport into views (single active view).
4. refactor(render): store last cursor line in cache (for old cursor repaint inclusion) & add metric counters scaffolding.
5. feat(render): implement partial line hash comparison logic (no terminal writer yet; still full fallback) with unit tests for changed/unchanged classification.
6. feat(terminal): introduce writer helper (MoveTo, ClearLine, Print) and integrate into full render path as internal abstraction (behavior unchanged) to prepare partial path.
7. feat(render): activate partial rendering for CursorOnly (repaint two lines) + metrics.
8. feat(render): extend to Lines semantic delta (candidate set, threshold fallback) + metrics.
9. feat(render): handle resize invalidation (force full + clear cache) + tests.
10. feat(render): heuristic large candidate escalation & metric.
11. feat(undo): snapshot dedupe by hash + metrics.
12. refactor(model): minor cleanup & rustdoc for multi-view invariants.
13. test(render): integration tests verifying visual parity (full vs partial) for representative edit/motion sequences (simulate by forcing both paths via flag or instrumentation).
14. docs(render): update crate docs (partial pipeline, metrics, invalidation policy) & design note on future scroll optimization.
15. chore(phase): plan progress update & quality gate (fmt, clippy, tests) – Phase 3 closure.

## 11. Exit Criteria

- Cursor-only motions repaint only prior + new cursor lines (no other lines emitted).
- Editing single line (no newline) repaints only that line + cursor line(s).
- Editing producing newline splits repaints original & new line (and cursor) without repainting unaffected lines.
- Scroll events trigger full redraw (metric increment) with cache rebuild.
- Resizes trigger full redraw + cache clear (metric recorded) and subsequent edits use partial path.
- Large dirty candidate sets (>= threshold) escalate to full render and record escalation metric.
- Undo identical successive snapshots skipped; dirty flag semantics unchanged.
- Tests: line hash diff correctness, partial vs full parity, tracker behavior, resize invalidation.
- No clippy warnings; fmt clean; all tests pass.

## 12. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Over-repainting due to conservative dirty marking | Acceptable MVP; later diff refine. |
| Cursor overlay stale on untouched old line | Force old cursor line into repaint candidates. |
| Hash collision skips repaint | (len, u64 hash) pair; collision probability negligible. Fallback: periodic full (scroll/resize) naturally refreshes. |
| Terminal writer out-of-order commands causing flicker | Sequence: MoveTo -> ClearLine -> Print; tested in integration tests. |
| Multi-view scaffolding regression | Limit surface: only active view; tests assert invariants. |
| Undo dedupe misidentifies different snapshots | Include length + hash; improbable; provide test with near-collision patterns. |
| Threshold too low/high increasing full fallbacks | Make constant; document tuning; metrics guide future adaptive logic. |

## 13. Deferred / Follow-Up

- Scroll optimization using terminal scroll region; shift cache instead of rebuild.
- Timer/Tick events for periodic status refresh & async tasks.
- Line-level diff refinement (prefix/suffix skip) for fewer bytes written.
- Multi-view rendering & split layout (draw multiple viewports simultaneously).
- Diff-based undo (delta application vs full snapshot restoral).
- Syntax/token highlight pipeline & themed color output.
- Plugin/LSP/Git integration leveraging async event injection.
- Horizontal scrolling & gutters (line numbers, diagnostics signs).
- Performance dashboard command (exposes metrics snapshot in status or command result).

## 14. References

- Phase 2 design (render delta semantics, auto-scroll margin).
- ropey crate docs (efficient line operations & char indexing).
- crossterm docs (cursor movement, clearing lines, styling).
- ahash crate for high-performance hashing.

## 15. Notes

- Breadth-first preserved: partial path added progressively (first cursor-only, then lines).
- RenderDelta remains semantic-only; physical diff knowledge isolated in dirty tracker + cache.
- Always-on multi-view scaffolding avoids feature fragmentation; future splits become additive.
- Aggressive simplicity: Scroll & resize full fallbacks keep correctness obvious while gathering telemetry for optimization ROI.
- Metrics-first approach ensures data-driven refinement in later phases (Phase 4+).

### Step 3.2 Migration Details (moved from Progress Log)

- Result: Cursor and viewport ownership fully transferred from `EditorState` to `core-model::View`; `EditorState` no longer contains `position` or `viewport_first_line` and has no `auto_scroll` logic.
- Highlights:
  - Added `View::auto_scroll` with vertical margin handling; ported & re-homed auto-scroll tests into `core-model` (now validate margins, boundaries, small viewport clamp).
  - Dispatcher, render engine, and binary event loop updated to operate on `(&EditorState, &View)` with external cursor threading for undo/redo and coalescing APIs.
  - All core-state tests refactored to use local `Position` variables; snapshot/insert coalescing calls updated (`push_snapshot(kind, cursor)`, `begin_insert_coalescing(cursor)`, `undo(&mut cursor)`, etc.).
  - Removed legacy auto-scroll + viewport tests from `core-state` (they now live where the logic resides).
  - Verified via `cargo test` across all crates (green) and formatting checks; no lingering references to deprecated fields.
- Rationale: Enforces architectural separation (model/view own presentation-related cursor+viewport state) preparing for multi-view expansion and enabling per-view scrolling semantics in later steps.

---
(Each completed step updates this document; commit subjects follow template and reference Phase 3 step numbers.)

## 16. Progress Log

(Will be updated as steps complete.)

- [x] Step 1 – DirtyLinesTracker integration (dispatcher markings)
- [x] Step 2 – Line hash structs + PartialCache skeleton (ahash dep)
- [x] Step 3.1 – Add View struct & single-view storage
- [x] Step 3.2 – Migrate cursor & viewport_first_line into View + auto_scroll refactor (merged former 3.2/3.3)
- [ ] Step 3.3 – Cleanup & docs finalize migration (was 3.4)
- [ ] Step 4 – Cache last cursor line + metrics scaffold
- [ ] Step 5 – Hash compare logic tests (still full fallback)
- [ ] Step 6 – Terminal writer abstraction (prep partial)
- [ ] Step 7 – Activate CursorOnly partial rendering
- [ ] Step 8 – Extend partial to Lines semantic delta
- [ ] Step 9 – Resize invalidation (force full + clear cache)
- [ ] Step 10 – Large candidate escalation heuristic
- [ ] Step 11 – Undo snapshot dedupe + metric
- [ ] Step 12 – Multi-view rustdoc & cleanup
- [ ] Step 13 – Integration tests (partial vs full parity)
- [ ] Step 14 – Documentation updates (partial pipeline & metrics)
- [ ] Step 15 – Phase closure quality gate

---
