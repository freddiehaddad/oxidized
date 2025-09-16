# Refactor Checkpoint R2: Render Engine, IO Isolation & Pre-Partial Foundations (Planned)

## 1. Objective

Prepare the codebase for Phase 3 (partial rendering + early performance & multi-source event expansion) by extracting minimal forward-facing abstractions without changing user-visible behavior. Maintain breadth-first integrity: each step builds, passes tests, and preserves runtime semantics (still full-frame redraw, single active buffer view). Remove accumulated transitional seams (Colon variant, dispatcher IO entanglement) and introduce test/telemetry scaffolding to safely evolve rendering and event ingestion.

## 2. Scope (In / Out)

In Scope:

- Render engine facade extraction (decouple frame assembly from future diff application).
- Cursor overlay separation + prior span metadata (no visual change yet).
- New semantic delta variant for scroll (`Scroll { old_first, new_first }`).
- Status line segment model (future dynamic segments / truncation) preserving identical output.
- File IO logic isolation (open/write) from dispatcher into dedicated pure helpers.
- High-level model wrapper for future multi-view/split layout (thin `EditorModel`).
- Config re-clamp hook on terminal resize.
- Key input normalization (remove `KeyCode::Colon`).
- Scheduler metrics grouping & scenario tests for semantic delta sequences.
- Render performance timing hook (non-invasive, stored metric only).
- Comprehensive refactor design document (this file) tracking progress.

Out of Scope (Deferred to Phase 3 or later):

- Actual partial rendering / line diff emission.
- Multi-buffer visible UI / split tree.
- Async file IO & encoding detection.
- Undo diff snapshots or time-based coalescing.
- Plugin host, LSP, diagnostics, git integrations.
- Output of metrics to external sinks (remain internal counters for now).

## 3. Architectural Touchpoints

| Concern | Current | Limitation | R2 Change | Future Benefit |
|---------|---------|------------|-----------|----------------|
| Frame Rendering | `build_frame` in `ox-bin` | Hard to branch partial vs full | Move to `RenderEngine` | Swap strategies w/out editing loop |
| Cursor Paint | Intermixed with content | Hard to isolate diff of cursor only | Separate overlay + track prior span | Single-span repaint path |
| Scroll Invalidation | Marks `Full` | Over-paints on small scroll | Add `Scroll` semantic (still Full effective) | Smooth incremental scroll in Phase 3 |
| Status Line | Single formatter | Rigid for segments | Segment enum + compose | Add LSP/git segments easily |
| File IO | Embedded in dispatcher | Blocks async evolution | Extract helpers | Async drop-in later |
| Input Colon Handling | Dual variant | Redundant regression risk | Normalize & remove variant | Simplify translation |
| Config Margin | One-shot clamp | Resize not re-applied | Recompute on resize | Correct margin after resize |
| Metrics | Scattered atomics | Hard to snapshot/report | Metrics struct | Central perf/export path |
| Multi-view Prep | Raw `EditorState` only | Splits need new layering | `EditorModel` wrapper | Introduce view tree incrementally |
| Delta Tests | Unit-level only | Scenarios untested | Add action→delta tests | Safer partial logic refactors |

## 4. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Engine extraction introduces regressions | Golden status string + existing cursor tests retained; no logic rewrite inside loops |
| Scroll variant unused / drifts | Scenario tests assert semantic output; doc references Phase 3 activation |
| Status segment abstraction over-engineered | Keep enum minimal; formatting still linear push onto String |
| IO extraction duplicates code | Central helper returns structured result; dispatcher thin wrapper |
| Colon removal breaks tests | Introduce normalization test; migrate translator tests first |
| Metrics struct adds contention | Still relaxed atomics; struct just namespaces fields |
| EditorModel premature | Wrapper only; no behavioral logic until splits arrive |
| Increased test surface slows CI | Leverage focused scenarios; small buffer fixtures |

## 5. Step Breakdown (Each = One Commit)

0. (Planned) Plan scaffold (this file): objectives, scope, steps, risks.  (You are here)
1. Extract RenderEngine: move `build_frame` + render call path; add `render_full` stub & unused `render_partial`.
2. Separate cursor overlay: split content assembly vs cursor overlay; store previous cursor span metadata.
3. Introduce `RenderDelta::Scroll { old_first, new_first }`. Mark viewport vertical shifts with this semantic (effective render still Full in Refactor R2). Collapse precedence: Full > Lines > Scroll > StatusLine > CursorOnly. Collapse rules: multiple Scroll events coalesce (earliest old_first, latest new_first); any Lines or Full suppress Scroll; exclusive Scroll preserved. Add DELTA_SCROLL metric + tests (merge, suppression by Lines, exclusive preservation, precedence, decision effective still Full).
4. Status segment model: add `StatusSegment` enum + `compose_status` → `format_status`; preserve exact output; add regression test comparing to legacy formatter.
5. Extract file IO helpers (`open_file(path)`, `write_file(state)`) into `core-actions::io_ops`; dispatcher delegates; tests updated.
6. Introduce `EditorModel` wrapper (contains `EditorState`); adapt `ox-bin` usage; no behavior changes; rustdoc updates.
7. Config re-clamp on resize: add `Config::recompute_after_resize(height)`; call in resize path; if margin changes, mark status or scroll as needed (Full for now); tests.
8. Key normalization: remove `KeyCode::Colon`; add `normalize_keycode`; adjust input thread & translator tests; purge dual handling.
9. Metrics grouping: `RenderDeltaMetrics` with increment + snapshot API; replace free statics; adjust tests.
10. Semantic delta scenario tests: new integration test driving sequences (cursor move, insert, scroll) asserting semantic variant.
11. Render perf timing hook: capture last render duration (nanoseconds) into atomic; add getter & unit smoke test (non-zero after a render).
12. Doc sweep: update affected module headers (render engine, dispatcher IO section, events key normalization); link Phase 3 prerequisites.
13. Final Gate: run fmt/clippy/tests; mark Steps 0–13 Done here; commit closure.

## 6. Exit Criteria

- All steps committed with template messages referencing Refactor R2 / Step N.
- No change to visual output or user interaction semantics.
- All existing + new tests pass; status line output identical pre vs post segment refactor.
- `KeyCode::Colon` removed (or decisively normalized) with translator tests green.
- Scroll operations produce `Scroll` semantic (visible only in logs/tests) while effective remains full.
- Metrics accessible via new struct API.
- Engine abstraction compiles and is used by `ox-bin`.
- Plan file updated marking every step Done at final commit.

## 7. Non-Goals / Deferred

- Implementing partial painting logic.
- Introducing multiple views or buffer tabs UI.
- Async runtime changes for IO.
- Performance optimization beyond instrumentation hook.

## 8. Follow-Up After R2 (Targets for Phase 3)

- Activate partial rendering path for `Lines` & `Scroll` semantics.
- Introduce diff-based undo snapshot representation.
- Add secondary event sources (timer heartbeat, config watcher) using normalized key/event pipeline.
- Expand status segments (git branch placeholder, diagnostics counts).

## 9. References

- Phase 1, Phase 2 design docs.
- Refactor R1 document for extraction patterns.
- Current `core-render` and `ox-bin` render integration code.

## 10. Progress Log

(Will be updated as steps complete.)

- [x] Step 0 – Plan scaffold (this document)
- [x] Step 1 – RenderEngine extraction
- [x] Step 2 – Cursor overlay separation
- [x] Step 3 – Scroll delta variant
- [ ] Step 4 – Status segment model
- [ ] Step 5 – IO helper extraction
- [ ] Step 6 – EditorModel wrapper
- [ ] Step 7 – Config re-clamp on resize
- [ ] Step 8 – Key normalization removal Colon
- [ ] Step 9 – Metrics grouping struct
- [ ] Step 10 – Semantic delta scenarios tests
- [ ] Step 11 – Render perf timing hook
- [ ] Step 12 – Documentation sweep
- [ ] Step 13 – Final gate & closure

---
(Plan will be updated incrementally; each completed step commits with the defined template and updates this file.)
