# Oxidized

⚙️ A friendly, modern re‑imagining of Vim/Neovim in pure Rust — currently in active development.

*Nothing is stable. Everything can change. Come build it with us.*

## Status

Early days. Expect sharp edges, missing features, and intentional breakage while the core shape settles. If you want a daily driver: not yet. If you like watching (and influencing) a clean architecture grow from the ground up: welcome.

## Why remake a legend?

Vim/Neovim are incredible, but decades of layered behavior + historical constraints make certain evolutions awkward. Oxidized starts fresh with modern Rust, shedding legacy compromises so core pieces can stay small, testable, and fearless to change.

## Trying it (for the curious)

```console
cargo run -- some_file.rs
```

## Contributing

Right now the best help is feedback on architecture, clarity of crate boundaries, and uncovering Unicode or rendering edge cases. Open issues early; we gladly refactor while things are still soft clay.

*Rule of thumb:* if a change crosses more than one concern, split it. If an invariant isn’t obvious, document it right in the code.

## FAQ (tiny & growing)

**Is this a Neovim fork?** No — completely fresh Rust code.

**Will it embed Vimscript / Lua?** Probably not as‑is. A small, capability‑scoped extension layer will come later.

**Why rewrite instead of contribute to Neovim?** Different experiment: see how far a fully value‑oriented, aggressively modular design can go without legacy ballast.

## Dual License

* [MIT](LICENSE-MIT.txt).
* [Apache 2.0](LICENSE-APACHE.txt).

---
If this kind of clean-slate editor architecture excites you: star, watch, and drop ideas. The fun part (real features) is just getting started.
