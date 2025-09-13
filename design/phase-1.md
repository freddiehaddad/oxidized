# Phase 1: Basic Editing & Cursor Foundations

## 1. Objective

Introduce fundamental text editing capabilities and cursor/motion primitives while preserving the event-driven architecture. This phase delivers Normal + Insert modes with basic motions, insertion, deletion, a minimal undo/redo stack, and a visible command/status line (command echo). Rendering remains full-frame (no diff optimization yet) but now reflects cursor position and updates after text mutations.

## 2. Scope

In Scope:

* Add `Mode::Insert` and mode switching via `i`, `Esc`.
* Cursor tracking with grapheme-cluster awareness (line, byte offset at grapheme boundary) and visual column computation via cluster width.
* Basic Normal mode motions: `h`, `j`, `k`, `l`, `0`, `$`, `w`, `b` (word motions naive; ASCII or first-char classification). Motions traverse grapheme clusters, not raw codepoints.
* Insertion in Insert mode: printable characters (entire grapheme cluster as typed), newline (Enter), backspace deletion of previous full grapheme (including emoji / combining sequences).
* Simple deletion in Normal mode: `x` (delete char under cursor).
* Minimal undo/redo stack (linear) using snapshot or diff-based approach (choose simplest working: rope snapshots with coalescing for bursts of insert in Insert mode).
* Command/status line with live echo of `:` commands while typing (support `:q` still) and show current mode & cursor coordinates.
* Event additions needed to represent cursor updates & undo/redo triggers.
* Tests: buffer edit operations, cursor boundary conditions, undo/redo, motion correctness (selected subset), command echo display logic.

Out of Scope (Deferred):

* Visual mode, Operator-pending commands beyond `x`.
* Multi-window/split management.
* Persistent macros, registers, yanking/pasting.
* Multi-cursor or selection.
* Full Unicode word boundary correctness (UAX #29 nuanced cases, extended pictographic sequences) beyond naive cluster grouping.
* Syntax highlighting, Tree-sitter, folding.
* Performance diff rendering (still full redraw).
* Search (`/`), replace, join, indentation logic.
* Multi-buffer editing (still single active buffer, though mutations allowed).
* Auto-repeat key handling for held keys (still filtered to Press events).

## 3. Architectural Touchpoints

Existing crates extended rather than adding new ones:

* `core-events`: Add motion & edit events; possibly CursorMoved event; Introduce Undo/Redo events.
* `core-state`: Add `Cursor` struct, integrate into `EditorState`; track mode; maintain undo stack.
* `core-text`: Add mutable editing operations (insert_char, insert_newline, delete_at). Possibly small wrapper around ropey mutation APIs.
* `core-render`: Render cursor position (highlight cell or rely on terminal cursor MoveTo). Add status/command line composition.
* `core-input`: Map raw key events.
* `core-actions`: Translate raw key events + lightweight contextual state (mode, pending command) into semantic `Action` values (motions, edits, mode changes, undo/redo, command input).
* `core-terminal`: Provide API for setting terminal cursor position (if not already) for direct cursor placement.
* `ox-bin`: Expand event loop to handle new event variants, route motions/edit operations, update status line, manage undo stack triggers.

No new crate is strictly required; future phases (syntax, LSP, plugins) will introduce new crates.

## 4. Event Additions

Extend `Event` / `InputEvent` mapping indirectly via a translation layer. Initially we interpreted `InputEvent::Key` directly inside the main loop; we will now (still within Phase 1) introduce an explicit `Action` abstraction and translation function to future‑proof undo coalescing, macro recording, and background producers.

Add (logical) action set (not necessarily new enum variants yet, but documented for clarity):

* Motions: Left, Right, Up, Down, LineStart, LineEnd, WordForward, WordBackward.
* Edits: InsertChar(char), InsertNewline, Backspace, DeleteCharUnder.
* Mode changes: EnterInsert, LeaveInsert.
* Undo, Redo (map to `u` and `Ctrl-R`).
* CursorMoved (implicit after motion/edit) used internally to trigger render.

Action Abstraction (Added mid‑Phase 1):

* `Action` enum (Motion, Edit, ModeChange, Undo, Redo, CommandInput, CommandExecute, Quit) sits between raw input and state mutation.
* Pure translator: key + state + pending_command -> `Option<Action>`.
* Dispatcher applies action -> state delta, returns dirty flag.
* Render scheduler stub decides when to flush (currently immediate full redraw; future diff integration hooks here).
* Breadth‑first principle preserved: behavior identical after each incremental commit.
* Ordering note: Dispatcher (Action -> state) implemented before async channel migration to minimize simultaneous behavioral + concurrency changes. Async channel migration (Task 9.6) now complete: main loop awaits `tokio::mpsc` events (still single producer) with identical user-visible behavior, enabling future background action producers without architectural rewrite. Render scheduler stub (Task 9.8) not yet extracted – current dirty tracking inline.
* Update: Render scheduler stub (Task 9.8) now introduced as a minimal `RenderScheduler` struct encapsulating a dirty flag and providing `mark_dirty` / `consume_dirty`. Still triggers immediate full-frame redraw; future diff/debounce logic will extend this API without changing call sites.

## 5. Data Model Changes

`core-state` additions:

* `Cursor { line: usize, byte: usize }` — line index and byte offset (always at a grapheme boundary). Visual column derived dynamically via grapheme iteration.
* `EditorState` fields:
  * `cursor: Cursor`
  * `mode: Mode` now includes `Insert`.
  * `undo_stack: Vec<EditSnapshot>`; `redo_stack: Vec<EditSnapshot>`.
* `EditSnapshot` (struct) capturing minimal info to restore prior text:
  * Approach: store entire buffer rope clone for simplicity (Phase 1) with coalescing rule: while in Insert mode, consecutive InsertChar within a short time window (< N ms) merge into one snapshot; leaving Insert mode always forces a snapshot boundary. (Performance acceptable for small early files; will optimize later.)

`core-text` additions:

* `insert_grapheme(line, byte, &str)` returning new byte offset & visual info.
* `insert_newline(line, byte)` split line at grapheme boundary.
* `delete_grapheme_before(line, byte)` (backspace) & `delete_grapheme_at(line, byte)` (Normal `x`).
* Helpers: previous_boundary, next_boundary, visual_col(line, byte), clamp.

## 6. Steps

0. (DONE) Unicode Foundations: Added `unicode-segmentation` & `unicode-width`, grapheme helper module (`core-text::grapheme`) providing boundaries, visual column, width, and basic word classification. All cursor logic and future mutations will rely on byte offsets restricted to grapheme boundaries.

1. Add `Mode::Insert` variant and (INITIAL) grapheme-aware `Cursor` struct to `core-state` with constructor & clamp helpers. (Refactored: relocated to `core-text` as plain `Position` before adding motions to keep text-centric concerns co-located. Future richer cursor/multi-selection logic will likely live in a dedicated `core-cursor` crate.)
2. Implement grapheme mutation APIs in `core-text` (leveraging ropey insert & remove). Provide safe wrappers that adjust positions.
3. (Hybrid 4a COMPLETE) Snapshot infrastructure: `EditSnapshot`, capped undo/redo stacks, push/restore API, guard logic for Insert coalescing (begin/end run) plus unit tests validating single-run snapshot capture, redo clearing, and stack cap.
4. (Hybrid 5a COMPLETE) Minimal Insert subset: enter Insert (`i`), insert printable graphemes (no newline/backspace yet), Esc back to Normal. Uses snapshot API from 4a: first insertion triggers pre-edit snapshot; Esc sets coalescing boundary. Dynamic mode indicator added.
5. (Hybrid 4b COMPLETE) Wire Undo/Redo actions (`u`, `Ctrl-R`) to stacks now that minimal Insert exists. Test multi-character coalesced undo/redo.
6. (NEW 4c PLANNED) Snapshot mode semantics refinement: introduce `SnapshotKind` so edit undos/redos do not restore Insert mode (e.g. `iabc<Esc>u` leaves Normal). Tag snapshots; ignore mode for `SnapshotKind::Edit` on restore; add tests.
7. (Hybrid 5b) Complete Insert mechanics: newline insertion, backspace (grapheme delete / line join), finalize simple coalescing boundaries (Esc or newline). Update tests.
8. Extend rendering: draw buffer; move terminal cursor to current logical position (translate to (x,y)) before flush; compose status/command line: `[NORMAL|INSERT]  Ln {line+1}, Col {col+1}  :{pending_command}`.
9. Enhance input handling in main loop:
   * Normal mode key mapping: motions (h/j/k/l/0/$/w/b), `i` -> enter Insert, `x` -> delete char under cursor, `u` -> undo, `Ctrl-R` -> redo, `:` -> command-line start.
   * Insert mode: printable -> insert char; Enter -> newline; Backspace -> delete previous char (col>0 or join lines); Esc -> leave Insert, finalize snapshot.
10. Implement word motion helpers (naive: alphanumeric+underscore cluster sequences vs others) using grapheme iteration. (Completed alongside basic motions earlier to keep motion module cohesive.)
11. Integrate undo stack coalescing refinement: sequence of plain character inserts inside Insert mode forms a single snapshot (boundary = Esc or newline). Time-based merging deferred.
12. Add command-line echo: maintain `pending_command` state; render at status line; execute on Enter (still only `:q` recognized this phase).
13. Update tests: buffer mutation (insert/delete/newline), cursor motion clamping, undo/redo restoring previous text, status line string composition.
14. Add tracing spans for `edit_op`, `motion`, and `undo_cycle`.
15. Update design docs & README to reflect new capabilities and hybrid ordering (4a -> 5a -> 4b -> 5b).
16. Run build, clippy, fmt, tests; ensure clean exit still works.

### Refactor Checkpoint R1 (Inserted Mid-Phase)

Before proceeding to Task 7 (status / command line enhancements), we introduce **Refactor Checkpoint R1** (`design/refactor-r1.md`). This checkpoint extracts overgrown responsibilities from `main.rs`, replaces the boolean Insert run flag with an enum, adds a command line state struct, and moves scheduling/dispatcher/status formatting into dedicated modules. No user-visible features change; this is a structural hardening pass to preserve breadth-first momentum and prevent technical debt from compounding ahead of splits, diff rendering, and LSP integration.

Refer to the separate plan for the ordered commit breakdown. Task 7 will resume on top of the cleaner substrate once R1 exits.

## 7. Exit Criteria

* Build + clippy + fmt check all pass (`cargo build`, `cargo clippy -- -D warnings`, `cargo fmt --all -- --check`).
* Enter Insert mode with `i`, type text, see it appear in buffer; Esc returns to Normal with cursor adjusted (one-left typical Vim behavior optional: adopt: keep cursor on last inserted char unless at line end; decide simple: leave as rope insertion position for Phase 1, adjust later).
* Motions `h j k l 0 $ w b` update cursor without panics and stay in bounds.
* `x` deletes the character under the cursor.
* Backspace in Insert mode deletes previous full grapheme cluster or joins lines when at line start.
* Newline insertion splits line correctly and moves cursor to line start of new line.
* Undo (`u`) reverts last mutation; Redo (`Ctrl-R`) reapplies it.
* Command/status line shows current mode, cursor (Ln/Col), and live `:` command input.
* `:q` still exits cleanly; terminal always restored on panic or normal exit.
* All new public APIs documented.

## 8. Telemetry / Logging

* Add spans: `motion`, `insert`, `delete`, `newline`, `undo`, `redo`, `grapheme_nav`.
* Debug logs for snapshot push/pop (size of rope, stacks lengths).
* Trace-level logging for each key translated into an action.

## 9. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Performance overhead of full-rope snapshots | Acceptable for small files; document and schedule diff-based undo optimization later. |
| Cursor desync after edits | Centralize all text mutations through `core-text` API returning updated cursor. Add tests. |
| Word motion edge cases (Unicode) | Naive ASCII now; add TODO + tests for boundaries. |
| Undo stack memory growth | Cap stack length (e.g., 200 snapshots) and drop oldest with log warning. |
| Insert coalescing complexity | Start with boundary-based (Esc/newline) only; time-based merging deferred. |
| Rendering flicker with cursor move | Still full redraw; acceptable Phase 1; optimize later. |
| Inconsistent mode indicator after panic | Guard + panic hook already handle restoration; ensure mode not required for restoration path. |

## 10. Deferred Items

* Grapheme caching / performance optimization (current approach recalculates per motion).
* Time-based insert coalescing.
* Multi-line operators (dd, dw, etc.).
* Visual/Operator-pending modes.
* Registers, yanking/pasting, clipboard integration.
* Multi-buffer & window management.
* Persistent undo tree (branching).
* Search, replace, incremental highlight.
* Diff-based rendering & damage tracking.
* Tree-sitter syntax highlighting.
* LSP/DAP/git integration.
* Plugin host (WASM) interaction.

## 11. References

* Phase 0 design.
* Ropey editing API docs.
* Vim motion semantics (subset) for comparison.

## 12. Notes

* Keep architecture event-driven: key/input -> translate -> Action -> dispatch -> (mark dirty) -> render.
* No polling introduced; input thread remains blocking; channel migration to `tokio::mpsc` enables async producers later without redesign.
* Maintain press-only key filtering for determinism; revisit auto-repeat after diffed rendering arrives.
* Simplicity over optimality: full snapshots and full redraws.
* 9.9 (Documented, deferred): Multi-producer readiness & render diff hook. Additional async producers (config watcher, timers, future LSP, plugin host) will feed actions through either an added `Event::Action(Action)` variant or a parallel action channel merged with `tokio::select!`. Decision intentionally deferred until first new producer to avoid dormant enum variants. `RenderScheduler` will evolve from a boolean dirty flag to collecting `RenderDelta` values (Full, Lines(range), Status, CursorOnly) merged into a `Damage` set consumed by a future diff-capable renderer. Current full-frame redraw semantics preserved until structured deltas exist.
* 9.3 Rescope: Only motion helpers (`apply_motion`, `apply_vertical_motion`) extracted now; dedicated `apply_edit` / `apply_command_input` helpers will be introduced when Insert mode editing & undo stack (Tasks 4–6) are implemented to prevent premature abstraction and churn.
* Hybrid Ordering Rationale: Landing snapshot infrastructure (4a) before user-visible edits (5a) isolates API design risk. Minimal Insert (5a) then allows early verification of snapshot correctness. Wiring undo (4b) afterward prevents prematurely baking in coalescing semantics. Completing Insert (5b) finally exercises newline/backspace edge cases against a stabilized snapshot API.
