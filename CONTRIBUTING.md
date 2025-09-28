# Contributing to Oxidized

> *A clean‑slate, aggressively modular terminal editor in Rust — breadth‑first, event‑driven, Unicode‑correct, and unapologetically experimental.*
>
> **Nothing is stable yet.** We optimize for architectural clarity and iteration speed; APIs and internal crate boundaries may shift. If that excites you more than it scares you: welcome.

---

## Quick Start (TL;DR)

```console
# fork + clone
git clone https://github.com/<you>/oxidized
cd oxidized

# install latest stable rust (if you have not)
rustup toolchain install stable
rustup default stable

# run full test + style + lint cycle (what CI + pre‑commit expect)
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --all-targets --all-features --no-fail-fast

# run the editor
cargo run
```

Set up the local git hooks (enforces commit message format + style/lint/tests on each commit):

```console
# One-time (copies the repo's hook scripts into .git/hooks via a core.hooksPath)
git config core.hooksPath .githooks
# or globally (optional) so any repo using .githooks/ works
# git config --global core.hooksPath .githooks
```

You can now commit; the hooks will block non‑conforming messages or failing code.

---

## Project Vision

Oxidized aims to become a modern, scriptable terminal editor with **Vim/Neovim behavioral fidelity** where it matters (motions, modes, registers, macros, splits, undo semantics) while enabling cleaner evolution in areas historically constrained by legacy design.

Guiding goals:

* **Breadth‑First Development** – Always runnable; features appear in coarse form first, then optimized/refined (e.g. full redraw before diff trimming, then partial granular paths, then scroll region shifts, etc.).
* **Event‑Driven & Async** – No busy loops. React to OS/input events; use async channels & tasks instead of ad‑hoc threads blocking on sleeps.
* **Unicode Correctness** – All text operations respect extended grapheme clusters. No rendering or editing that splits a cluster.
* **Extensibility & Modularity** – Clean crate boundaries by concern (text, render, terminal, input, actions, state, config, etc.).
* **Performance by Design** – Partial rendering, batching, scroll region usage, intelligent dirty set reduction, and metrics instrumentation from the start.
* **Fearless Evolution** – API breakage is fine until 1.0. We remove deprecated code immediately rather than carry legacy ballast.

---

## Repository / Crate Layout (High Level)

| Crate | Purpose (evolving) |
|-------|--------------------|
| `ox-bin` | Binary entrypoint: CLI parsing, tracing subscriber setup, event loop bootstrap. |
| `core-text` | Unicode cluster model, width probing, motions groundwork. |
| `core-render` | Rendering engine: partial diffing, status line, dirty tracking, batching, scheduling. |
| `core-terminal` | Terminal capability probing & abstraction (scroll region, etc.). |
| `core-input` | Async input service, key event normalization & translation. |
| `core-events` | Event source abstraction + orchestration. |
| `core-actions` | Editor actions (motions, edits, undo/redo dispatch, command handling). |
| `core-state` | Editor state structures (buffers, views, undo log). |
| `core-config` | Configuration loading (TOML), future settings surface. |
| `core-model` | Layout abstractions & higher-level editor view model. |
| `core-plugin` | (Scaffold) future plugin / extension surface. |

---

## Architecture & Design Tenets

1. **Event‑Driven** – Systems emit events; consumers react. Avoid polling; blocking allowed only on meaningful waits (input, channel recv, async join).  
2. **Async First** – Prefer async channels (e.g. tokio, crossbeam) for coordination; spawn tasks with crisp ownership boundaries.  
3. **Rendering Quality** – Flicker‑free, minimal cell emission. Always correct over clever; optimize only after correctness tests pass.  
4. **Unicode Fidelity** – Treat grapheme clusters as atomic for cursoring, deletion, width measurement, diff emission.  
5. **Modular Crates** – Public API boundaries reflect conceptual seams (text vs render vs input). Cross‑crate dependencies should feel directional (lower layers unaware of higher ones).  
6. **Breadth Before Depth** – Land skeletal versions early (feature flags or minimal stable surfaces) then iterate.  
7. **No Deprecated Graveyard** – Remove old code immediately once replaced; git history suffices.  
8. **Rust Idioms** – Favor iterators, ownership clarity, minimal unsafe (none currently). Zero‑cost abstractions > premature micro‑optimizations.  
9. **Latest Stable Toolchain** – Track stable Rust; update freely.  
10. **Documentation as Code Evolves** – Keep module docs, rustdoc comments, and design notes current with behavior.  
11. **Config = TOML** – Human‑friendly, explicit, versionable.  
12. **Strict CI Hygiene** – Warnings are errors. Formatting enforced. Tests green.  
13. **Metrics & Tracing Built‑In** – Spans around render phases, motions, edits; counters for diff paths, trim successes, etc.

### Logging & Tracing (Summary)

Structured logging uses `tracing` with namespaced targets (e.g. `actions.dispatch`, `actions.translate`, `render.engine`, `render.scheduler`, `state.undo`, `runtime`, `io`).

Level intent:

* **error** – user-visible failure (IO/read/write, render error).
* **warn** – anomaly but continuing (mixed line endings, truncated write).
* **info** – lifecycle & explicit user actions (startup, shutdown, `:metrics` toggle, file load/save result).
* **debug** – decision heuristics (render escalation cause, operator apply resolution, scheduler collapse rationale).
* **trace** – hot-path per-event detail (motions, edits, undo push/pop, scheduler marks) kept lean.

Emit structured fields (`motion`, `line`, `byte`, `semantic`, `effective`, `count`, `register`, etc.) instead of concatenated text. Prefer events; use spans only when timing or nested operations matter (e.g. `render_cycle`). Avoid logging full buffer text at trace/debug—log sizes/counts instead.

Full target table, field catalogue, examples, and checklist live in [`docs/logging.md`](docs/logging.md).

---

## Getting Set Up

Install prerequisites:

* Latest stable Rust (`rustup` recommended).
* `cargo-nextest` for the test hook:  

  ```console
  cargo install cargo-nextest --locked
  ```

* A reasonably modern terminal emulator that supports ANSI control sequences (scroll region optimizations may be capability gated).

(Optional) Enable incremental re‑build speedups:

```console
# use sccache (if you like)
cargo install sccache --locked
set SCCACHE_DIR=~/.cache/sccache
```

---

## Git Commit Conventions (Enforced)

The hook `.githooks/commit-msg` rejects messages not matching:

```text
<type>: <summary>
<type>(<scope>): <summary>
```

Rules:

* `type` = lowercase letters `[a-z]+` (examples: `feat`, `fix`, `refactor`, `docs`, `test`, `perf`, `build`, `ci`, `chore`).
* Optional `(scope)` = `[a-z0-9._-]+` (crate name, subsystem, or concise target: `core-text`, `render`, `undo`, `metrics`).
* One space after the colon.
* Subject length ≤ 50 chars.
* If a body is present: single blank line after subject, hard‑wrap lines ≤ 72 chars, exactly one blank line between paragraphs, no trailing blank lines at end of file.
* Trailers (e.g. `Co-authored-by:`) allowed if they obey wrapping & no trailing blank lines.
* Allowed bypass: messages starting with `fixup!` or `squash!`, and merge commits.

Examples:

```text
feat(render): batch contiguous plain cells
fix(core-text): correct grapheme width for family emoji
refactor: collapse redundant render delta merges
docs: add partial rendering overview
```

If the hook fails it will explain the violation; amend after fixing:

```console
git commit --amend
```

---

## Development Workflow

1. **Discuss Early** – Open an issue or draft PR for architectural shifts; small fixes can go straight to PR.  
2. **Branch Naming** – `feat/<short>`, `fix/<short>`, `refactor/<short>`, `docs/<short>`, etc. (Not enforced; just helps scanning).  
3. **Keep Commits Focused** – One conceptual change per commit where practical (helps bisectability).  
4. **Prefer Small PRs** – If a feature spans subsystems, land enabling refactors first.  
5. **Tests Before / With Behavior** – Add or adjust tests demonstrating the change (unicode edge, rendering optimization, undo semantics).  
6. **Trace Strategically** – Use `tracing::info_span!` / `trace_span!` with stable, kebab‑case names. Avoid noisy per‑cell logs.  
7. **Review Expectations** – PR description: what changed, why, any perf notes, follow‑ups.  

---

## Coding Standards

* **Style** – `cargo fmt` enforced. Avoid needless `#[allow(...)]`; prefer fixing root cause.
* **Warnings** – Denied under Clippy. If a lint is noisy but intentional, justify with a scoped `#[allow(lint_name)]` and a comment.
* **Error Handling** – Use `anyhow` at edges (binary) for context; internal crates can prefer rich enums if that clarifies logic.
* **Allocation Awareness** – Rendering paths should avoid incidental `String` churn; prefer slices & iterators.
* **No Global State** – Pass dependencies explicitly; minimize `lazy_static` / `once_cell` unless clearly beneficial.
* **Unsafe** – Currently zero; introduce only with a compelling benchmark & documented invariants.
* **Unicode** – Always operate at grapheme cluster boundaries (`unicode-segmentation`); never index raw bytes for logical cursoring.
* **Concurrency** – Use async channels / tasks for decoupling; avoid shared mutable state unless synchronized explicitly.

---

## Testing

We use `cargo nextest` for speed + isolation. Command invoked by the pre‑commit hook:

```console
cargo nextest run --all-targets --all-features --no-fail-fast
```

Guidelines:

* Prefer **behavioral tests** that assert external effects (render dirty classification, unicode cluster handling) over tautological unit tests.
* Keep tests deterministic; avoid sleeping for timing—instrument metrics or expose hooks.
* Name tests clearly (`feature_condition_expectation`).
* When fixing a bug, add a test that fails before your fix and passes after.

---

## Performance & Metrics

Performance evolution is incremental. Existing instrumentation tracks frame types, dirty funnel counts, scroll shift savings, trim attempts/success, and batching metrics. When adding a new optimization:

* Demonstrate correctness first.
* Add counters/spans rather than `println!` noise.
* Note any measurable deltas (lines repainted, cells emitted) in the PR description.

Future: a richer `:metrics` UI & automated perf guardrails.

---

## Documentation & Design Notes

* Update rustdoc comments when altering behavior (public or internal) — treat them as part of the interface for maintainers.
* Keep examples minimal but runnable where possible.

---

## Contributing New Features (Checklist)

1. Clarify the scope (issue or short proposal) — especially for editor model, rendering pipeline, or plugin surface changes.
2. Land enabling refactors (separate PRs) if they simplify the main change.
3. Implement breadth-first (get a working baseline quickly) then layer refinements.
4. Add/extend tests covering core invariants.
5. Add tracing spans or metrics if performance-sensitive.
6. Update docs / design notes.
7. Ensure `pre-commit` passes locally.
8. Keep commit messages conformant.

---

## Handling Breaking Changes

Before 1.0 we break internal APIs freely to preserve conceptual clarity. Provide a crisp commit subject & PR description referencing the motivation; update any impacted design docs.

---

## Issue Labels (Planned)

Once volume grows we may introduce:

* `good-first-issue` – Small, low-risk starter tasks.
* `needs-design` – More discussion/spec required.
* `perf` – Performance & instrumentation work.
* `unicode` – Grapheme/width correctness tasks.
* `rendering` – Partial diff / batching / viewport logic.

(Labels will appear gradually; propose new ones via issue if helpful.)

---

## FAQ for Contributors

**Can I add feature X right now (LSP, DAP, Git, Copilot, collaborative editing)?**  
Bigger integrations will wait until core rendering, windowing, and text model layers are sturdier. Spikes/design docs welcome; large code drops premature.

**Why are some subsystems simplistic?**  
Breadth-first: correctness > micro-optimization; detail work comes after end-to-end viability.

**How strict is commit formatting?**  
Exact — the hook rejects on violations to keep history uniformly scannable.

**What if I need multiple paragraphs in a commit body?**  
One blank line between each; lines wrapped ≤ 72 chars; no trailing blank line at end.

**Can I submit draft PRs?**  
Yes, encouraged for early design validation.

**I hit a Unicode edge case. What helps?**  
Provide the exact grapheme sequence (escape form if ambiguous), cursor expectations, observed vs expected behavior, terminal type.

---

## Code of Conduct

Use welcoming, constructive language. Assume positive intent. No harassment, discriminatory behavior, or hostility. Disagreements focus on code & design, not people. (A formal document may be added later; for now: be excellent to each other.)

---

## License

Dual-licensed under Apache-2.0 or MIT (your choice). Contributions are accepted under the same terms; by submitting a PR you agree your work is provided under both.

---

## Final Note

This project is deliberately in the fun phase: shaping primitives, proving rendering paths, tightening invariants. If you enjoy early architecture, spelunking Unicode, or shaving milliseconds off terminal diff emission — you are very much in the right place.

Welcome aboard ⛵
