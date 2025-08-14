# Contributing Guide (Developer Focus)

This guide complements [CONTRIBUTING.md](../CONTRIBUTING.md) with deeper technical context to help you navigate the codebase and add features safely.

See also:

- [ARCHITECTURE.md](./ARCHITECTURE.md) (full guide)
- [ARCHITECTURE_QUICKSTART.md](./ARCHITECTURE_QUICKSTART.md) (one-page overview)

## Build, Test, and Lint

- Build/run: `cargo build`, `cargo run <filename>`
- Lints: `cargo clippy --all-features --workspace -- -D warnings`
- Format: `cargo fmt`
- Tests: `cargo test --all-features --workspace`

Tips:

- Run a single integration test file: `cargo test --test ex_command_tests`
- Run a single test by name substring: `cargo test name_substring`
- On Windows, if tests fail due to file locks, ensure no running oxidized.exe holds files in target/.
- For debugging, set `RUST_LOG=debug` (logs typically go to file when attached to a TTY).

Benches:

- Run Criterion benchmarks: `cargo bench` (see `benches/` for available suites).
- Current suites: `search_bench`, `wrap_bench`, `viewport_hscroll_bench`, `gutter_status_bench`, `visual_block_bench`.
- Selection performance: `visual_block_bench` confirms block highlight span math is trivial (no caching needed yet).

Selection semantics:

- `Selection.start` is the anchor (original point). Do not assume ordering; derive ordered ranges via helpers.
- Avoid manual normalization that would swap columns on same-row backward selections.

## Quick triage flow

When iterating locally, prefer this tight loop:

1. Build: `cargo build`
2. Lint: `cargo clippy --all-features --workspace -- -D warnings`
3. Test fast path: filter by name, e.g., `cargo test editor_basic` or a single file: `cargo test --test ex_command_tests`
4. Run a smoke: `cargo run <file>` and exercise the change

If you touched public behavior, add/update tests, then repeat steps 1–3 until green.

## High‑signal tests to consult

- [tests/editor_tests.rs](../tests/editor_tests.rs): editor core behaviors and redraw expectations
- [tests/keymap_tests.rs](../tests/keymap_tests.rs): key sequence → action wiring
- [tests/ex_command_tests.rs](../tests/ex_command_tests.rs): ex commands and :set/:setp
- [tests/search_integration.rs](../tests/search_integration.rs): search engine and navigation
- [tests/grapheme_cursor_tests.rs](../tests/grapheme_cursor_tests.rs): grapheme/emoji edge cases
- [tests/ui_tests.rs](../tests/ui_tests.rs): renderer and statusline

## Code Conventions

- 50/72 commits enforced via hooks; use `<type>(scope): subject`.
- Run `cargo fmt` and `cargo clippy -- -D warnings` before committing (pre-commit hook also does this).
- Prefer small PRs that each add tests.

## Where to Put Things

- New editor actions: [src/input/keymap.rs](../src/input/keymap.rs) (wire key to action), then call into Editor/Buffer.
- Rendering tweaks: [src/ui/renderer.rs](../src/ui/renderer.rs) (avoid unnecessary full redraws; keep width/grapheme correctness).
- Buffer mutations: [src/core/buffer.rs](../src/core/buffer.rs) (ensure undo/redo deltas and grapheme safety).
- Configurable behavior: src/config/** (update schema, defaults, hot-reload, :set wiring).
- Async/background: prefer event-driven flows via [src/input/event_driven.rs](../src/input/event_driven.rs) and
 [events.rs](../src/input/events.rs). The async syntax worker lives in [src/features/syntax.rs](../src/features/syntax.rs) and sends
 results over a channel consumed by the dispatcher thread in
 [src/input/event_driven.rs](../src/input/event_driven.rs).

## Adding a Feature: Mini-Checklist

- Define inputs/outputs and edge cases (empty buffer, EOF, multi-byte graphemes, windows resize).
- Add or update tests in tests/ or src/**/tests.rs.
- Implement minimal change; keep public APIs stable.
- Verify: build, clippy -D warnings, tests, manual smoke (optional).

## Architecture Notes

- The main event loop blocks on events (no periodic tick). The input thread
 polls every 16ms (EVENT_TICK_MS) via `crossterm::event::poll` to stay
 responsive and allow graceful shutdown without being stuck in a blocking
 `event::read`. The config watcher blocks on file events, and the syntax
 results dispatcher thread blocks on its channel. If you want fewer idle
 wakeups, you can:
  - switch the input to blocking `event::read` (simpler, but shutdown waits
  for the next input unless you add an interrupt mechanism), or
  - implement an adaptive backoff: increase poll timeout when idle, reset on
  activity.
- Async syntax uses a bounded work queue, coalescing by (buffer,line) with
 priority, and a monotonic version token. Results older than the current
 version are dropped before applying.
- A small LRU cache limits per-line highlight storage; cache stats are exposed
 via the editor for debugging.

## Architecture Diagrams

The repository uses simple inline ASCII diagrams embedded in Markdown
documents (see ARCHITECTURE.md and ARCHITECTURE_QUICKSTART.md). There are no
external diagram sources or rendered SVGs to maintain.
