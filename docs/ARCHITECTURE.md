# Oxidized Architecture Guide

This document gives contributors a high-level and practical overview of how Oxidized works under the hood. It includes links to code, key flows, and inline ASCII diagrams to help you get oriented fast.

- Target audience: developers/contributors
- Prereqs: Rust, terminal UI basics

## Top-level Modules

- src/core: buffer, editor, mode, window manager
- src/ui: renderer + terminal glue
- src/input: event loop, key handling, event types
- src/features: syntax highlighting, search, macros, text objects, LSP (stub)
- src/config: editor/theme/keymap config + file watchers

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

## Windows and Viewports

- WindowManager manages splits, sizing, and viewport for each window.
- EditorRenderState contains per-buffer highlight cache keyed by (buffer_id, line_index).

## Testing

- Unit tests under tests/ and src/**/tests.rs
- Grapheme/emoji regression tests live in tests/grapheme_cursor_tests.rs

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
