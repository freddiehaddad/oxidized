# Development & Debugging

This page gathers build, logging, testing, benchmarking, and troubleshooting
guides.

## Building from Source

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

Note (Windows): if `cargo test` fails to remove `target\\debug\\oxidized.exe`
with "Access is denied (os error 5)", ensure no running editor instance is
holding a file lock and retry.

## Logging

Oxidized writes logs to `oxidized.log` in the working directory. Tail it while
you run the editor.

Defaults:

- Debug builds: `debug`
- Release builds: `info`
- Override via `RUST_LOG`

Quick reference:

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

Log levels: `error`, `warn`, `info`, `debug`, `trace`

Notes:

- If `RUST_LOG` is unset, Oxidized uses `debug` (debug builds) and `info`
  (release)
- Logs append to `oxidized.log`; fallback to stderr if creation fails

### Event-driven runtime and quit behavior

The main event loop blocks on events. Exiting with `:q` or `:q!` sets a quit
flag; the loop checks this after handling any event for prompt exit.

### Module-specific logging

```bash
# Core editing
RUST_LOG=oxidized::editor=trace
# Buffer operations
RUST_LOG=oxidized::buffer=debug
# Syntax highlighting
RUST_LOG=oxidized::syntax=debug
# Config
RUST_LOG=oxidized::config=info
# Search
RUST_LOG=oxidized::search=debug
# Theme
RUST_LOG=oxidized::theme=info
# Macros
RUST_LOG=oxidized::features::macros=trace
```

### Performance debugging

```bash
# Timing logs
RUST_LOG="oxidized::ui=debug,oxidized::buffer=debug" cargo run large_file.txt
# Highlighting performance & incremental parse decisions
RUST_LOG=oxidized::syntax=trace cargo run code_file.rs
# Memory/allocations
RUST_LOG=trace cargo run --features debug-allocations
```

## Testing

```powershell
# Windows - Run all tests
cargo test
# Run specific modules
cargo test ui_wrap_tests
cargo test visual_block_mode_tests
cargo test search_integration
cargo test macro_tests
# Detailed output
cargo test -- --nocapture --test-threads=1
```

High-signal tests:

- tests/editor_tests.rs — editor core behaviors and redraw expectations
- tests/keymap_tests.rs — key sequence → action wiring
- tests/ex_command_tests.rs — ex commands and :set/:setp
- tests/search_integration.rs — search engine and navigation
- tests/grapheme_cursor_tests.rs — grapheme/emoji edge cases
- tests/ui_tests.rs — renderer and statusline

## Benchmarking

Criterion benches (HTML reports enabled). Examples:

```powershell
# Windows (PowerShell)
cargo bench
cargo bench --bench wrap_bench
```

Benches:

- search_bench — search engine micro-benchmarks
- wrap_bench — wrapping and Unicode display width paths
- viewport_hscroll_bench — no-wrap horizontal scrolling under render
- gutter_status_bench — gutter width and status line layout
- visual_block_bench — block selection highlight span computation

Notes:

- Benches are headless-safe; a real TTY is not required.
- Use `--quick` for faster, lower-precision runs.

## Installation & Distribution

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

## Troubleshooting

### Application Won't Start

`oxidized` command not found or permission denied.

```powershell
# Windows - PATH and permissions
where oxidized
$env:PATH += ";C:\path\to\oxidized\target\release"
# If building from source
cargo build --release
.\target\release\oxidized.exe filename.txt
```

```bash
# Linux/macOS - PATH and permissions
which oxidized
export PATH="$PATH:/path/to/oxidized/target/release"
# Fix permissions if needed
chmod +x ./target/release/oxidized
./target/release/oxidized filename.txt
```

### Configuration Not Loading

Custom settings in TOML files are ignored.

```bash
# Verify file locations
ls -la *.toml
# Windows
Get-ChildItem *.toml
```

```powershell
# Windows - Detailed config logging
$env:RUST_LOG="oxidized::config=trace"
cargo run filename.txt
Get-Content oxidized.log | Select-String "config"
```

```bash
# Reset to defaults
mv editor.toml editor.toml.backup
mv keymaps.toml keymaps.toml.backup
mv themes.toml themes.toml.backup
cargo run
```

### Performance Issues

Slow response times or high memory usage.

```bash
# Detailed performance logging
RUST_LOG="oxidized::ui=debug,oxidized::buffer=debug" cargo run large_file.txt
# Syntax bottlenecks
RUST_LOG=oxidized::syntax=trace cargo run source_code.rs
```

```toml
# In editor.toml - disable syntax highlighting temporarily
[syntax]
enabled = false
```

```toml
# In editor.toml - reduce or disable syntax (example keys; subject to change)
[syntax]
enabled = true
incremental = true
background_prefetch = "nearby"  # options: off|nearby|aggressive
```

```powershell
# Windows - monitor process
Get-Process -Name "oxidized" | Format-Table CPU,PM,VM -AutoSize
```

### Terminal Display Issues

Corrupted display, missing characters, or color problems.

```powershell
# Windows - use Windows Terminal
winget install Microsoft.WindowsTerminal
$env:TERM="xterm-256color"
```

```bash
# Ensure proper locale
export LC_ALL=en_US.UTF-8
export LANG=en_US.UTF-8
```

### File Access Problems

```powershell
# Windows - permissions and ownership
Get-Acl filename.txt
icacls filename.txt
# Elevated run if needed
Start-Process "cargo run filename.txt" -Verb RunAs
```

```bash
# Linux/macOS - fix permissions
ls -la filename.txt
chmod 644 filename.txt
sudo chown $USER:$USER filename.txt
```

### Macro Recording Issues

```bash
# Enable detailed macro logging
RUST_LOG="oxidized::features::macros=trace" cargo run
# Common issues:
# 1) Recording already in progress — press 'q' to stop current recording
# 2) Invalid register name — use a-z, A-Z, or 0-9
# 3) Empty macro — ensure you performed actions during recording
```

### Getting Help

```bash
# Comprehensive debugging
RUST_LOG=trace cargo run filename.txt 2>&1 | tee debug_output.log
```

When reporting bugs, include: OS, terminal, Rust version; commit hash; your
TOML files; debug logs; and minimal repro steps.
