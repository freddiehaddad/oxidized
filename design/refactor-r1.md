# Refactor Checkpoint R1: Structural Extraction & Readiness

## 1. Objective

Stabilize Phase 1 foundations before expanding UX (Task 7+) by extracting overgrown responsibilities from `ox-bin/src/main.rs`, removing incidental `unsafe`, and introducing minimal abstractions that future phases (splits, diff rendering, LSP, macros, plugins) can build on without large rewrites. No new user-visible features; binary remains runnable after each step.

## 2. Scope (In / Out)

In Scope:

* Extract dispatcher logic out of `main.rs` (pure function + types) without semantic change.
* Remove `unsafe` motion wrappers by restructuring borrow pattern.
* Introduce `CommandLineState` struct (replaces raw `pending_command` string) – same behavior.
* Move `RenderScheduler` into `core-render::scheduler` module.
* Add status line formatter module (`core-render::status`) returning a status string.
* Introduce `InsertRun` enum (replaces boolean) in `core-state` (inactive / active { started_at, edits }). Behavior unchanged (Esc/newline boundaries still only triggers) but struct supports future time-based coalescing.
* Add lightweight motion & translation spans (`motion`, `translate_key`) for telemetry completeness.
* Add bounded event channel constant + TODO for backpressure policy (still unbounded until second producer lands – we codify decision and single change site).
* Provide `ActionObserver` trait + no-op list hook in main loop (enable future macro/recording without loop surgery).
* Document viewport abstraction placeholder (`Viewport { first_line, height }`) and integrate a fixed instance (no scrolling yet).

Out of Scope (Deferred to later phases):

* Diff rendering / damage tracking implementation.
* Time-based insert coalescing trigger.
* Undo strategy rewrite (log/diff representation).
* Multi-buffer and window layout engine.
* Plugin host, LSP client wiring.

## 3. Architectural Changes Summary

| Concern | Current | Target (R1) | Benefit |
|---------|---------|-------------|---------|
| Dispatcher | Inline in `main.rs` | `core-actions::dispatcher` (or new crate) | Testable isolation, reuse |
| Motion helpers | `unsafe` reborrow dance | Direct safe calls | Remove unsafe surface |
| Command line state | Raw `String` + sentinel backspace | `CommandLineState` struct | Extensible (history, completion) |
| Render scheduler | Local struct in binary | `core-render::scheduler` | Encapsulated evolution to diff |
| Status line | Inline string building | Dedicated formatter | Separation of presentation |
| Insert run flag | `bool insert_run_active` | `InsertRun` enum | Future time-based boundary support |
| Spans coverage | Edits + undo/redo | + motion + translation | Better tracing filters |
| Event channel | Unbounded directly | Constant & doc for planned bounded swap | Single future change site |
| Observer hook | None | `ActionObserver` trait & vector | Macro/plugin extensibility |
| Viewport | Implicit (line 0 origin) | Stub struct used in render | Prepares scrolling/splits |

## 4. Step Breakdown (Each = One Commit)

1. Extract Status Line Formatter (COMPLETED)
   * Implemented `core-render::status::{StatusContext, build_status(&StatusContext) -> String}`.
   * `render()` now constructs a `StatusContext` (mode, line, col, command activity) and calls `build_status`.
   * Inline formatting removed; unit tests added for Normal/Insert with and without command buffer.

2. Introduce `CommandLineState` (COMPLETED)
   * Added `CommandLineState` in `core-state` holding a single `buf: String`.
   * Activity inferred via `is_active()` (checks leading ':'); no separate `active` boolean stored.
   * Methods: `begin`, `push_char`, `backspace`, `clear`, `buffer`, `is_active`.
   * Main loop and dispatcher updated to use `state.command_line` instead of local `pending_command`.
   * Behavior remains identical; tests updated accordingly.

3. Remove Unsafe Motion Wrappers (COMPLETED)
   * Removed `apply_motion` / `apply_vertical_motion` which used raw pointer reborrow + `unsafe`.
   * Replaced with `apply_horizontal_motion` and `apply_vertical_motion_safe` performing:
     * Copy of `state.position` into a local mutable `pos`.
     * Invoke motion with a shared `&Buffer` reference (no aliasing of mutable state).
     * Write back updated `pos` into `state.position`.
   * Eliminated all `unsafe` in motion path; no semantic changes (verified by existing tests).
   * Preserves breadth-first behavior while enabling future extraction of motion logic without unsafe blocks.

4. Move `RenderScheduler`
   * New module `core-render::scheduler` with identical API (`mark_dirty`, `consume_dirty`).
   * Replace struct in `main.rs` with imported type.

5. Extract Dispatcher Module
   * Create `core-actions::dispatcher` (or `core-dispatch` crate if size grows) exporting `dispatch(Action, &mut EditorCtx) -> DispatchResult`.
   * Define `EditorCtx` bundling `EditorState`, `CommandLineState`, `sticky_visual_col`.
   * Adjust tests to use new API.

6. Insert Run Enum (PENDING)
   * Replace `insert_run_active: bool` in `EditorState` with:

     ```rust
     pub enum InsertRun { Inactive, Active { started_at: std::time::Instant, edits: u32 } }
     ```

   * Update begin/end helpers; existing semantics preserved.
   * Add rustdoc & simple tests.

7. Add Motion & Translation Spans (PENDING)
   * `translate_key` -> span `translate_key` at trace level.
   * Dispatcher motion arm spans: `motion` with `kind=?kind`.

8. Action Observer Hook (PENDING)
   * Define `ActionObserver` trait.
   * Add `Vec<Box<dyn ActionObserver>>` (empty for now) in main loop and call `on_action(&action)` prior to `dispatch`.

9. Viewport Stub (PENDING)
   * Introduce `Viewport { first_line: usize, height: usize }`.
   * `render()` calculates visible lines using viewport instead of implicit 0.
   * No scrolling yet; tests confirm unchanged output baseline.

10. Channel Policy Documentation (PENDING)
    * Define `EVENT_CHANNEL_CAP: usize` constant (unused for now) + comment explaining deferred bounded migration (link to Phase 2 plan).

## 5. Exit Criteria

* All steps compile and pass tests after each commit.
* No behavior regressions (snapshot tests still green).
* No `unsafe` code remains in motion path.
* Dispatcher no longer resides in `main.rs` (file size materially reduced).
* Status formatting logic fully separated; test covers typical modes (Normal/Insert) and command line active/inactive states.
* Insert run tests pass with new enum.
* Additional spans appear in trace logs (`motion`, `translate_key`).

## 6. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Hidden coupling in dispatcher extraction | Introduce `EditorCtx` wrapper to avoid many parameter changes |
| Insert run enum introduces regressions | Add regression tests replicating prior boolean behavior |
| Timeline creep (feature stagnation) | Limit scope: strictly no new end-user features |
| Over-abstraction early | Keep modules minimal; no trait indirection unless clearly needed |

## 7. Deferred (After R1)

* Bounded channel activation & drop strategy.
* Time-based coalescing using `started_at` + edit spacing threshold.
* Macro recorder using `ActionObserver`.
* Diff rendering replacing full-frame redraw.

## 8. References

* Phase 1 design doc for original scope.
* Undo refinement notes (SnapshotKind rationale).

## 9. Notes

Breadth-first preserved: each step is a behavior-neutral refactor. R1 completes before resuming Phase 1 Task 7 feature work so that upcoming status line enhancements land on a cleaner substrate.
