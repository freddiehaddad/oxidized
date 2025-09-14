# Phase 1 Task Breakdown (Editing + Grapheme-Aware Cursor)

## Legend

- [ ] = not started
- [~] = in progress
- [x] = complete

Keep changes incremental; commit after each numbered block is green (build + clippy + focused tests).

---

## 0. Unicode Foundations

**Goal:** Safe grapheme-aware cursor positioning & deletion so we never split emoji/combining clusters.

Tasks:
0.1 Add deps: `unicode-segmentation`, `unicode-width` to workspace.
0.2 Module `core-text::grapheme` (or inline for now) providing:
     - iter(line: &str) -> iterator of &str clusters
     - prev_boundary(line, byte) -> byte
     - next_boundary(line, byte) -> byte
     - visual_col(line, byte) -> usize (sum widths of clusters < byte)
     - cluster_width(&str) -> usize (unicode_width)
0.3 Tests: single emoji, family emoji (👨‍👩‍👧‍👦), combining mark (é), CJK full-width char, mixed ASCII.
0.4 Backspace & delete operate on whole cluster.
Acceptance: Cursor never lands inside cluster; deletion removes entire cluster; widths consistent.

---

## 1. State & Modes

1.1 Add `Mode::Insert`.
1.2 Add `Cursor { line, byte }` + constructors + clamp helpers.
1.3 Integrate into `EditorState`.
Tests: new state initializes at (0,0), mode Normal.

---

## 2. Text Mutation APIs

2.1 `insert_grapheme` (string slice, usually 1 cluster).
2.2 `insert_newline` splitting rope at byte.
2.3 `delete_grapheme_before` / `delete_grapheme_at`.
2.4 Helpers for joining lines on backspace at start.
Tests: middle-of-line insert, newline split, delete at end, join lines.

---

## 3. Cursor Motions  

**Status:** [x] complete (2025-09-13)

- [x] 3.1 Basic motions h/l (prev/next boundary)
- [x] 3.2 j/k (line +/- with sticky visual column)
- [x] 3.3 0 / $ (line start / line end at last cluster boundary)
- [x] 3.4 Word motions w/b (naive first-char classification)
- [x] Tests: horizontal boundaries, vertical sticky column, word motion punctuation & cross-line

Notes: Word motions implemented with a simplified single-step algorithm (naive ASCII/underscore classification). Future improvement (UAX #29) deferred per design.

---

## 4. Undo/Redo (Snapshot)

**Status:** [x] 4a complete / [x] 4b complete / [x] 4c complete

Goal: Land snapshot infrastructure (4a) before wiring undo keys (4b) to stabilize API, then complete coalescing semantics after minimal Insert exists.

Checklist (Hybrid Sequencing):

- [x] 4.1 (4a) Define `EditSnapshot` struct capturing: full rope clone, cursor position, mode (optional, for future mode-aware undo). Simplicity first: `Arc<Rope>` or plain clone (plain clone acceptable Phase 1).
- [x] 4.2 (4a) Add `undo_stack`, `redo_stack` to `EditorState` (or dedicated `UndoHistory` helper) with MAX_DEPTH constant (e.g. 200) and drop-oldest logic + debug log.
- [x] 4.3 (4a) Implement core APIs: `push_snapshot(state)`, `restore_snapshot(state, snapshot)`, `undo(state) -> bool`, `redo(state) -> bool` (return dirty flag).
- [x] 4.4 (4a) Guard logic: push pre-edit snapshot only if not already in an active insert run (Insert run tracking boolean or counter in state).
- [x] 4.5 (4a) Unit tests: single insert sequence captured once; multiple snapshots capped; redo cleared after new edit.
- [x] 4.6 (4b) Wire `Action::Undo` (`u`) and `Action::Redo` (`Ctrl-R`) in dispatcher after minimal Insert (5a) merged. (Physical key translation for `Ctrl-R` was accidentally omitted initially and added during Final Gate; see Correction 13.C1. Design intent unchanged.)
- [x] 4.7 (4b) Integration tests: perform inserts -> undo -> redo path; ensure cursor restored.
- [x] 4.8 (4b) Coalescing logic (boundary-based): character inserts while in Insert mode coalesce until Esc or newline (newline added in 5b). Implementation: track `coalescing_active` flag; Esc/newline toggles off.
- [x] 4.9 (4b) Snapshot push for Normal mode edits (`x`) always discrete (implemented in Task 6).
- [x] 4.10 (4b) Logging: trace each snapshot push/pop with stack sizes.
- [x] 4.11 (Deferred) Time-based coalescing placeholder comment (no timers yet) referencing future diff rendering.
- [x] 4c Snapshot mode semantics refinement: introduce `SnapshotKind`; edit undos do not restore Insert mode. Tests: `iabc<Esc>u` leaves Normal; redo restores text but remains Normal. Update design docs.

Acceptance:

- Undo reverts entire multi-character Insert run (pre-Esc) in one step.
- Redo reinstates identical text & cursor.
- Redo stack cleared on new edit post-undo.
- Stack never exceeds MAX_DEPTH.

---

## 5. Insert Mode Mechanics

**Status:** [x] 5a complete / [x] 5b complete (newline, backspace, boundaries, logging, rustdoc)

Goal: Introduce a minimal Insert experience (5a) to validate snapshot infra, then expand to full mechanics (5b) including newline/backspace and coalescing boundaries.

Checklist:

- [x] 5.1 (5a) Map `i` -> `Action::ModeChange(EnterInsert)`; ensure any pending coalescing run is ended before switching.
- [x] 5.2 (5a) Printable grapheme insertion: translation maps visible chars to `Action::Edit(InsertChar(cluster))` when in Insert mode.
- [x] 5.3 (5a) Dispatcher inserts grapheme, marks dirty, sets/maintains an `insert_run_active` flag (begins with first inserted char after entering Insert).
- [x] 5.4 (5a) Esc handling: translate to `Action::ModeChange(LeaveInsert)`; dispatcher ends insert run (coalescing boundary) without modifying cursor position (Phase 1 simplicity).
- [x] 5.5 (5a) Tests: type "abc" Esc => one undo restores empty buffer; redo replays all.
- [x] 5.6 (5b) Enter key -> newline insertion (`Action::Edit(InsertNewline)`), ends coalescing run (boundary) and starts a fresh run after next char.
- [x] 5.7 (5b) Backspace -> delete previous full grapheme or join with previous line (cursor moves to join point).
- [x] 5.8 (5b) Ensure backspace within a run does NOT prematurely end coalescing; newline or Esc only.
- [x] 5.9 (5b) Cursor adjustment rules after newline/backspace validated (stay at start of new line after newline; at join offset after join).
- [x] 5.10 (5b) Tests: newline mid-line split; backspace at start of line joins; multi-grapheme clusters (emoji) deleted as single unit.
- [x] 5.11 (5b) Logging/tracing: insert, newline, backspace spans.
- [x] 5.12 (5b) Rustdoc comments for Insert semantics & coalescing boundaries.

Acceptance:

- Minimal Insert (5a) supports multi-character typing + Esc + single undo.
- Full Insert (5b) includes newline & backspace cluster correctness, with undo boundaries at Esc/newline only.
- Backspace never splits a grapheme cluster; joining lines preserves subsequent text.

---

## 6. Normal Mode Editing

**Status:** [x] complete (2025-09-13)

- [x] 6.1 `x` -> delete_grapheme_at (no-op at end).
- [x] 6.2 Snapshot capture before each `x` (discrete snapshots, simplest approach).
Tests: multiple x + undos.

---

## 7. Command / Status Line

7.1 Track `pending_command` across modes.
7.2 Render line: `[NORMAL|INSERT] Ln X, Col Y :<cmd>` (only show colon section when active).
7.3 Echo `:q` and preserve existing quit behavior.
Tests: building formatted status string.

### Task 7 Enhancement Breakdown (2025-09-14)

Refactor R1 introduced a minimal `CommandLineState` plus `Action::{CommandInput,CommandExecute}`. Task 7 now formalizes and hardens the command / status line path so later phases (history, completion, multi-command parsing) evolve without touching the main loop.

Substeps (each intended to land in an independent commit – breadth first, always runnable):

- [x] 7.1 Action Enum Refinement: Replace sentinel uses with explicit variants: `CommandStart`, `CommandChar(char)`, `CommandBackspace`, `CommandCancel`, `CommandExecute(String)`. (Done 2025-09-14)
- [x] 7.2 Translator Unification: All colon handling moved into `translate_key`; main loop no longer special‑cases `:`. (Done 2025-09-14)
- [x] 7.3 Status Formatting Clarification: Single visible colon; internal buffer keeps leading ':' sentinel; tests updated. (Done 2025-09-14)
- [x] 7.4 Dispatcher Parsing Stub: Minimal parse for `:q` triggers quit; other commands clear line. (Done 2025-09-14)
- [x] 7.5 Command Line Editing Tests: Added translation and execution tests (start, char, backspace, cancel, execute). (Done 2025-09-14)
- [x] 7.6 Rustdoc & Design Sync: Updated docs (this section) & status module; colon variant regression documented below. (Done 2025-09-14)

Colon Key Regression (Postmortem / Decision):
The input layer emitted `KeyCode::Colon` while the translator only matched `KeyCode::Char(':')`, preventing command mode activation. Short-term fix: translator now matches both (`KeyCode::Char(':') | KeyCode::Colon`). This is an intentional breadth‑first patch; a later consolidation will likely remove the dedicated `Colon` variant or introduce a normalization shim. Tracking note added; no architectural impact.

#### Rationale

- Explicit variants improve clarity and eliminate sentinel coupling between translator and dispatcher.
- Stripping the internal sentinel before rendering aligns with typical editors (one visible colon).
- Early parse stub provides a stable seam for future multi-word commands (e.g. `:w filename`).

#### Edge Cases Considered

- Repeated `:` while command inactive should not stack colons – only first starts command mode.
- Backspace when only `":"` present should cancel command mode (buffer cleared).
- Escape while command active should cancel (no quit) and redraw status line without command section.
- Enter on empty `":"` should just clear (no quit).

Commit Template: `feat(phase1-task7-stepX): <summary>` where X = substep number above.

Upon completion all checkboxes above will be `[x]` and this section remains as historical record for Task 7 implementation decisions.

---

## 8. Rendering & Cursor Placement

Status: [x] 8.1 complete / [x] 8.2 complete / [ ] 8.3 deferred

8.1 Compute visual column (sum widths) for cursor. (Implemented earlier for status line; formalized with explicit mixed-sequence test `visual_col_mixed_sequences` covering emoji, combining marks, CJK, and family emoji. Test asserts non-decreasing columns and cluster lower bound.)
8.2 Hardware cursor placement implemented (2025-09-14): after full frame draw (`Renderer::render`) we compute grapheme-aware visual column of the active cursor using `grapheme::visual_col` and move the terminal cursor (`crossterm::cursor::MoveTo`) if within the visible text area (excludes reserved status line). Implemented directly in `ox-bin/src/main.rs` to keep backend abstraction minimal in Phase 1; later phases may lift this into a higher-level renderer API when diff rendering lands. Ordering (draw first, then place cursor) chosen to minimize flicker. Viewport currently static (offset 0) but code comments reference future offset use.
8.3 Optionally highlight cell (defer): Decision: rely on terminal's native cursor for Phase 1. A visual highlight layer (inverse/video or color) deferred until diff rendering exists to avoid redundant full-frame styling.
Manual test checklist (executed ad-hoc after 8.2 implementation):
     - Single-width ASCII typing: OK.
     - Wide emoji (😀) alignment: Column consistent with status col.
     - Combining mark sequences (é) advance cursor one cell: OK.
     - CJK characters alignment stable across motions: OK.
     - Family emoji (👨‍👩‍👧‍👦) left/right motions do not desync: OK.

Rationale: Reuse of the same visual column function for both status reporting and hardware placement ensures continuous validation. Deferring highlight avoids premature styling complexity—native cursor is sufficient for baseline UX.

---

## 9. Event Loop Integration

9.1 Key→action mapping per mode (press-only).
9.2 Ensure render after every motion/edit.
9.3 Separate small helpers for motion vs edit vs command input.
9.4 Introduce `Action` enum (Motion, Edit, ModeChange, Undo, Redo, CommandInput, CommandExecute, Quit).
9.5 Key translation function (pure) `translate(InputEvent + state + pending_command) -> Option<Action>`.
9.6 Migrate channel to `tokio::mpsc` and async loop (await actions) – still single producer.
9.7 Dispatcher function `apply_action(Action, &mut EditorState, &mut pending_command) -> bool /*dirty*/`.
9.8 Render scheduler stub (tracks dirty flag; still immediate full redraw).
9.9 TODO (deferred): additional action producers (config watcher, timers) & diff render integration hook.

Status: 9.1 & 9.2 COMPLETE (2025-09-13) for Normal mode motion keys with status line line/column display.

Checklist:

- [x] 9.1 Normal mode motion key mapping (h j k l 0 $ w b, arrows) wired.
- [x] 9.2 Render occurs after each handled input (motions currently).
- [ ] 9.3 (Moved to Refactor Checkpoint R1) Extraction of broader helpers (edit/command) along with dispatcher & scheduler relocation. See `design/refactor-r1.md`.
- [x] 9.4 Action enum introduced (`core-actions` crate) & compiled.
- [x] 9.5 Translation function skeleton (`translate_key`) added (no wiring yet).
- [x] 9.6 Async tokio channel + loop.
- [x] 9.7 Dispatcher & dirty flag (implemented ahead of 9.6 for lower-churn refactor).
- [x] 9.8 Render scheduler stub.
- [x] 9.9 Deferred multi-producer & diff hook documented.

Notes: Dispatcher landed before async channel migration (9.6) to reduce simultaneous complexity. Initial render bug fixed by performing a first-frame render at startup before event loop (ensures visible buffer without input). Render scheduler stub (9.8) implemented and extracted into `core-render::scheduler` during Refactor R1. Viewport stub and `ActionObserver` hook also added via R1 without altering user-visible behavior.

Notes: Replaced temporary unsafe raw pointer borrowing with safe helper functions (`apply_motion`, `apply_vertical_motion`) before proceeding to Undo/Redo to avoid accruing technical debt. Introduced new `core-actions` crate for semantic intent separation (motions/edits/mode changes) per modularity goal.

9.9 Documentation (Deferred Implementation):
Multi-producer architecture will permit additional asynchronous sources of `Action` beyond the input thread: configuration watcher, timers (cursor blink / debounce), future LSP client, plugin host, diagnostics generators. These producers will communicate via additional async tasks feeding a unifying layer. Two candidate wiring options retained: (A) extend `Event` with an `Action(Action)` variant; (B) introduce a parallel `action_rx` and merge using `tokio::select!`. Current choice is to defer adding a new enum variant until the first non-input producer lands to avoid unused code. Render diff hook: the current `RenderScheduler` exposes a single `mark_dirty` path; future phases introduce structured deltas (`RenderDelta` -> merged `Damage`) enabling partial line or cursor-only updates. Interim strategy remains full-frame redraw for simplicity; delta collection API will be introduced before optimization to avoid refactoring call sites.

---

## 10. Telemetry

Status: [x] 10.1 complete / [x] 10.2 complete (2025-09-14)

### Checklist

- [x] 10.1 Instrument tracing spans for core editing & navigation paths:
  - [x] motion (covers horizontal + vertical + word motions; also serves as grapheme navigation span)
  - [x] edit_insert (single grapheme insertion within Insert mode)
  - [x] edit_newline (newline insertion boundary; ends coalescing run)
  - [x] edit_backspace (cluster delete or line join within Insert mode)
  - [x] edit_delete_under (Normal mode `x` deletion)
  - [x] undo
  - [x] redo
- [x] 10.2 Snapshot debug logs (already present): trace push/pop with stack depths + rope line count proxy.

### Rationale & Notes

Unified motion span: A separate `grapheme_nav` span would duplicate every horizontal navigation emission. Keeping a single `motion` span simplifies downstream aggregation and avoids noisy log inflation. If future analysis needs to distinguish vertical/word vs grapheme‑wise motions we can add a `span!(..., kind = "horizontal"|"vertical"|"word")` attribute or introduce the deferred alias at that time.
Span naming consistency: All edit-related spans share the `edit_` prefix for easy filtering (`RUST_LOG=trace` with a future subscriber layer). Undo/redo intentionally top-level (no `edit_` prefix) to make history traversals visually distinct while scanning traces.
Snapshot metrics: Current lightweight approach logs stack depths and rope line counts without performing diff computations. This is sufficient for Phase 1 to validate coalescing boundaries and stack discipline. Richer metrics (character delta counts, time-based coalescing windows) are deferred to Phase 2 when diff rendering lands.
Performance considerations: Spans are extremely low-cost in the no-subscriber path. We purposefully avoided per-grapheme width or diff calculations inside the span constructor to keep hot paths lean.

Deferred alias decision: Dropped; unified `motion` span is sufficient. Future differentiation (if required) will use a span field (e.g. kind="horizontal"|"vertical"|"word") rather than a new span name.

Acceptance:

- All core user actions (motions, inserts, newline, backspace, delete-under, undo, redo) emit a trace span.
- Snapshot push/pop events emit depth + rope line count.
- No span introduces additional allocation beyond what the underlying action already performs.

---

## 11. Tests & QA Bundle

**Status:** [x] 11.1 complete / [ ] 11.2 deferred (optional fuzz)

11.1 Added edge tests:
     - Empty buffer backspace is a no-op (no panic, position stable).
     - End-of-line vertical motions clamp correctly (line end preserved when returning; clamped when longer/shorter lines traversed).
     - Delete-under at EOF safe (no mutation when cursor at end of line; no crash).
     - Newline + subsequent insert at file end undo/redo as single coalesced run.
     - Cross-line word motions over blank lines: forward skips blank lines; backward handles landing on blank/word lines safely (test tolerates naive implementation variance).
     - Additional word motion regression scenario with multiple blank lines.

11.2 (Optional) quick fuzz: random sequence of safe ops (not implemented Phase 1 — deferred; value add low until more editing primitives exist).

Acceptance (11.1): All listed scenarios covered by deterministic unit tests; full suite passes with zero failures.

---

## 12. Docs & Sync

**Status:** [x] complete (2025-09-14)

12.1 README updated (feature snapshot, limitations, telemetry note, Unicode promise, light humor about cursor) — DONE.
12.2 `phase-1.md` synchronized (removed `grapheme_nav`, added telemetry rationale, limitations pointer) — DONE.
12.3 Rustdoc sweep: dispatcher + state modules annotated with final span names & snapshot semantics — DONE.

Acceptance:

- Design narrative matches implemented telemetry & mechanics.
- README exposes only high-level friendly summary (technical depth lives in design docs).
- No stale references to removed spans or obsolete ordering.

---

## 13. Final Gate

13.1 Quality gates: `cargo build` / tests / `cargo clippy -D warnings` / `cargo fmt --all -- --check` — COMPLETE (2025-09-14).
13.2 Manual smoke checklist + friendly Phase 1 completion note added to README (run instructions, what to try) — COMPLETE (2025-09-14).

13.3 Annotated tag `phase-1-complete` created (2025-09-14) pointing at final Phase 1 documentation commit.

Acceptance:

- 13.1 All gates green on main branch commit (no warnings, all tests pass).
- 13.2 README contains a concise “Try this” list (enter insert, type emoji, newline, undo/redo, word motions) and clarifies Phase 1 scope without deep internals.
- 13.3 Annotated git tag created pointing at last Phase 1 commit.

Correction 13.C1 (Final Gate): During Task 13 verification we found the physical key translation for `Ctrl-R` (Redo) missing even though `Action::Redo` logic and tests existed from Task 4.6. Added translator mapping + dedicated test; design intent and acceptance for 4.6 unchanged. (Cross-reference: see 4.6 note.)

---

## Deferred (To Track, Not Implement Now)

- Time-based insert coalescing.
- Advanced Unicode word boundaries (UAX #29).
- Grapheme boundary caching / width tables.
- Operator-pending & Visual modes.
- Diff rendering.
- Multi-buffer / window management.

---

## Notes

Refactor Checkpoint R1 introduced (see `design/refactor-r1.md`) to keep Phase 1 incremental while preventing `main.rs` bloat and preparing for Task 7. Items moved: helper extraction (9.3), dispatcher relocation, status formatter, command line state struct, Insert run enum, viewport stub, observer hook, and scheduling module extraction.
Keep changes linear: each numbered section should leave code runnable. Avoid starting undo stack before mutation APIs exist, etc.

---

End of Phase 1 task log.
