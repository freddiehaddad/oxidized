# Keymaps Guide

Oxidized ships with a comprehensive Vim-style keymap. You rarely need to copy
the entire `keymaps.toml` — override only what you need.

## Modes

- `normal_mode`
- `insert_mode`
- `visual_mode`
- `visual_line_mode`
- `visual_block_mode`
- `select_mode`
- `select_line_mode`
- `replace_mode`
- `command_mode`
- `search_mode`
- `operator_pending_mode`

See the repository `keymaps.toml` for all defaults. A drift-guard test
(`tests/keymaps_drift_guard.rs`) ensures the embedded defaults match the file
at the root.

## Examples

- Add an alternative to Visual Block when Ctrl+V is intercepted by your
  terminal:

```toml
[normal_mode]
"Alt+v" = "visual_block_mode"
```

- Quick save in Insert mode:

```toml
[insert_mode]
"Ctrl+s" = "save_file"
```

- Toggle the Markdown preview with F8 (default binding):

```toml
[normal_mode]
"F8" = "markdown_preview_toggle"
```

## Tips

- Keep overrides small; merging is by table, so only changed keys are needed.
- Use `:map` style testing by running the app and trying your keys; errors
  show in the status line.
- For new actions, wire keys in `src/input/keymap.rs` and add tests under
  `tests/`.
