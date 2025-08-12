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
2. EventDrivenEditor wraps Editor and spawns threads (input, config watch, syntax, render stub) and an event bus.

### Timing and Cadence

- The main loop and input polling share a unified tick (EVENT_TICK_MS, default 16ms) to keep cadence simple and responsive.
  
- These values can be tuned at runtime via hot reload; the event threads read them before each sleep.

1. Input thread reads terminal events and sends Input events.
2. EventDrivenEditor processes events, mutates Editor as needed, and sends UI RedrawRequest when state changes.
3. Editor::render() snapshots EditorRenderState and asks UI to draw via Terminal.

Sequence (input → state → render):

  +-------------+     +--------------------+     +-----------+
  |  Terminal   | --> | Input/Event Thread | --> |  Editor   |
  +-------------+     +--------------------+     +-----------+
                                             \              \
                                              \ Redraw req.  \
                                               v              v
                                           +--------+   +-----------+
                                           |  UI    |<- | Renderer  |
                                           +--------+   +-----------+

## Data Model

- Buffer: lines, cursor, selection, undo/redo stacks, marks, clipboard.
- Editor: buffer set, window manager, mode, status, config, themes, async syntax state.
- UI: theme, syntax theme, flags; computes gutter/columns; renders status/command lines.
- Events: strongly-typed enums for Input/UI/Config/Window/Search/Macro/System/LSP.

Component overview (simplified):

  +---------+    owns     +---------+     manages     +-----------+
  | Editor  | ----------> | Buffers | <-------------  |  Windows  |
  +---------+              +---------+                 +-----------+
       |                         |                           |
       | uses                    | contains                  | displays
       v                         v                           v
  +---------+             +------------+               +-----------+
  |  Modes  |             |   Marks    |               |   UI/   |
  +---------+             +------------+               | Renderer |
                                                      +-----------+

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

- ConfigWatcher polls for changes; EventDrivenEditor translates to Config events and forces a full redraw.
- ThemeConfig load_with_default_theme applies color scheme; UI reads it on init and reload.

## Async Syntax Highlighting

- AsyncSyntaxHighlighter runs in a background thread and sets needs_syntax_refresh; the event loop triggers redraws when this flag is observed.

## Windows and Viewports

- WindowManager manages splits, sizing, and viewport for each window.
- EditorRenderState contains per-buffer highlight cache keyed by (buffer_id, line_index).

## Testing

- Unit tests under tests/ and src/**/tests.rs
- Grapheme/emoji regression tests live in tests/grapheme_cursor_tests.rs

## Next Steps for Contributors

- Start in src/input/keymap.rs for keybindings and actions
- Follow into src/core/editor.rs for state changes and rendering
- Inspect src/ui/renderer.rs to understand drawing logic
- Explore src/features/syntax.rs for Tree-sitter integration
