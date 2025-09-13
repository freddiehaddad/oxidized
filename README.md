# Oxidized

⚙️ A friendly, modern re‑imagining of Vim/Neovim in pure Rust.

*Nothing is stable. Everything can change. Come build it with us.*

## Snapshot

Early—but past “toy”: real partial rendering, undo stack, registers, command stub, metrics overlay. Not a daily driver; perfect if you like shaping architecture while edges are still soft clay.

## What Works (Today)

**Core editing**: insert, backspace (cluster‑aware), newline, delete, undo/redo (run coalescing + duplicate snapshot skip).  
**Motion**: `h j k l 0 $ w b`, half‑page `Ctrl-D / Ctrl-U` honoring margin.  
**Unicode**: Extended grapheme clusters preserved end‑to‑end (emoji families, ZWJ, combining marks, skin tones, CJK) for cursoring, deletion, rendering.  
**Registers**: Unnamed + rotation & numbered behavior; named scaffold present (write support emerging).  
**Paste**: Basic unnamed register paste after cursor (early semantics).  
**Command line (stub)**: `:q` to quit, `:e <path>` load file, `:metrics` toggle overlay. Others no‑op gracefully.  
**Rendering pipeline**: cursor‑only, selective line diff, scroll‑region shift, trimmed interior diffs, status skip cache, safe full redraw fallback.  
**Metrics overlay**: Counts frames, paths (cursor/lines/full/scroll), trim attempts/success, status skips, operator/register counters, cells/commands emitted.  
**Tracing**: Spans around motions, edits, render cycle.  
**Layout groundwork**: Single active view + future split scaffolding (no user‑visible splits yet).  
**Terminal capability probe**: Scroll region detection stub gating optimizations.

## Not Yet (Deliberately)

Splits (actual multiple visible views), search, syntax highlighting & theming, LSP/DAP, completion engine, git integration/mergetool, macro record/replay, plugin runtime, time‑based undo coalescing, advanced batching & diff segmentation, collaborative editing, Copilot integration. Word motions still naive by design.

## Why Start Fresh?

Clean Rust crates let rendering, text model, input, and future extension surfaces evolve without legacy ballast. Breadth‑first lets us optimize only after correctness + boundaries feel right.

## Quick Start

```console
git clone https://github.com/freddiehaddad/oxidized
cd oxidized
cargo test
cargo run
```

Try: type with `i` (throw in 👨‍👩‍👧‍👦), backspace clusters cleanly, move around, insert newline + undo (`u`) / redo (`Ctrl-R`), open a file `:e Cargo.toml`, toggle metrics `:metrics`, quit `:q`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) (commit hooks, architecture tenets, workflow). Draft PRs & early issues welcome. Tone: breadth‑first, event‑driven, Unicode‑correct, metrics everywhere.

### Input System (NGI)

Oxidized ships a Next‑Gen Input (NGI) pipeline: enriched `InputEvent` variants (KeyPress, TextCommit, PasteStart/Chunk/End, Mouse, Focus, RawBytes, CompositionUpdate), NFC normalization and grapheme‑aware inserts, and a trie‑based mapping engine with timeout handling. Paste is streamed and normalized; logging avoids content, emitting only sizes and counts.

- Design: see docs/new_input_system_design.md
- Logging discipline: see docs/logging.md

## Roadmap Pulse (Short Horizon)

Refine scroll + batching, flesh out registers & paste semantics, introduce search & early styling, lay real split windows, then begin syntax + extension surface.

## License

Dual: [Apache 2.0](LICENSE-APACHE.txt) OR [MIT](LICENSE-MIT.txt) — choose what you prefer.

---
If clean‑slate editor architecture, Unicode spelunking, and terminal diff shaving sound fun — star, watch, and jump in.
