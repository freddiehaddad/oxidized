# Oxidized Architecture Quickstart (At a Glance)

This one-pager gives a fast visual overview of how Oxidized fits together. See [ARCHITECTURE.md](./ARCHITECTURE.md) for full details.

## Core ideas

- Editor orchestrates buffers, windows, UI, input, search, macros, and async syntax.
- Event-driven runtime: background threads send typed events; the main loop reacts and renders.
- UI renders a snapshot (EditorRenderState) and uses cached per-line highlights.
- Async syntax highlighting runs off-thread, prioritized, versioned, and cached via a small LRU.
- Visual selection is anchor-oriented: `Selection.start` is the anchor and is not always
      ordered before `end`. Helpers derive ordered spans; this preserves direction for motions.

## Event-driven threads and event bus

Legend:

- Boxes = components/threads; arrows = messages.
- mpsc = std::sync::mpsc for EditorEvents.
- RedrawRequest -> Editor::render -> UI draws to Terminal.

```text
+---------------------+    mpsc::Sender     +------------------+
| Input thread (poll) |-------------------->|  Event bus (mpsc) |
+---------------------+                     +------------------+
        ^                                              |
  crossterm::event                                     v
                                                       |
+------------------------+  file changes        +---------------------+
| Config watcher thread  |--------------------->| EditorEvent::Config |
+------------------------+                      +---------------------+
                                                       |
                                                       v
+------------------------+  result_rx            +------------------+
| Syntax dispatcher thr. |<----------------------+ Async worker     |
+------------------------+                       | (Tree-sitter)    |
        | apply results                          +------------------+
        v                                                ^
+-------------+         RedrawRequest                    |
|   Editor    |------------------------------------------+
+-------------+            render()
        v
     +-----+     draw commands
     | UI  |-------------------> Terminal (crossterm)
     +-----+
```

## Async syntax highlighting pipeline

Legend:

- work_tx is bounded (backpressure); result_tx is unbounded.
- version is monotonic; stale results are dropped by dispatcher.
- Priority: Critical (cursor) > High (visible) > Medium/Low (nearby/bg).

```text
[Editor] request_visible_line_highlighting
        |   enqueues WorkItem { buf, line, full_content, lang, prio, ver }
        v
  (work_tx, bounded 256) ---> [Worker]
                               - coalesce by (buf,line)
                               - prefer higher prio
                               - highlight one line via Tree-sitter
                               |
                               v
                         (result_tx, unbounded)
                               |
                               v
                         [Dispatcher]
                         - drop if result.ver < highlight_version
                         - cache set((buf,line) -> highlights)
                         - send RedrawRequest
```

## Window layout (example)

Legend:

- Reserved rows: status line + optional command line.
- Each window has its own viewport_top and horiz_offset.

```text
+---------------- Terminal (W x H) ----------------+
| +----------+ +----------------------------+      |
| | Window 1 | |          Window 2          |      |
| +----------+ +----------------------------+      |
| +--------------------------+ +-------------+     |
| |        Window 3          | |  Status     |     |
| +--------------------------+ +-------------+     |
| Command line (optional)                          |
+--------------------------------------------------+
```

## Rendering: gutter, wrapping, and highlights

Legend:

- '#' = gutter; '|' = column edge; highlights are byte ranges per line.
- Wrap width uses display columns (unicode-width), not bytes.

```text
No wrap:
####|This is a line of text...            |

Wrap (width=12):
####|fn main() {       |
    |    println!("hi");
    |}                  |

Shifting ranges:
- Slice by horiz_offset/wrap -> display_slice; shift HighlightRange.{start,end} accordingly.
```

## Compact glossary

- Editor: Central orchestrator; produces EditorRenderState for UI.
- Buffer: Text, cursor, selection, undo/redo, marks, clipboard.
- Window/Manager: Splits and per-window viewport/horizontal scroll.
- UI/Terminal: Renderer and crossterm-backed terminal IO.
- EventDrivenEditor: Spawns input/config/syntax threads; main event loop.
- Event bus: mpsc channel carrying EditorEvent enums.
- AsyncSyntaxHighlighter: Worker + per-line LRU cache of highlights.
- WorkItem/HighlightResult: Job and result for one (buffer,line).
- Priority: Critical/High/Medium/Low scheduling for syntax tasks.
- highlight_version: Atomic counter to drop stale results safely.
- LRU cache: Fixed-capacity per-line highlights; cleared on theme change.

## Testing (snapshot)

- Fast, mostly unit-style integration tests in `tests/` (sub‑second).
- Coverage buckets: buffer edits, motions (`gE`/`ge`, word/WORD), visual + select modes & wrapped selections, text objects, search, macros, keymaps/events, commands & config persistence, UI/status/wrap/layout, completion, replace/paste, window mgmt, grapheme safety.
- Motion pattern: baseline, punctuation, cross-line, start-of-buffer, repetition.
- Add regressions by appending to existing domain file; short `//!` doc comment when needed.
- Run all: `cargo test`  |  Focus: `cargo test ge_`  |  Single: `cargo test --test g_motion_tests ge_hyphen_treated_as_separate_word`
- Lint gate: `cargo clippy -- -D warnings`.

---

See the complete guide: [ARCHITECTURE.md](./ARCHITECTURE.md)
