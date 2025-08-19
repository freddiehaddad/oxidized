# Configuration Guide

Oxidized uses human-friendly TOML files with live reload and a clear split
between session-only and persisted changes.

- Session-only: use `:set` to apply a change until exit.
- Persisted: use `:setp` to write the change back to your TOML config.

## Files

- `editor.toml` — global editor behavior and UI options
- `keymaps.toml` — key bindings per mode
- `themes.toml` — themes and syntax/UI colors

You can keep these at the repo root or under `~/.config/oxidized/` (XDG). The
editor searches the current directory first, then the config directory.

## Editor settings (editor.toml)

We recommend keeping only what you change from the defaults, rather than
copying the full file. See the repo root `editor.toml` for the authoritative
defaults. Common examples:

```toml
[display]
show_line_numbers = true
color_scheme = "default"

[behavior]
ignore_case = true
smart_case = true

[interface]
completion_menu_height = 10
percent_path_root = true
```

Notes:

- Live reload applies on save.
- Query/toggle at runtime: `:set wrap?`, `:set nowrap`, `:setp nowrap`.

## Keymaps (keymaps.toml)

Prefer small overrides vs full copies. Define only the keys you want to
change; unspecified keys fall back to defaults.

Example: remap save to Ctrl+S in Insert mode as well:

```toml
[insert_mode]
"Ctrl+s" = "save_file"
```

See [KEYMAPS.md](./KEYMAPS.md) for mode names and tips.

## Themes (themes.toml)

Theme entries can be partial; unspecified values use built-in defaults. Start
by copying the minimal skeleton and tweak colors gradually.

```toml
[theme]
current = "my_theme"

[themes.my_theme]
name = "My Theme"

[themes.my_theme.ui]
background = "#1d1f21"
plain_text = "#c5c8c6"

[themes.my_theme.tree_sitter]
keyword = "#b294bb"
string = "#b5bd68"
```

Completion popup colors are themeable; see the defaults in the repo
`themes.toml` and the README section on completion UI.

## Tips

- Use `%` rooted paths in `:e`/`:w` when `percent_path_root` is enabled.
- Keep diffs readable: avoid committing full copies of defaults; store deltas.
- For portability, check in a minimal `editor.toml` in your project and keep
  personal tweaks under your home config.

## Markdown preview settings and commands

You can control the built-in Markdown preview via config and Ex commands.

Editor settings (in `editor.toml`, typically under a dedicated table):

```toml
[markdown_preview]
# When to refresh the preview: "manual" | "on_save" | "live"
update = "live"

# Debounce (milliseconds) used when update = "live"
debounce_ms = 150

# Keep the preview viewport aligned with the source buffer
scrollsync = true

# Wrap long lines in the preview window only (independent from main editor "wrap")
wrap = true

# Inline/block math passthrough in the preview: "off" | "inline" | "block"
math = "off"

# Large-file behavior: "truncate" (cap preview lines) | "disable" (render all)
large_file_mode = "truncate"
```

Runtime commands:

- `:MarkdownPreviewOpen` — open a right split with the preview
- `:MarkdownPreviewClose` — close the preview split
- `:MarkdownPreviewToggle` — toggle the preview on/off (bound to `F8` and to
  the leader mapping `leader m p` by default)
- `:MarkdownPreviewRefresh` — re-render the preview now

Runtime tweaks (session-only) and persistence (write to config):

- Session-only: `:set mdpreview.update live`, `:set mdpreview.scrollsync`
- Persisted: `:setp mdpreview.update on_save`, `:setp nomdpreview.scrollsync`

Preview wrapping:

- Toggle for this session: `:set mdpreview.wrap` / `:set nomdpreview.wrap`
- Persist to config: `:setp mdpreview.wrap` / `:setp nomdpreview.wrap`
- This only affects the preview pane; the main editor uses `wrap` under
  `[behavior]`.

Notes:

- Completions suggest valid values for `mdpreview.*` options.
- The preview uses a terminal-safe renderer (pulldown-cmark) and no header banner.

### Preview rendering behavior

The built-in preview renders plain text (no HTML) with semantic spans for
theming. Key behaviors:

- Headings: no leading `#`; the text is followed by an underline (`=` for H1,
  `-` for H2–H6) sized by the Unicode display width, then a blank separator
  line.
- Links: shown as `[text]` only; the URL is hidden. Brackets are styled as
  punctuation; inner text as an attribute.
- Emphasis/strong: italic and bold are styled on the text itself without
  leaving marker characters; no duplicate styling inside links or headings.
- Inline code: shown without backticks; code content is styled as a comment.
- Code blocks: fences are not shown; code lines are indented by 4 spaces and
  styled as comments. A blank line is inserted before a code block inside a
  list item and after any code block.
- Blockquotes: each nesting level is prefixed with `▎`; a blank line is added
  before the first outermost quote and after it closes.
- Lists and paragraphs: exactly one blank line is enforced between paragraphs
  and lists, after an outermost list, and around list/code/heading transitions
  to keep groups readable.
