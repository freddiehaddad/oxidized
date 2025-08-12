# Contributing Guide (Developer Focus)

This guide complements CONTRIBUTING.md with deeper technical context to help you navigate the codebase and add features safely.

## Build and Run

- cargo build, cargo run filename.txt
- For Windows file-lock issues during tests, ensure no running oxidized.exe is locking target files.
- Recommended: `RUST_LOG=debug` in debug builds; logs go to file by default when TTY.

## Code Conventions

- 50/72 commits enforced via hooks; use `<type>(scope): subject`.
- Run `cargo fmt` and `cargo clippy -- -D warnings` before committing (pre-commit hook also does this).
- Prefer small PRs that each add tests.

## Where to Put Things

- New editor actions: src/input/keymap.rs (wire key to action), then call into Editor/Buffer.
- Rendering tweaks: src/ui/renderer.rs (avoid unnecessary full redraws; keep width/grapheme correctness).
- Buffer mutations: src/core/buffer.rs (ensure undo/redo deltas and grapheme safety).
- Configurable behavior: src/config/** (update schema, defaults, hot-reload, :set wiring).
- Async/background: consider event-driven flows via src/input/event_driven.rs and events.rs.

## Adding a Feature: Mini-Checklist

- Define inputs/outputs and edge cases (empty buffer, EOF, multi-byte graphemes, windows resize).
- Add or update tests in tests/ or src/**/tests.rs.
- Implement minimal change; keep public APIs stable.
- Verify: build, clippy -D warnings, tests, manual smoke (optional).

## Architecture Diagrams

The repository now uses simple inline ASCII diagrams embedded in Markdown
documents (see ARCHITECTURE.md). There are no external diagram sources or
rendered SVGs to maintain.
