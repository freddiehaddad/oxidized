# Oxidized: A High-Performance Vim Clone in Rust

**Oxidized** is a modern terminal-based text editor that brings Vim's powerful modal editing to the 21st century. Built from the ground up in Rust, it combines Vim's time-tested editing philosophy with cutting-edge architecture, delivering exceptional performance, memory safety, and extensibility.

## 📋 Table of Contents

- [🚀 Key Features](#-key-features)
- [🔧 Installation & Setup](#-installation--setup)  
- [📖 Quick Start Guide](#-quick-start-guide)
- [⚙️ Configuration System](#️-configuration-system)
- [🏗️ Architecture Overview](#️-architecture-overview)
- [📋 Feature Status](#-feature-status)
- [🛠️ Development & Debugging](#️-development--debugging)
  - [Testing](#testing)
  - [Benchmarking](#benchmarking)
- [🔧 Troubleshooting](#-troubleshooting)
- [🧰 Dependencies](#-dependencies)
- [🤝 Contributing](#-contributing)
- [🎯 Roadmap](#-vimneovim-feature-parity-roadmap)
- [💡 Inspiration](#-inspiration)
- [📜 License](#-license)

## 🚀 Key Features

### Revolutionary Configuration System

- **TOML-Based Configuration**: Replace Vim's cryptic rc files with intuitive, structured TOML configuration
- **Live Reloading**: Configuration changes apply instantly without restart
- **Automatic Persistence**: `:set` commands automatically save to configuration files

### Advanced Text Editing Engine  

- **Complete Modal System**: Normal, Insert, Command, Visual (character/line/block), Replace, and Search modes
- **Professional Text Objects**: Full support for words, paragraphs, quotes, brackets, tags, and custom objects
- **Operator Integration**: All operators (`d`, `c`, `y`, `>`, `<`, `~`) work seamlessly with text objects and visual selections
- **Sophisticated Undo System**: Multi-level undo/redo with full operation tracking
- **Macro Recording System**: Full Vim-compatible macro recording and playback with registers a-z, A-Z, 0-9

### Modern Performance Architecture

- **Async Syntax Highlighting**: Background Tree-sitter processing with priority-based rendering
- **Multi-Buffer Management**: Efficient buffer handling with instant switching
- **Advanced Window System**: Complete window splitting, navigation, and resizing
- **Optimized Rendering**: Smart viewport management and efficient screen updates
- **Soft Line Wrapping**: Optional word-aware wrapping for long lines
- **Horizontal Scrolling**: No-wrap mode with smart side scroll offsets

### Cross-Platform Terminal Integration

- **Alternate Screen Mode**: Clean terminal entry/exit without disrupting scrollback
- **Unicode Support**: UTF-8 safe, grapheme/width-aware rendering (emoji, ZWJ,
combining marks) with proper display width calculation
- **Configurable Timeouts**: Customizable key sequence and mode transition timings

## 🔧 Installation & Setup

### Prerequisites

- **Rust 1.70+** - Install from [rustup.rs](https://rustup.rs/)
- **Terminal**: Modern terminal emulator with Unicode support

### Windows Installation

```powershell
# Install Rust (if not already installed)
Invoke-RestMethod -Uri https://win.rustup.rs/ | Invoke-Expression

# Clone and build Oxidized
git clone https://github.com/freddiehaddad/oxidized.git
cd oxidized
cargo build --release

# Run the editor
cargo run filename.txt

# Or use the built binary
.\target\release\oxy.exe filename.txt

# Install system-wide (optional)
cargo install --path .
# Binary will be available as 'oxy' in your PATH
```

### Linux/macOS Installation

```bash
# Install Rust (if not already installed)  
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build Oxidized
git clone https://github.com/freddiehaddad/oxidized.git
cd oxidized
cargo build --release

# Run the editor
cargo run filename.txt

# Or use the built binary
./target/release/oxy filename.txt

# Install system-wide (optional)
cargo install --path .
# Binary will be available as 'oxy' in your PATH
```

## 📖 Quick Start Guide

### First Steps

1. **Launch**: `oxy filename.txt` or `oxy` for a new buffer
2. **Insert Text**: Press `i` to enter Insert mode, type your content
3. **Navigate**: Use `hjkl` or arrow keys in Normal mode
4. **Save**: Type `:w` to write the file
5. **Exit**: Type `:q` to quit, or `:wq` to save and quit

### Essential Commands

**Basic Movement:**

- `hjkl` - Character movement (left/down/up/right)
- `w/b/e` - Word movement (next/previous/end)
- `0/$` - Line start/end
- `gg/G` - Buffer start/end

**Editing Operations:**

- `i/a` - Insert mode (before/after cursor)
- `dd` - Delete line
- `yy` - Copy line  
- `p/P` - Paste (after/before cursor)
- `x` - Delete character

**Search & Navigation:**

- `/pattern` - Search forward
- `?pattern` - Search backward
- `n/N` - Next/previous search result
  - Case options: configure `ignore_case` and `smart_case` in `editor.toml`

**Visual Modes Tip:**

- `v` - Visual (character)
- `V` - Visual Line
- `Ctrl+v` or `Alt+v` - Visual Block (use Alt+V if your terminal intercepts Ctrl+V). Press again to exit.

**Macro Recording & Playback:**

- `qa` - Start recording macro to register 'a' (use any a-z, A-Z, 0-9)
- `q` - Stop macro recording
- `@a` - Execute macro from register 'a'
- `@@` - Repeat last executed macro
- `3@a` - Execute macro 'a' three times

### Advanced Window Management

**Window Creation:**

- `:split` or `:sp` - Horizontal split
- `:vsplit` or `:vsp` - Vertical split
- `Ctrl+w s/v` - Direct split creation

**Window Navigation:**

- `Ctrl+w hjkl` - Move between windows
- `Ctrl+w c` - Close current window
- `Ctrl+w o` - Close all other windows

**Window Resizing:**

- `Ctrl+w >/<` - Wider/narrower
- `Ctrl+w +/-` - Taller/shorter
- `Ctrl+w =` - Equalize sizes

## ⚙️ Configuration System

Oxidized uses a revolutionary TOML-based configuration system that's both human-readable and powerful.

### Editor Settings (`editor.toml`)

```toml
[display]
show_line_numbers = false
show_relative_numbers = true
show_cursor_line = true
color_scheme = "default"
syntax_highlighting = true

[behavior]
tab_width = 4
expand_tabs = false
auto_indent = true
ignore_case = false
smart_case = false
highlight_search = true
incremental_search = true
wrap_lines = false
line_break = false

[editing]
undo_levels = 1000
persistent_undo = false
backup = false
swap_file = false
auto_save = false
text_object_timeout = 1000
operator_pending_timeout = 1000

[interface]
show_status_line = true
status_line_format = "default"
command_timeout = 1000
show_command = true
scroll_off = 3
side_scroll_off = 0
window_resize_amount = 1
completion_menu_width = 30
completion_menu_height = 8

[languages]
default_language = "text"

[languages.extensions]
"rs" = "rust"
"toml" = "toml"
"md" = "markdown"
"txt" = "text"
"json" = "json"
```

#### Search behavior

- ignore_case (ic): When true, searches are case-insensitive by default.
- smart_case (scs): When true and the search pattern contains any uppercase letter, the search becomes case-sensitive for that query only; otherwise it follows ignore_case.

#### Wrapping & Scrolling

- wrap_lines: When true, long lines are soft-wrapped visually into multiple rows. Line numbers appear on the first visual row of a logical line.

#### Path completion

- percent_path_root (alias: ppr): When enabled (default), file path completion for :e/:w treats a leading '%' as the current buffer's directory. Examples:
  - :e %/src completes files under the current buffer's folder
  - Toggle with :set ppr / :set noppr, query with :set ppr?

- line_break: When wrapping, prefer breaking at whitespace boundaries (word
   wrapping). When false, wrap strictly by display columns.
- side_scroll_off: In no-wrap mode, horizontally scroll the view to keep the
   cursor away from the left/right edges by this many columns when possible.

### Keymap Customization (`keymaps.toml`)

```toml
[normal_mode]
# Movement
"h" = "cursor_left"
"j" = "cursor_down"
"k" = "cursor_up"
"l" = "cursor_right"
"w" = "word_forward"
"b" = "word_backward"
"e" = "word_end"
"0" = "line_start"
"^" = "line_first_char"
"$" = "line_end"
"gg" = "buffer_start"
"G" = "buffer_end"

# Mode transitions
"i" = "insert_mode"
"a" = "insert_after"
"o" = "insert_line_below"
"v" = "visual_mode"
"V" = "visual_line_mode"
"Ctrl+v" = "visual_block_mode"      # Note: some terminals intercept Ctrl+V
"Alt+v" = "visual_block_mode"       # Alternative binding if Ctrl+V is intercepted

# Delete/Edit operations
"x" = "delete_char_at_cursor"
"dd" = "delete_line"
"yy" = "yank_line"
"p" = "put_after"
"P" = "put_before"

# Search
"/" = "search_forward"
"n" = "search_next"
"N" = "search_previous"

# Window management
"Ctrl+w s" = "split_horizontal"
"Ctrl+w v" = "split_vertical"
"Ctrl+w h" = "window_left"
"Ctrl+w j" = "window_down"
"Ctrl+w k" = "window_up"
"Ctrl+w l" = "window_right"

# Viewport control
"zz" = "center_cursor"
"zt" = "cursor_to_top"
"zb" = "cursor_to_bottom"

# Scrolling
"Ctrl+f" = "scroll_down_page"
"Ctrl+b" = "scroll_up_page"
"Ctrl+d" = "scroll_down_half_page"
"Ctrl+u" = "scroll_up_half_page"
```

### Theme Configuration (`themes.toml`)

```toml
# Theme configuration for oxidized editor
[theme]
current = "default"

[themes.default]
name = "Rust Theme"
description = "Rust-inspired color palette with warm oranges and earth tones"

[themes.default.ui]
background = "#1f1611"
status_bg = "#ce422b"
status_fg = "#ffffff"
status_modified = "#f74c00"
line_number = "#8c6239"
line_number_current = "#deb887"
cursor_line_bg = "#2d2318"
empty_line = "#4a3728"
command_line_bg = "#1f1611"
command_line_fg = "#deb887"
selection_bg = "#8c4a2b"
warning = "#ff8c00"
error = "#dc322f"

[themes.default.tree_sitter]
# Rust-inspired color scheme with warm earth tones
keyword = "#ce422b"      # Rust orange for keywords
function = "#b58900"     # Golden brown for function names
type = "#268bd2"         # Steel blue for types
string = "#859900"       # Olive green for strings
number = "#d33682"       # Magenta for numbers
comment = "#93a1a1"      # Light gray for comments
identifier = "#deb887"   # Burlywood for identifiers
variable = "#deb887"     # Burlywood for variables
operator = "#cb4b16"     # Orange-red for operators
punctuation = "#839496"  # Gray for punctuation
```

## 🏗️ Architecture Overview

### Core Components

**Editor Engine:**

- **Modal System**: Complete implementation of Normal, Insert, Command, Visual, Replace, and Search modes
- **Buffer Management**: Multi-buffer support with efficient switching and state management
- **Window System**: Advanced window splitting, navigation, and resizing with independent viewports
- **Undo Engine**: Sophisticated multi-level undo/redo with operation tracking

**Rendering Pipeline:**

- **Async Syntax Highlighter**: Background Tree-sitter processing with priority queues
- **Viewport Manager**: Efficient screen updates with scroll optimization
- **Terminal Interface**: Cross-platform terminal handling with alternate screen support
- **Unicode Engine**: UTF-8 safe, grapheme-cluster aware width calculation

**Configuration Framework:**

- **TOML Parser**: Structured configuration with automatic validation
- **Hot Reloading**: Live configuration updates with file system watching
- **Command Integration**: `:set` commands persist automatically to TOML files
- **Theme Engine**: Dynamic theme switching with semantic color schemes

### Performance Features

- **Efficient Rendering**: Minimized redraws and buffered terminal updates
- **Background Processing**: Syntax highlighting and file operations run asynchronously
- **Memory Management**: Rust's ownership system ensures memory safety without garbage collection
- **Pragmatic Data Structures**: Efficient line-based model today; advanced gap/rope structures are planned
- **Fast Search Path (ASCII)**: Case-insensitive search uses an ASCII fast path to avoid per-line lowercase allocations when possible; Unicode-insensitive search preserves exact matching semantics

## 📋 Feature Status

### ✅ Implemented Features

**Core Editing:**

- Complete modal editing system (Normal, Insert, Command, Visual, Replace, Search)
- Professional text objects (words, paragraphs, quotes, brackets, tags)
- Full operator support (`d`, `c`, `y`, `>`, `<`, `~`) with text object integration
- Multi-level undo/redo system with operation tracking
- Sophisticated clipboard operations with line/character modes
- **Complete visual mode operations** with character, line, and block selection
- **Macro recording and playback system** with support for 62 registers (a-z, A-Z, 0-9)

**Visual Mode Operations:**

- **Character-wise visual selection** (`v`) with precise cursor positioning
- **Line-wise visual selection** (`V`) for complete line operations  
- **Block-wise visual selection** (`Ctrl+v`) for rectangular text regions
- **All visual operators**: `d` (delete), `c` (change), `y` (yank), `>` (indent), `<` (unindent), `~` (case toggle)
- **Visual highlighting** with configurable selection colors
- **Mode transitions** between visual modes and seamless operator integration

**Navigation & Movement:**

- Character movement (`hjkl`, arrow keys)
- Word movement (`w`, `b`, `e`, `W`, `B`, `E`)
- Line navigation (`0`, `^`, `$`, `gg`, `G`)
- Viewport control (`zz`, `zt`, `zb`)
- Scrolling commands (page, half-page, line scrolling)

**Window Management:**

- Complete window splitting system (horizontal/vertical)
- Window navigation with `Ctrl+w` combinations
- Dynamic window resizing with configurable increments
- Independent viewport and cursor positioning per window
- Visual window borders and active window indication

**Search & Replace:**

- Regex-capable search engine with forward/backward search
- Incremental search with live result highlighting
- Search result navigation (`n`, `N`)
- Case-sensitive and case-insensitive search modes

**Configuration System:**

- TOML-based configuration files (`editor.toml`, `keymaps.toml`, `themes.toml`)
- Live configuration reloading with file system watching
- Automatic persistence of `:set` commands
- Over 30 configurable editor settings

**Syntax Highlighting:**

- Async Tree-sitter integration with background processing
- Priority-based syntax highlighting for visible regions
- Rust language support with semantic color schemes
- Configurable themes with semantic color meaning

**Terminal Integration:**

- Alternate screen mode for clean entry/exit
- Cross-platform terminal handling (Windows, Linux, macOS)
- Unicode support with grapheme/emoji-aware width calculation
- Professional status line with mode indication

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

### Building from Source

**Development Build:**

```powershell
# Windows (PowerShell)
git clone https://github.com/freddiehaddad/oxidized.git
cd oxidized

# Build in debug mode with comprehensive logging
cargo build

# Run with automatic debug logging
cargo run filename.txt
# Debug logs automatically written to oxidized.log
```

```bash
# Linux/macOS (Bash)
git clone https://github.com/freddiehaddad/oxidized.git
cd oxidized

# Build in debug mode with comprehensive logging
cargo build

# Run with automatic debug logging
cargo run filename.txt
# Debug logs automatically written to oxidized.log
```

**Release Build:**

```powershell
# Windows - Optimized release build
cargo build --release

# Run with custom log level
$env:RUST_LOG="debug"; .\target\release\oxy.exe filename.txt
```

```bash
# Linux/macOS - Optimized release build
cargo build --release

# Run with custom log level
RUST_LOG=debug ./target/release/oxy filename.txt
```

### 📊 Comprehensive Logging System

Oxidized provides extensive logging capabilities for development, debugging, and performance monitoring:

#### **Log Levels and Usage**

**Available Log Levels** (in order of verbosity):

- `error` - Critical errors only
- `warn` - Warnings and errors
- `info` - General information, warnings, and errors
- `debug` - Detailed debugging information (**default for debug builds**)
- `trace` - Ultra-verbose tracing (development only)

> **Note**: Debug builds automatically enable `debug` level logging without requiring `RUST_LOG` to be set. Release builds respect the `RUST_LOG` environment variable and default to `info` level.

#### **Environment Variables**

```powershell
# Windows PowerShell - Set logging level
$env:RUST_LOG="debug"                    # Debug level for all modules
$env:RUST_LOG="oxidized=trace"           # Trace level for oxidized only
$env:RUST_LOG="oxidized::editor=debug"   # Debug level for editor module only

# Multiple modules with different levels
$env:RUST_LOG="oxidized::buffer=trace,oxidized::syntax=debug,warn"

# Run with custom logging
cargo run filename.txt
```

```bash
# Linux/macOS - Set logging level
export RUST_LOG=debug                    # Debug level for all modules
export RUST_LOG=oxidized=trace           # Trace level for oxidized only
export RUST_LOG=oxidized::editor=debug   # Debug level for editor module only

# Multiple modules with different levels
export RUST_LOG="oxidized::buffer=trace,oxidized::syntax=debug,warn"

# Run with custom logging
cargo run filename.txt
```

#### **Log File Management**

**Automatic Logging:**

- Debug builds: Automatic `debug` level logging to `oxidized.log` (enabled by default)
- Release builds: `info` level logging (configurable via `RUST_LOG`)
- Log rotation: Logs are appended, manual cleanup recommended

**Real-time Log Monitoring:**

```powershell
# Windows PowerShell - Monitor logs in real-time
Get-Content oxidized.log -Wait -Tail 50

# Filter specific log levels
Get-Content oxidized.log -Wait | Select-String "ERROR|WARN"

# Watch specific modules
Get-Content oxidized.log -Wait | Select-String "editor|buffer"
```

```bash
# Linux/macOS - Monitor logs in real-time  
tail -f oxidized.log

# Filter specific log levels
tail -f oxidized.log | grep -E "(ERROR|WARN)"

# Watch specific modules
tail -f oxidized.log | grep -E "(editor|buffer)"
```

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

# Search engine and regex operations
RUST_LOG=oxidized::search=debug

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

#### **Common Debugging Scenarios**

**Troubleshooting Startup Issues:**

```powershell
# Windows
$env:RUST_LOG="error"; cargo run filename.txt 2>&1 | Tee-Object -FilePath startup_debug.log
```

```bash
# Linux/macOS
RUST_LOG=error cargo run filename.txt 2>&1 | tee startup_debug.log
```

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

**Macro System Debugging:**

```bash
# Trace macro recording and playback
RUST_LOG="oxidized::features::macros=trace" cargo run
```

### Testing

**Run Test Suite:**

```powershell
# Windows - Run all tests (200+ comprehensive tests)
cargo test

# Run specific test modules (substring match)
cargo test ui_wrap_tests
cargo test visual_block_mode_tests
cargo test search_integration
cargo test macro_tests

# Run with detailed output
cargo test -- --nocapture --test-threads=1
```

### Benchmarking

Run micro-benchmarks for the search engine using Criterion (HTML reports enabled):

```powershell
# Windows (PowerShell)
cargo bench --bench search_bench -- --quick    # fast sanity run
cargo bench --bench search_bench               # full run with reports
```

```bash
# Linux/macOS (Bash)
cargo bench --bench search_bench -- --quick    # fast sanity run
cargo bench --bench search_bench               # full run with reports
```

Benchmark reports are generated under `target/criterion/` (open the `report/index.html` inside each benchmark folder).

CI also runs quick benches on push and weekly (Mon 06:00 UTC) across Ubuntu, macOS, and Windows. HTML reports are uploaded as artifacts named `criterion-<os>` on each run.

```bash
# Linux/macOS - Run all tests (200+ comprehensive tests)
cargo test

# Run specific test modules (substring match)
cargo test ui_wrap_tests
cargo test visual_block_mode_tests
cargo test search_integration
cargo test macro_tests

# Run with detailed output
cargo test -- --nocapture --test-threads=1
```

**Test Categories:**

The comprehensive test suite covers:

- **Buffer Operations**: Text manipulation, cursor movement, undo/redo
- **Search Engine**: Regex functionality, incremental search, case sensitivity
- **Syntax Highlighting**: Tree-sitter integration, async processing
- **Editor Modes**: Modal transitions, command execution, state management
- **Text Objects**: Word/paragraph/bracket selection, operator combinations
- **Configuration**: TOML parsing, validation, live reloading
- **Macro System**: Recording, playback, error handling, register management
- **Window Management**: Splitting, navigation, resizing, viewport handling

### Installation & Distribution

```powershell
# Windows - Install system-wide
cargo install --path .

# Create distributable binary
cargo build --release
# Binary located at: .\target\release\oxy.exe
```

```bash
# Linux/macOS - Install system-wide
cargo install --path .

# Create distributable binary
cargo build --release  
# Binary located at: ./target/release/oxy
```

## 🔧 Troubleshooting

### Common Issues and Solutions

#### **Application Won't Start**

**Issue**: `oxy` command not found or permission denied

**Solutions:**

```powershell
# Windows - Check PATH and permissions
where oxy                                    # Verify installation location
$env:PATH += ";C:\path\to\oxidized\target\release"  # Add to PATH if needed

# If building from source
cargo build --release
.\target\release\oxy.exe filename.txt       # Run directly
```

```bash
# Linux/macOS - Check PATH and permissions
which oxy                                    # Verify installation location
export PATH="$PATH:/path/to/oxidized/target/release"  # Add to PATH if needed

# Fix permissions if needed
chmod +x ./target/release/oxy
./target/release/oxy filename.txt           # Run directly
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
   Get-Process -Name "oxy" | Format-Table CPU,PM,VM -AutoSize
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

### Core Dependencies

- **crossterm**: Cross-platform terminal manipulation and event handling
- **toml**: TOML configuration file parsing and serialization
- **serde**: Serialization framework for configuration management
- **anyhow**: Ergonomic error handling for operations
- **regex**: Regular expression engine for search functionality

### Advanced Features

- **tree-sitter**: Abstract syntax tree parsing for syntax highlighting
- **tree-sitter-rust**: Rust language grammar for Tree-sitter
- **notify**: File system monitoring for configuration hot reloading
- **tokio**: Async runtime for background processing
- **unicode-width/unicode-segmentation**: Unicode text handling

### Development

- **log/env_logger**: Logging infrastructure for debugging
- **criterion**: Benchmarking framework for performance testing

## 🤝 Contributing

Oxidized is an open-source learning project focused on understanding text editor architecture. Contributions are welcome!

### Getting Started

1. **Fork and Clone**: Fork the repository and clone your fork
2. **Set up Environment**: Install Rust 1.70+ and your preferred IDE
3. **Build and Test**: Run `cargo build && cargo test` to ensure everything works
4. **Pick an Issue**: Check the issue tracker for features to implement
5. **Submit PR**: Create a pull request with your changes

### Development Guidelines

- **Code Quality**: Follow Rust best practices and use `rustfmt` for formatting
- **Testing**: Add tests for new functionality and ensure existing tests pass
- **Documentation**: Update documentation and comments for new features
- **Architecture**: Maintain clean module separation and well-defined interfaces

### Areas for Contribution

- **Feature Implementation**: Help implement planned features from the roadmap
- **Performance Optimization**: Improve rendering speed and memory usage
- **Platform Support**: Enhance cross-platform compatibility
- **Documentation**: Improve user guides and developer documentation
- **Testing**: Add more comprehensive test coverage

---

## 🎯 Vim/Neovim Feature Parity Roadmap

This section outlines our plan to achieve complete feature parity with Vim/Neovim while maintaining oxidized's performance advantages.

### ✅ **Currently Implemented**

- **Modal Editing**: Complete with Normal, Insert, Visual, Command, Replace, Search modes
- **Basic Movement**: hjkl, word movement (w/b/e), line navigation (0/$, gg/G)
- **Text Objects**: Comprehensive implementation with words, paragraphs, quotes, brackets
- **Operators**: Full operator system (d/c/y/>/</~) with text object integration
- **Window Management**: Splits, navigation, resizing with Ctrl+w commands  
- **Buffer Management**: Multi-buffer support with switching and management
- **Search**: Forward/backward search with n/N navigation
- **Undo/Redo**: Multi-level undo system with operation tracking
- **Configuration**: TOML-based config with live reloading
- **Syntax Highlighting**: Tree-sitter integration with async processing
- **Clipboard Operations**: Basic yank/put with character and line modes
- **Scrolling**: Complete scrolling system (Ctrl+f/b/d/u, zz/zt/zb)
- **Command System**: Ex-commands with completion (:w, :q, :set, etc.)
- **Cursor Shape**: Mode-aware cursor changes (block/line/underline)

### 🚧 **Phase 1: Essential Vim Features (High Priority)**

#### 1. **Named Registers System**

```rust
// Priority: HIGH - Essential for advanced editing
// Implementation: src/features/registers.rs
pub struct RegisterSystem {
    named_registers: HashMap<char, ClipboardContent>,    // a-z
    numbered_registers: VecDeque<ClipboardContent>,      // 0-9
    special_registers: HashMap<char, ClipboardContent>,  // "/%, etc.
}
```

- **"{register}**: Access named registers (a-z, A-Z)
- **Numbered registers**: 0-9 for deleted text
- **Special registers**: "/, "%, ":, "., etc.

#### 3. **Complete Visual Mode Operations** ✅ **IMPLEMENTED**

- **Visual selection**: Proper character selection with highlighting ✅
- **Visual line selection**: Complete line selection (V) ✅  
- **Visual block selection**: Rectangular selection (Ctrl+V) ✅
  - Tip: If your terminal intercepts Ctrl+V (e.g., VS Code integrated terminal), use Alt+V as an alternative shortcut.
- **Selection operations**: d, c, y, >, <, ~ with visual selections ✅

#### 4. **Enhanced Search & Replace**

- **Search history**: Up/Down arrows in search mode
- **Search options**: \c, \C for case sensitivity
- **Substitute command**: :s/pattern/replacement/flags
- **Global replace**: :%s/pattern/replacement/g
- **Interactive replace**: Confirmation prompts

#### 5. **Marks and Jumps**

```rust
// Priority: MEDIUM-HIGH - Navigation enhancement
// Implementation: src/features/marks.rs
pub struct MarkSystem {
    local_marks: HashMap<char, Position>,     // a-z
    global_marks: HashMap<char, (PathBuf, Position)>, // A-Z
    jump_list: VecDeque<Position>,
}
```

- **m{mark}**: Set local marks (a-z) and global marks (A-Z)
- **'{mark}**: Jump to mark
- **Ctrl+O/Ctrl+I**: Navigate jump list

### 🔧 **Phase 2: Advanced Vim Features (Medium Priority)**

#### 6. **Tabs Support**

- **:tabnew**: Create new tab
- **gt/gT**: Navigate tabs
- **:tabclose**: Close current tab

#### 7. **Complete Character Navigation**

- **f/F/t/T**: Enhanced character finding with repeat
- **;/,**: Repeat character search forward/backward
- **Bracket matching**: % for bracket/quote/tag matching

#### 8. **Improved Undo System**

- **Undo branches**: g+/g- for undo tree navigation
- **:undolist**: Show undo history
- **Earlier/later**: :earlier 5m, :later 10s

#### 9. **Ex Command System Enhancement**

- **More ex-commands**: :copy, :move, :delete, :join
- **Command ranges**: :1,5d, :.,+3y, :%s//
- **Command history**: Up/Down arrows in command mode

#### 10. **Folding System**

- **zf**: Create fold
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
