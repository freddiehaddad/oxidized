# Vim Command Parity Regression Harness

This document describes the regression coverage that enforces the Vim parity contract for the `core-actions` crate. The harness replays real command sequences (recorded in Vim and mirrored in the NGI adapter) so we can assert buffers, cursors, registers, and undo stacks stay bit-for-bit aligned with stock Vim.

## Running the suite

```console
cargo test -p core-actions --test vim_regressions
```

Each scenario programs the editor model the same way a user would: key by key through the NGI translator and dispatcher. Unicode text is used in every scenario to satisfy the Unicode Fidelity tenet.

## Scenario snapshots

### 1. Linewise change + paste (Unicode)

Keys replayed: `0`, `c`, `w`, `χ`, `α`, `ρ`, `ά`, `<Esc>`, `0`, `y`, `y`, `j`, `p` over the buffer `"καλη μέρα\nemoji🙂 line\nalpha βeta\n"`.

Validates:

- `cw` over Greek text matches Vim word semantics
- Inserted replacement graphemes leave the buffer as `"χαρά μέρα"`
- `yy` + `p` duplicate the line linewise and move the cursor to the new line
- Unnamed and numbered registers receive the pasted line exactly once
- Undo stack records the structural change; redo stack is empty

### 2. Undo/redo with named registers (emoji payload)

Keys replayed: `0`, `y`, `y`, `p`, `u`, `<C-r>`, `"`, `a`, `y`, `y`, `j`, `"`, `a`, `p` over the buffer `"emoji 🙂 test\nalpha\n"`.

Validates:

- Linewise `yy` promotes the payload into unnamed, numbered, and named registers
- `p` + `u` + `<C-r>` faithfully round-trip through the undo stack
- Register `a` paste reproduces the emoji payload with correct newline
- Cursor placement after redo/paste matches Vim (line start of pasted text)

## Extending coverage

When new commands reach parity, add another scenario alongside the existing ones and capture:

- Keys exactly as a Vim session would emit them (including counts, registers, visual toggles, command-line entries)
- Expected buffer contents (full text with newlines)
- Cursor `(line, byte)` tuple
- Relevant register slots (`unnamed`, numbered ring index `0`, explicit named registers)
- Undo/redo depths after the sequence

Keeping the assertions comprehensive ensures we detect regressions in parity or Unicode handling immediately.
