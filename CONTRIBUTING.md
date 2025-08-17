# Contributing

Thanks for your interest in contributing!

Before you start, skim the developer docs under `docs/` for an overview and
architecture diagrams:

- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) — high-level guide

## Commit message conventions (50/72)

We follow the classic 50/72 commit message style:

- Subject line: max 50 characters, written in the imperative mood
- Blank line between subject and body
- Body lines: wrap at 72 characters

Recommended subject format: `<type>(scope): subject`

Common types: build, chore, ci, docs, feat, fix, perf, refactor, test

### Examples

- `feat(ui): add soft wrapping with word-aware breaks`
- `docs: update README for wrapping and scrolling`

### Tips

- Explain what and why; avoid low-level how unless crucial
- Reference issues/PRs: `Fixes #123`, `Refs #456`
- Use bullets; keep each line ≤ 72 chars

## Git hooks and templates

This repository includes a helpful Git pre-commit hook under `.githooks`:

- `pre-commit`: runs `cargo fmt` and `cargo clippy -D warnings` and restages
  formatted files

To enable them:

```sh
# Use the repo's hooks directory
git config core.hooksPath .githooks
```

You can set this globally with `--global` if you prefer.

## Expectations before opening a PR

- Format: `cargo fmt` (pre-commit will do this automatically)
- Lint: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Test: `cargo test` (close any running oxy.exe on Windows if test teardown
 fails to remove the binary)
- Docs: Update README and/or docs/ when changing user-visible behavior
 (architecture notes live in docs/ARCHITECTURE.md). For async syntax or
 event-loop changes, ensure both README and ARCHITECTURE are updated.
- Visual selection changes: preserve anchor semantics (`Selection.start` is
  anchor; not always <= end). Use helpers (`highlight_span_for_line`,
  `get_selection_range`) for ordered spans and reflect any semantic shifts in
  docs/ + tests.

### Benchmarks

Current Criterion benches (run with `cargo bench`):

- `search_bench` – search engine
- `wrap_bench` – wrapping & Unicode width
- `viewport_hscroll_bench` – horizontal scrolling
- `gutter_status_bench` – gutter + status layout
- `visual_block_bench` – block selection highlight span computation (validates
  nanosecond-scale cost)

If adding a new bench:

- Keep iterations deterministic and avoid I/O.
- Precompute shared data outside the innermost loop.
- Document rationale and expected optimization targets in the commit body.
