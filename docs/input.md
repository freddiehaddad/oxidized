# Next-Gen Input (NGI) Overview

The NGI pipeline is Oxidized's end-to-end bridge from terminal events to high-level editing actions. It focuses on three promises: keep user intent intact (including multi-codepoint graphemes), keep latency predictable, and keep observability rich without leaking sensitive payloads.

## Goals

- **Unicode fidelity**: normalize and buffer text commits without breaking grapheme clusters, emoji families, or width overrides.
- **Deterministic mapping**: translate raw events into Vim-compatible actions with explicit timeout handling and replayable sequences.
- **Streaming safety**: treat bracketed paste as a first-class session so large payloads move efficiently while staying redacted in logs.
- **Telemetry-first**: surface trace spans and counters for input threads, translator states, and paste statistics without exposing raw content.

## Handshake summary

- `InputEvent::KeyPress` is the sole keypath event emitted by the async input task; legacy `InputEvent::Key` production is considered a bug.
- `EditorRuntime` owns a single `NgiTranslator` instance, invoking `translate_keypress` for every keypress and `flush_pending_literal` on tick-driven timeouts.
- Timeout deadlines derive entirely from keypress timestamps, enabling deterministic tests (`crates/core-actions/tests/ngi_translator.rs`) and parity harnesses.
- Structured tracing is centralized on `input.event`, `runtime.input`, and `actions.translate`, making taxonomy regressions easy to catch with the coverage added in Step 7.

## Event flow

1. **Async input task (`core-input`)** enables bracketed paste, listens on `crossterm::EventStream`, and cooperatively awaits either new events or a shutdown signal via `tokio::select!`.
2. Events enter a bounded `tokio::mpsc` channel as `core_events::Event::Input`, maintaining backpressure and metrics (`CHANNEL_BLOCKING_SENDS`, `PASTE_*`) while tracking async lifecycle counters (`ASYNC_INPUT_*`).
3. **Paste FSM** distinguishes between normal keypresses and bracketed paste sessions, emitting `PasteStart`, `PasteChunk`, and `PasteEnd` markers while tracing only lengths.
4. **NGI translator (`core-actions::NgiTranslator`)** lives inside the runtime and consumes `InputEvent::KeyPress` values, resolving counts, register prefixes, and multi-key sequences into high-level actions via the `core-keymap` trie with explicit timeout deadlines.
5. **Editor runtime (`ox-bin::EditorRuntime`)** owns the translator and timeout ledger, calling `translate_keypress` on each keypress and `flush_pending_literal` on ticks before forwarding resolved actions to the dispatcher.
6. **Dispatcher (`core-actions`)** applies resolved actions to the editor state, keeping undo, registers, and render scheduling in sync with Vim parity expectations.

## Unicode handling

- Keycodes are normalized through `core_events::normalize_keycode` so control characters and printable text share the same downstream rules.
- Text commits are funneled through the centralized normalization and segmentation adapter established in the Vim parity plan (Step 4), keeping grapheme boundaries intact for undo coalescing and rendering.
- Paste chunks flush once buffers reach configurable thresholds, ensuring large emoji-rich payloads stay chunk-aligned without splitting clusters.

## KeyPress lifecycle

- **Emission**: The async input task maps each `crossterm::event::KeyEvent` into a `KeyEventExt` token and enqueues `InputEvent::KeyPress`. Legacy `InputEvent::Key` events are no longer emitted; the runtime keeps a defensive trace when it encounters one so unexpected producers can be spotted quickly.
- **Token semantics**:
  - `KeyToken::Char` carries printable Unicode scalars (already normalized via `normalize_keycode`).
  - `KeyToken::Named` represents non-printable logical keys (Esc, Enter, arrows, function keys).
  - `KeyToken::Chord` pairs a base token with a `ModMask`, preserving modifier combinations like `<C-d>` or `<A-S-Tab>`.
- **Timestamp**: Each `KeyPress` records the `Instant` observed by the input task, enabling deterministic timeout handling and future latency metrics. Timestamps are monotonic per task; consumers must not synthesize or reorder them.
- **Repeat flag**: `repeat = true` only when the terminal reports an auto-repeat (e.g., holding `j`). The retry-aware timeout logic in `ox-bin` can use this to avoid flushing pending trie state prematurely.
- **Logging**: Follow `docs/logging.md`—log chord discriminants (`?token`) and modifier masks, never raw graphemes. The input task emits `trace!(target="input.event", kind="keypress", repeat, mods=?mods)` while the runtime records `trace!(target="runtime.input", kind="keypress_receive", ...)` and downstream translation/dispatch layers rely on `actions.translate` and `actions.dispatch` targets for structured diagnostics.

## Timeout & resolution

- The translator tracks whether a pending sequence requires more input (e.g., distinguishing `d` vs. `dw`).
- `NgiResolution` exposes the resolved action, any pending state, and an optional deadline so the host (e.g., `ox-bin`) can trigger timeouts deterministically.
- Literal sequences (like `<C-v>` inserts) are replayed exactly as Vim would, keeping parity scenarios reliable.

## Observability

- Each stage emits structured tracing:
  - `input.thread` now captures the async task lifecycle (startup, shutdown reason, and any stream errors).
  - `input.event` and `runtime.input` log keypress emission and runtime ingestion metadata without exposing raw content.
  - `input.paste` logs session start/end and chunk lengths.
  - `actions.translate` and `actions.dispatch` capture translator decisions and dispatcher outcomes.
- Counters in `core-events` record channel pressure and paste throughput; the metrics overlay surfaces these values live inside the TUI.

## Extending the pipeline

- New key sequences: add trie entries in `core-keymap` and cover them with NGI translation tests (`crates/core-actions/tests/ngi_*`), including `ngi_translator.rs` for logging/timeout assertions.
- Additional event sources (mouse, focus, IME composition) can enqueue new `InputEvent` variants before translation.
- When broadening command coverage, record real Vim keystrokes and add scenarios to `tests/vim_regressions.rs` so NGI changes stay parity-safe.

For logging guidelines, see `docs/logging.md`. Use the regression harness documented in `docs/commands.md` to verify end-to-end behavior.
