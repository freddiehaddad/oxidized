# Contributing

Thanks for your interest in contributing!

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

This repository includes a `commit-msg` Git hook under `.githooks` to
validate the 50/72 rule locally, plus a commit message template at
`.gitmessage`.

To enable them:

```sh
# Use the repo's hooks directory
git config core.hooksPath .githooks

# Use the commit message template by default
git config commit.template .gitmessage
```

You can set these globally with `--global` if you prefer.
