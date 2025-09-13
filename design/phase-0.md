# Phase 0: Skeleton & Clean Exit

## 1. Objective

Establish the minimal async, event-driven skeleton of Oxidized that can:

* Initialize terminal (alt screen + raw mode) using a pluggable backend (crossterm impl).
* Load a single in-memory buffer (welcome text) backed by `ropey`.
* Spawn an async input task translating raw terminal events into normalized `Event`s.
* Run a central event loop consuming events and triggering a full-screen redraw on each key.
* Display mode indicator (Normal) and buffer content.
* Cleanly shutdown and restore terminal state on `:q`, or `Ctrl-C`.

## 2. Scope

In Scope:

* Event enum + channels.
* Terminal backend trait + crossterm implementation.
* Renderer skeleton (full redraw only, no diffing).
* Rope-based buffer abstraction + simple EditorState.
* Input decoding (basic keys, Esc, Enter, Ctrl-C).
* Basic command parser stub recognizing `:q`.
* Config loader stub (reads if exists; ignoring content for now).

Out of Scope (Deferred):

* Multiple windows/tabs.
* Insert/editing (buffer is read-only this phase).
* Undo, macros, marks.
* Syntax highlighting and theming.
* LSP/DAP/git/plugin subsystems.
* WASM runtime.
* Performance optimizations (damage tracking) — redraw whole frame.

## 3. Architectural Touchpoints

Crates introduced this phase:

* `core-events`: Event types + channel helpers.
* `core-terminal`: TerminalBackend trait + crossterm impl.
* `core-render`: Frame, Cell, Renderer (full redraw).
* `core-text`: Buffer wrapper around `ropey`.
* `core-state`: EditorState (buffers, active window placeholder, mode).
* `core-input`: Async input task converting crossterm events.
* `core-config`: Config discovery + loader (TOML parse stub).
Binary crate:
* `ox-bin`: wires everything together.

## 4. Event Additions

`Event` (core):

* `Input(InputEvent)`
* `Command(CommandEvent)` (only Quit for now)
* `RenderRequested` (internal trigger)
* `Shutdown` (graceful termination)
Input variants minimal: Key(key_code, modifiers), Resize(w,h), Paste(String?) (maybe placeholder), CtrlC.

## 5. Data Model Changes

* `EditorState` holds: current mode (Normal), `Vec<Buffer>`, active buffer index.
* `Buffer` holds `ropey::Rope` and name.
* `Mode` enum stub.

## 6. Steps

1. Create crates + workspace wiring.
2. Define base event types & channels (bounded mpsc) in `core-events`.
3. Implement terminal backend trait + crossterm enter/leave alt + raw mode guard.
4. Implement Frame/Cell + Renderer with full redraw (write all cells each frame).
5. Implement `core-text` buffer wrapper using ropey.
6. Implement `EditorState` (single buffer + mode) in `core-state`.
7. Implement input task: map crossterm events -> InputEvents -> send to channel.
8. Implement basic command parser stub (detect `:` sequence & 'q').
9. Implement config loader (path resolution + optional parse).
10. Wire main loop in `ox-bin`: receive events, update state, request render.
11. Add panic hook & Drop guard to restore terminal.
12. Add basic tests (buffer line retrieval, event send/recv compiles).
13. Run clippy & ensure no warnings; adjust lint settings.
14. Document crate-level docs & inline docs for public APIs.

## 7. Exit Criteria

* `cargo build` succeeds (stable toolchain pinned via rust-version = 1.89).
* `cargo clippy -- -D warnings` passes.
* `cargo fmt --all --check` passes.
* Running binary shows welcome buffer and mode indicator.
* Pressing keys triggers redraw (even if no change).
* Typing `:q` (then Enter) or Ctrl-C exits cleanly with terminal restored.
* No panics, no flicker beyond unavoidable initial switch.
* All new public items documented (cargo doc warns minimal or zero).

## 8. Telemetry / Logging

* For now use `tracing` crate (add dependency) with env filter control.
* Minimal spans: startup, input_task, render_cycle.

## 9. Risks & Mitigations

* Terminal left in raw mode if panic: add panic hook + guard object.
* Event channel saturation: choose bounded size (e.g., 1024); on full, drop oldest InputEvent (log debug) — acceptable early.
* Crossterm Windows quirks: test raw mode enter/leave via small integration test (later phases).
* Unicode width issues: initial naive width; plan: integrate `unicode-width` crate later.

## 10. Deferred Items

* Damage-based diff rendering.
* Insert mode & editing.
* Multi-buffer navigation.
* Tree-sitter integration.
* WASM plugin host.
* LSP client.
* Persistent macros.
* Git integration.

## 11. References

* `ropey` crate docs
* `crossterm` crate docs
* Prior art: Neovim event loop separation, Helix architecture (for comparisons)

## 12. Notes

Design choices locked in per clarifications: crossterm, ropey, tree-sitter (later), WASM (wasmtime) future plugin host, JSON-RPC LSP client, no telemetry.

### Deviations / Implemented Adjustments

* Input repeat filtering: we only forward `KeyEventKind::Press` events (crossterm) to avoid duplicate key handling during Phase 0. Auto-repeat handling will be revisited when Insert mode arrives.
* Channel strategy: using standard unbounded `std::sync::mpsc` for simplicity; the bounded + drop-old policy is deferred until higher throughput scenarios appear (later phases with rendering + LSP).
* Command echo: Phase 0 does not yet display a live command line while typing `:q`. This is considered ergonomic polish and will be added alongside a proper status/command line component in a subsequent phase.
* Spans: Added minimal `input_thread`, `event_loop`, and `render_cycle` spans via `tracing`.
* Panic safety: Implemented panic hook logging + RAII terminal guard to guarantee restore.
