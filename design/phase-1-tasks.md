# Phase 1 Task Breakdown (Editing + Grapheme-Aware Cursor)

## Legend

- [ ] = not started
- [WIP] = in progress
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

4.1 `EditSnapshot` (clone rope + cursor + mode?).
4.2 Stacks with max length & drop-old warning.
4.3 Coalescing: Insert bursts (boundary = Esc or newline only).
4.4 API: begin_edit_if_needed, push_after_edit, undo, redo.
Tests: sequence of inserts undone in one step; newline creates new snapshot.

---

## 5. Insert Mode Mechanics

5.1 Map `i` -> enter Insert (ensure snapshot boundary).
5.2 Printable input -> insert_grapheme.
5.3 Enter -> newline insertion.
5.4 Backspace logic (cluster before; join line above if at start).
5.5 Esc -> leave Insert, finalize snapshot.
Tests: type "hi" + Esc + undo restores original.

---

## 6. Normal Mode Editing

6.1 `x` -> delete_grapheme_at (no-op at end).
6.2 Snapshot capture before first `x` in a run (simplest: every x).
Tests: multiple x + undos.

---

## 7. Command / Status Line

7.1 Track `pending_command` across modes.
7.2 Render line: `[NORMAL|INSERT] Ln X, Col Y :<cmd>` (only show colon section when active).
7.3 Echo `:q` and preserve existing quit behavior.
Tests: building formatted status string.

---

## 8. Rendering & Cursor Placement

8.1 Compute visual column (sum widths) for cursor.
8.2 Move terminal cursor with backend before flush.
8.3 Optionally highlight cell (defer if terminal cursor suffices).
Manual test with wide/CJK and emoji.

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
- [ ] 9.3 Extract broader helpers (motion/edit/command). (Partial: inline `apply_motion` / `apply_vertical_motion` added; full separation pending edits & command handling.)
- [x] 9.4 Action enum introduced (`core-actions` crate) & compiled.
- [x] 9.5 Translation function skeleton (`translate_key`) added (no wiring yet).
- [x] 9.6 Async tokio channel + loop.
- [x] 9.7 Dispatcher & dirty flag (implemented ahead of 9.6 for lower-churn refactor).
- [x] 9.8 Render scheduler stub.
- [ ] 9.9 Deferred multi-producer & diff hook documented.

Notes: Dispatcher landed before async channel migration (9.6) to reduce simultaneous complexity. Initial render bug fixed by performing a first-frame render at startup before event loop (ensures visible buffer without input). Render scheduler stub (9.8) still pending—current dirty flag logic exists inline; it will move into a dedicated struct during 9.8.

Notes: Replaced temporary unsafe raw pointer borrowing with safe helper functions (`apply_motion`, `apply_vertical_motion`) before proceeding to Undo/Redo to avoid accruing technical debt. Introduced new `core-actions` crate for semantic intent separation (motions/edits/mode changes) per modularity goal.

---

## 10. Telemetry

10.1 Spans: motion, insert, delete, newline, undo, redo, grapheme_nav.
10.2 Debug logs: snapshot push/pop (rope char count, stack sizes).

---

## 11. Tests & QA Bundle

11.1 Additional edge tests (empty buffer backspace, end-of-line motions, multi-line word motion start/end).
11.2 (Optional) quick fuzz: random sequence of safe ops (if added later).

---

## 12. Docs & Sync

12.1 Update README (features list now: basic editing, grapheme-aware cursor).
12.2 Update `phase-1.md` Notes if deviations occur.
12.3 Rustdoc for new APIs (Cursor, snapshots, grapheme helpers).

---

## 13. Final Gate

13.1 `cargo build` / `cargo clippy -D warnings` / `cargo fmt --all -- --check`.
13.2 Manual smoke script (document in README dev section).
13.3 Tag `phase-1-start` (optional) then after completion `phase-1-complete`.

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

Keep changes linear: each numbered section should leave code runnable. Avoid starting undo stack before mutation APIs exist, etc.
