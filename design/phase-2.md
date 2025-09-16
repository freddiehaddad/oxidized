# Phase 2: Scrolling, File IO, Render Delta Scaffold & Software Cursor

## 1. Objective

Deliver realistic file editing (open/save), vertical scrolling, a software wide‑character-aware cursor, and a forward-compatible render delta scheduling interface while retaining full-frame redraw for simplicity. Establish parsed configuration + initial theme scaffold and bounded event channel activation (first performance/backpressure posture) without introducing multi-buffer lists or split windows yet.

## 2. Scope

In Scope:

- File IO: open existing file via CLI arg and `:e <path>`; save current buffer via `:w`.
- Buffer metadata: filename, dirty flag, last write time snapshot.
- Dirty tracking + status line indicators.
- Vertical scrolling (auto-scroll when cursor leaves viewport). No scroll margin (margin = 0 per decision).
- Page motions: `Ctrl-D` (half page down), `Ctrl-U` (half page up).
- Software cursor: reverse-video (or style flag) highlight spanning grapheme width (emoji / combining safe). Hardware cursor hidden.
- Render delta scaffold: `RenderDelta` enum + merge logic in scheduler (still forces full redraw; no partial painting yet).
- Bounded event channel activation using documented `EVENT_CHANNEL_CAP` (8192) + simple overflow telemetry counters.
- Parsed config (TOML → struct) with table `[scroll.margin]` containing key `vertical` (u16). Default = 0 if absent. Applied immediately to vertical scroll behavior (context lines kept above/below cursor). If value larger than half viewport height, clamp to half-height. Namespace chosen to allow future `[scroll.behavior]` or additional margin axes without renaming.
- Command extensions: `:w` / `:e <path>` / error feedback ephemeral message (status line transient area).
- Render logging: trace when deltas collected and final collapse result (Full for now).

Out of Scope (Deferred):

- Multiple simultaneous buffers or buffer list UI (replacing active buffer only).
- Split windows / horizontal or vertical layout beyond single viewport.
- Horizontal scrolling / wrapping.
- Diff painting (still full-frame physical render).
- Operators, Visual mode, registers, macros, search.
- Syntax highlighting, Tree-sitter parsing.
- LSP, Git integration, plugin host.
- Time-based insert coalescing & diff-based undo.
- Theme configuration beyond cursor + status minimal fields.
- Theme system scaffold (palette parsing, named themes, cursor color overrides) — deferred to avoid inactive config placeholders in Phase 2.

## 3. Architectural Touchpoints

- `core-state`: extend buffer struct or add `BufferMeta` (filename, dirty, last_write). Add ephemeral status message store.
- `core-text`: no structural change; add helper for loading content from string on replace.
- `core-actions`: introduce new MotionKinds for PageDown/PageUp (Ctrl-D/U) and new Actions for WriteFile / EditFile if routed semantically via dispatcher or treat as `CommandExecute` parse results.
- `core-render`: add Cell styling fields; implement software cursor marking; add `RenderDelta` + merge in scheduler.
- `core-events`: switch from unbounded to bounded channel; add counters for dropped events (none expected single producer but future-proof). Potential `Event::Metrics(MetricsSnapshot)` placeholder deferred.
- `core-config`: parse TOML into struct; return default if missing; log applied values.
- `ox-bin`: CLI arg parse for optional path; integrate file open at startup; adapt main loop to use bounded channel creation; ephemeral status message rendering. Add `--config <path>` (Clap v4.5.47, derive) to override default `oxidized.toml` resolution in current working directory.

## 4. Event / Action Additions

- Key translation: Map `Ctrl-D` / `Ctrl-U` to new page motion actions (rely on key modifiers + char detection or dedicated codes from crossterm).
- Command parser: Detect `:w` and `:e <path>` in `CommandExecute` branch; produce internal actions or directly invoke file ops (decide simplicity: direct in dispatcher acceptable now, but record design note).
- No new top-level Event variants required Phase 2.

## 5. Data Model Changes

- Extend `EditorState`:
  - `file_name: Option<PathBuf>`
  - `dirty: bool`
  - `last_write_hash` or simple size + timestamp snapshot (optional; for now bool is enough).
  - `ephemeral_status: Option<EphemeralMessage>` with message + expiration instant.
  - Viewport fields: move from transient to state? (Add `viewport: Viewport`).
- `Viewport` gains methods: `scroll_to_include(line)`, `page_down(height)`, `page_up(height)`, total text height awareness.
- `RenderScheduler` additions: queue Vec for render deltas; a `mark(delta)` API; `consume()` returns collapsed damage enum (Phase 2 always maps to Full).
- `Cell` struct: add optional style flags (bitflags): CURSOR, BOLD (reserved), REVERSE (cursor uses REVERSE if no colors). Potential `fg: Option<Color>`, `bg: Option<Color>` minimal.
- Config struct: `Config { scroll: ScrollConfig }` where `ScrollConfig { margin: MarginConfig }` and `MarginConfig { vertical: u16 }`.
- Theme support deferred (remove previous placeholder struct from scope).

Clamp Semantics (Vertical Margin):

- User-specified `[scroll.margin].vertical = V` is clamped to `V' = min(V, (h - 2) / 2)` where `h` is the current viewport height (u16, integer division, saturating at 0).
- Rationale: always reserve at least two interior lines (when `h >= 4`) for content between top/bottom margins; prevents excessive empty banding.
- Example: h=25 → max=11; h=4 → max=1; h=3 → max=0.
- Clamping is logged at info level exactly once per startup (no log if not clamped).
- Future horizontal margin will mirror this as `[scroll.margin].horizontal` with width-based formula `(w - 2) / 2`.

## 6. Steps (Ordered, Each = One Commit)

IDs final after confirmation; commit messages embed Phase/Step.

1. (Done) Add buffer metadata fields (`file_name`, `dirty`) + show filename & dirty marker in status line (no IO yet). Initial dirty flag always false until mutation tracking added in later step.
2. (Done) Implement CLI open: if `oxidized <path>` provided, load file contents into buffer (blocking read, UTF-8 only). On error fallback to welcome buffer and log error (ephemeral status messaging deferred until step 6).
3. (Done) Command parse: extend `CommandExecute` handling for `:e <path>` (replace current buffer) + tests (loads file, resets cursor, updates file_name, dirty=false; errors logged only until ephemeral status in step 6).
4. (Done) Add `:w` handling: write current buffer to existing filename; if none, log error (ephemeral status deferred) and leave dirty=true. Successful write clears dirty.
5. (Done) Dirty tracking: buffer `dirty` set on first mutation after open/write across all edit kinds (insert grapheme, newline, backspace, delete-under). Undo/redo do NOT auto-clear; only successful `:w` resets. Tests cover: first insert sets dirty, undo leaves dirty, write clears then subsequent edit re-sets dirty.
6. (Done) Ephemeral status: `EditorState::ephemeral_status` + `set_ephemeral` / `tick_ephemeral`. Messages (Open failed / Opened / Wrote / Write failed / No filename) right-align on status line when command inactive; hidden while command buffer active. 3s TTL checked each event loop iteration; expiration triggers redraw. Tests cover lifecycle, :e success/failure, :w no filename.
7. (Done) Elevate viewport to state: add `viewport_first_line` to `EditorState` (initial 0); render path now uses this persistent field instead of constructing a transient `Viewport`. Scrolling logic still pending (Step 8) so value remains 0 until auto-scroll/page motions mutate it.
8. (Done) Auto-scroll logic: added `EditorState::auto_scroll(text_height)` keeping cursor within `[first_line, first_line+height)`. Adjusts `viewport_first_line` upward or downward (placing cursor at bottom when scrolling down). Integrated into main event loop; redraw triggered only when first line changes. Tests cover downward scroll beyond bottom and upward scroll above top. Also fixes hardware cursor placement to account for `viewport_first_line` and suppresses rendering of raw `\r`/`\n` control terminators (pending full normalization Step 9).
9. (Done) Line ending normalization & preservation: on file load (CLI arg or `:e`) we scan raw text counting CRLF, LF, and standalone CR sequences in a single linear pass. Majority style chosen with deterministic precedence CRLF > LF > CR for ties. Mixed flag set when more than one non-zero style and at least one count differs from majority (prevents false positive on uniform files). Content is normalized to an internal LF-only buffer (replacing CRLF and CR with `\n`) so editor logic & rendering never handle raw `\r` characters. We record two metadata fields in `EditorState`: `original_line_ending` (majority style) and `had_trailing_newline` (whether original file ended with any newline sequence). On write (`:w`) we reconstruct output by iterating buffer lines (which may each end in an internal `\n`) stripping the internal terminator and appending the original style between lines; we append a final terminator only if the original file had one. Mixed files trigger a `warn!("mixed_line_endings_detected[_startup]")` but still select the majority style for round‑trip (no automatic normalization rewrite). Tests cover: CRLF round‑trip, LF, CR, mixed majority (ensures `mixed=true` and majority style selection), and absence/presence of trailing newline.
10. (Done - Hotfix) Unicode-safe line ending normalization rewrite & round-trip property tests: Replace per-byte reconstruction that cast raw UTF-8 bytes to `char` (violating Unicode correctness for multi-byte sequences) with span-copy algorithm preserving original UTF-8 sequences while substituting CRLF/CR with `\n`. Added Unicode-rich CRLF, CR, mixed tests (emoji + variation selector + combining mark). Added round-trip property test (normalize + re-expand + re-normalize idempotence) and guardrail assertions (no `\r` in normalized output). Risks & Mitigations updated with regression note; added guardrail forbidding arbitrary `u8 as char` casting in normalization code paths.
11. (Done) Page motions: map `Ctrl-D` / `Ctrl-U` to half-page jump preserving sticky column; clamp; tests.
12. (Done) Software cursor: extended `Cell` with `flags: CellFlags` (bitflags: `REVERSE`, `CURSOR`). During frame construction we compute the grapheme cluster under the logical cursor (using `visual_col`, `next_boundary`, `cluster_width`) and paint a reverse-video span whose width matches the cluster (wide emoji get two cells, combining sequences remain width 1). Hardware cursor placement removed; renderer wraps flagged cells with ANSI `\x1b[7m` (invert) and resets after each cell for simplicity (acceptable Phase 2 full‑frame redraw). Tests cover: ASCII single width, wide emoji (two-cell span with second cell space-filled & flagged), combining mark sequence width=1, and end-of-line blank cell (synthesized space) all carrying cursor flags.
13. (Done - Hotfix) Renderer origin positioning fix: Explicitly send `MoveTo(0,0)` before painting each full frame to eliminate reliance on prior implicit cursor homing side‑effects (previously often hidden by a full-screen `Clear`). Root cause: after certain motion + render sequences the terminal's hardware cursor was not at (0,0); the first painted cell (top‑left) was emitted at the stale cursor position (e.g. bottom of screen), leaving the original top‑left glyph (like the initial `W`) visually unchanged or overwritten later, creating the appearance of a disappearing first character. Internal frame data was always correct (diagnostic logs confirmed), but absence of deterministic cursor homing caused on‑screen divergence. Fix: prepend a single `MoveTo(0,0)` prior to iterating cells; retain removal / de‑emphasis of full-screen clear to avoid flicker. No public API or test changes required; behavior validated manually with prior repro steps. Logged here as a hotfix step for historical traceability (mirrors Step 10 pattern).
14. (Done) Config parsing (TOML): parse `oxidized.toml` (or `--config <path>` override) extracting `[scroll.margin] vertical` (default 0). Clamp via `(h - 2)/2` at startup using initial viewport height minus status line, log single info on clamp (when clamped). Store effective margin in `EditorState.config_vertical_margin` (not yet consumed by scroll logic—deferred intentionally to Step 15). Unknown fields ignored for forward compatibility. Added sample root `oxidized.toml` documenting the current key with comments so users can tweak immediately. Tests: missing file default=0, explicit value parse, clamp overflow.
15. (Done) Integrate `scroll.margin.vertical` into auto-scroll logic: updated `EditorState::auto_scroll` to enforce a configurable margin `m` (clamped to `text_height/2`) keeping the cursor at least `m` lines from top/bottom where possible. Upward scroll triggers when `cursor_line < top + m`; downward when `cursor_line + m >= bottom`. Margin is stored in `config_vertical_margin` (populated Step 14). Tests added: zero margin baseline parity, earlier downward scroll with margin, bottom boundary computation (cursor at end), and small viewport height (h=3) clamping behavior.
16. (Done) Bounded channel activation: replaced unbounded channel with `mpsc::channel(EVENT_CHANNEL_CAP)`; input thread now uses `blocking_send` (natural backpressure, no drops). Added atomic counters: `CHANNEL_SEND_FAILURES` (channel closed) and `CHANNEL_BLOCKING_SENDS` (each successful blocking send). Updated `core-events` commentary to activated policy; removed obsolete TODOs in `main.rs`. Added async test (tiny capacity=2) validating pending send resumes after a receive. Future: introduce prioritized control channel + selective drop policy when additional producers (timers/LSP) arrive; current fidelity > lossy optimization.
17. (Done) RenderDelta enum & scheduler queue: introduced `RenderDelta` { Full, Lines(range), StatusLine, CursorOnly } plus `RenderScheduler::mark/consume`. Collapse logic merges line spans, honors Full override, and prefers Lines > StatusLine > CursorOnly. Phase 2 still forces full redraw: `consume()` always returns Full but logs merged form for telemetry. Updated main loop: dispatch result maps to Lines (line change or insert edit), CursorOnly (pure motion), StatusLine (ephemeral/status changes), Full (scroll/resize). Added unit tests (span merge, full override, status+cursor, consume idempotence). Tracing emits `render_mark` and `render_delta_collapse`. Future phases will leverage non-Full variants for partial painting.
18. Integrate delta usage in render loop (still full redraw; ignore partial). Place TODO for Phase 3/4.
19. Refine status line format final (e.g. `[NORMAL] file.rs* Ln X, Col Y :` where `*` = dirty). Update tests & docs.
20. Documentation sweep: rustdoc for new structs; update Phase 2 design file with any adjustments discovered.
21. Quality gate run & finalize Phase 2 checklist marking done.

## 7. Exit Criteria

- Launch with/without path argument loads file (or welcome buffer fallback).
- Scrolls when navigating beyond screen vertically; page motions functional.
- Software cursor visually spans wide emoji / combining sequence correctly.
- `:e <path>` replaces current buffer content; errors show ephemeral message.
- `:w` writes file; dirty flag clears; failure surfaces ephemeral message.
- Status line displays filename and dirty marker; command mode still works.
- Bounded event channel in use (capacity constant enforced); no deadlocks in single-producer scenario.
- RenderScheduler records deltas (trace logs) but renderer still does full redraw.
- Config file (if present) parsed (or overridden via `--config`); `[scroll.margin].vertical` applied & clamped per formula; info log emitted only on clamp.
- All tests (new + existing) pass; no clippy or fmt issues.

## 8. Telemetry / Logging Additions

- Trace spans: `file_open`, `file_write`, `scroll_adjust`, `page_motion`, `render_delta_collapse`.
- Counters: dropped events (expected zero), delta kind frequencies.
- Log warnings: non-UTF-8 read attempt (placeholder), margin value clamped (when applicable).

## 9. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Blocking file IO stalls UI | Document acceptable Phase 2; later async load; keep files modest in manual tests. |
| Bounded channel backpressure misconfiguration | Start with generous 8192; add simple metrics; test with artificially tiny capacity in unit test. |
| Software cursor style conflicts with future theming | Encapsulate styling in one function; theme API stable. |
| RenderDelta logic unused leading to drift | Unit tests + trace logs asserting collapse always returns Full. |
| Unicode corruption during normalization (regression Step 9) | Rewritten UTF-8 safe span-copy algorithm (Step 10), property + Unicode tests; guardrail: forbid per-byte `u8 as char` casting in normalization; code review checklist item added. |
| Ephemeral status flicker overriding command | Command takes precedence; ephemeral displayed only when not in command mode. |
| Page motions mis-handle short buffers | Clamp logic; tests for buffers < half page height. |

## 10. Deferred / Follow-Up Items

- Horizontal scrolling & scroll margin semantics.
- Buffer list / multiple loaded buffers simultaneously.
- Diff rendering activation (partial paints) using accumulated deltas.
- Non-blocking file IO and encoding detection.
- Theme color application beyond reverse-video cursor styling.
- Operators, Visual mode, registers, yanking.
- Syntax highlighting (Tree-sitter) integration.
- LSP / Git / Plugins / Macros.
- Time-based insert coalescing & diff-based undo representation.

## 11. References

- Phase 1 design and Refactor R1 docs.
- `ropey` documentation (file load from string).
- crossterm color & attribute usage guides.

## 12. Notes

- Breadth-first preserved: each step runnable & independently testable.
- RenderDelta introduced early to freeze API surface before optimization pressure.
- Config parsing minimal by design: avoids premature option sprawl.
- Channel capacity constant reused; input thread adaptation keeps semantics identical.
- Software cursor sets stage for future mode-dependent shapes (block vs bar) without early complexity.
- Theme/color system intentionally deferred to a later phase to avoid shipping inactive placeholders; current visual differentiation relies solely on reverse-video cursor.
- Namespace note: `[scroll.margin]` adopted instead of flat `scroll_margin` or `[scroll] vertical` to reduce future rename churn when adding horizontal margin or behavioral scroll settings.

---
(Plan will be updated incrementally; each completed step commits with the defined template and updates this file.)
