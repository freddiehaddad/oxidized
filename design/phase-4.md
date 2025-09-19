<!-- Copilot: Do not wrap long lines. Preserve full line length for readability and diff clarity. -->
<!-- Copilot: Ensure `cargo fmt -- --check` passes before commit. -->
<!-- Copilot: Ensure `clippy --all-targets --all-features -- -D warnings` passes before commit. -->
<!-- Copilot: Ensure `cargo nextest run --all-targets --all-features --no-fail-fast` passes before commit. -->
<!-- Copilot: Ensure `cargo nextest run --all-targets --all-features --no-fail-fast` passes before commit. -->
# Phase 4: Performance Optimization & Operator/Count Activation

## 1. Objective

Accelerate core editing and rendering paths while turning on the first user‑visible modal editing primitives (counts + delete/yank/change operators) with correct interaction semantics for motions already implemented. Reduce terminal output volume (scroll region + trimmed diff emission), expose real metrics snapshot via `:metrics`, and lay scaffolds for forthcoming features (registers, asynchronous tasks, syntax highlighting pipeline). Preserve breadth‑first ethos: editor remains runnable after every step; correctness preferred over premature micro‑optimizations.

## 2. Scope

In Scope:

- Activate count parsing in `KeyTranslator` (prefix digits, max u32).
- Operator pending state machine: `d`, `y`, `c` combined with existing motions (grapheme & word motions, line start/end, half‑page motions).
- Implement delete & yank behavior over motion spans (insert mode exit for `c{motion}`; cut text removal for `d{motion}`; yank copies only).
- Introduce unnamed (`"`) register and numbered yank/delete ring scaffolding (store results; paste deferred to Phase 5).
- Render path scroll optimization using terminal scroll region when a pure vertical scroll semantic delta occurs (shift partial cache instead of full redraw).
- Partial line repaint trimming: emit only changed middle segment with prefix/suffix skip heuristic (simple first implementation).
- Metrics snapshot command `:metrics` returns multi‑line ephemeral or toggles a temporary metrics overlay (decide cheapest initial path – ephemeral multi‑line message truncated to viewport width).
- Performance counters expansion (operators, trim efficiency, scroll shift usage, bytes written per frame, average partial duration).
- Lightweight task ticker (monotonic trigger) for ephemeral refresh & future async integration (no external IO yet).
- StatusLine semantic refinement: introduce dedicated `StatusOnly` variant (if not already) OR cache to skip unchanged status paints.
- Internal abstraction for motion span resolution (operator engine depends on canonical span builder).

Out of Scope (Deferred):

- Paste (`p`, `P`) and explicit register selection (`"a`, `"*`).
- Visual mode & block/line selections.
- Multi‑view simultaneous rendering (still single active view region).
- Horizontal scrolling & gutters/line numbers.
- Syntax highlighting actual tokenization (scaffold only if needed).
- LSP, DAP, Git, Copilot, plugin host wiring.
- Macro recording/playback, marks, jumps.
- Collaborative editing / networking.
- Persistent config for operators/registers (runtime only now).

## 3. Architectural Touchpoints

- `core-actions`: - Extend `KeyTranslator` for count + operator pending transitions. - Introduce `OperatorState` enum (Idle, Pending{op, count}, Ready{op, count, motion}). - Add motion span resolver used by operator application.
- `core-state`: - Add `Registers` struct storing unnamed and numbered ring. - Provide buffer span delete & extract APIs preserving undo coalescing semantics.
- `core-text`: - Utility for computing byte span for a motion (grapheme / word aware - inclusive/exclusive rules matching Vim semantics subset).
- `core-render`: - Scroll region shift path: when viewport moves by N lines small delta without content mutation (pure scroll), emit scroll commands - repaint only entering lines. - Line diff trim (prefix/suffix) producing minimal repaint segments. - Metrics extensions & snapshot assembly.
- `core-terminal`: - Expose safe wrapper for scroll region enable/disable & scroll up / down commands gated by `TerminalCapabilities`.
- `ox-bin`: - Integrate ticker (simple periodic event) for ephemeral expiration & potential metrics overlay refresh.
- `core-config`: - Optionally add `[performance]` toggles (feature‑gated) BUT default to auto‑enabled; may defer if adds noise.

## 4. Data Structures

```rust
// core-actions
pub struct KeyTranslator {
    pending_count: Option<u32>,
    pending_op: Option<OperatorKind>,
    // future: pending register, motion cache
}

pub enum OperatorState {
    Idle,
    Pending { op: OperatorKind, count: u32 },
    Ready { op: OperatorKind, count: u32, motion: MotionKind },
}

// core-state
pub struct Registers {
    pub unnamed: String,        // last yanked or deleted text
    pub numbered: Vec<String>,  // ring (0..=9) newest at 0
}

// core-render
pub struct ScrollShiftResult {
    pub lines_scrolled: i32,    // +down, -up
    pub entering_start: usize,  // first new viewport line index
    pub entering_count: usize,  // number of new lines to repaint
}

pub struct TrimmedDiffSegment {
    pub line_index: usize,
    pub start_col: u16,
    pub text: String,           // already clipped to width
}

pub struct MetricsSnapshot { /* aggregated counters + durations */ }
```

Constants:

```rust
pub const MAX_COUNT: u32 = 999_999; // clamp safety
pub const SCROLL_SHIFT_MAX: usize = 8; // threshold for shift vs full
pub const TRIM_MIN_SAVINGS_COLS: u16 = 4; // require benefit
```

## 5. Algorithms / Flows

### 5.1 Count & Operator Translation

1. Digit (0-9) in Normal & no pending operator: - If current count None and digit == '0' → motion to line start (Vim rule) unless a prior digit present; else accumulate.
2. Operator key (`d`,`y`,`c`): capture pending op; store count (=1 or accumulated) then await motion (Step 2 adds multiplicative count logic with optional secondary count after operator and before motion, e.g. `2d3w`).
3. Motion arrives → produce `Action::ApplyOperator { op, motion, count }` and clear pending (count = prefix_count * post_op_count, defaulting missing parts to 1). `d0` special-cases `0` as LineStart motion.
4. Escape cancels pending count/op state.

### 5.2 Motion Span Resolution

Given (cursor position, motion, count) produce (start_byte, end_byte, inclusive/exclusive) applying count iterations of the primitive motion. Return normalized span `[start,end)` for deletion/yank; for change also sets editor to Insert mode post deletion.

### 5.3 Register Update Rules (Subset)

- Delete (`d`): yanked text written to unnamed and pushed onto numbered ring at index 0 (truncate length > 10). Change (`c`) behaves like delete then enter insert mode.
- Yank (`y`): copy span to unnamed and numbered[0] only (no removal).

### 5.4 Scroll Region Optimization

Preconditions: semantic delta = Scroll, terminal supports scroll region, absolute viewport shift `|Δ| <= SCROLL_SHIFT_MAX`, and no other semantic (Lines/Status/Cursor) in collapsed decision. Steps:

1. Enable scroll region (full content area minus status line) if not active.
2. Emit scroll up/down commands by `Δ`.
3. Repaint entering lines only using existing hashing for change detection just for those lines.
4. Adjust partial cache indices by shift (slice rotate / logical offset update) to avoid full rebuild.
5. Metrics: increment `scroll_region_shifts` and add saved line count.

### 5.5 Trimmed Line Diff

For a changed line:

1. Compute common prefix (grapheme cluster aware) until first diff.
2. Compute common suffix (reverse) stopping before prefix crossover.
3. If trimmed interior length + prefix skip produces savings >= `TRIM_MIN_SAVINGS_COLS`, emit segment repaint starting at prefix column else fall back to full line repaint.
4. Maintain cursor overlay correctness by ensuring cursor cell within repainted region or by repainting overlay separately.
5. Metrics: track `trim_attempts`, `trim_success`, `cols_saved`.

### 5.6 Metrics Snapshot Command

On `:metrics`:

1. Aggregate counters: frames (full/partial/cursor/lines), bytes written, scroll shifts, dirty funnel (marked/candidates/repainted), operator applications, trim stats.
2. Format into bounded multi-line string (truncate rows exceeding viewport - 1 leaving status line).
3. Present via ephemeral (extended TTL 5s) or toggle overlay mode until next keypress.

### 5.7 Ticker Event

Simple monotonic interval (e.g. 250ms) generating `Event::Tick` pushing status refresh (ephemeral expiry) and optional metrics overlay repaint as `StatusOnly` (or StatusLine) delta.

## 6. Steps (Ordered, One Commit Each)

1. feat(input): count accumulation in KeyTranslator (tests: basic, 0 motion rule).
2. feat(input): operator pending state & ApplyOperator action emission. (complete)
3. feat(state): introduce Registers struct + unnamed/numbered ring.
4. feat(actions): motion span resolver (byte span tests across motions).
5. feat(undo): integrate span delete with undo snapshots (coalescing boundaries respected).
6. feat(operators): implement delete (d{motion}[count]) semantics.
7. feat(operators): implement yank (y{motion}[count]) storing in registers (no paste yet).
8. feat(operators): implement change (c{motion}[count]) = delete + enter insert; cursor placement rules (start of span).
9. feat(metrics): operator counters + registers snapshot fields.
10. feat(render): scroll region enable + shift path (fallback safety & metrics).
11. feat(render): partial cache shift logic (adjust indices + entering line hash fill) with tests.
12. feat(render): trimmed line diff emission (heuristic) + metrics.
13. feat(render): status line cache skip unchanged (StatusOnly delta or internal short‑circuit) + tests.
14. feat(events): introduce ticker Event::Tick (bounded channel usage).
15. feat(command): implement real :metrics snapshot output.
16. refactor(render): unify repaint path for full/partial/trimmed for code reuse (ensure no perf regression).
17. test(integration): performance parity scenarios (large scroll, multi-line edits) assert reduced bytes vs baseline counters.
18. docs(phase): update design & crate docs for operators + perf paths.
19. chore(phase): quality gate & Phase 4 closure.

## 7. Exit Criteria

- Counts preceding motions and operators honored (e.g. `5l`, `3dw`).
- Delete/yank/change apply over resolved spans; undo/redo parity preserved; change enters Insert mode.
- Registers store last operation text; numbered ring rotates.
- Scroll region optimization active for small vertical scroll deltas (lines entering only repainted) with metrics counting shifts.
- Trimmed diff reduces emitted columns when large unchanged prefix & suffix exist (metrics show >0 trim_success).
- :metrics displays snapshot with all key counters present (frames, operators, scroll shifts, trim stats, bytes written, dirty funnel).
- Status line not repainted when identical (metrics reflect skips).
- All tests (including new motion span, operator, render optimization tests) green; clippy / fmt clean.
- No regressions in Unicode cursor width or grapheme motion boundaries.

## 8. Metrics Additions

- `operator_delete`, `operator_yank`, `operator_change` counts.
- `register_writes`, `numbered_ring_rotations`.
- `scroll_region_shifts`, `scroll_region_lines_saved`.
- `trim_attempts`, `trim_success`, `cols_saved_total`.
- `status_skipped`, `bytes_written_frame_last`, `bytes_written_total`.
- Moving averages (deferred) but keep `last_partial_ns_avg` placeholder.

## 9. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Count parsing ambiguity with leading 0 | Mirror Vim rule; tests for `0w` vs `10w`. |
| Off‑by‑one in motion span (inclusive/exclusive) | Centralized span resolver tests across edge cases. |
| Register ring overflow mis-rotates | Bound length & explicit rotate tests. |
| Scroll region misuse on unsupported terminals | Gate by `TerminalCapabilities`; fallback to full. |
| Cache shift logic corrupts hashes | Unit tests shifting up/down various deltas + assertions. |
| Trim algorithm mis-detects grapheme boundaries | Use grapheme iterator for prefix/suffix; fallback to full on anomaly. |
| Increased complexity harms maintainability | Incremental commits + refactor step (16) to unify duplication. |
| Ticker floods event channel | Fixed low frequency (>=250ms) + bounded channel metrics. |
| Metrics output truncates critical lines | Guarantee ordering priority; document truncation. |

## 10. Deferred / Follow-Up

- Paste & explicit register selection.
- Visual mode (line/block/char) expanding operator span sources.
- Multi-view/split rendering & independent scroll regions.
- Syntax highlighting token pipeline + theming.
- Async LSP requests & diagnostics integration.
- Gutter (line numbers, diff signs) + horizontal scrolling.
- Advanced diff trimming (interior multi-segment) & batching.
- Adaptive thresholds (candidate escalation & trim savings).
- Memory pooling for writer command buffers.
- Pasteboard / system clipboard integration.

## 11. References

- Phase 3 design & Refactor R3 docs.
- Vim help: `:help operator`, `:help quotequote`, `:help quote_number`.
- crossterm scroll region & scroll commands documentation.
- Unicode grapheme iteration (unicode-segmentation crate).

## 12. Notes

- Breadth-first preserved: each optimization guarded by fallback.
- Operators limited scope (no visual/paste) to control regression risk.
- Performance wins instrumented first; later phases can tune thresholds with empirical data.
- Trim heuristic intentionally simple; complexity postponed until real metrics show ROI.
- Scroll region path isolated; safe to disable if anomalies detected.

## 13. Progress Checklist

(Will be updated as steps complete)

- [x] Step 1 – Count accumulation in KeyTranslator (complete)
- [x] Step 2 – Operator pending state & ApplyOperator emission (complete)
- [x] Step 3 – Registers struct (unnamed + numbered ring scaffold) (complete)
- [x] Step 4 – Motion span resolver (byte span tests) (complete)
- [x] Step 5 – Integrate span delete with undo (complete)
- [x] Step 6 – Delete operator d{motion}[count] (complete)
- [x] Step 6.1 – Ctrl-D precedence fix (complete) – KeyTranslator short-circuits Ctrl-D / Ctrl-U to half-page motions before operator/count logic, resetting any pending operator & counts to preserve Vim parity.
- [x] Step 6.2 – Linewise vertical delete semantics (complete) – vertical motions with delete (e.g. dj, d2j) now compute a linewise span (start line through target line inclusive) outside the generic charwise span resolver; structural edit path reuses existing delete_snapshot. Added tests: dj (2 lines), 2dj (3 lines), d2j (3 lines). Yank/change steps will mirror this logic.
- [x] Step 6.3 – Structural multi-line edit invalidation (complete) – multi-line deletes and their undo/redo now mark `buffer_replaced` forcing a full repaint, eliminating transient duplicated / out-of-order line artifacts. Temporary guard to be replaced by granular scroll/shift + line range semantics in Steps 10–11.
- [x] Step 6.4 – Operator & structural invariant test hardening (complete) – added reusable scenario harness (`tests/operator_scenarios.rs`) covering multi-step sequences (e.g. `dj` undo restoration, stacked `dj dj u u`, intra-line `2dw u`) asserting buffer restoration & structural repaint flags. Added operator×motion matrix (`tests/operator_matrix.rs`) validating delete across word & vertical motions with counts (charwise vs linewise structural expectations). Establishes foundation for forthcoming yank/change operators to plug into same harness, minimizing regression surface.
- [x] Step 7 – Yank operator y{motion}[count] storing registers (complete) – implements non-destructive copy over motion spans. Vertical motions (y{j,k,CTRL-D/U half-page}) reuse linewise span logic mirroring delete without mutation; multi-line yanks never mark structural. Characterwise spans resolved via existing span resolver and collected without buffer mutation. Registers updated through `record_yank` (unnamed + numbered ring rotation). Tests added: yw, 2yw, y2w, yj, 2yj plus matrix extensions asserting buffer unchanged & non-structural semantics.
- [x] Step 8 – Change operator c{motion}[count] enters insert (complete) – implements delete+insert semantics. Vertical motions reuse delete linewise span computation; span removed recorded via `record_delete` and multi-line changes flagged structural (buffer_replaced). Cursor placed at span start (linewise -> line start; charwise -> original start position) and mode transitioned to Insert. Tests: cw, 2cw, c2w, cj, 2cj plus matrix change cases asserting dirty flag, structural expectations, register population, and insert mode transition.
- [x] Step 9 – Operator & register metrics counters (complete) – added `OperatorMetrics` (delete, yank, change counts; register_writes; numbered_ring_rotations) stored in `EditorState`. Instrumented dispatcher operator application paths and register record functions to increment counters. Exposed snapshot API (`operator_metrics_snapshot`) for forthcoming `:metrics` command integration. Added tests validating per-operator increments and ring rotation after exceeding capacity.
- [x] Step 10 – Scroll region enable + shift path (initial impl, later degraded) – initial version introduced `SCROLL_SHIFT_MAX` (12) and a pseudo scroll fast path that only repainted entering lines without emitting real terminal scroll commands, causing stale intermediate rows. Fast path has been **temporarily degraded to full renders** (recording `scroll_shift_degraded_full`) to restore correctness pending proper terminal scroll implementation.
- [x] Step 10.1 – Real scroll region emission (complete) – emits ANSI scroll region + S/T, repaints entering lines, repaints previous cursor line to clear stale highlight, overlays current cursor & status. Added test `scroll_shift_cursor_trail` asserting old cursor line repaint & correct `lines_saved` accounting. Mini parity harness deferred to Step 10.2.
- [x] Step 10.2 – Parity & invariant harness (complete) – introduced scroll shift invariant tests (multi-step up/down small deltas) asserting: (a) cursor uniqueness via old cursor line repaint, (b) repaint set size == entering lines (+ old cursor line when still visible), (c) cumulative lines_saved increments by expected delta each shift. Provides scaffold for future diff/trim parity (placeholder for full command stream capture deferred to trimmed diff step). DirtyPlan builder deferred until trimmed diff (Step 12) to avoid premature abstraction.
- [x] Step 11 – Partial cache shift logic + tests (complete) – extracted in-place reuse abstraction `PartialCache::shift_for_scroll` replacing ad-hoc slice shifting in `render_scroll_shift`. Added dedicated up/down shift tests asserting: (a) reused hash positions preserve prior values, (b) only entering line hashes recomputed, (c) viewport_start updated. This isolates future trimmed diff logic from low-level cache mutation concerns and reduces risk of divergence across partial paths.
- [x] Step 12 – Trimmed line diff emission + metrics (complete) – Added shadow screen text storage (`prev_text`) in `PartialCache` enabling grapheme‑aware longest common prefix/suffix heuristic. Emits only interior mutation when savings >= `TRIM_MIN_SAVINGS_COLS` (4 columns). Conservative fallback (full line repaint) for: small savings, empty interior after deletion, ambiguous boundaries. On shrink we currently clear full line (simple correctness) then repaint interior; refinement to targeted suffix clear deferred to Step 16 refactor. Metrics added: `trim_attempts`, `trim_success`, `cols_saved_total`. Tests: successful interior trim, below‑threshold fallback, unicode wide cluster trim, interior deletion fallback/neutral path, scroll shift preservation of `prev_text`, cache shift invariants, resize repopulation. Ensures no partial repaint splits grapheme clusters; any anomaly degrades to full repaint maintaining flicker‑free guarantee.
- [ ] Step 13 – Status line cache skip unchanged (pending)
- [ ] Step 14 – Ticker Event::Tick introduction (pending)
- [ ] Step 15 – Real :metrics snapshot output (pending)
- [ ] Step 16 – Repaint path refactor (full/partial/trimmed unify) (pending)
- [ ] Step 17 – Integration perf parity tests (pending)
- [ ] Step 18 – Documentation updates (operators + perf paths) (pending)
- [ ] Step 19 – Quality gate & Phase 4 closure (pending)
