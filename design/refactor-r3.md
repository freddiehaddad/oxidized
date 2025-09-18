# Refactor Checkpoint R3: Pre‑Phase 4 Structural Hardening

## 1. Objective

Prepare the codebase for Phase 4 (Performance & Keybinding Expansion) by isolating mutation concerns, reducing dispatcher breadth, enriching render delta semantics, and laying safe scaffolds for scroll optimization, operator/ count handling, multi‑view layout, and future async subsystems (LSP, plugins). No user‑visible behavioral changes; every step keeps the editor runnable and tests green.

## 2. Motivation (Why Now)

| Area | Current Pain | Phase 4 Pressure | Refactor Relief |
|------|--------------|------------------|-----------------|
| Dispatcher (1000+ loc) | Mixed motions, edits, command execution, file IO | Operators, counts, yanks would balloon complexity | Modular sub-dispatchers keep changes local |
| Key translation | Stateless fn; adding counts/operators = ad‑hoc branching | Need prefix digits + operator-pending state | `KeyTranslator` struct encapsulates transient input state |
| Status line repaint | Piggybacks on line or cursor deltas | Performance metrics skewed, wasted redraw | Distinct `StatusLine` delta reduces noise |
| Writer granularity | Per-cell prints (partial batching only implicit) | Scroll region & throughput work needs baseline | BatchWriter groups plain cells, metrics added |
| Undo snapshots | Embedded logic; delta store prototype harder | Delta-based undo needs isolation | Extract `UndoEngine` wrapper |
| Layout | Single view implicit | Multi-view / splits upcoming | Layout & region model stub introduced |
| Command handling | Inline parsing in dispatcher | More commands (`:metrics`, `:config-reload`) soon | Parser/handler layer isolates growth |
| Terminal capabilities | Assumed uniform | Scroll optimization path | Capability probe stub centralizes feature gates |

Doing this after Phase 4 would force intertwined structural + behavioral diffs, raising regression risk.

## 3. Scope (In)

1. Dispatcher decomposition into submodules (`motion.rs`, `edit.rs`, `command.rs`, `undo.rs`, `mode.rs`, `mod.rs`).
2. Command parsing layer (`ParsedCommand`, `CommandParser`, `CommandOutcome`).
3. `KeyTranslator` stateful translator (digit count & operator-pending scaffolds).
4. Extend `Action` with doc-hidden operator variants (inactive).
5. Add `RenderDelta::StatusLine` + scheduler collapse precedence.
6. Status-only change detection (mode switch, command buffer mutation, ephemeral status expiration) -> mark `StatusLine` delta.
7. Writer batching foundation (`BatchWriter` + metrics: `print_commands`, `cells_printed`).
8. Extract `UndoEngine` from `EditorState` (snapshot push/undo/redo logic).
9. Layout scaffold: `Layout`, `LayoutRegion` (single region for now) threaded into render engine.
10. Terminal capability stub (`TerminalCapabilities { supports_scroll_region: bool }`).
11. Add `:metrics` command stub (ephemeral "Metrics OK").
12. Remove legacy full `Renderer` if no longer referenced; else flag for removal at Phase 4 start.
13. Documentation: crate-level updates + new `design/refactor-r3.md` (this file) maintained as steps complete.
14. Quality gate step (fmt, clippy -D warnings, tests).

## 4. Non-Goals (Explicitly Deferred)

- Implementing operators, counts application, registers, yank/paste semantics.
- Scroll region command emission / cache shifting.
- Multi-view simultaneous rendering or layout negotiation.
- Async multi-producer event loop (`tokio::select!`).
- Delta-based undo store (only structural isolation achieved).
- Syntax highlighting, LSP integration, plugin system wiring.
- Performance tuning beyond basic batching (e.g., prefix/suffix diff, adaptive thresholds).

## 5. Detailed Step Breakdown

Each step = one commit; runnable + tests green.

### Step 1: Dispatcher Module Split

- Move logic into submodules; `dispatch()` in `mod.rs` orchestrates.
- Zero semantic changes; update internal use paths & tests.
- Add doc comment on each submodule's responsibility.

### Step 2: Command Parsing Extraction

- Add `ParsedCommand` enum: `Quit`, `Edit(PathBuf)`, `Write`, `Metrics`, `Unknown(String)`.
- `CommandParser::parse(&str) -> ParsedCommand` (assumes leading ':').
- Dispatcher `CommandExecute` branch now: parse -> handle -> produce `DispatchResult` + ephemeral messages.

### Step 3: KeyTranslator Introduction

- New struct holding optional `pending_count: Option<u32>` & operator scaffold (`pending_op: Option<OperatorKind>` future).
- Method `translate(&mut self, mode, command_active, key_event) -> Option<Action>`.
- Existing free `translate_key` becomes wrapper instantiating a thread-local translator (temporary) or is left for backward compatibility tests; mark deprecated in docs for removal after Phase 4 start.

### Step 4: Action Enum Pre-Variants

- Add (doc-hidden) `BeginOperator(OperatorKind)` & `ApplyOperator { op: OperatorKind, motion: MotionKind, count: u32 }`.
- `OperatorKind` enum with placeholder variants: `Delete`, `Yank`, `Change` (derive Debug, Copy, Eq).
- No translation paths create these yet; ensures forward compatibility for tests soon.

### Step 5: RenderDelta::StatusLine

- Extend enum + scheduler collapse logic (precedence: Full > Scroll > Lines > StatusLine > CursorOnly).
- Update existing sites; add tests verifying collapse with combined status & cursor.

### Step 6: Status-Only Detection

- Mode changes, command buffer mutations, ephemeral expiry mark `StatusLine` (not Lines) when no text edit occurred.
- Adjust event loop mapping; keep existing render heuristics for Lines vs CursorOnly unaffected.
- Add tests: issuing `:` then typing chars triggers only StatusLine delta (not Lines) until an edit.

### Step 7: BatchWriter

- Implement grouping of consecutive plain, single-width, non-reverse cells into one `Print` command.
- Metrics counters: `print_commands` (commands emitted), `cells_printed` (logical cells). Baseline test asserts commands <= cells.
- Preserve full + partial path correctness parity via existing parity tests.

### Step 8: UndoEngine Extraction

- Move snapshot structs & logic to `undo.rs` with `UndoEngine` methods: `push_snapshot(kind, buffer, position, mode)`, `undo(cursor)`, `redo(cursor)`, `skip_metric_inc()`.
- `EditorState` holds `undo: UndoEngine` and delegates methods (public API unchanged).
- Tests unchanged (confidence the extraction was behavior-neutral).

### Step 9: Layout Scaffold

- Add `core-model::layout::{Layout, LayoutRegion}`.
- Render engine `render_*` methods accept `&Layout`; currently always `Layout::single(w,h)` built in caller.
- Document invariants; no multi-region painting yet.

### Step 10: TerminalCapabilities Stub

- Simple detection (returns struct; on Windows & Unix set `supports_scroll_region=true` initially; later refined).
- Render engine stores capabilities for Scroll optimization activation in Phase 4.

### Step 11: Metrics Command Stub

- Command parser returns `ParsedCommand::Metrics` for `:metrics`.
- Handler sets ephemeral "Metrics OK" (placeholder). Real snapshot logic added Phase 4.

### Step 12: Legacy Renderer Removal

- If test suite no longer directly references `Renderer`, delete it and update docs. If still referenced in a single parity test, migrate test to use `RenderEngine::build_content_frame` helper (expose or internal test-only accessor) then remove.

### Step 13: Documentation Pass

- Update crate docs: `core-actions`, `core-render`, `core-state`, `core-model` to reflect new modules.
- Add section in this design file marking each step complete with date.

### Step 14: Quality Gate & Closure

- Run `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -D warnings`, `cargo test`.
- Confirm no public API docs referencing removed paths.

## 6. Exit Criteria

- All steps merged; zero behavior regressions (existing tests green; new tests passing).
- Dispatcher lines of code reduced significantly; submodules <= ~200 lines each initial target.
- Status-only interactions no longer trigger line repaints (validated via new test instrumentation or metrics delta count for a scripted session).
- Writer batching commands < cell count in baseline full frame test.
- Undo logic accessible only via `UndoEngine` wrapper.
- Layout & capabilities scaffolds present and inert (no rendering diff).
- Design doc updated with checklist.

## 7. Metrics Impact (Expected)

- Slight reduction in `print_commands` per full frame (batching). Baseline improvement recorded informally for future optimization references.
- Potential reduction in partial frame frequency of status-triggered full/cursor repaints (cleaner delta classification).

## 8. Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Dispatcher split introduces subtle borrow lifetimes issues | Compile failures / bugs | Incremental compile & test after Step 1 before proceeding |
| BatchWriter ordering bugs | Incorrect terminal output | Parity tests + restrict batching to plain non-reverse cells initially |
| StatusLine delta misses repaint | Stale mode/column display | Tests for mode switch, command typing, ephemeral expiry |
| UndoEngine extraction regression | Undo/redo failure | No API change, existing undo tests remain authoritative |
| Early operator variants misused | Dead code confusion | Doc-hidden, referenced only in design doc until Phase 4 activation |
| Layout parameter unused clutter | Noise | Rustdoc clearly marks as preparatory; keep function signatures stable |

## 9. Follow-Up After R3 (Phase 4 Enablers)

1. Implement counts & operator-pending logic inside `KeyTranslator`.
2. Activate `RenderDelta::StatusLine` optimization (skip status repaint if identical string cache – optional early win).
3. Scroll region & cache shift implementation using `TerminalCapabilities`.
4. Introduce register file + yank/paste.
5. Metrics command real snapshot (frame counts, dirty funnel, timings).

## 10. Go / No-Go Rationale

Proceeding with R3 reduces complexity for all Phase 4 tracks, confines structural churn to a dedicated window, and maintains breadth-first principles (no feature freeze). Deferring risks large intertwined diffs when adding operators + scroll optimization, raising review and regression cost.

## 11. Progress Checklist

(Will be updated as steps complete)

- [x] Step 1 – Dispatcher split (2025-09-18)
- [x] Step 2 – Command parser extraction (2025-09-18)
- [x] Step 3 – KeyTranslator struct (2025-09-18) – introduced stateful translator scaffold (counts/operator pending) with parity tests; clippy/test clean; behavior unchanged.
- [ ] Step 4 – Action operator pre-variants
- [ ] Step 5 – StatusLine delta
- [ ] Step 6 – Status-only detection
- [ ] Step 7 – BatchWriter
- [ ] Step 8 – UndoEngine extraction
- [ ] Step 9 – Layout scaffold
- [ ] Step 10 – TerminalCapabilities stub
- [ ] Step 11 – Metrics command stub
- [ ] Step 12 – Legacy Renderer removal
- [ ] Step 13 – Documentation pass
- [ ] Step 14 – Quality gate & closure

## 12. Notes

- Every step commits with message format:

  ```text
  refactor: [summary]

  Refactor R3 / Step X -- [step title]

  [wrapped rationale]
  ```

- If any step reveals an unexpected behavioral change risk, we pause and either adjust scope or defer that micro-step to Phase 4 with added tests.
- No new dependencies introduced (except potentially `once_cell` or `thread_local` if translator wrapper benefits; will assess necessity – prefer zero new deps in R3).

---
(End of initial R3 plan draft – iterate before execution.)
