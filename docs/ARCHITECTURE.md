# Oxidized Architecture Guide

This document gives contributors a high-level and practical overview of how Oxidized works under the hood. It includes links to code, key flows, and inline ASCII diagrams to help you get oriented fast.

- Target audience: developers/contributors
- Prereqs: Rust, terminal UI basics

See also: [Architecture Quickstart (At a Glance)](./ARCHITECTURE_QUICKSTART.md) for a one-page visual overview.

## Top-level Modules

- src/core: [buffer](../src/core/buffer.rs), [editor](../src/core/editor.rs), [mode](../src/core/mode.rs), [window manager](../src/core/window.rs)
- src/ui: [renderer](../src/ui/renderer.rs) + [terminal glue](../src/ui/terminal.rs)
- src/input: [event loop](../src/input/event_driven.rs), [key handling](../src/input/keymap.rs), [event types](../src/input/events.rs)
- src/features: [syntax highlighting](../src/features/syntax.rs), [search](../src/features/search.rs), [macros](../src/features/macros.rs), [text objects](../src/features/text_objects.rs), [LSP (stub)](../src/features/lsp.rs)
- src/config: editor/theme/keymap config + [file watchers](../src/config/watcher.rs)

### Component responsibilities (quick map)

- [core/buffer.rs](../src/core/buffer.rs)
  - Text storage as Vec<String> lines, cursor Position, selection, marks, clipboard.
  - Editing operations (insert/delete/indent/unindent/replace), undo/redo with delta tracking.
  - File IO (load/save), line ending handling, modified flag.
- [core/editor.rs](../src/core/editor.rs)
  - Orchestrates buffers, window manager, UI, terminal, input handling, search, macros.
  - Holds config and theme state, async syntax highlighter, completion engine, and flags for redraw.
  - Produces EditorRenderState for the UI on each render.
- [core/window.rs](../src/core/window.rs)
  - WindowManager and Window data structures: splits, sizes, active window, viewport, horizontal offset.
  - Reserved rows for status line and command line.
- [ui/renderer.rs](../src/ui/renderer.rs) + [ui/terminal.rs](../src/ui/terminal.rs)
  - Terminal abstraction with double-buffered queueing of draw commands.
  - Renderer computes gutter, wrapping, statusline, and draws highlighted text (from Editor state).
  - Grapheme-aware widths and safe UTF-8 slicing.
- [input/event_driven.rs](../src/input/event_driven.rs) + [input/events.rs](../src/input/events.rs) + [input/keymap.rs](../src/input/keymap.rs)
  - EventDrivenEditor: input thread, config watcher, syntax results dispatcher, (future) render thread.
  - Key handling maps key sequences to editor actions and Ex commands.
- [features/syntax.rs](../src/features/syntax.rs)
  - Tree-sitter based synchronous highlighter and an AsyncSyntaxHighlighter worker pipeline.
  - Small per-line LRU cache for highlight results.
- [features/search.rs](../src/features/search.rs), [features/macros.rs](../src/features/macros.rs), [features/text_objects.rs](../src/features/text_objects.rs), [features/completion.rs](../src/features/completion.rs)
  - Focused subsystems used by Editor and keymaps.
- [utils/command.rs](../src/utils/command.rs)
  - Ex-style command parser and executor, centralized :set handler (ephemeral vs persistent via :setp).
- config/* (e.g. [config/editor.rs](../src/config/editor.rs), [config/theme.rs](../src/config/theme.rs), [config/keymap.rs](../src/input/keymap.rs))
  - EditorConfig, ThemeConfig, Keymap config, file watcher, and hot reload hooks.

## Key Runtime Flow

1. main.rs initializes logging and creates an Editor.
2. EventDrivenEditor wraps Editor and spawns threads (input, config watch,
   syntax results dispatcher, render stub) and an event bus. Syntax
   highlighting runs in a dedicated worker thread; results are applied via a
   dispatcher that requests UI redraws.

### Timing and Cadence

- The input thread uses crossterm polling with EVENT_TICK_MS (default 16ms) to
  stay responsive. The main event loop uses a fully blocking recv and wakes
  only when events arrive. The config watcher blocks on filesystem events, and
  the syntax results dispatcher blocks on a dedicated channel from the async
  highlighter.

1. Input thread reads terminal events and sends Input events.
2. EventDrivenEditor processes events, mutates Editor as needed, and sends UI
  RedrawRequest when state changes.
3. A syntax results dispatcher thread listens for background highlight results,
  updates caches, drops stale versions, and triggers redraws.
4. Editor::render() snapshots EditorRenderState and asks UI to draw via
  Terminal.

Sequence (input → state → render):

```text
  +-------------+     +--------------------+     +-----------+
  |  Terminal   | --> | Input/Event Thread | --> |  Editor   |
  +-------------+     +--------------------+     +-----------+
                                             \              \
                                              \ Redraw req.  \
                                               v              v
                                           +--------+   +-----------+
                                           |  UI    |<- | Renderer  |
                                           +--------+   +-----------+
```

## Data Model

- Buffer: lines, cursor, selection, undo/redo stacks, marks, clipboard.
- Editor: buffer set, window manager, mode, status, config, themes, async syntax state.
- UI: theme, syntax theme, flags; computes gutter/columns; renders status/command lines.
- Events: strongly-typed enums for Input/UI/Config/Window/Search/Macro/System/LSP.

Component overview (simplified):

```text
  +---------+    owns     +---------+     manages     +-----------+
  | Editor  | ----------> | Buffers | <-------------  |  Windows  |
  +---------+             +---------+                 +-----------+
       |                         |                           |
       | uses                    | contains                  | displays
       v                         v                           v
  +---------+             +------------+               +----------+
  |  Modes  |             |   Marks    |               |   UI/    |
  +---------+             +------------+               | Renderer |
                                                       +----------+
```

Core classes and relationships (high-level):

  Buffer <--> Editor <--> WindowManager
      ^            |
      |            v
     Marks       Mode

## Rendering and Cursor

- UI::compute_gutter_width reserves space for numbers or marks.
- Rendering is width-aware using unicode-width; grapheme navigation/deletion uses unicode-segmentation.
- Cursor column (no-wrap) uses Unicode width between base offset and cursor byte index to keep visual and logical positions in sync.
- Visual selection semantics: `Selection.start` is always the anchor (original point where selection began) and is not reordered with `end` for character/block selections. Helpers like `highlight_span_for_line` and `Buffer::get_selection_range` derive ordered spans as needed. This preserves direction for motions and anchor-sensitive operations.

## Undo/Redo and Redraws

- Buffer implements delta-based undo/redo; Editor actions call buffer.undo()/redo().
- To ensure immediate UI feedback even when the cursor doesn’t move, key handlers request redraw after successful delete/undo/redo operations.

## Config & Hot Reload

- ConfigWatcher blocks on filesystem events (notify) and sends typed change events; no periodic polling. EventDrivenEditor translates them to Config events and forces a full redraw when applied.
- ThemeConfig load_with_default_theme applies color scheme; UI reads it on init and reload.

## Syntax Highlighting (async pipeline)

Oxidized uses a truly async syntax pipeline powered by Tree-sitter:

- A dedicated worker thread owns its own parser/theme and receives work items
  over a bounded channel. Work items contain (buffer_id, line_index,
  full_content, language, priority, version).
- The worker coalesces requests by (buffer,line), preferring the latest
  version and highest priority, then highlights line text using the provided
  full-file context for correctness.
- Results are sent over an unbounded results channel to a dispatcher thread.
- The dispatcher validates results against a monotonic version token held by
  the Editor and drops stale results. Valid results are applied to a small
  in-memory LRU cache keyed by (buffer_id, line_index), and a UI redraw is
  requested.

Priorities

- Critical: current cursor line
- High: visible viewport lines
- Medium/Low: nearby lines off-screen or opportunistic background work

Versioning and staleness

- Editor maintains highlight_version (AtomicU64). Actions that reshuffle
  context (scroll, resize, theme change) bump the version. Any result with a
  version lower than the current is discarded by the dispatcher.

Caching

- A small LRU cache bounds memory usage for per-line highlight results. The UI
  renders using cached results immediately when available, and async results
  update the cache in-place.

### End-to-end flow (code pointers)

- Editor::render collects visible lines for all windows and calls
  Editor::get_syntax_highlights(buffer_id, line_index, text, path).
  - Tries AsyncSyntaxHighlighter cache first; if miss, enqueues a request with
    full buffer content and returns empty highlights for now.
  - Sets needs_syntax_refresh so the UI redraws when results arrive.
- features/syntax.rs: AsyncSyntaxHighlighter
  - Bounded work queue (256) prevents unbounded growth. Each WorkItem contains
    buffer_id, line_index, full_content, language, priority, version.
  - Worker loop coalesces a small backlog by (buffer,line), prefers higher
    priority, then highlights that single line using a per-thread
    SyntaxHighlighter and Tree-sitter parser.
  - Results are sent as HighlightResult { buffer_id, line_index, version, highlights }.
- input/event_driven.rs: spawn_syntax_results_thread
  - Receives results, compares result.version with editor.highlight_version,
    drops stale results, then calls Editor::apply_syntax_highlight_result which
    writes to the AsyncSyntaxHighlighter cache and flips needs_syntax_refresh.
  - Emits a UI RedrawRequest event.
- ui/renderer.rs
  - Uses EditorRenderState.syntax_highlights map (collected by Editor::render)
    to render colored segments. Highlight ranges are shifted for wrap and
    horizontal scrolling.

### Why Tree-sitter per-line?

- We parse the full buffer text in the worker but compute highlights for the
  requested line only. This preserves correctness with language constructs that
  span lines while keeping rendering incremental and responsive.

### LRU cache purpose and behavior

- The AsyncSyntaxHighlighter owns a small, mutex-protected per-line LRU cache
  keyed by (buffer_id, line_index). Purpose:
  - Bound memory usage for highlight results.
  - Avoid recomputation during normal scrolling back-and-forth.
  - Provide instant highlights on repeated renders while worker catches up.
- Capacity is fixed (currently 2048 entries). Least-recently-used entries are
  evicted when capacity is reached.
- Cache is cleared on theme updates to prevent color/style mismatches. There is
  also support to invalidate entries for specific buffers when needed.

## Windows and Viewports

- WindowManager manages splits, sizing, and viewport for each window.
- EditorRenderState contains per-buffer highlight cache keyed by (buffer_id, line_index).

## Testing

Oxidized uses a broad, fast test suite emphasizing small, deterministic unit tests with a few higher-level integration/regression cases. The goals are: (1) protect core editing invariants (buffer text, cursor/selection semantics, undo/redo), (2) lock in motion/text-object behavior (including tricky Unicode + punctuation edges), and (3) ensure peripheral subsystems (config reload, search, macros, completion, syntax enqueue logic) remain stable.

### Layout & Categories

tests/ contains almost all tests (crate-level integration style) organized by concern:

- Buffer & Editing: `buffer_integration.rs`, `buffer_range_tests.rs`, `buffer_yank_put_tests.rs`, deletion / paste corner cases.
- Cursor & Grapheme Safety: `grapheme_cursor_tests.rs` (emoji, multi‑grapheme clusters) plus selection span tests.
- Motions & Modes:
  - Classic motions: covered across `g_motion_tests.rs`, `g_caret_motion_tests.rs`, `mode_tests.rs`.
  - New extended motions: `g_motion_tests.rs` houses `gE` (WORD backward end) and `ge` (small word backward end) regression cases including punctuation (hyphen, ellipsis) and cross-line behavior.
  - Visual / VisualLine / VisualBlock / Select / SelectLine: `visual_*`, `select_mode_tests.rs`, and `wrapped_visual_selection_tests.rs` for selection growth, wrapped line invariants, exit behavior, and typed char replacement semantics in Select modes.
- Text Objects: `text_objects_tests.rs` (parser + finder; word/WORD, quotes, brackets, sentences, paragraphs, inner/around variants).
- Search: `search_integration.rs` (case sensitivity, regex toggle, multiple matches, empty pattern guard).
- Macros: `macro_tests.rs` (record/playback, register management, filtering of control keys).
- Keymaps & Events: `keymap_tests.rs`, `events_tests.rs` (ensures dispatcher stability and no panics on representative sequences).
- Commands & Ex Layer: `command_tests.rs`, `ex_*_tests.rs` (write, saveas, force write, buffer edge cases, :set persistence vs ephemeral).
- Config & Persistence: `config_tests.rs`, `config_persistence_tests.rs` (hot reload, theme propagation, persisted vs session settings).
- Window/Layout/UI: `window_tests.rs`, `resize_tests.rs`, `statusline_tests.rs`, `ui_tests.rs`, `ui_wrap_tests.rs` (viewport math, status segments, wrapping correctness, horizontal offset behavior).
- Completion: `completion_tests.rs` (basic acceptance & cycling semantics).
- Replace / Insert Edge Cases: `replace_tests.rs`, paste & empty line handling tests.
- Text Objects & Selection Interop: `selection_span_tests.rs`, `visual_*` variants, and Select mode tests (character/line replacement path).

Some focused regression files carry a short doc comment (`//!`) at top describing the bug they lock in (e.g., visual mode exit, wrapped selection growth). When adding a regression, prefer appending to the most related existing file instead of creating a new one, unless the scenario is substantial and orthogonal.

### Style & Conventions

- Favor constructing minimal Buffers (helper constructors inside test files) instead of spinning a full `EventDrivenEditor` unless validating event threading / rendering interactions.
- Tests treat positions as byte indices but rely on helper methods (e.g., grapheme boundary utilities) indirectly through Buffer APIs; avoid manual slicing of potentially multi-byte graphemes.
- For motions that depend on classification (word vs WORD vs punctuation), each new motion gets: baseline behavior test, punctuation boundary test, whitespace/line-boundary test, buffer-start stability test, and (if relevant) multi-step repetition test.
- Visual selection tests assert both existence and span correctness without assuming selection ordering (direction preserved internally).
- Avoid sleeping or timing assertions; async syntax highlighting is validated structurally (enqueues & cache mechanics) elsewhere, not via timing-sensitive tests (worker thread currently lightly exercised – deeper integration tests can be introduced when LSP matures).

### Adding New Tests

1. Pick the closest existing file by domain; append tests near similar cases.
2. Name with clear intent: `feature_condition_expectedOutcome` or `regression_issueDescription`.
3. For regressions, add a short comment referencing the scenario (“Regression: previously panic when …”).
4. Prefer explicit indices / cursor setup over derived logic to keep failures obvious.
5. If introducing a new motion/operator: mirror the existing gE/ge pattern (baseline, punctuation, cross-line, start-of-buffer, repetition).

### Running Tests

- All tests:
  - `cargo test` (fast; suite avoids heavy IO/network)
- Focus by prefix substring:
  - `cargo test gE_` or `cargo test ge_`
- Single test (exact):
  - `cargo test --test g_motion_tests ge_hyphen_treated_as_separate_word`

Clippy (lint gate) should stay clean before committing motion or buffer changes:

```console
cargo clippy -- -D warnings
```

### Future Enhancements

- Add targeted async syntax pipeline tests exercising version invalidation & LRU eviction ordering (currently validated indirectly via rendering path assumptions).
- Introduce property tests (e.g., shrinking random edit sequences ensure undo stack invariants) using `proptest` once core APIs stabilize.
- LSP client integration tests (scaffold only today).

### Guiding Principle

Keep tests descriptive, minimal, and colocated with similar behavior. Fast feedback (sub‑second `cargo test`) is a priority; defer heavier end‑to‑end benchmarks to `benches/` or future integration harnesses.

## Alternatives and trade-offs

- Blocking main loop (current): We now use a fully blocking recv for the main
  event loop to reduce wakeups and idle CPU. Previously we used a short
  recv_timeout; the switch simplifies control flow and shutdown.
- Channel select: Using a select over multiple channels (e.g., crossbeam) can
  provide more flexible waiting. Today we use std::mpsc plus dedicated
  threads per source, which keeps dependencies minimal and behavior simple.
- Input thread using blocking read: Switching from crossterm::event::poll to
  blocking event::read can further reduce wakeups, but graceful shutdown then
  requires an interrupt; the current 16ms poll balances responsiveness and
  simple shutdown.

## Next Steps for Contributors

- Start in src/input/keymap.rs for keybindings and actions
- Follow into src/core/editor.rs for state changes and rendering
- Inspect src/ui/renderer.rs to understand drawing logic
- Explore src/features/syntax.rs for Tree-sitter integration

## Editor internals (deeper dive)

- Render lifecycle
  - Editor::render clones only buffers currently visible in windows (reduces
    work on large projects). It assembles EditorRenderState that includes mode,
    status, command line, current window layout, visible syntax highlights,
    completion state, and the current config snapshot.
  - UI::render drives terminal drawing and status/command lines based on that
    state. Terminal size is refreshed each draw to adapt to late resizes.

- Viewport, wrap, and horizontal scroll
  - Window.viewport_top and Window.horiz_offset control what’s shown. Editor
    updates these on cursor moves and scroll commands respecting scrolloff and
    sidescrolloff from the config. Wrap mode switches the renderer into a
    grapheme-aware multi-row algorithm.

- Status line content
  - Left: mode, filename, modified flag. Middle: status message. Right: cursor
    pos, indent style/width, encoding, EOL, filetype, macro REC, search index,
    and progress. Each segment can be toggled via config.

- Ex commands and settings
  - utils/command.rs implements :w, :q, :bd, :e, :ls, split/vsplit/close, etc.
  - :set toggles and queries are ephemeral (session-only). :setp persists to
    editor.toml. Both feed into Editor’s config and update UI and behavior at
    runtime.

- Search, text objects, and macros
  - SearchEngine supports case sensitivity and smartcase behavior. Results are
    integrated into statusline and navigation commands.
  - Text objects parse motions like iw, aw, i(, a", paragraphs, sentences, etc.
  - MacroRecorder handles q/<register> recording, @ and @@ playback.

- LSP (current state)
  - features/lsp.rs is a scaffold for future JSON-RPC client integration
    (completions, diagnostics). The architecture leaves a dedicated EditorEvent
    branch for LSP to plug into the event loop without blocking.

## Operational tips

- Logging
  - Use RUST_LOG=debug to see syntax pipeline traces (worker enqueue, UI
    highlight usage). File-based logs reduce TTY noise.

- Performance knobs
  - EVENT_TICK_MS controls input polling. LRU cache size and async work queue
    size are set in features/syntax.rs.
  - Bumping highlight_version is cheap and a good way to invalidate in-flight
    work after big changes (theme switch, large scrolls).

## ASCII diagrams (visual overview)

### Event-driven threads and event bus

Legend:

- Boxes = threads/components; arrows = message flow.
- mpsc = std::sync::mpsc channel for EditorEvents.
- crossbeam channels: work_tx (bounded) and result_rx (unbounded) for syntax.
- RedrawRequest triggers Editor::render and UI drawing.

```text
+---------------------+                    +--------------------+
| Input thread (poll) |------------------->|  Event bus (mpsc)  |
+---------------------+                    | (EditorEvent chan) |
  ^                                        +--------------------+
  |                                                  ^
 crossterm::event                                    |
                                                     |
+------------------------+                           |
| Config watcher thread  |---------------------------+
+------------------------+

+------------------------+        result_rx        +--------------------+
| Syntax dispatcher thr. |<------------------------|    Async worker    |
+------------------------+                         |    (Tree-sitter)   |
  | apply results                                  +--------------------+
  v
+-----------------+         work_tx (bounded)
|     Editor      |--------------------------------------+
+-----------------+                                      |
  | render()                                             |
  v                                                      v
+-----------------+        draw commands           +--------------------+
|       UI        |------------------------------->|      Terminal      |
+-----------------+                                +--------------------+
```

### Async syntax highlighting pipeline (with versioning and cache)

Legend:

- bounded queue (256) provides backpressure for syntax work; results are unbounded.
- version is monotonic; results with result.version < highlight_version are dropped.
- Priority order: Critical > High > Medium > Low.

```text
[Editor] --request_visible_line_highlighting--> (work_tx, bounded 256)
  |                                                 |
  | enqueues WorkItem { buffer_id, line_index, full_content,
  |                     language, priority, version }
  v                                                 v
          +----------------------------+
          | Worker thread              |
          | - coalesce by (buf,line)   |
          | - prefer higher priority   |
          | - parse with Tree-sitter   |
          +-------------+--------------+
               |
          HighlightResult
               v  (result_tx, unbounded)
          +-------------+--------------+
          | Syntax dispatcher thread   |
          | - drop if result.version < |
          |   editor.highlight_version |
          | - editor.apply_* (cache)   |
          | - send UI RedrawRequest    |
          +-------------+--------------+
               |
               v
        [AsyncSyntaxHighlighter cache (LRU)]
```

### Window layout and splits (example)

Legend:

- Reserved rows = status line + optional command line.
- Each window tracks viewport_top and horiz_offset independently.
- Active window id controls cursor-line highlight.

```text
+---------------- Terminal (width x height) ----------------+
| +----------+ +----------------------------+               |
| | Window 1 | |          Window 2          |               |
| | (id=1)   | | (id=2)                     |               |
| | buf=...  | | buf=...                    |               |
| +----------+ +----------------------------+               |
| +--------------------------+ +-------------+              |
| |        Window 3          | |  Status     |              |
| | viewport_top=...         | |  line       |              |
| +--------------------------+ +-------------+              |
| Command line (optional)                                   |
+-----------------------------------------------------------+
```

### Rendering: gutter, wrapping, and highlights

```text
Legend: '#' = gutter (numbers/marks), '|' = column boundary

Row view (no wrap):
####|This is a line of text...              |
####|Next line ...                          |

Wrap enabled (width=12):
####|fn main() {       |
    |    println!("hi");
    |}                  |

Highlight shifting:
- Base line bytes [0..N), slice by horiz_offset/wrap to display_slice.
- Each HighlightRange {start,end} is shifted by slice.start before draw.
```

Notes:

- Wrap width is measured in display columns (unicode-width), not bytes.
- Highlight ranges are byte-based; shifting occurs after safe slicing.

### LRU cache behavior (per-line highlights)

Legend:

- Touch on get/put moves key to newest; capacity eviction pops oldest.
- Current capacity: 2048 entries; theme update clears all entries.
- Buffer-specific invalidation is supported (drop all K where K.buffer_id == X).

```text
Keys: (buffer_id, line_index)

Map<K,V>  <---->  Order (VecDeque<K>)
           [oldest]  k1  k2  ...  kn  [newest]

get(k):   return map[k]; move k to newest end in Order
put(k,v):  if k exists, replace and move to newest
     else if len == cap, evict oldest: pop_front -> remove from map
     then insert (k,v) at newest

Theme update -> clear() -> drop all entries; visible lines re-enqueued
```

## FAQs

- Why don’t I see highlights immediately?
  - Editor enqueues work for visible lines and renders right away; highlights
    appear on the next redraw when results arrive from the worker.

- How are stale highlight results prevented from flashing?
  - Each request carries a version. The dispatcher drops results older than
    Editor.highlight_version, so only the latest context applies.

- Does the cache lead to stale colors after theme change?
  - The cache is cleared on theme update; visible lines are re-enqueued with a
    new version and repainted.

## Glossary

- Editor: Central orchestrator (core/editor.rs) managing buffers, windows, UI, input, search, macros, and async syntax.
- Buffer: In-memory file/text model (core/buffer.rs) with lines, cursor, selection, undo/redo, marks, clipboard.
- Window/WindowManager: Split layout and per-window viewport/horizontal offset control (core/window.rs).
- UI/Renderer: Drawing logic over the Terminal; renders buffers, status/command lines, highlights (ui/renderer.rs).
- Terminal: Thin wrapper over crossterm for buffered terminal IO (ui/terminal.rs).
- EventDrivenEditor: Runtime that spawns input, config watcher, syntax dispatcher threads and processes EditorEvents (input/event_driven.rs).
- Event bus: mpsc::Sender/Receiver channel that carries typed EditorEvent enums among threads.
- Input thread: Polls crossterm events at EVENT_TICK_MS, converts to InputEvent, sends to the bus.
- Config watcher: Watches editor/keymap/theme files, emits ConfigEvent; blocks on filesystem notifications (config/watcher.rs).
- AsyncSyntaxHighlighter: Background worker + cache managing per-line highlights using Tree-sitter (features/syntax.rs).
- SyntaxHighlighter: Per-thread parser/theme that computes HighlightRange values from text (features/syntax.rs).
- HighlightRange: Byte range [start,end) with a HighlightStyle applied by the renderer.
- Priority: Scheduling hint for syntax requests: Critical (cursor), High (visible), Medium/Low (nearby/background).
- highlight_version: Atomic counter on Editor; bumps invalidate in-flight syntax results to avoid stale flashes.
- LRU cache: Small per-line highlight cache in AsyncSyntaxHighlighter keyed by (buffer_id, line_index) with fixed capacity.
- WorkItem: A syntax job: buffer_id, line_index, full_content, language, priority, version.
- HighlightResult: Output of the worker for a single line; validated by dispatcher then cached.
- RenderState: Compact snapshot in EventDrivenEditor for change detection between redraws.
- EditorRenderState: Full state passed to UI::render (buffers shown, layout, highlights, status, config).
- Mode: Editor mode (Normal, Insert, Replace, Visual, VisualLine, VisualBlock, Command, Search, OperatorPending).
- Selection: Visual selections or operator ranges tracked with line/column positions.
- Viewport/horiz_offset: Vertical top row and horizontal column offset used for rendering visible slices.
- Gutter: Left column for line numbers and/or marks; width computed per buffer length and settings.
- Wrap: Grapheme-aware wrapping of logical lines into multiple rows within a window’s content width.
- ThemeConfig/UITheme/SyntaxTheme: Theme system loaded from themes.toml; UI colors and syntax mappings.
- CommandCompletion: Command line completion engine for : commands and paths (features/completion.rs).
- SearchEngine: Text search subsystem with case sensitivity and smartcase (features/search.rs).
- MacroRecorder: Records/plays macros via registers (features/macros.rs).
- TextObjectFinder: Finds text object ranges for operators (features/text_objects.rs).
- LSP (stub): Scaffold for Language Server Protocol client integration (features/lsp.rs).
- crossterm: Terminal input/output library used for events and rendering.
- tree-sitter: Incremental parsing library used to power syntax highlighting.
- crossbeam-channel/std::mpsc: Channels used for async pipelines and event bus communication.
