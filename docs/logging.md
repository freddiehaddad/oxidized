# Logging & Tracing Guide

> Canonical conventions for emitting structured diagnostics in Oxidized.
>
> Goals: fast root–cause during regressions, stable field names for tooling, and **noise discipline** (low default verbosity; rich opt‑in detail).

---

## 1. Philosophy

Breadth‑first development means behavior shifts rapidly. High‑quality tracing lets us validate invariants (render decisions, operator spans, undo stack) without peppering ad‑hoc `println!`. We keep the signal high by:

* Using level semantics consistently (error/warn/info/debug/trace).
* Namespacing via `target` for selective filtering.
* Adding structured fields (cursor, motion, counts) instead of unstructured text.
* Preferring spans only when there is meaningful nested work; otherwise events.

---

## 2. Targets Namespace

| Target Prefix | Purpose | Examples |
|---------------|---------|----------|
| `runtime`     | Process lifecycle & high level loop | startup, shutdown |
| `io`          | File / disk / OS operations | file_open, file_write |
| `config`      | Config load / clamp results | scroll_margin clamped |
| `input.event` | Keypress emission (async task) | keypress, repeat |
| `input.thread`| Async input lifecycle | startup, shutdown |
| `runtime.input` | Runtime key ingestion + timeout bookkeeping | keypress_receive, timeout_flush |
| `events`      | Async event source registry lifecycle | spawning event source |
| `actions.translate` | Key translation decisions | counts, operator apply |
| `actions.dispatch`  | State mutations (motions, edits, operators) | motion, edit_insert |
| `state.undo`  | Undo/redo push/pop + coalescing | push_snapshot |
| `state.registers` | Register writes / rotations | register_write |
| `render.scheduler` | Render delta marks / collapses | render_mark, delta_collapse |
| `render.engine` | Actual frame path choice, hash trims, scroll shift | render_cycle, lines_repaint |
| `render.overlay` | Metrics / status overlay decisions | status_skip |

Avoid inventing new top‑level targets lightly; prefer extending with a suffix (e.g. `render.syntax` later).

---

## 3. Level Taxonomy

| Level  | Use When | Must Include |
|--------|----------|--------------|
| error  | Operation failed and user visible or aborting path (open failure, render error) | `?err`, operation id, path if applicable |
| warn   | Anomaly but continuing (mixed line endings, truncated write) | context fields |
| info   | Rare lifecycle or explicit user action outcome (startup, shutdown, `:metrics` toggle, file loaded/saved) | key result summary |
| debug  | Decision heuristics and medium‑frequency internals (render escalation reason, operator apply summary) | discriminants, counts |
| trace  | High frequency granular events/spans (per motion, edit, delta mark, undo stack churn) | minimal stable fields |

Default recommended filter for development: `info,runtime=info,render.engine=debug,actions.dispatch=debug,actions.translate=debug`.

CI / tests run at `warn` or env override to keep noise down.

---

## 4. Structured Fields Catalogue

| Field | Meaning | Example |
|-------|---------|---------|
| `line` / `byte` | Cursor logical position before action | `line=12 byte=5` |
| `to_line` / `to_byte` | Cursor position after motion (debug only) |  |
| `motion` | `MotionKind` discriminant | `motion="WordForward"` |
| `op` | `OperatorKind` | `op="Delete"` |
| `count` | Expanded count (prefix * post) | `count=6` |
| `register` | Explicit register char (if any) | `register="a"` |
| `semantic` | Requested render delta | `semantic="CursorOnly"` |
| `effective` | Post‑escalation effective delta | `effective="Full"` |
| `dirty_lines` | Candidate dirty line count | `dirty_lines=3` |
| `repainted` | Lines actually repainted |  |
| `escalated` | Bool: lines -> full escalation |  |
| `file` | Path (lossy UTF‑8) | `file="Cargo.toml"` |
| `size_bytes` | File length or buffer bytes |  |
| `line_count` | Buffer total lines |  |
| `snapshot_depth` | Undo depth after push |  |
| `undo_run_len` | Coalesced run grapheme count |  |
| `hash_hits` / `hash_misses` | (Future) line hash reuse metrics |  |
| `trim_cols_saved` | Diff trimming saved columns |  |

Concision rule: Only add a new field if (a) at least one test or alert will rely on it OR (b) repeated debugging has needed it >2 times.

---

## 5. Spans vs Events

Use spans (`info_span!`, `debug_span!`, `trace_span!`) when:

* Multiple child operations attach structured events (e.g., `render_cycle`).
* Timing is measured (store start timestamp inside span entry).

Otherwise prefer a single event macro with fields.

Guidelines:

* `render_cycle` → `debug_span!(target: "render.engine", ...)` with semantic/effective.
* Per‑motion: **event** `trace!(target: "actions.dispatch", motion=?kind, line=view.cursor.line, byte=view.cursor.byte, "motion");`. Avoid opening thousands of spans per second.

---

## 6. Performance Considerations

* Trace level must not allocate large strings each event. Reuse discriminants (`?enum` prints Debug form) or short constants.
* Avoid formatting large buffers or full line contents—emit lengths / counts.
* Wrap optional expensive serializations under `if tracing::enabled!(target: "render.engine", Level::DEBUG) { ... }` guard.
* High‑cardinality values (raw text) go only into debug logs when essential.

---

## 7. Examples

Startup:

```rust
tracing::info!(target: "runtime", version = env!("CARGO_PKG_VERSION"), "startup");
```

File open:

```rust
match std::fs::read_to_string(&path) {
    Ok(content) => {
        tracing::debug!(target: "io", file = %path.display(), size_bytes = content.len(), "file_read_ok");
    }
    Err(e) => {
        tracing::error!(target: "io", file = %path.display(), ?e, "file_read_error");
    }
}
```

Motion:

```rust
tracing::trace!(target: "actions.dispatch", motion=?kind, line=view.cursor.line, byte=view.cursor.byte, "motion");
```

Operator apply:

```rust
tracing::debug!(target: "actions.translate", op=?op, motion=?motion, count, register, "operator_apply_resolved");
```

Render decision mark:

```rust
tracing::trace!(target: "render.scheduler", ?delta, "render_mark");
```

Render cycle:

```rust
let span = tracing::debug_span!(target: "render.engine", "render_cycle", semantic=?decision.semantic, effective=?decision.effective, width=w, height=h);
```

Undo snapshot push:

```rust
tracing::trace!(target: "state.undo", depth=self.undo_stack.len(), kind=?kind, "undo_push_snapshot");
```

---

## 8. Migration Notes

Applied changes (initial migration):

1. Standardized targets per table.
2. Converted high‑level loop and render cycle spans to `debug_span!`.
3. Added structured fields for motions, operators, render decisions.
4. Introduced `operator_apply_resolved` debug event.
5. Normalized IO errors to include `file` and `?e` and target `io`.

---

## 9. Future Extensions (Open Issues)

| Idea | Rationale |
|------|-----------|
| Feature flags for ultra‑verbose hot paths (e.g. line hashing) | Keep default trace cheap |
| Sampling / rate limiting for motion trace floods | Large macro playback scenarios |
| On-demand `:diag` command dumps snapshot bundle | Quick capture for bug reports |
| JSON log layer (optional) | External tooling / structured ingestion |
| Dynamic filter updates via command | Avoid restart for verbosity change |

---

## 10. Contributor Checklist

Before opening a PR touching logging:

* [ ] Chosen correct target & level.
* [ ] Structured fields used instead of concatenated text.
* [ ] No large text payloads emitted each frame.
* [ ] Added new field to catalogue if novel & justified.
* [ ] Updated tests if they rely on message strings (prefer not to).

---

Questions or improvements? Open a `docs` or `infra` labeled issue.
