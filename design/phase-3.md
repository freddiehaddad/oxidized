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

### Step 3.2 Migration Details

- Result: Cursor and viewport ownership fully transferred from `EditorState` to `core-model::View`; `EditorState` no longer contains `position` or `viewport_first_line` and has no `auto_scroll` logic.
- Highlights:
  - Added `View::auto_scroll` with vertical margin handling; ported & re-homed auto-scroll tests into `core-model` (now validate margins, boundaries, small viewport clamp).
  - Dispatcher, render engine, and binary event loop updated to operate on `(&EditorState, &View)` with external cursor threading for undo/redo and coalescing APIs.
  - All core-state tests refactored to use local `Position` variables; snapshot/insert coalescing calls updated (`push_snapshot(kind, cursor)`, `begin_insert_coalescing(cursor)`, `undo(&mut cursor)`, etc.).
  - Removed legacy auto-scroll + viewport tests from `core-state` (they now live where the logic resides).
  - Verified via `cargo test` across all crates (green) and formatting checks; no lingering references to deprecated fields.
- Rationale: Enforces architectural separation (model/view own presentation-related cursor+viewport state) preparing for multi-view expansion and enabling per-view scrolling semantics in later steps.

### Step 4 Details

- Added `last_cursor_line` field to `PartialCache` persisting prior frame cursor line for future selective repaint (old cursor overlay removal).
- Introduced `RenderPathMetrics` (renamed from initial `PartialMetrics`) with atomic counters + timing fields (`full_frames`, `partial_frames`, cursor/lines frame counters, dirty line statistics, escalation, resize invalidations, `last_full_render_ns`, `last_partial_render_ns`).
- Clarified metric layering: `RenderDeltaMetrics` (semantic intent frequency) vs `RenderPathMetrics` (executed strategy + repaint internals).
- Integrated metrics & cache updates into `RenderEngine::render_full` (record duration + last cursor line after each frame).
- Added snapshot API (`metrics_snapshot`) and tests covering cursor line persistence and full frame count increment.
- No behavioral rendering changes yet; still full frame path only (breadth-first scaffolding).
- Prepares for Steps 5–8 without churn in engine signatures.

### Step 5 Details

- Added `core-render::partial_diff` module with `classify_viewport_changes` performing per-viewport line hash comparison against `PartialCache`.
- Cold criteria: (a) cache empty, (b) viewport start changed, or (c) width changed.  Cold path resets cache and classifies all visible text rows as changed. Visible rows = `height - 1` to reserve a status line; includes trailing empty line when the buffer ends with a newline (ropey exposes it as a line) — test documents this.
- Warm path recomputes `(len, hash)` after trimming trailing newline/CR and only updates entries + records lines whose pair changed. Height-only viewport changes that produce a cache length mismatch also trigger a conservative cold reset (breadth-first correctness > premature complexity).
- Metrics: increments `dirty_lines_marked` & `dirty_candidate_lines` by viewport line count (candidate filtering deferred) and `dirty_lines_repainted` by number of changed lines (all on cold start, possibly zero on warm unchanged frame).
- Integration: invoked at start of `RenderEngine::render_full` (still full frame emission) so hashing path is battle-tested before partial output activation in Steps 7–8. No behavioral terminal change yet → flicker-free baseline intact.
- Tests: cold population, unchanged warm frame, single-line edit (cluster insert), newline insertion shifting lines. Adjusted initial test to account for trailing empty line semantics; added explanatory comment to prevent future regressions.
- Aligns doc terminology with code (`dirty_candidate_lines` instead of earlier draft `dirty_lines_candidates`). Single source of truth principle upheld.

---
(Each completed step updates this document; commit subjects follow template and reference Phase 3 step numbers.)

### Step 6 Details

- Added `core-render::writer` module providing a minimal `Writer` abstraction with a queued `commands: Vec<Command>`.
- `Command` enum variants: `MoveTo(x, y)`, `ClearLine(x, y)` (placeholder no-op for now), and `Print(String)` (owns its text to sidestep lifetime issues and allow future pooling/segmentation).
- Integrated into `RenderEngine::render_full` via new helper `render_via_writer(&Frame)` that performs a full traversal of the `Frame` (unchanged ordering) translating each cell into writer commands. Behavior intentionally matches legacy `Renderer::render` (full repaint every frame) — still breadth-first.
- Legacy `Renderer` struct retained temporarily (transition aid / potential parity tests) but no longer invoked by the engine. Removal deferred until after partial activation (ensures rapid rollback path if subtle escape sequencing differences surface during Steps 7–8 tests).
- Cursor highlighting (reverse video) preserved using the same ANSI wrapper `\x1b[7m` ... `\x1b[0m` with identical per-cell wrapping semantics as the old renderer.
- Metrics unaffected this step (all frames still counted as full); partial counters remain dormant until Step 7.
- Design Decision: Keep writer internal to `core-render` instead of new `core-terminal` crate (Option A) to minimize crate churn and accelerate experimentation; can be extracted later if multi-backend support emerges.
- Invariants:
  - Flush order preserves stable top-left to bottom-right painting ensuring deterministic cursor positioning.
  - Exactly one initial `MoveTo(0,0)` emitted; subsequent `MoveTo` only when cell iteration detects a coordinate jump (same logic as legacy renderer ensuring minimal cursor reposition commands for full-frame path).
  - No partial clearing yet; `ClearLine` retained as semantic placeholder for upcoming selective line repaints (Steps 7–8) where it will precede printing changed line content.
  - No allocation amplification beyond per-cell small `String` creations (acceptable for MVP; later optimization may batch contiguous plain cells into a single `Print`).
- Risk Mitigation: By inserting writer before enabling partial output, any escape sequencing or buffering bugs surface while still using the simpler full repaint comparison baseline; simplifies debugging.
- Next Steps Dependency: Steps 7–8 will reuse writer to emit only changed lines (adding `ClearLine` usage and selective `MoveTo` calls) while preserving existing frame-building code for status line composition until a dedicated partial line composer is carved out.

### Step 6.1 Hotfix Details – Writer Row Alignment / Wrap Leak

- Problem: Long content lines that exactly fill the terminal width (or include wide emoji clusters at the boundary) caused the terminal to perform an implicit wrap. Our writer logic assumed cursor position continuity and did not reassert the start-of-line position for the next logical row. Consequences:
  - First visible line sometimes appeared "missing" (actually rendered but vertically shifted by a preceding wrap side-effect).
  - Bullet list indentation could appear incorrect (lost leading spaces due to misaligned cursor start).
  - A spill of the first character of the next logical line (or a content character) into the status line's first cell when the final text row wrapped into the reserved status row.
- Root Cause: Sequential flat iteration over `frame.cells` avoided redundant `MoveTo` commands relying on our internal `(current_x,current_y)` tracking. Terminal implicit wrap changed the actual cursor while our logical tracker remained optimistic.
- Fix: Emit an explicit `MoveTo(0, y)` at the start of every row (row‑major iteration) and abandon the implicit continuity optimization (micro cost << correctness gain). This guarantees re-synchronization each row and prevents wrap leakage into subsequent rows or status line.
- Implementation Adjustments:
  - Rewrote `render_via_writer` to iterate row-by-row (`for y in 0..frame.height`), issuing a leading `MoveTo(0,y)` and then printing each cell in that row sequentially with no reliance on inferred cursor state.
  - Retained reverse video styling behavior unchanged.
  - Added inline comment referencing Phase 3 / Step 6.1 for future audit.
- Performance Consideration: Adds `height` additional `MoveTo` commands per full frame. For typical terminal heights (< 100), negligible versus per-cell Print overhead. Future partial path will drastically reduce number of printed cells making this overhead even more trivial.
- Future Optimization (Deferred): Batch contiguous non-styled single-width glyph runs into one `Print` string to reduce queue calls. Not required for correctness; postpone until after partial rendering stabilization.
- Testing Gap Noted: Current tests do not simulate terminal wrap mechanics. Follow-up (later Step 13 parity tests) should include constructing frames with exact-width lines to assert no status line contamination in command emission transcript.

### Step 7 Details – Activate CursorOnly Partial Rendering

- Goal: When the merged semantic delta is `CursorOnly`, avoid a full frame rebuild and repaint only:
  - The previous cursor line (erase old reverse-video overlay) if still in viewport and different.
  - The new cursor line with fresh overlay.
  - Leave status line untouched (unless a future semantic status change arrives) preserving flicker-free baseline.
- Conditions:
  - `RenderDecision.semantic == CursorOnly` AND `RenderDecision.effective == CursorOnly`.
  - Viewport (start/width/height) unchanged since last full frame (scroll/resize always forces Full elsewhere).
- Scheduler Update: `consume()` sets `effective = CursorOnly` when semantic == CursorOnly; otherwise remains Full (Lines path comes in Step 8).
- Rendering Path:
  - Add `RenderEngine::render_cursor_only`.
  - Skip `classify_viewport_changes` (no content mutation expected).
  - Emit per-line via Writer:
    - For each candidate line: `MoveTo(0,row)` → `ClearLine` → re-render textual content.
    - After line paint, overlay cursor cells on new cursor line.
  - Update `PartialCache.last_cursor_line`.
- Writer Enhancement: Implement `ClearLine` using `crossterm::terminal::Clear(ClearType::CurrentLine)`.
- Metrics:
  - Increment `partial_frames` and `cursor_only_frames`.
  - Store duration in `last_partial_render_ns`.
  - Do not touch dirty line counters (introduced for lines path in Step 8).
- Invariants & Safety:
  - Cursor movement alone never invalidates line hashes; cache remains valid.
  - Scroll semantic would supersede CursorOnly preventing incorrect partial choice.
- Testing (Step 7):
  - Full → cursor-only sequence updates metrics (`full_frames == 1`, `partial_frames == 1`, `cursor_only_frames == 1`).
  - Moving cursor between lines repaints old + new cursor lines (observable via injecting a temporary test writer harness – minimal unit assertion on cache last_cursor_line suffices now).
- Deferred (Step 8): hash-driven arbitrary line repaints & candidate escalation.
- Failure Handling: Bubble errors; future improvement may auto-escalate to full.

### Step 8 Details – Extend Partial Rendering to `Lines` Semantic Delta

- Goal: When text edits affect one or more lines (but not a scroll / resize), repaint only the subset of viewport lines whose content actually changed plus the old & new cursor lines (always) while leaving untouched lines and the status line intact. This generalizes the cursor-only path into a line-diff driven partial path.
- Trigger Conditions:
  - `RenderDecision.semantic == Lines` (content mutation localized) AND scheduler decides `effective == Lines` (may still force Full if cold / large candidate set or cache invalid).
  - Viewport start, width stable (scroll / resize produce Full).
  - Partial cache warm (viewport_start & width match; otherwise escalate to Full and rebuild).
- Scheduler Update (Step 8 code work): allow `effective = Lines` when semantic merged value is Lines; prior steps forced Full for anything except CursorOnly.
- Candidate Collection Algorithm:
  1. Take dirty indices from `DirtyLinesTracker::take()` after event loop dispatch pass.
  2. Intersect with `[viewport_first_line, viewport_first_line + visible_rows)`.
  3. Insert `cache.last_cursor_line` (if still within viewport and different from current).
  4. Insert current cursor line.
  5. Deduplicate & sort (SmallVec -> Vec fallback if > inline capacity).
  6. If candidate count >= `LINES_ESCALATION_THRESHOLD_PCT * visible_rows` → escalate to Full (metric: `escalated_large_set`) and short-circuit.
- Hash / Change Classification:
  - For each candidate line, recompute `(len, hash)` as in Step 5 logic but only for those indices (avoid scanning full viewport). Compare with cached entry; repaint if (a) entry missing, (b) pair differs, or (c) line is old/new cursor line (overlay correctness).
  - Update cache entry on repaint; leave unchanged entries untouched to preserve warm path.
- Writer Emission Sequence per repainted line:
  - `MoveTo(0, row)` (row = line_index - viewport_first_line)
  - `ClearLine`
  - Print grapheme sequence, respecting column clipping at terminal width, computing display width (Unicode correctness) identical to full path. Avoid trailing newline print (editor renders logical lines only).
- Cursor Overlay Application:
  - After all line repaints, re-run overlay on current cursor line (similar to cursor-only path).
  - Guarantee old cursor line was repainted if different; otherwise overlay removal would be stale.
- Metrics (incremental extensions):
  - Increment `partial_frames` and `lines_partial_frames` (new counter) distinct from
    `cursor_only_frames`.
  - Track `dirty_lines_marked` (sum of raw marks pre-intersection) – already collected earlier.
  - Track `dirty_lines_candidates` (post intersection + cursor additions) for this frame.
  - Track `dirty_lines_repainted` (actual lines emitted via writer) for this frame.
  - Record `last_partial_render_ns` duration; optionally introduce moving average later.
- Escalation Causes Enumerated:
  1. Cold cache (viewport moved, width changed, or empty) → Full (metric unaffected except full_frames).
  2. Large candidate percentage (>= threshold) → Full + `escalated_large_set`.
  3. Explicit semantic `Full` or `Scroll` already handled prior (unchanged from earlier steps).
- Safety / Correctness Invariants:
  - Every repaint line either differs in content OR is necessary for cursor overlay cleanup.
  - No line outside candidate set is touched, preventing flicker.
  - Cache coherence: every repainted line has its hash entry updated before method return.
  - After escalation, cache is fully rebuilt by full path ensuring future warm classifications.
- Performance Considerations:
  - Bounds number of hash computations to candidate set size (vs whole viewport in Step 5).
  - SmallVec capacity tuned (e.g., 8) to cover common small edit bursts; heap allocate only on larger sets.
  - Threshold initially 60%; documented constant `LINES_ESCALATION_THRESHOLD_PCT: f32 = 0.6`.
  - Future optimization (Phase 4): prefix/suffix diff to trim printing trailing unchanged segments.
- Testing Strategy (added in Step 8 commit):
  - Single line edit → exactly that line + cursor line repainted (if same line, count = 1).
  - Multi-line contiguous edits below threshold → only those lines repainted.
  - Candidate count just below threshold does partial; just above triggers escalation (assert metrics).
  - Viewport boundary edits (first & last visible line) still partial; off-viewport edits produce no repaint until scrolled.
  - Old cursor line inclusion verified when moving during multi-line edit sequences.
- Deferred / Out-of-Scope Here:
  - Horizontal scroll / gutter painting (kept simple column 0 origin).
  - Batch grouping of adjacent repainted lines (each repainted independently for MVP readability).
  - Region-based dirty compression (line indices only, no ranges) — potential later improvement.
- Failure Handling:
  - On writer error mid-frame, propagate; higher layer may choose to force a Full next cycle.
  - On unexpected cache mismatch (should not occur), escalate to Full + log warning (defensive path).

Rationale: Step 8 converts semantic Lines into an actual selective physical repaint to capture the majority of typical editing workloads (insertion, deletion within a small vicinity) while preserving a simple fallback strategy. It leverages prior hashing groundwork and cursor-only path confidence, advancing breadth-first goals without premature micro-optimizations.

### Step 8.1 Hotfix – Horizontal Motions Not Updating Status Line

Observed Issue:
Horizontal cursor motions (e.g. `h` / `l`) left the status line column value stale; it only updated after a vertical motion or edit. This created the perception of random column jumps and hid potential Unicode width issues.

Root Cause:
The initial CursorOnly partial render path intentionally repainted only the old and new cursor lines, skipping the status line to minimize output. The status line, however, encodes cursor position (line/column) and must refresh on every cursor movement. Horizontal motions scheduled a CursorOnly delta, which never touched the status line, leaving stale column text.

Chosen Fix:
Modify `render_cursor_only` to always rebuild and repaint the status line (single bottom row) in addition to the necessary text lines. This confines the change to the render layer without altering scheduler semantics mid-phase.

Why This Approach:
Low risk (one extra line write per cursor-only frame), preserves existing semantics, and avoids introducing a new semantic delta variant mid-Phase. Future refinement may add a dedicated `StatusLine` semantic for finer granularity.

Metrics Impact:
Slight increase in bytes written for frequent cursor motions; negligible relative to correctness gain. Existing partial frame counters (`cursor_only_frames`) continue to reflect actual frames.

Tests Added:

- New test ensuring repeated horizontal motions trigger cursor-only frames (implicitly exercising status repaint path).

Future Follow-Up (Deferred):
Introduce a `RenderDelta::StatusLine` to allow coalescing with other deltas and optionally skip repaint if purely cosmetic fields unchanged.

### Step 8.2 Hotfix – Unicode Status Column Correctness

Observed Issue:
After Step 8.1 began repainting the status line on every horizontal cursor motion, a latent Unicode column bug surfaced: moving the cursor one grapheme to the right over a leading multi-byte emoji (e.g. `😀`) caused the status column to jump from `Col 1` to `Col 7` (example value) rather than reflecting the emoji's true display width (2). The cursor-only partial path passed a raw byte index (`view.cursor.byte`) to the status builder instead of the visual (grapheme / display cell) column used in the full frame path.

Root Cause:
Full renders compute `col` via `grapheme::visual_col(trimmed_line, cursor_byte)`. The cursor-only partial repaint path (added in Step 7, amended in Step 8.1 to repaint status) reused the raw byte offset. Multi-codepoint grapheme clusters and wide characters (emoji, CJK, combining sequences, ZWJ sequences) have byte lengths that exceed their display cell width, producing inflated reported columns.

Chosen Fix:
Replace the raw byte index with the same grapheme-aware visual column computation inside `render_cursor_only` when constructing the `StatusContext`. This maintains a single source of truth for column width logic (currently the `visual_col` helper) without introducing premature caching complexity.

Why This Approach:
Surgical change (one code site) restores invariant: status column always reflects visual cell position independent of partial/full path. Keeps breadth-first momentum; defers optimization (e.g., incremental visual column tracking or per-line width prefix sums) until a later performance phase.

Tests Added:

- `unicode_status_col.rs`: validates that after a cursor-only partial render over a leading emoji, the status line shows the correct 1-based column (`Col 3` for a width-2 emoji followed by a space).

Performance Considerations:
`visual_col` performs a forward grapheme iteration over the (trimmed) line each time. For typical short lines and given this runs only on cursor-only frames, overhead is negligible at current scale. Future optimization ticket (deferred) will add incremental visual column updates or caching keyed by (buffer revision, line id).

Follow-Up (Deferred):

- Broaden test matrix to include combining marks, zero-width joiner sequences, wide CJK characters, and (if/when implemented) tab expansion interactions with preceding wide clusters.
- Optional `compute_visual_col` abstraction that can later dispatch to a cached fast path.

Risk Assessment:

- Low: Purely affects a diagnostic value (status text) and shares existing, tested utility logic. No change to rendering of buffer content or cursor overlay.

Result:
Status line column now matches full render behavior for Unicode grapheme clusters in both full and cursor-only partial paths.

### Step 9 Details – Resize Invalidation (Force Full + Clear Cache)

Goal:
Guarantee correctness after a terminal size change by invalidating the partial render cache so the next frame is a full rebuild with fresh line hashes sized for the new viewport (width & height). Prevents stale hash comparisons or truncated line artifacts when width shrinks, and ensures new lines entering view are hashed when height grows.

Design:
Introduce `RenderEngine::invalidate_for_resize()` which:

1. Clears `PartialCache` (line hashes, width, viewport_start, last_cursor_line).
2. Increments `resize_invalidations` metric.
3. Defers actual rendering to the normal event-driven loop; caller (terminal resize handler) sets a flag that results in an effective Full render next tick (scheduler policy already specifies scroll/resize => Full).

Implementation Notes:

- Added `PartialCache::clear()` to centralize cache reset semantics.
- Chose not to immediately render inside invalidation to preserve the event-driven design (avoid synchronous side effects in size signal path).
- `last_cursor_line` cleared to avoid repaint assumptions about prior cursor overlay region across dimension changes.

Testing:

- New test `resize_invalidation_clears_cache_and_increments_metric` renders a full frame, calls `invalidate_for_resize`, asserts metric increment, then performs another full render at a different size (verifying no panic and two full frame counts).

Risks & Mitigations:

- Risk: Missed invalidation leads to partial path using stale width; mitigated by forcing caller contract: resize event must call invalidate.
- Risk: Spurious extra full frames if resize fired rapidly; acceptable MVP – future debounce optimization could coalesce consecutive resizes.

Deferred Optimization:

- Track last known (w,h) inside engine and auto-detect mismatch to self invalidate once (belt & suspenders) – postponed to keep MVP minimal.
- Potential future incremental cache shift when only height changes: treat added rows as cold appended lines (Phase 4 candidate).

Integration Update (Runtime Wiring Added):
The initial Step 9 implementation covered engine APIs, metrics, and tests but did not wire the terminal resize event in the main event loop. The runtime is now updated so the resize handler:

1. Calls `render_engine.invalidate_for_resize()`.
2. Marks `RenderDelta::Full` to guarantee a complete repaint on the next render cycle.
3. Continues to recompute vertical margin and (if changed) marks a `StatusLine` delta.

This resolves transient stale content after a shrink before the first cursor motion. Design intent unchanged; this completes Step 9 integration scope.

Outcome:
Editor returns to a safe, cold state after resize ensuring partial rendering never reads invalid cache entries and maintaining flicker-free correctness.

### Step 9.1 Details – Buffer Replacement Invalidation (`:e <path>` Full Escalation)

Observed Issue:
Opening a new file via `:e <path>` after enabling partial rendering (Steps 7–8) only repainted the cursor line (and status) until an additional motion/edit occurred. The previous buffer's remaining visible lines persisted on screen, creating a confusing mixed display (old content + new status).

Root Cause:
The `:e` command path completely replaces the active buffer's rope contents and resets the cursor, but the dispatcher only returned a generic `dirty` flag. The runtime heuristic (Phase 2 Step 17) mapped this to a narrow `Lines` or `CursorOnly` semantic delta depending on cursor movement, allowing the partial path to skip repainting untouched (but now invalid) lines. The partial line hash cache remained "warm" (viewport start & width unchanged) so no cold/full fallback occurred automatically.

Design Decision:
Introduce an explicit structural signal from dispatch: `DispatchResult { buffer_replaced: true }`.  The event loop treats this as a mandatory Full render trigger and clears the partial cache (reuse existing `invalidate_for_resize()` semantics for cache reset).  This keeps detection localized (single return site in dispatcher) and avoids fragile content-length or hash mismatch heuristics in the render layer.

Implementation:

1. Extend `DispatchResult` with `buffer_replaced: bool` and constructor `buffer_replaced()`.
2. On successful `:e <path>` open, return `DispatchResult::buffer_replaced()` early.
3. In the main event loop, branch before the generic `dirty` heuristic: if `buffer_replaced` then call `render_engine.invalidate_for_resize()` (cache clear) and `scheduler.mark(RenderDelta::Full)`.
4. Leave existing resize invalidation path unchanged (shared cache clear logic).

Metrics Impact:
Counts as a normal Full frame (increments `full_frames`). No new metric added in this step; reuse existing instrumentation (rare operation vs frequent edits).

Safety / Correctness:

- Guarantees all visible lines repaint for new buffer content.
- Avoids double invalidation by not separately marking `Lines` or `CursorOnly`.
- Cache cleared so subsequent partial frames compute hashes against the new buffer.

Alternatives Considered:

- Detect buffer length mismatch vs cache population: rejected (indirect & brittle).
- Force cold path by mutating viewport start: rejected (obscures intent).

Testing Strategy (manual until Step 13 parity tests):

1. Start editor, open file A (default).
2. Execute `:e B` where file B has visibly different first few lines.
3. Verify entire viewport updates immediately (no stale lines).
4. Move cursor: subsequent motions use partial cursor-only path as expected.

Future Extension:
If future multi-view introduces per-view buffer switches, propagate a similar structural flag per affected view to escalate only those viewports.

Status: Implemented (Phase 3 Step 9.1).

### Step 10 Details – Large Candidate Escalation Heuristic

Goal:
Avoid inefficient partial repaint cycles when a large fraction of the viewport would be repainted. If the candidate repaint set for a Lines semantic delta meets or exceeds a threshold proportion of visible text rows, escalate to a full frame render rather than issuing many discrete line clears/prints.

Threshold:
`LINES_ESCALATION_THRESHOLD_PCT = 0.60` (60%). For `visible_rows = h - 1`, escalate when `candidates.len() >= 0.60 * visible_rows`.

Implementation:

1. Promote inline constant to `pub const LINES_ESCALATION_THRESHOLD_PCT` in `render_engine.rs` (documented for tests & future tuning).
2. After candidate collection + dedupe in `render_lines_partial`, compare candidate count against threshold; early return with a call to `render_full` when exceeded.
3. Increment `escalated_large_set` metric only on escalation; do not increment `lines_frames` (partial path abandoned). `full_frames` increments via the delegated full render.
4. Leave existing cold-cache / resize / scroll full fallbacks unchanged.

Rationale:
When most of the viewport changes (bulk paste, re-indent, large deletion), a full render is simpler and often faster than many partial operations. This keeps partial rendering targeted at its high-value narrow edits and cursor motions.

Testing:

- New test file `large_candidate_escalation.rs`:
  - `large_candidate_set_escalates_to_full_and_increments_metric` (>= 60% lines): asserts `full_frames` and `escalated_large_set` increment; `lines_frames` unchanged.
  - `candidate_set_below_threshold_stays_partial` (< 60% lines): asserts `lines_frames` increments and no escalation metric change.

Alternatives Considered:

- Adaptive threshold (based on moving averages) – deferred to Phase 4.
- Byte-output estimation instead of line count – premature; revisit after styling & gutters introduce wider variance.

Future Work:

- Adaptive or per-buffer threshold tuning driven by metrics.
- Run-length grouping prior to threshold evaluation (optimize borderline cases).

Status: Implemented (Phase 3 Step 10).

## 11. Undo Snapshot Dedupe + Metric

Goal: Avoid pushing redundant undo snapshots when successive calls observe an identical buffer state (no textual change) while counting how often this occurs to inform future upstream coalescing refinements.

Problem: Certain edit paths can conservatively call `push_snapshot` even if the underlying text did not change (e.g. defensive calls around mode boundaries or future features). Re-cloning the full buffer wastes memory and inflates undo depth without semantic benefit.

Approach (Phase 3 simplicity):

1. Extend `EditSnapshot` with a `hash: u64` field representing the full buffer content at capture time.
2. Compute hash via a straightforward iteration over all lines feeding a `DefaultHasher` (stable for the process; not persisted disk format).
3. On `push_snapshot`, compare new hash to the last snapshot's hash; if equal increment `undo_snapshots_skipped` metric and return early (do NOT clear redo stack since no new edit was introduced).
4. Record trace event `snapshot_dedupe_skip` with depths & hash for diagnostics.
5. Provide getter `undo_snapshots_skipped()` on `EditorState` for future status reporting / dashboards.

Metric: `undo_snapshots_skipped` (monotonic counter on `EditorState`).

Testing:

- `snapshot_dedupe_skips_identical`: pushes snapshot twice without mutation; asserts stack length unchanged and metric increments to 1.
- `snapshot_dedupe_allows_changed`: mutates buffer between pushes; asserts two snapshots present and metric remains 0.

Future Work:

- Adopt incremental / rolling hashes avoiding full traversal on large buffers.
- Track per-snapshot byte length to support memory usage reporting.
- Integrate with future differential snapshots (store deltas instead of full clones).

Status: Implemented (Phase 3 Step 11).

## 12. Multi-View Rustdoc & Cleanup

Goal: Consolidate multi-view scaffolding intent, invariants, and future expansion points into authoritative rustdoc attached to the `core-model` crate while performing light internal cleanup (documentation-only) to prepare for upcoming split rendering work without altering runtime behavior.

Problem: Initial multi-view migration (Steps 3.1–3.3) moved cursor & viewport state into `View` but left only brief high-level comments. As we approach later steps introducing additional views and parity tests, lack of centralized invariants risks drift and accidental misuse (e.g., exposing mutable access to the internal `views` vector prematurely).

Scope (Step 12 only):

1. Expand crate-level rustdoc in `core-model/src/lib.rs` detailing:

- Rationale for `View` extraction.
- Active invariants (single active view, buffer index validity, cursor range guarantees, auto-scroll safety).
- Forward roadmap (split layout, per-view status, horizontal scroll, generational IDs).
- Safety & non-goals for Phase 3.

1. Add documentation for `ViewId` newtype, noting potential generational upgrade later.
1. Leave all data structures and function signatures unchanged (breadth-first stability; no new APIs yet).
1. Update design plan with this section & mark progress log entry for Step 12.

Non-Goals:

- Implementing multiple simultaneously rendered views.
- Introducing view creation/destruction APIs.
- Adding per-view configuration or horizontal scrolling fields.

Outcome:

- Centralized authoritative description reduces future design friction.
- Clear checklist for future mutations: adding a field to `View` requires updating the documented invariants.

Testing:

- Pure documentation change; existing tests unchanged and expected to pass.

Status: Implemented (Phase 3 Step 12).

## 13. Integration Tests – Partial vs Full Parity

Goal: Validate that partial rendering paths (CursorOnly, Lines, escalation fallback, resize invalidation, buffer replacement) yield a final visual frame identical to a full render of the resulting editor state while
asserting minimal repaint scope via test-only instrumentation.

Problem: Prior unit tests covered hashing, metrics increments, and isolated partial behaviors but did not assert holistic parity between a sequence of edits/motions rendered partially and the canonical full frame output. Lack of integration coverage risks regression (e.g., missed cursor overlay cleanup, stale status line) as future optimizations land.

Approach (Step 13):

1. Add lightweight always-on instrumentation to `RenderEngine` capturing
  repainted buffer line indices and a simple tag describing partial path kind
  (`cursor_only`, `lines`, `escalated_full`).
2. Introduce `partial_full_parity.rs` integration tests exercising:
   - Cursor-only motion parity.
   - Single-line in-place edit parity (line + cursor overlay repaint).
   - Multi-line contiguous edits below escalation threshold parity.
   - Large candidate set escalation parity (verification of escalation tag).
   - Resize invalidation followed by partial edit parity.
   - Buffer replacement (:e) full repaint parity.
3. Each test seeds cache via an initial full render, performs state mutation(s), invokes the appropriate partial path (or escalation), then builds a fresh full frame snapshot for equality comparison.
4. Assert repaint scope minimality using captured `last_repaint_lines` where applicable (cursor-only = old + new; lines = old cursor + changed lines; escalation = empty set + escalated tag).

Non-Goals:

- Performance benchmarking (timings) – deferred to a later phase.
- Capturing actual terminal escape sequences (validated indirectly via frame equality and writer path unit coverage).
- Multi-view parity (single view only at this stage).

Instrumentation Justification:
Always compiled (tiny Vec + Option) so integration tests (separate crate targets) can access it. Overhead is negligible (clears + few pushes only when partial paths execute) and avoids feature flags / cfg complexity.

Outcome:

- Increased confidence partial rendering is visually lossless relative to full frames under covered scenarios.
- Test scaffolding ready for future additions (Unicode stress, multi-view).

Status: Implemented (Phase 3 Step 13).

## 14. Documentation Updates – Partial Pipeline & Metrics

Goal: Promote the partial rendering subsystem (cursor-only & lines paths, escalation heuristic, resize/buffer replacement invalidation, instrumentation) into authoritative documentation. Establish a stable reference for future optimization iterations (scroll region usage, batched printing, Unicode perf improvements) without changing behavior.

Scope:

- Crate-level rustdoc (core-render) summarizing pipeline & metrics.
- Design plan section (this) capturing decision inputs, lifecycle, policies.
- Explicit articulation of metrics intent & interpretation heuristics.
- Catalog deferred optimizations with rationale for deferral.

Pipeline (Current State):

1. Scheduler merges semantic deltas into an effective delta per frame.
2. Full / Scroll / cold / resize / buffer replacement => full frame build + emit.
3. CursorOnly (warm) => repaint old/new cursor lines + status; skip hashing.
4. Lines (warm) => collect dirty + cursor lines, threshold check, selective
  hash compare + repaint, then cursor overlay.

Decision Inputs:

- Semantic delta expresses high-level intent (motion/edit/scroll).
- Effective delta may escalate to Full (threshold, cold, structural change).

Cache Lifecycle:

- Full render always refreshes entire viewport hash snapshot.
- Lines path updates hashes only for repainted lines (changed or cursor).
- Resize or buffer replacement clears cache (cold start next frame).

Metrics (RenderPathMetrics) Overview:

- Frame Counts: full_frames, partial_frames, cursor_only_frames, lines_frames.
- Dirty Line Funnel: dirty_lines_marked -> dirty_candidate_lines -> dirty_lines_repainted.
- Escalation & Environment: escalated_large_set, resize_invalidations.
- Timing: last_full_render_ns, last_partial_render_ns.

Interpretation Heuristics:

- High candidate vs repainted delta suggests hash/classification wins.
- Rising escalated_large_set implies threshold tuning or need for scroll region.
- Large partial frame times vs full may signal redundant hashing or excessive
  ClearLine usage (future diff micro-optimizations).

Invalidation & Escalation Policies:

- Resize / buffer replacement: unconditional cache clear + forced Full.
- Cold cache detection (viewport start or width mismatch) => Full.
- Lines threshold (>=60% visible rows) => escalate to Full.
- CursorOnly never hashes; relies on correctness of prior full frame.

Deferred Optimizations:

- Terminal scroll region usage for scroll deltas.
- Prefix/suffix diff trimming for line repaint output minimization.
- Command batching (merge adjacent plain cells into single Print writes).
- Moving average / percentile latency instrumentation.
- Unicode width caching keyed by (line revision, span) for visual_col.

Non-Goals (Step 14):

- Adding new metrics counters.
- Altering existing heuristic constants.
- Enabling multi-view simultaneous rendering.

Status: Implemented (Phase 3 Step 14).

## 15. Phase Closure – Quality Gate

Goal: Formally close Phase 3 by auditing quality gates (fmt, clippy, tests), verifying documentation parity (design plan vs crate rustdoc), enumerating deferred backlog items with rationales, and capturing a baseline metrics snapshot interpretation to guide Phase 4 optimization priorities.

Quality Gate Results (at closure):

- Formatting: `cargo fmt -- --check` passes (no diffs).
- Linting: `cargo clippy --all-targets --all-features -D warnings` passes with zero warnings (aside from any upstream cargo metadata advisory not under source control scope).
- Tests: All unit + integration tests green (hash diff, partial/full parity, resize/buffer replacement invalidation, undo dedupe, escalation).
- Build Health: No deprecated symbols retained; legacy full `Renderer` kept only if still referenced by parity tests (removal deferred to Phase 4 start to avoid destabilizing newly landed partial pipeline). If not referenced at Phase 4 kickoff, remove immediately per deprecation policy.

Documentation Parity Audit:

- Design plan (this file) Step sections 1–14 align with crate-level rustdoc in `core-render` describing pipeline, cache lifecycle, metrics, escalation, and invalidation policies.
- Metrics field rustdoc (`RenderPathMetrics`) mirrors taxonomy documented under Section 14; no divergent naming (e.g. `dirty_candidate_lines` used consistently).
- Multi-view invariants documented in `core-model` crate rustdoc match design scope (single active view only; split rendering deferred).
- Any future change to threshold constant or cache invalidation rules must update both: design plan (historical rationale) + crate rustdoc (operational reference).

Deferred Backlog (carried forward):

1. Scroll Region Optimization: Replace full redraw on scroll with terminal scroll commands + cache shift (highest performance ROI next phase).
2. Prefix/Suffix Diff Trimming: Reduce bytes written for long but minimally changed lines.
3. Command Batching: Merge adjacent `Print` commands for contiguous plain glyph runs to lower syscalls / I/O overhead.
4. Unicode Width Caching: Cache grapheme width / visual column mapping per (line revision, span) to amortize repeated `visual_col` scans.
5. Moving Averages & Percentiles: Add latency distribution metrics to detect tail latency regressions hidden by last-frame timings.
6. Adaptive Escalation Threshold: Tune or auto-adjust 60% heuristic based on observed repaint efficiency metrics.
7. StatusLine Semantic Delta: Introduce dedicated semantic variant to avoid repaints when only overlay lines changed (decouple from text lines).
8. Remove Legacy Full Renderer: After confirming no remaining parity reliance (or convert into a test-only helper).
9. Partial Path Error Resilience: Auto-escalate to full on writer errors / inconsistencies with a diagnostic metric counter.
10. Performance Dashboard Command: Surface metrics snapshot inside editor.

Baseline Metrics Interpretation (qualitative; numeric snapshot intentionally omitted to avoid staleness):

- Partial Frame Ratio: Expect high during navigation & localized edits; sharp drops signal excessive escalations (investigate candidate threshold or missing dirty filtering).
- Dirty Funnel Efficiency: Large gap between `dirty_candidate_lines` and `dirty_lines_repainted` indicates hashing successfully prevents redundant writes (desired). If nearly equal frequently, pursue prefix/suffix diff.
- Escalation Frequency: Low single-digit percentage acceptable; sustained elevation implies either threshold too low or missing scroll optimization.
- Resize Invalidations: Should correlate only with user terminal resizes; unexpected spikes may indicate indirect dimension detection issues.

Exit Criteria Review (Section 11): All listed criteria satisfied; parity tests affirm selective repaint correctness and invariant maintenance (cursor overlay cleanup, status accuracy, cache rebuild on structural changes).

Phase Health Summary: Partial rendering MVP stable, flicker-free, and instrumented. Architectural tenets upheld (event-driven path preserved; semantic deltas remain abstract; breadth-first incremental layering; Unicode correctness maintained for tested scenarios). Ready to advance to Phase 4 focused on scroll optimization and diff/throughput refinements.

Status: Completed (Phase 3 Step 15).

## 16. Progress Log

(Will be updated as steps complete.)

- [x] Step 1 – DirtyLinesTracker integration (dispatcher markings)
- [x] Step 2 – Line hash structs + PartialCache skeleton (ahash dep)
- [x] Step 3.1 – Add View struct & single-view storage
- [x] Step 3.2 – Migrate cursor & viewport_first_line into View + auto_scroll refactor (merged former 3.2/3.3)
- [x] Step 3.3 – Cleanup & docs finalize migration (was 3.4)
- [x] Step 4 – Cache last cursor line + metrics scaffold
- [x] Step 5 – Hash compare logic tests (still full fallback)
- [x] Step 6 – Terminal writer abstraction (prep partial)
- [x] Step 6.1 – Writer row alignment hotfix
- [x] Step 7 – Activate CursorOnly partial rendering
- [x] Step 8 – Extend partial to Lines semantic delta
- [x] Step 8.1 – Hotfix: horizontal motions repaint status line
- [x] Step 8.2 – Hotfix: Unicode status column correctness (visual vs byte)
- [x] Step 9 – Resize invalidation (force full + clear cache)
- [x] Step 9.1 – Buffer replacement invalidation (Full escalation on :e)
- [x] Step 10 – Large candidate escalation heuristic
- [x] Step 11 – Undo snapshot dedupe + metric
- [x] Step 12 – Multi-view rustdoc & cleanup
- [x] Step 13 – Integration tests (partial vs full parity)
- [x] Step 14 – Documentation updates (partial pipeline & metrics)
- [x] Step 15 – Phase closure quality gate

---
