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
2. EventDrivenEditor wraps Editor and spawns threads (input, config watch, render stub) and an event bus. Syntax is refreshed event-driven (no background syntax thread).

### Timing and Cadence

- The input thread uses crossterm polling with EVENT_TICK_MS (default 16ms) to stay responsive; the main loop blocks on events and wakes only when events arrive.

1. Input thread reads terminal events and sends Input events.
2. EventDrivenEditor processes events, mutates Editor as needed, and sends UI RedrawRequest when state changes.
3. Editor::render() snapshots EditorRenderState and asks UI to draw via Terminal.

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

## Syntax Highlighting (event-driven)

- No background syntax thread. Highlighting occurs on-demand (synchronously for small units like single lines), and the editor sets a needs_syntax_refresh flag to prompt redraws as needed.

Design note: we prioritize responsiveness and simplicity over long-running parsing. Expensive full-file highlighting is intentionally avoided in the synchronous path.

## Windows and Viewports

- WindowManager manages splits, sizing, and viewport for each window.
- EditorRenderState contains per-buffer highlight cache keyed by (buffer_id, line_index).

## Testing

- Unit tests under tests/ and src/**/tests.rs
- Grapheme/emoji regression tests live in tests/grapheme_cursor_tests.rs

## Alternatives and trade-offs

- Fully blocking main loop without a tick: Replace the short recv_timeout with a blocking recv and rely entirely on incoming events to drive progress. This can reduce wakeups, but you’ll need a separate waker event for timed UI updates if ever introduced.
- Channel select instead of periodic polling: Using a select over channels (e.g., crossbeam-channel) would allow clean blocking on multiple sources (config events, system signals) without a tick. The current std::mpsc + short timeout keeps dependencies minimal and is already efficient.
- Input thread using blocking read: Switching from crossterm::event::poll to blocking event::read can further reduce wakeups, but graceful shutdown then requires an interrupt mechanism; the current 16ms poll balances responsiveness and simple shutdown.

## Next Steps for Contributors

- Start in src/input/keymap.rs for keybindings and actions
- Follow into src/core/editor.rs for state changes and rendering
- Inspect src/ui/renderer.rs to understand drawing logic
- Explore src/features/syntax.rs for Tree-sitter integration
