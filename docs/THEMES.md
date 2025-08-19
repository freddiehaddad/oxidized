# Themes and Colors

Themes are defined in `themes.toml`. You can define multiple themes and select
the active one via `[theme].current` or at runtime (`:setp colorscheme <name>`).

## Minimal theme example

```toml
[theme]
current = "my_theme"

[themes.my_theme]
name = "My Theme"

[themes.my_theme.ui]
background = "#1e1e1e"
line_number = "#5a5a5a"
line_number_current = "#a0a0a0"

[themes.my_theme.tree_sitter]
plain_text = "#d4d4d4"
keyword = "#c586c0"
string = "#ce9178"
comment = "#6a9955"
```

## Completion popup colors

The command-line completion popup uses dedicated UI colors:

- `completion_key_fg`
- `completion_alias_fg`
- `completion_value_fg`
- `completion_desc_fg`
- `completion_menu_bg`
- `completion_selected_bg`

See the root `themes.toml` for the canonical list and default values.

## Tips

- Unspecified keys fall back to built-in defaults.
- After editing `themes.toml`, changes apply live; the syntax cache is cleared
  and visible lines are re-enqueued for highlighting.

## Markdown preview theming

The Markdown preview uses semantic categories that map to the
`[themes.<name>.tree_sitter]` colors.

Common categories used by the preview:

- `comment` — blockquote prefixes and inline/code block content
- `punctuation` — link brackets `[` and `]`, task list markers
- `attribute` — inner link text (e.g., `[text]`)
- `delimiter` — heading underline lines, horizontal rules
- `type` — base heading text (non-bold/italic parts)
- `constant` — bold text regions
- `string` — italic text regions

You can tweak these keys in your theme to change the preview appearance.
