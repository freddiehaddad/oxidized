# Oxidized

⚙️ A friendly, modern re‑imagining of Vim/Neovim in pure Rust -- currently in active development.

*Nothing is stable. Everything can change. Come build it with us.*

## Status

Still very early. Expect sharp edges, missing pieces, and occasional intentional breakage while the core shape settles. Not a daily driver yet—perfect if you enjoy watching (and nudging) a clean architecture grow.

## Why remake a legend?

Vim/Neovim are incredible, but decades of layered behavior + historical constraints make certain evolutions awkward. Oxidized starts fresh with modern Rust, shedding legacy compromises so core pieces can stay small, testable, and fearless to change.

## Trying it (for the curious)

```console
git clone https://github.com/freddiehaddad/oxidized
cd oxidized
cargo test
cargo run
```

That’s it. No special flags, no build script surprises.

## Contributing

Right now the best help is feedback on architecture, clarity of crate boundaries, and uncovering Unicode or rendering edge cases. Open issues early; we gladly refactor while things are still soft clay.

*Rule of thumb:* if a change crosses more than one concern, split it. If an invariant isn’t obvious, document it right in the code.

## FAQ (tiny & growing)

1. **Is this a Neovim fork?** No — completely fresh Rust code.
2. **Does it do much yet?** Enough to move around, insert text, undo/redo, and quit. That’s the point: breadth first.
3. **Will it embed Vimscript / Lua?** Likely not directly. A lean, capability‑scoped extension layer will arrive later.
4. **Why rewrite instead of contribute to Neovim?** Different experiment: explore how far a fresh, aggressively modular Rust design can go without legacy ballast.
5. **Should I daily‑drive it?** Not yet. Follow along, kick the tires, file issues.

## Dual License

Licensed under [Apache 2.0](LICENSE-APACHE.txt) or [MIT](LICENSE-MIT.txt) -- pick whichever suits your project.

---
If this kind of clean-slate editor architecture excites you: star, watch, and drop ideas. The fun part (real features) is just getting started.
