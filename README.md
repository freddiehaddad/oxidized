# Oxidized

⚙️ A friendly, modern re-imagining of Vim/Neovim in pure Rust.

*Nothing is stable. Everything can change. Come build it with us.*

## Current status

- Core editing, motions, operators, counts, registers, and paste semantics mirror stock Vim, including Visual mode and named register workflows.
- Unicode fidelity is enforced end-to-end: grapheme clusters, emoji families, and width overrides stay intact through the input pipeline, editor state, and renderer.
- Command-line actions (`:q[!]`, `:w[!]`, `:e[!]`, `:metrics`) match Vim safeguards while exposing Oxidized diagnostics like the metrics overlay.
- A rendering engine chooses between cursor-only, partial, scroll, and full updates while tracking performance counters in the overlay.
- Structured tracing and logging expose motion, edit, render, config, and input events for debugging without leaking pasted content.
- A parity regression harness replays real Vim keystrokes to keep behavior aligned with upstream Vim across Unicode-heavy scenarios.

## Still on the roadmap

- Multiple visible views and split window management
- Search, substitution, syntax highlighting, and theming
- LSP/DAP, completion surface, macro recording, and plugin runtime
- Advanced batching heuristics, collaborative editing, Copilot integration

## Quick start

```console
git clone https://github.com/freddiehaddad/oxidized
cd oxidized
cargo test
cargo run
```

Try: type with `i` (throw in 👨‍👩‍👧‍👦), backspace clusters cleanly, move around, insert newline + undo (`u`) / redo (`Ctrl-R`), open a file `:e Cargo.toml`, toggle metrics `:metrics`, quit `:q`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) (commit hooks, architecture tenets, workflow). Draft PRs & early issues welcome. Tone: breadth‑first, event‑driven, Unicode‑correct, metrics everywhere.

For a deeper dive, check out the NGI design notes (`docs/input.md`) and the logging taxonomy (`docs/logging.md`).

## License

Dual: [Apache 2.0](LICENSE-APACHE.txt) OR [MIT](LICENSE-MIT.txt) — choose what you prefer.

---
If clean‑slate editor architecture, Unicode spelunking, and terminal diff shaving sound fun — star, watch, and jump in.
