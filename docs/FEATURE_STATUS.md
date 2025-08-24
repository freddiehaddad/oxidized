# Feature Status

This page summarizes what Oxidized supports today. For the development roadmap, see [ROADMAP.md](./ROADMAP.md).

## Implemented

- Modal editing (Normal, Insert, Command, Visual, Replace, Search)
- Text objects (words, paragraphs, quotes, brackets, tags)
- Operators (`d`, `c`, `y`, `>`, `<`, `~`) with text object integration
- Multi-level undo/redo
- Clipboard operations (character/line)
- Visual selections (character/line/block) and transitions
- Window management (splits, navigation, resize)
- Navigation and movement (hjkl, word/line/viewport)
- Search (forward/backward, n/N)
- Ex-commands (`:w`, `:q`, `:set`, `:setp`, ...)
- Config system (TOML, live reload)
- Syntax highlighting (Tree-sitter, event-driven incremental worker)
  - Incremental reparses with span reuse (per-line state machine; no LRU cache)
  - Markdown: block + inline grammars merged for accurate highlighting
- Terminal integration (alt screen, cross-platform)
- Markdown preview (split view)
  - Open/Close/Toggle/Refresh commands and F8 toggle
  - Update modes: manual, on_save, live with debounce
  - One-way scroll sync (source → preview) when enabled
  - Terminal-safe rendering via pulldown-cmark (no HTML); headings with
    underlines, `[text]`-only links, styled emphasis/strong, inline/code blocks
    without backticks/fences, list/paragraph spacing rules, and semantic spans
    for theming

## In Progress

- Advanced search/replace (regex substitution)
- Code folding and indentation
- File explorer and session support

## Planned

- LSP client and diagnostics
- Completion and navigation (go-to, hover)
- Plugin system and scripting
- Git and project tools
- Terminal emulator and sessions
