# Oxidized

⚙️ A friendly, modern re‑imagining of Vim/Neovim in pure Rust -- currently in active development.

*Nothing is stable. Everything can change. Come build it with us.*

## Status

Still early – but now with a real partial rendering MVP under the hood. Expect sharp edges, missing subsystems, and occasional intentional breakage while the core shape settles. Not a daily driver yet — perfect if you enjoy watching (and nudging) a clean architecture grow.

What works today:

* Classic movement: `h j k l 0 $` plus naive `w / b` word hops.
* Half‑page motions: `Ctrl-D` (down) / `Ctrl-U` (up) honoring vertical margin.
* Insert text (full Unicode grapheme clusters — emoji families, combining marks, ZWJ sequences, skin tone modifiers, CJK) without shredding them.
* Backspace removes whole clusters (no half‑emoji horror show).
* Undo / Redo with sensible insert run coalescing (Esc or newline = boundary) + duplicate snapshot dedupe (skips redundant undo states).
* Command line stub: `:q` exits; others smile and vanish (buffer replacement `:e <path>` triggers a full repaint correctly).
* Grapheme‑aware cursor placement (cluster‑accurate; visual polish for some terminals still evolving).
* Partial rendering pipeline (cluster‑aware):
  * Cursor‑only path repaints just old/new cursor lines + status line.
  * Lines path selectively repaints changed lines via line hash diff + dirty tracking.
  * Scroll-region shift path emits real ANSI scroll commands and repaints only entering lines (+ old cursor line) saving lines per frame.
  * Trimmed diff emission repaints only interior mutations (prefix/suffix skip) for large unchanged line regions.
  * Safe full redraw fallback for resize, cold cache, structural edits, or large dirty sets (>=60% of viewport).
  * Status line skip cache avoids repaint when content unchanged (increments metric).
  * Cluster‑aware full + partial emission: every path prints complete clusters (no truncated variation selectors or combining marks).
  * Status‑only semantic delta classification (mode switch, command typing) avoids marking unrelated lines dirty.
* Writer batching foundation: consecutive plain single‑width cells coalesced (lower print command count baseline).
* Resize + buffer replacement invalidation (cache clears; next frame full + rebuild).
* Metrics instrumentation (full vs partial frame counts, dirty line funnel, scroll shifts + lines saved, trim attempts/success, status skips, timings).
* Multi‑view scaffolding (internal single active view; real splits later) + `Layout` abstraction.
* Terminal capability probe stub (scroll region support flag) readying scroll optimization work.
* Real `:metrics` snapshot (multi-line counters: frames, operators, trim, scroll, status skips, print commands, cells printed).
* Tracing spans for motions & edits for future profiling.

Still missing / deferred:

* Advanced batching, multi-region layouts, segmented diff trimming improvements.
* Multiple simultaneously visible splits / window layout (layout regions beyond 1).
* Search, syntax highlighting, theming, plugins, LSP/DAP, completion, git integration.
* Smarter word motions (current word logic intentionally naive).
* Time‑based undo coalescing.
* Performance dashboard overlay mode (current snapshot only ephemeral); richer UI pending.

If that sounds fun rather than disappointing — you get the vibe.

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

1. **Is this a Neovim fork?** No — completely fresh Rust code.
2. **Does it do much yet?** Enough to move around, insert text, undo/redo, and now selectively repaint only what changed. Breadth first, polish later.
3. **Will it embed Vimscript / Lua?** Probably not directly. Expect a lean capability‑scoped extension / plugin layer in a later phase.
4. **Why rewrite instead of contribute to Neovim?** Different experiment: explore how far a clean, aggressively modular Rust design can go sans legacy ballast.
5. **Should I daily‑drive it?** Not yet. Follow along, kick the tires, file crisp issues.
6. **Why is the cursor sometimes bashful with super wide emoji?** Terminal quirks & differing width heuristics. The internal model is fully cluster‑aware; remaining issues are presentation polish.
7. **Does it still redraw the whole screen every keypress?** No. Cursor moves repaint just the affected lines; small edits repaint only changed lines. Scroll/resize/large edit bursts still force a full frame (on purpose) until scroll region optimization lands.
8. **What’s next?** Deeper scroll performance & batching refinements, multi‑viewport layout groundwork, then early syntax / styling layers atop the cluster model.
9. **How do I see performance metrics?** Internally tracked (frame counts, dirty funnel, timings, cluster render stats) but not exposed yet — a dashboard command is on the backlog.
10. **Will there be LSP / completion / git soon?** Yes, but only after core rendering + windowing are sturdier. Foundation first.

## Dual License

Licensed under [Apache 2.0](LICENSE-APACHE.txt) or [MIT](LICENSE-MIT.txt) -- pick whichever suits your project.

---
If this kind of clean-slate editor architecture excites you: star, watch, and drop ideas. The fun part (real features) is just getting started.
