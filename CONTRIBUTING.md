# Contributing

Thanks for your interest in contributing!

Before you start, skim the developer docs under `docs/` for an overview and
inline ASCII diagrams of the architecture:

- docs/ARCHITECTURE.md — high-level guide

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

This repository includes helpful Git hooks under `.githooks`:

- `commit-msg`: validates the 50/72 rule for commit messages
- `pre-commit`: runs `cargo fmt` and `cargo clippy -D warnings` and restages
 formatted files

There is also a commit message template at `.gitmessage`.

To enable them:

```sh
# Use the repo's hooks directory
git config core.hooksPath .githooks

# Use the commit message template by default
git config commit.template .gitmessage
```

You can set these globally with `--global` if you prefer.

## Expectations before opening a PR

- Format: `cargo fmt` (pre-commit will do this automatically)
- Lint: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Test: `cargo test` (close any running oxy.exe on Windows if test teardown
 fails to remove the binary)
- Docs: Update README and/or docs/ when changing user-visible behavior
 (architecture notes live in docs/ARCHITECTURE.md). For async syntax or
 event-loop changes, ensure both README and ARCHITECTURE are updated.
