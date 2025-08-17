# Vim/Neovim Feature Parity Roadmap

This roadmap tracks progress toward Vim/Neovim parity while preserving Oxidized's performance and UX goals.

## ✅ Currently Implemented

- Modal Editing (Normal/Insert/Visual/Command/Replace/Search)
- Movement (hjkl, words, lines, viewport)
- Text Objects (word/sentence/paragraph/quotes/brackets/tags)
- Operators (d/c/y/>/</~) integrated with motions/objects
- Window Management (splits, navigation, resizing)
- Buffer Management (multi-buffer)
- Search (forward/backward, n/N)
- Undo/Redo (multi-level)
- Configuration (TOML + live reload)
- Syntax Highlighting (Tree-sitter async)
- Clipboard Operations (line/char)
- Scrolling (page/half-page/line, zz/zt/zb)
- Command System (:w, :q, :set, :setp, ...)
- Cursor Shape (mode-aware)

## 🧱 Phase 1: Essential Vim Features (High Priority)

1. Named Registers System (a-z, A-Z, 0-9, special registers)
2. Search & Replace enhancements (history, \c/\C, :s//)
3. Macros: recording/playback improvements
4. Visual mode polish: multi-cursor style edits (investigate)

## 🧭 Phase 2: IDE Features

- LSP client (hover, diagnostics, goto, completion)
- Project navigation and workspace index
- Inline diagnostics and quickfix list

## 🧪 Phase 3: Extensibility

- Plugin system and API (Lua or WASM)
- Configurable commands and keymaps via scripts

## 📈 Performance & UX

- Rendering throughput improvements
- Memory footprint audits
- Large file and long-line stress tests

---

Notes:

- Milestones will be revised as core architecture evolves.
- Tests and benchmarks will accompany major features.
