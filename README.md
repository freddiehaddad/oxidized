# Oxidized: A High-Performance Vim Clone in Rust

<div align="left">

<!-- Status Badges -->
<a href="https://github.com/freddiehaddad/oxidized/actions/workflows/ci.yml">
   <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/freddiehaddad/oxidized/ci.yml?branch=master&label=build%2Ftest&logo=github&logoColor=white&color=dea584" />
</a>
<a href="https://github.com/freddiehaddad/oxidized/actions/workflows/ci.yml">
   <img alt="Clippy" src="https://img.shields.io/badge/lint-clippy-orange?logo=rust&logoColor=white&color=ce422b" />
</a>
<a href="https://github.com/freddiehaddad/oxidized/blob/master/LICENSE">
   <img alt="License" src="https://img.shields.io/badge/license-MIT-yellow?color=997f5f" />
</a>

</div>

**Oxidized** is a modern terminal-based text editor that brings Vim's powerful
modal editing to the 21st century. Built from the ground up in Rust, it
combines Vim's time-tested editing philosophy with cutting-edge architecture,
delivering exceptional performance, memory safety, and extensibility.

> Note: This project is under active development. Features, behavior, and APIs
> may change.

Docs:

- User & Dev index: [docs/README.md](./docs/README.md)
- Architecture: [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) · [Quickstart](./docs/ARCHITECTURE_QUICKSTART.md)
- Configuration: [docs/CONFIGURATION.md](./docs/CONFIGURATION.md)
- Keymaps: [docs/KEYMAPS.md](./docs/KEYMAPS.md)
- Themes: [docs/THEMES.md](./docs/THEMES.md)
- Completion UI: [docs/COMPLETION.md](./docs/COMPLETION.md)

## ✨ Unique to Oxidized

First‑class Markdown authoring in a terminal editor:

- Tree‑sitter powered Markdown highlighting that combines the block and inline grammars for accurate headings, emphasis, code spans, links, lists, tables, and more. Safe integration (no runtime mismatch) and rust‑inspired colors.
- Built‑in Markdown preview: right split, live updates (debounced), one‑way scroll sync, and terminal‑safe rendering via pulldown‑cmark (no HTML in the TTY). Toggle with F8, `<leader> m p` (leader defaults to Space), or `:MarkdownPreviewToggle`; control behavior with `editor.toml`.

Learn more:

- Configuration and commands: [docs/CONFIGURATION.md](./docs/CONFIGURATION.md#markdown-preview-settings-and-commands)
- Keymaps (F8 / `<leader> m p`): [docs/KEYMAPS.md](./docs/KEYMAPS.md)
- Status and roadmap: [docs/FEATURE_STATUS.md](./docs/FEATURE_STATUS.md) · [docs/ROADMAP.md](./docs/ROADMAP.md)

See status and timeline: [docs/FEATURE_STATUS.md](./docs/FEATURE_STATUS.md) and [docs/ROADMAP.md](./docs/ROADMAP.md).

## 🏗️ Architecture Overview

Advanced contributors: see [CONTRIBUTING_ARCH.md](./docs/CONTRIBUTING_ARCH.md)
for architecture contribution guidelines.

Developer docs and diagrams:

- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) — high-level guide with
   inline Mermaid diagrams

### Core Components

**Editor Engine:**

- **Modal System**: Complete implementation of Normal, Insert, Command,
   Visual, Replace, and Search modes
- **Buffer Management**: Multi-buffer support with efficient switching and
   state management
- **Window System**: Advanced window splitting, navigation, and resizing with
   independent viewports
- **Undo Engine**: Sophisticated multi-level undo/redo with operation
   tracking

**Rendering Pipeline:**

- **Async Syntax Highlighter**: Dedicated worker thread with request
   coalescing, priority levels, and a versioned results pipeline. A dispatcher
   thread applies results to an LRU cache and triggers redraws.
- **Viewport Manager**: Efficient screen updates with scroll optimization
- **Terminal Interface**: Cross-platform terminal handling with alternate
   screen support
- **Unicode Engine**: UTF-8 safe, grapheme-cluster aware width calculation

**Configuration Framework:**

- **TOML Parser**: Structured configuration with automatic validation
- **Hot Reloading**: Live configuration updates with file system watching
- **Command Integration**: `:set` (session) and `:setp` (persistent) command
   pair for runtime configuration
- **Theme Engine**: Dynamic theme switching with semantic color schemes

### Performance Features

- **Efficient Rendering**: Minimized redraws and buffered terminal updates
- **Background Processing**: Syntax highlighting runs asynchronously via a
   worker thread and event-driven dispatcher
- **Versioned Results**: A monotonic token prevents stale highlight results
   from being applied after scroll/resize/theme changes
- **Bounded Cache**: A small in-memory LRU stores per-line highlights for
   fast reuse without unbounded growth
- **Memory Management**: Rust's ownership system ensures memory safety without
   garbage collection
- **Pragmatic Data Structures**: Efficient line-based model today; advanced
   gap/rope structures are planned
- **Fast Search Path (ASCII)**: Case-insensitive search uses an ASCII fast
   path to avoid per-line lowercase allocations when possible; Unicode-
   insensitive search preserves exact matching semantics

## 📋 Feature Status

For a current, curated list of implemented, in-progress, and planned
features, see:

- docs: [FEATURE_STATUS.md](./docs/FEATURE_STATUS.md)

### 🚧 In Progress

**Advanced Editing:**

- Advanced search and replace with regex substitution
- Code folding and automatic indentation

**File Management:**

- File explorer and directory navigation
- Advanced buffer management with session support
- File type detection and language-specific settings

### 📅 Planned Features

**IDE Integration:**

- LSP (Language Server Protocol) client integration
- Autocompletion with intelligent suggestions
- Go-to definition and hover information
- Diagnostics and error highlighting

**Extensibility:**

- Lua scripting API for custom commands and functions
- Plugin system with package management
- Custom syntax highlighting definitions
- User-defined text objects and operators

**Advanced Features:**

- Git integration with diff highlighting
- Terminal emulator within the editor
- Project-wide search and replace
- Session management with workspace support

## 🛠️ Development & Debugging

Quick link: [Logging](#-logging)

### Building from Source

**Development Build:**

```powershell
# Windows (PowerShell)
git clone https://github.com/freddiehaddad/oxidized.git
cd oxidized

# Build in debug mode
cargo build

# Run (debug builds default to debug-level logging)
cargo run filename.txt
```

```bash
# Linux/macOS (Bash)
git clone https://github.com/freddiehaddad/oxidized.git
cd oxidized

# Build in debug mode
cargo build

# Run (debug builds default to debug-level logging)
cargo run filename.txt
```

Note (Windows): if `cargo test` fails to remove `target\\debug\\oxidized.exe` with
"Access is denied (os error 5)", ensure no running editor instance is holding a
file lock (close the editor or kill the process) and retry.

**Release Build:**

```powershell
# Windows - Optimized release build
cargo build --release

# Run with custom log level
$env:RUST_LOG="debug"; .\target\release\oxidized.exe filename.txt
```

```bash
# Linux/macOS - Optimized release build
cargo build --release

# Run with custom log level
RUST_LOG=debug ./target/release/oxidized filename.txt
```

### 📊 Logging

Oxidized always writes logs to a local file named `oxidized.log` in the working directory. Use your shell to tail this file while you run the editor.

Defaults:

- Debug builds default to level `debug`.
- Release builds default to level `info`.
- Set `RUST_LOG` to override the level or select modules.

#### Quick reference

```powershell
# Windows (PowerShell)
# 1) Run and follow the log file
cargo run filename.txt
Get-Content .\oxidized.log -Wait -Tail 50

# 2) Module-focused logging
$env:RUST_LOG="oxidized=info,oxidized::editor=debug"; cargo run
```

```bash
# Linux/macOS (Bash)
# 1) Run and follow the log file
cargo run filename.txt & tail -f oxidized.log

# 2) Module-focused logging
RUST_LOG="oxidized=info,oxidized::editor=debug" cargo run
```

#### Log levels

- `error`, `warn`, `info`, `debug` (default in debug builds), `trace`

Notes:

- If `RUST_LOG` is unset, Oxidized uses `debug` in debug builds and `info` in release builds.
- Logs are appended to `oxidized.log`. If the file cannot be created, logging falls back to stderr.

### Event-driven runtime and quit behavior

Oxidized’s main event loop now blocks on events. Exiting with `:q` or `:q!`
sets a quit flag; the loop checks this immediately after handling any event,
ensuring prompt exit without waiting for further input.

#### **Module-Specific Logging**

Key modules you can monitor individually:

```bash
# Core editing functionality
RUST_LOG=oxidized::editor=trace

# Buffer operations and text manipulation
RUST_LOG=oxidized::buffer=debug

# Syntax highlighting and Tree-sitter
RUST_LOG=oxidized::syntax=debug

# Configuration system and file watching
RUST_LOG=oxidized::config=info

# Theme system and color management
RUST_LOG=oxidized::theme=info

# Macro recording and playback
RUST_LOG=oxidized::features::macros=trace
```

#### **Performance Debugging**

```bash
# Monitor performance with timing logs
RUST_LOG="oxidized::ui=debug,oxidized::buffer=debug" cargo run large_file.txt

# Trace syntax highlighting performance
RUST_LOG=oxidized::syntax=trace cargo run code_file.rs

# Debug memory usage and allocations
RUST_LOG=trace cargo run --features debug-allocations
```

#### Common debugging scenarios

For startup issues, performance investigations, config problems, and macro debugging, see:

- docs: [DEVELOPMENT.md](./docs/DEVELOPMENT.md#troubleshooting)

**Investigating Performance Problems:**

```bash
# Comprehensive performance logging
RUST_LOG="oxidized::ui=debug,oxidized::buffer=trace" cargo run large_file.txt
```

**Debugging Configuration Issues:**

```bash
# Watch configuration loading and validation
RUST_LOG="oxidized::config=trace" cargo run
```

### 🧪 Testing and Benchmarking

## 🤝 Contributing Resources

Contributions are welcome. See:

- docs: [CONTRIBUTING_USER.md](./docs/CONTRIBUTING_USER.md)
- docs: [CONTRIBUTING_ARCH.md](./docs/CONTRIBUTING_ARCH.md)

See tests, benches, and tips in:

- docs: [DEVELOPMENT.md](./docs/DEVELOPMENT.md#testing)

### Installation & Troubleshooting

See install steps and troubleshooting recipes in:

- docs: [DEVELOPMENT.md](./docs/DEVELOPMENT.md#installation--distribution)
- docs: [DEVELOPMENT.md](./docs/DEVELOPMENT.md#troubleshooting)

**High‑signal test files:**

- [tests/editor_tests.rs](./tests/editor_tests.rs) — editor core behaviors and redraw expectations
- [tests/keymap_tests.rs](./tests/keymap_tests.rs) — key sequence → action wiring
- [tests/ex_command_tests.rs](./tests/ex_command_tests.rs) — ex commands and :set/:setp
- [tests/search_integration.rs](./tests/search_integration.rs) — search engine and navigation
- [tests/grapheme_cursor_tests.rs](./tests/grapheme_cursor_tests.rs) — grapheme/emoji edge cases
- [tests/ui_tests.rs](./tests/ui_tests.rs) — renderer and statusline

### Benchmarking

Criterion benches (HTML reports enabled): run all benches or target one.

- search_bench — search engine micro-benchmarks
- wrap_bench — wrapping and Unicode display width paths
- viewport_hscroll_bench — no-wrap horizontal scrolling under render
- gutter_status_bench — gutter width and status line layout
- visual_block_bench — block selection highlight span computation (validates that span math is nanosecond-scale and caching is unnecessary)

Run a single bench or everything:

```powershell
# Windows (PowerShell)
cargo bench                                 # run all benches
cargo bench --bench wrap_bench              # run just one bench
cargo bench --bench search_bench -- --quick # fast sanity run
```

```bash
# Linux/macOS (Bash)
cargo bench                                  # run all benches
cargo bench --bench wrap_bench               # run just one bench
cargo bench --bench search_bench -- --quick  # fast sanity run
```

Compare against a saved baseline locally:

```powershell
# Save a baseline (e.g., from main)
cargo bench -- --save-baseline main
# After changes, compare to baseline
cargo bench -- --baseline main
```

Reports live under `target/criterion/<bench>/report/index.html`.

Notes:

- Benches are headless-safe; a real TTY is not required.
- Use `--quick` for faster, lower-precision runs; omit it for full fidelity.

**Test Categories:**

The comprehensive test suite covers:

- **Buffer Operations**: Text manipulation, cursor movement, undo/redo
- **Search Engine**: Regex functionality, incremental search, case sensitivity
- **Syntax Highlighting**: Tree-sitter integration, async processing
- **Editor Modes**: Modal transitions, command execution, state management
- **Text Objects**: Word/paragraph/bracket selection, operator combinations
- **Configuration**: TOML parsing, validation, live reloading
- **Macro System**: Recording, playback, error handling, register management
- **Window Management**: Splits, navigation, resizing, viewport handling

### Installation & Distribution

```powershell
# Windows - Install system-wide
cargo install --path .

# Create distributable binary
cargo build --release
# Binary located at: .\target\release\oxidized.exe
```

```bash
# Linux/macOS - Install system-wide
cargo install --path .

# Create distributable binary
cargo build --release  
# Binary located at: ./target/release/oxidized
```

## 🔧 Troubleshooting

### Common Issues and Solutions

#### **Application Won't Start**

**Issue**: `oxidized` command not found or permission denied

**Solutions:**

```powershell
# Windows - Check PATH and permissions
where oxidized                               # Verify installation location
$env:PATH += ";C:\path\to\oxidized\target\release"  # Add to PATH if needed

# If building from source
cargo build --release
.\target\release\oxidized.exe filename.txt  # Run directly
```

```bash
# Linux/macOS - Check PATH and permissions
which oxidized                               # Verify installation location
export PATH="$PATH:/path/to/oxidized/target/release"  # Add to PATH if needed

# Fix permissions if needed
chmod +x ./target/release/oxidized
./target/release/oxidized filename.txt      # Run directly
```

#### **Configuration Not Loading**

**Issue**: Custom settings in TOML files are ignored

**Debugging Steps:**

1. **Verify file locations:**

   ```bash
   # Configuration files should be in current directory or ~/.config/oxidized/
   ls -la *.toml                            # Linux/macOS
   Get-ChildItem *.toml                     # Windows
   ```

2. **Check TOML syntax:**

   ```powershell
   # Windows - Enable detailed config logging
   $env:RUST_LOG="oxidized::config=trace"
   cargo run filename.txt
   Get-Content oxidized.log | Select-String "config"
   ```

3. **Reset to defaults:**

   ```bash
   # Backup and reset configuration
   mv editor.toml editor.toml.backup
   mv keymaps.toml keymaps.toml.backup  
   mv themes.toml themes.toml.backup
   cargo run                               # Will recreate defaults
   ```

#### **Performance Issues**

**Issue**: Slow response times or high memory usage

**Diagnostic Commands:**

```bash
# Monitor performance with detailed logging
RUST_LOG="oxidized::ui=debug,oxidized::buffer=debug" cargo run large_file.txt

# Check for syntax highlighting bottlenecks
RUST_LOG=oxidized::syntax=trace cargo run source_code.rs
```

**Performance Solutions:**

1. **Disable syntax highlighting temporarily:**

   ```toml
   # In editor.toml
   [syntax]
   enabled = false
   ```

2. **Reduce background processing:**

   ```toml
   # In editor.toml
   [performance]
   async_processing = false
   max_syntax_workers = 1
   ```

3. **Monitor system resources:**

   ```powershell
   # Windows - Monitor while running
   Get-Process -Name "oxidized" | Format-Table CPU,PM,VM -AutoSize
   ```

#### **Terminal Display Issues**

**Issue**: Corrupted display, missing characters, or color problems

**Solutions:**

1. **Terminal Compatibility:**

   ```powershell
   # Windows - Use Windows Terminal for best results
   winget install Microsoft.WindowsTerminal
   
   # Set proper terminal environment
   $env:TERM="xterm-256color"
   ```

2. **Unicode Issues:**

   ```bash
   # Ensure proper locale settings
   export LC_ALL=en_US.UTF-8
   export LANG=en_US.UTF-8
   ```

3. **Color Support:**

   ```bash
   # Test terminal color support
   curl -s https://gist.githubusercontent.com/HaleTom/89ffe32783f89f403bba96bd7bcd1263/raw/ | bash
   ```

#### **File Access Problems**

**Issue**: Cannot read/write files, permission errors

**Solutions:**

```powershell
# Windows - Check file permissions and ownership
Get-Acl filename.txt
icacls filename.txt                          # View detailed permissions

# Run with elevated permissions if needed
Start-Process "cargo run filename.txt" -Verb RunAs
```

```bash
# Linux/macOS - Check and fix permissions
ls -la filename.txt                          # Check current permissions
chmod 644 filename.txt                       # Fix read/write permissions
sudo chown $USER:$USER filename.txt         # Fix ownership if needed
```

#### **Macro Recording Issues**

**Issue**: Macros not recording or playing back correctly

**Debugging:**

```bash
# Enable detailed macro logging
RUST_LOG="oxidized::features::macros=trace" cargo run

# Common issues and solutions:
# 1. Recording already in progress - press 'q' to stop current recording
# 2. Invalid register name - use a-z, A-Z, or 0-9
# 3. Empty macro - ensure you performed actions during recording
```

### Getting Help

**Enable Debug Logging:**

```bash
# Comprehensive debugging
RUST_LOG=trace cargo run filename.txt 2>&1 | tee debug_output.log
```

**Report Issues:**

When reporting bugs, please include:

1. **System Information**: OS, terminal emulator, Rust version
2. **Oxidized Version**: `cargo --version` and commit hash
3. **Configuration Files**: Your `editor.toml`, `keymaps.toml`, `themes.toml`
4. **Debug Logs**: Output with `RUST_LOG=debug`
5. **Reproduction Steps**: Minimal steps to reproduce the issue

**Community Support:**

- **GitHub Issues**: [github.com/freddiehaddad/oxidized/issues](https://github.com/freddiehaddad/oxidized/issues)
- **Documentation**: Check the comprehensive guides in this README
- **Contributing**: See the [Contributing](#-contributing) section below

## 🧰 Dependencies

See the curated list of core, advanced, and development dependencies:

- docs: [DEPENDENCIES.md](./docs/DEPENDENCIES.md)

## 🤝 Contributing

For contributor steps, guidelines, and areas to help, see:

- docs: [CONTRIBUTING_USER.md](./docs/CONTRIBUTING_USER.md)
- docs: [CONTRIBUTING_ARCH.md](./docs/CONTRIBUTING_ARCH.md)

---

## 🎯 Vim/Neovim Feature Parity Roadmap

See the dedicated roadmap for phases and milestones:

- docs: [ROADMAP.md](./docs/ROADMAP.md)
- **zo/zc**: Open/close fold
- **zM/zR**: Close/open all folds
- **Fold methods**: Manual, indent-based, syntax-based

### 🌟 **Phase 3: Modern Features (Medium-Low Priority)**

#### 11. **Complete LSP Integration**

- **Go to definition**: gd
- **Find references**: gr
- **Hover information**: K
- **Diagnostics**: Real-time error highlighting
- **Code actions**: Refactoring suggestions

#### 12. **Plugin System**

- **Lua scripting**: Full mlua integration
- **Plugin manager**: Install/update plugins
- **API bindings**: Expose editor functionality to Lua

#### 13. **Git Integration**

- **Git status**: Show modified lines in gutter
- **Git blame**: :Gblame command
- **Git diff**: :Gdiff command

### 📈 **Implementation Strategy**

**Phase 1 Timeline** (12-14 weeks):

1. **~~Macro System~~**: ✅ **COMPLETED** (4-6 weeks)
2. **Named Registers**: 2-3 weeks  
3. **Visual Mode Completion**: 3-4 weeks
4. **Search & Replace**: 3-4 weeks
5. **Marks & Jumps**: 2-3 weeks

**Success Metrics**:

- **90% Vim compatibility**: Most common Vim workflows work identically
- **Performance**: Sub-100ms response time for all operations
- **Stability**: No crashes during normal editing sessions

**Contributing**: Pick any feature from Phase 1 to start contributing! Each feature is designed to be implemented independently.

## 💡 Inspiration

Oxidized draws inspiration from exceptional editors that have shaped the text editing landscape:

- **[Vim](https://www.vim.org/)**: The legendary modal editor that defined efficient text manipulation
- **[Neovim](https://neovim.io/)**: The extensible, modernized Vim with Lua scripting
- **[Helix](https://helix-editor.com/)**: A Kakoune-inspired editor with Tree-sitter integration
- **[Xi Editor](https://xi-editor.io/)**: Google's experimental editor with async architecture

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

**Ready to experience the future of text editing?** Install Oxidized today and discover what happens when Vim's power meets Rust's performance and safety!
