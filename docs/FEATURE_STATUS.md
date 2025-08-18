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
- Syntax highlighting (Tree-sitter, async worker)
  - Markdown: block + inline grammars merged for accurate highlighting
- Terminal integration (alt screen, cross-platform)

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
- Markdown live preview mode (VS Code–style split preview, terminal-safe)
