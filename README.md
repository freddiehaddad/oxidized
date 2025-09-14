# Oxidized

⚙️ A friendly, modern re‑imagining of Vim/Neovim in pure Rust -- currently in active development.

*Nothing is stable. Everything can change. Come build it with us.*

## Status

Still very early. Expect sharp edges, missing pieces, and occasional intentional breakage while the core shape settles. Not a daily driver yet -- perfect if you enjoy watching (and nudging) a clean architecture grow.

What works today:

* Move around with classic hjkl, 0/$, naive w/b word hops.
* Insert text (full Unicode grapheme clusters — emoji families, combining marks, CJK) without tearing them apart.
* Backspace respects whole clusters (no half‑emoji horror).
* Undo / Redo with sensible insert run coalescing (Esc or newline = boundary).
* Command line stub: `:q` exits; everything else politely shrugs.
* Grapheme‑aware hardware cursor placement (occasionally cheeky with the widest emoji, but trying its best).
* Tracing spans for motions & edits so we can later profile without ripping things back open.

Not (yet) there:

* Diff/partial rendering (full frame redraw for now, but flicker‑free).
* Fancy word boundary logic (currently a friendly, naive take).
* Multiple buffers, search, syntax, plugins, or highlighting.
* Time‑based undo coalescing.

If that sounds fun rather than disappointing -- you get the vibe.

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

### Quick things to try

* `i` then type some text (throw in an emoji or two 👨‍👩‍👧‍👦) then `Esc`.
* Backspace over a combining mark or emoji cluster -- it disappears cleanly.
* Move with `h j k l 0 $ w b` and watch the status line update.
* At end of a line press `w` a few times -- naive word hops but it behaves.
* Insert a newline (Enter) then undo (`u`) and redo (`Ctrl-R`).
* Type `:q` to exit. (Other commands just smile and vanish.)

If the cursor looks a little shy around very wide glyphs, it’s just early -- we haven’t taught it every trick yet.

## Contributing

Right now the best help is feedback on architecture, clarity of crate boundaries, and uncovering Unicode or rendering edge cases. Open issues early; we gladly refactor while things are still soft clay.

## FAQ

1. **Is this a Neovim fork?** No -- completely fresh Rust code.
2. **Does it do much yet?** Enough to move around, insert text, undo/redo, and quit. That’s the point: breadth first.
3. **Will it embed Vimscript / Lua?** Likely not directly. A lean, capability‑scoped extension layer will arrive later.
4. **Why rewrite instead of contribute to Neovim?** Different experiment: explore how far a fresh, aggressively modular Rust design can go without legacy ballast.
5. **Should I daily‑drive it?** Not yet. Follow along, kick the tires, file issues.
6. **Why is the cursor sometimes bashful with super wide emoji?** Terminal quirks + early rendering path. We’ll tighten it up when diff rendering lands.
7. **Will performance tank with full redraws?** Not for the tiny files we test with. We’ll switch to dirty / diff updates before scale matters.
8. **Why does a lonely `#` sometimes flash on a blank line when I move with `j`/`k`?** That’s an early rendering quirk: the naive full‑frame path still writes raw newline control characters into the buffer, so the terminal cursor hops unexpectedly and a nearby heading `#` can momentarily appear on the blank line. It’s harmless, will disappear once we strip control chars (or land the RenderDelta + smarter painter), and is intentionally left unfixed until we reach the planned render cleanup step.

## Dual License

Licensed under [Apache 2.0](LICENSE-APACHE.txt) or [MIT](LICENSE-MIT.txt) -- pick whichever suits your project.

---
If this kind of clean-slate editor architecture excites you: star, watch, and drop ideas. The fun part (real features) is just getting started.
