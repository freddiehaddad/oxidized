# Oxidized: A High-Performance Vim Clone in Rust

<div align="left">

<!-- Status Badges -->
<a href="https://github.com/freddiehaddad/oxidized/actions/workflows/ci.yml">
   <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/freddiehaddad/oxidized/ci.yml?branch=master&label=build%2Ftest&logo=github&logoColor=white&color=dea584" />
</a>
<a href="https://github.com/freddiehaddad/oxidized/actions/workflows/ci.yml">
   <img alt="Clippy" src="https://img.shields.io/badge/lint-clippy-orange?logo=rust&logoColor=white&color=ce422b" />
</a>
<a href="https://freddiehaddad.github.io/oxidized/badges/coverage.json">
   <img alt="Coverage" src="https://img.shields.io/endpoint?url=https%3A%2F%2Ffreddiehaddad.github.io%2Foxidized%2Fbadges%2Fcoverage.json&logo=rust&logoColor=white" />
</a>
<a href="https://github.com/freddiehaddad/oxidized/blob/master/LICENSE">
   <img alt="License" src="https://img.shields.io/badge/license-MIT-yellow?color=997f5f" />
</a>

</div>

**Oxidized** is a modern terminal-based text editor that brings Vim's powerful modal editing to the 21st century. Built from the ground up in Rust, it combines Vim's time-tested editing philosophy with cutting-edge architecture, delivering exceptional performance, memory safety, and extensibility.

> Note: This project is under active development. Features, behavior, and APIs may change.

Quick docs:

- Architecture: see [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) and [docs/ARCHITECTURE_QUICKSTART.md](./docs/ARCHITECTURE_QUICKSTART.md)
- Contributing: see [CONTRIBUTING.md](./CONTRIBUTING.md) and [docs/CONTRIBUTING_ARCH.md](./docs/CONTRIBUTING_ARCH.md)

## 📋 Table of Contents
<!-- markdownlint-disable MD007 -->

- [🚀 Key Features](#-key-features)
- [🔧 Installation & Setup](#-installation--setup)  
- [📖 Quick Start Guide](#-quick-start-guide)
- [⚙️ Configuration System](#️-configuration-system)
- [🏗️ Architecture Overview](#️-architecture-overview)
- [📋 Feature Status](#-feature-status)
- [🛠️ Development & Debugging](#️-development--debugging)
   - [Testing](#testing)
   - [Benchmarking](#benchmarking)
   - [Logging](#-logging)
- [🔧 Troubleshooting](#-troubleshooting)
- [🧰 Dependencies](#-dependencies)
- [🤝 Contributing](#-contributing)
- [🎯 Roadmap](#-vimneovim-feature-parity-roadmap)
- [💡 Inspiration](#-inspiration)
- [📜 License](#-license)
<!-- markdownlint-enable MD007 -->

## 🚀 Key Features

### Revolutionary Configuration System

- **TOML-Based Configuration**: Replace Vim's cryptic rc files with intuitive, structured TOML configuration
- **Live Reloading**: Configuration changes apply instantly without restart
- **Flexible Persistence**: `:set` updates settings for the current session only; use `:setp` (persist) to write changes back to configuration files

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
.\target\release\oxidized.exe filename.txt

# Install system-wide (optional)
cargo install --path .
# Binary will be available as 'oxidized' in your PATH
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
./target/release/oxidized filename.txt

# Install system-wide (optional)
cargo install --path .
# Binary will be available as 'oxidized' in your PATH
```

### Try it now

Run the editor against this repo's README to get a feel quickly:

- Windows (PowerShell):

```powershell
cargo run README.md
```

- Linux/macOS (Bash):

```bash
cargo run README.md
```

## 📖 Quick Start Guide

### First Steps

1. **Launch**: `oxidized filename.txt` or `oxidized` for a new buffer
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

**Counts (numeric prefixes):**

- Use a number before a motion or action to repeat it.
- Examples: `10j` moves down 10 lines, `3dd` deletes 3 lines, `5x` deletes 5 characters, `10w` jumps 10 words.
- Numbers are accumulated until a non-digit key is pressed. `0` acts as a digit only after another digit; otherwise it maps to `0` (line start).

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

Notes:

- After pressing `q`, the next character selects the macro register. That key is consumed and will not trigger its normal mapping (e.g., `a` won’t enter insert mode).
- Press `q` again to stop recording. While recording, the statusline shows `REC @<register>`.
- If you pressed `q` by mistake, press `Esc` to cancel the pending register selection without starting a recording.

**Marks (bookmarks):**

- `ma` sets mark `a` at the current cursor position (use any a–z, A–Z, 0–9)
- `'a` jumps to the start of the line of mark `a`
- `` `a `` jumps to the exact cursor position of mark `a`

Notes:

- Marks are currently buffer-local. If a mark is not set, a status message
   will indicate it.

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
# ###############################################################################
# Oxidized Editor Configuration
#
# This file enumerates all supported settings with their default values.
# Use :set for session-only changes (not persisted) and :setp for persistence.
# ###############################################################################

[display]
show_line_numbers = true     # Absolute line numbers
show_relative_numbers = true # Relative line numbers
show_cursor_line = true      # Highlight current line
color_scheme = "default"     # Theme name from themes.toml
syntax_highlighting = true   # Enable syntax highlighting
show_marks = true            # Show marks in gutter

[behavior]
tab_width = 4             # Display width of a tab
expand_tabs = false       # Insert spaces instead of tab chars
auto_indent = true        # Maintain indentation on new lines
ignore_case = false       # Case-insensitive search by default
smart_case = false        # Override ignore_case when pattern has capitals
highlight_search = true   # Highlight matches after a search
incremental_search = true # Show matches while typing pattern
wrap_lines = false        # Soft wrap long lines
line_break = false        # Break lines at word boundaries when wrapping

[editing]
undo_levels = 1000      # Max undo history entries
persistent_undo = false # Persist undo history to disk
backup = false          # Create backup files on write
swap_file = false       # Create swap/backup for crash recovery
auto_save = false       # Auto-save modified named buffers

[interface]
show_status_line = true    # Show status line
command_timeout = 1000     # Mapping timeout (ms)
show_command = true        # Show pending keys in statusline
scroll_off = 0             # Context lines above/below cursor
side_scroll_off = 0        # Horizontal context columns
window_resize_amount = 1   # Amount for :resize commands
completion_menu_width = 36 # Popup completion width (chars)
completion_menu_height = 8 # Popup completion height (rows)
percent_path_root = true   # % prefix roots paths at current buffer dir

[statusline]
show_indent = true   # Show current indent width (tabs/spaces)
show_eol = true      # Show end-of-line style (LF/CRLF)
show_encoding = true # Show file encoding (e.g., UTF-8)
show_type = true     # Show detected language/filetype
show_macro = true    # Show active macro recording register
show_search = true   # Show current search pattern/status
show_progress = true # Show buffer progress (line/percent)
separator = "  "     # Spacing between right-aligned segments

[languages]
# Default language used when detection fails
default_language = "text" # Fallback language when no extension matches

[languages.extensions]
md = "markdown" # Files ending in .md use Markdown
toml = "toml"   # Files ending in .toml use TOML
txt = "text"    # Plain text files
rs = "rust"     # Rust source files
```

Note: The left number column acts as a gutter. When `show_line_numbers` is false but
`show_marks` is true, a minimal gutter is still rendered so marks remain
visible without enabling line numbers. The mark indicator uses the theme color `ui.mark_indicator`.

You can toggle marks at runtime with:

- `:set showmarks` / `:set smk` enable for this session
- `:setp showmarks` / `:setp smk` enable and persist
- `:set noshowmarks` / `:set nosmk` disable for this session
- `:setp noshowmarks` / `:setp nosmk` disable and persist

### Command-line Completion System

The command-line (":" prompt) features an aligned, multi-column popup for `:set` / `:setp` options:

- Column 1: Canonical option name (aliases collapsed; e.g. only `showmarks` not `smk`)
- Column 2: Known short alias(es) (e.g. `smk`)
- Column 3: Current value in brackets (e.g. `[true]`, numeric values, or active colorscheme name)

Behavior & rules:

- `:setp` provides the same suggestions as `:set`; suggestions are rewritten to use the exact prefix you typed (`set` vs `setp`).
- Aliases are suppressed when their canonical form is also present; the popup shows each setting once for clarity.
- Query forms (`:set option?`) are hidden unless you actually type the trailing `?`.
- Negative forms (`nooption`) only appear when you begin your input with `:set no…` (to reduce clutter).
- Settings that take a value use positional arguments (no `=`). Examples: `:set tabstop 2`, `:set scrolloff 3`, `:set timeoutlen 750`, `:set undolevels 200`, `:set percentpathroot true`, `:set colorscheme default`.
- Value suggestions appear after a space for numeric/boolean options (`tabstop`, `scrolloff`, `timeoutlen`, `undolevels`, `percentpathroot`) and for enumerated/dynamic lists like `colorscheme` (populated from `themes.toml`).
- File path completion honors `%` as the current buffer directory when `percent_path_root` (alias: `ppr`) is enabled.

Theme integration:

Three dedicated UI colors control the completion popup columns (defined per theme in `themes.toml`):

```toml
completion_key_fg   = "#deb887"  # Canonical option name
completion_alias_fg = "#cccccc"  # Alias column
completion_value_fg = "#ffe6c7"  # Value column
```

Adjust `interface.completion_menu_width` / `interface.completion_menu_height` in `editor.toml` to size the popup (defaults: width 36, height 8). A sensible minimum width is enforced to keep three columns readable.

#### Search behavior

- ignore_case (ic): When true, searches are case-insensitive by default.
- smart_case (scs): When true and the search pattern contains any uppercase letter, the search becomes case-sensitive for that query only; otherwise it follows ignore_case.

#### Wrapping & Scrolling

- wrap_lines: When true, long lines are soft-wrapped visually into multiple rows. Line numbers appear on the first visual row of a logical line.

#### Path completion

- percent_path_root (alias: ppr): When enabled (default), file path completion for :e/:w treats a leading '%' as the current buffer's directory. Examples:
  - :e %/src completes files under the current buffer's folder
  - Toggle with `:set ppr` / `:set noppr` (session only) or `:setp ppr` / `:setp noppr` (persist); query with `:set ppr?`

- line_break: When wrapping, prefer breaking at whitespace boundaries (word
   wrapping). When false, wrap strictly by display columns.
- side_scroll_off: In no-wrap mode, horizontally scroll the view to keep the
   cursor away from the left/right edges by this many columns when possible.

### Keymap Customization (`keymaps.toml`)

```toml
# ###############################################################################
# Oxidized Keymaps Configuration
#
# This file enumerates all supported default keybindings. Each mapping has an
# inline comment describing its purpose. You can customize or remove any map;
# unspecified actions fall back to built-in defaults.
#
# Format:
#   key = "action"
#   key = { action = "command", args = ["arg1", "arg2"] }
#
# Notes:
# - Some terminals intercept certain chords (e.g., Ctrl+V); alternatives are
#   provided where practical (see Alt+v for Visual Block).
# - After editing, save the file; the editor will live-reload keymaps.
# ###############################################################################

[normal_mode]
# Movement
"h" = "cursor_left"         # Move cursor left
"j" = "cursor_down"         # Move cursor down
"k" = "cursor_up"           # Move cursor up
"l" = "cursor_right"        # Move cursor right
"Left" = "cursor_left"      # Arrow: move cursor left
"Down" = "cursor_down"      # Arrow: move cursor down
"Up" = "cursor_up"          # Arrow: move cursor up
"Right" = "cursor_right"    # Arrow: move cursor right
"w" = "word_forward"        # Move to start of next word
"b" = "word_backward"       # Move to start of previous word
"e" = "word_end"            # Move to end of next word
"gE" = "word_end_backward"   # Move to end of previous WORD
"0" = "line_start"          # Go to column 0
"^" = "line_first_char"     # Go to first non-blank character
"$" = "line_end"            # Go to end of line
"gg" = "buffer_start"       # Go to first line of buffer
"G" = "buffer_end"          # Go to last line of buffer

# Mode transitions
"i" = "insert_mode"            # Insert before cursor
"I" = "insert_line_start"      # Insert at start of line
"a" = "insert_after"           # Insert after cursor
"A" = "insert_line_end"        # Insert at end of line
"o" = "insert_line_below"      # Open a new line below and insert
"O" = "insert_line_above"      # Open a new line above and insert
"v" = "visual_mode"            # Enter Visual (character) mode
"V" = "visual_line_mode"       # Enter Visual Line mode
"Ctrl+v" = "visual_block_mode" # Enter Visual Block mode
"Alt+v" = "visual_block_mode"  # Alternative for terminals that intercept Ctrl+V (e.g., VS Code)
"R" = "replace_mode"           # Enter Replace mode

# Search
"/" = "search_forward"      # Start forward search
"?" = "search_backward"     # Start backward search
"n" = "search_next"         # Jump to next match
"N" = "search_previous"     # Jump to previous match

# Character navigation (find/till)
"f" = "start_find_char_forward"    # Find next occurrence of a char to the right
"F" = "start_find_char_backward"   # Find previous occurrence of a char to the left
"t" = "start_till_char_forward"    # Move right before next occurrence of a char
"T" = "start_till_char_backward"   # Move left after previous occurrence of a char
";" = "repeat_char_search"         # Repeat last f/F/t/T in same direction
"," = "repeat_char_search_reverse" # Repeat last f/F/t/T in opposite direction

# Commands
":" = "command_mode"        # Enter command-line (ex) mode

# Delete operations
"x" = "delete_char_at_cursor"     # Delete character under cursor
"X" = "delete_char_before_cursor" # Delete character before cursor
"dd" = "delete_line"              # Delete current line
"D" = "delete_to_end_of_line"     # Delete from cursor to end of line

# Line operations
"J" = "join_lines"                # Join next line to current
"C" = "change_to_end_of_line"     # Change from cursor to end of line
"S" = "change_entire_line"        # Change entire line
"s" = "substitute_char"           # Replace character under cursor

# Bracket matching
"%" = "bracket_match"             # Jump to matching bracket/brace

# Paragraph movement
"{" = "paragraph_backward"        # Move to previous paragraph
"}" = "paragraph_forward"         # Move to next paragraph

# Sentence movement
"(" = "sentence_backward"         # Move to previous sentence

")" = "sentence_forward"          # Move to next sentence

# Section movement
"[[" = "section_backward"         # Move to previous section

"]]" = "section_forward"          # Move to next section

# Repeat operations
"." = "repeat_last_change"        # Repeat last change

# Operators (enter operator-pending mode)
"d" = "operator_delete"            # Delete operator (awaits motion/text object)
"c" = "operator_change"            # Change operator (delete then insert)
"y" = "operator_yank"              # Yank (copy) operator
">" = "operator_indent"            # Indent operator
"<" = "operator_unindent"          # Unindent operator
"~" = "operator_toggle_case"       # Toggle case operator

# Yank (copy) operations
"yy" = "yank_line"                 # Yank (copy) current line
"yw" = "yank_word"                 # Yank word
"y$" = "yank_to_end_of_line"       # Yank to end of line

# Put (paste) operations
"p" = "put_after"                  # Paste after cursor/line
"P" = "put_before"                 # Paste before cursor/line

# File operations
"Ctrl+s" = "save_file"             # Save file

# Macro operations
"q" = "start_macro_recording"  # q{register} - start/stop recording
"@" = "execute_macro"          # @{register} - execute macro

# Marks
"m" = "mark_set_start"         # m{register} - set mark
"'" = "mark_jump_line"         # '{register} - jump to line of mark
"`" = "mark_jump_exact"        # `{register} - jump to exact position of mark

# Undo/Redo
"u" = "undo"                       # Undo last change
"Ctrl+r" = "redo"                  # Redo

# Buffer management
"Ctrl+n" = "buffer_next"           # Next buffer
"Ctrl+p" = "buffer_previous"       # Previous buffer

# Scrolling operations (Vim-style)
"Ctrl+e" = "scroll_down_line"      # Scroll down one line
"Ctrl+y" = "scroll_up_line"        # Scroll up one line
"Ctrl+f" = "scroll_down_page"      # Scroll down one page (Page Down)
"Ctrl+b" = "scroll_up_page"        # Scroll up one page (Page Up)
"Ctrl+d" = "scroll_down_half_page" # Scroll down half page
"Ctrl+u" = "scroll_up_half_page"   # Scroll up half page

# Centering operations (z commands)
"zz" = "center_cursor"    # Center current line in viewport
"zt" = "cursor_to_top"    # Move current line to top of viewport
"zb" = "cursor_to_bottom" # Move current line to bottom of viewport

# Alternative center commands
"z." = "center_cursor"      # Center current line (alternative)
"z-" = "cursor_to_bottom"   # Move current line to bottom (alternative)
"z Enter" = "cursor_to_top" # Move current line to top (alternative)

# Window/Split navigation
"Ctrl+w h" = "window_left"      # Move to window left
"Ctrl+w j" = "window_down"      # Move to window down
"Ctrl+w k" = "window_up"        # Move to window up
"Ctrl+w l" = "window_right"     # Move to window right
"Ctrl+w Left" = "window_left"   # Move to window left (arrow key)
"Ctrl+w Down" = "window_down"   # Move to window down (arrow key)
"Ctrl+w Up" = "window_up"       # Move to window up (arrow key)
"Ctrl+w Right" = "window_right" # Move to window right (arrow key)

# Window splitting - basic
"Ctrl+w s" = "split_horizontal" # Split horizontally (below)
"Ctrl+w v" = "split_vertical"   # Split vertically (right)

# Window splitting - directional  
"Ctrl+w S" = "split_horizontal_above" # Split horizontally above current window
"Ctrl+w V" = "split_vertical_left"    # Split vertically left of current window

# Window management
"Ctrl+w c" = "close_window" # Close current window
"Ctrl+w q" = "close_window" # Close current window (alternative)

# Window resizing
"Ctrl+w >" = "resize_window_wider"    # Make window wider
"Ctrl+w <" = "resize_window_narrower" # Make window narrower
"Ctrl+w +" = "resize_window_taller"   # Make window taller
"Ctrl+w -" = "resize_window_shorter"  # Make window shorter

# Additional navigation keys
"Home" = "line_start"              # Go to start of line
"End" = "line_end"                 # Go to end of line
"PageUp" = "scroll_up_page"        # Map PageUp to scroll up page
"PageDown" = "scroll_down_page"    # Map PageDown to scroll down page

# Function keys for common operations
"F1" = "command_mode"   # Help/command mode
"F2" = "save_file"      # Quick save
"F3" = "search_forward" # Find
"F10" = "quit"          # Exit

[insert_mode]
# Basic editing
"Char" = "insert_char"           # Insert typed character
"Enter" = "new_line"             # Insert newline
"Backspace" = "delete_char"      # Delete character before cursor
"Delete" = "delete_char_forward" # Delete character under cursor
"Tab" = "insert_tab"             # Insert a tab character or spaces

# Movement in insert mode
"Left" = "cursor_left"          # Move cursor left
"Right" = "cursor_right"        # Move cursor right
"Up" = "cursor_up"              # Move cursor up
"Down" = "cursor_down"          # Move cursor down

# Mode transitions
"Escape" = "normal_mode"         # Return to Normal mode
"Ctrl+c" = "normal_mode"         # Return to Normal mode

# Additional navigation in insert mode
"Home" = "line_start"            # Move to start of line
"End" = "line_end"               # Move to end of line
"Ctrl+a" = "line_start"          # Move to start of line
"Ctrl+e" = "line_end"            # Move to end of line
"Ctrl+w" = "delete_word_backward" # Delete previous word

[command_mode]
# Command execution
"Enter" = "execute_command"   # Run the typed command
"Escape" = "normal_mode"      # Cancel and return to Normal mode
"Ctrl+c" = "normal_mode"      # Cancel and return to Normal mode

# Editing command line
"Char" = "append_command"           # Add character to command line
"Backspace" = "delete_command_char" # Delete last command character

# Command completion
"Tab" = "command_complete"       # Trigger command completion
"Ctrl+n" = "completion_next"     # Next completion item
"Ctrl+p" = "completion_previous" # Previous completion item
"Ctrl+y" = "completion_accept"   # Accept completion

[visual_mode]
# Movement (inherits from normal mode)
"h" = "cursor_left"     # Extend left
"j" = "cursor_down"     # Extend down
"k" = "cursor_up"       # Extend up
"l" = "cursor_right"    # Extend right
"w" = "word_forward"    # Extend to next word start
"b" = "word_backward"   # Extend to previous word start
"0" = "line_start"      # Extend to start of line
"$" = "line_end"        # Extend to end of line

# Actions
"d" = "delete_selection" # Delete selection
"y" = "yank_selection"   # Copy selection
"c" = "change_selection" # Change selection

# Mode transitions
"Escape" = "normal_mode"  # Return to Normal mode
"v" = "normal_mode"       # Toggle Visual off

[visual_line_mode]
# Movement (inherits from normal mode)
"h" = "cursor_left"   # Extend left by lines
"j" = "cursor_down"   # Extend down by lines
"k" = "cursor_up"     # Extend up by lines
"l" = "cursor_right"  # Extend right by lines
"w" = "word_forward"  # Extend to next word start
"b" = "word_backward" # Extend to previous word start
"0" = "line_start"    # Extend to start of line
"$" = "line_end"      # Extend to end of line

# Actions
"d" = "delete_selection" # Delete selected lines
"y" = "yank_selection"   # Copy selected lines
"c" = "change_selection" # Change selected lines

# Mode transitions
"Escape" = "normal_mode" # Return to Normal mode
"V" = "normal_mode"      # Toggle Visual Line off
"v" = "visual_mode"      # Switch to Visual (character)

[visual_block_mode]
# Movement (inherits from normal mode)
"h" = "cursor_left"   # Extend block left
"j" = "cursor_down"   # Extend block down
"k" = "cursor_up"     # Extend block up
"l" = "cursor_right"  # Extend block right
"w" = "word_forward"  # Extend to next word start
"b" = "word_backward" # Extend to previous word start
"0" = "line_start"    # Extend to start of line
"$" = "line_end"      # Extend to end of line

# Actions
"d" = "delete_selection" # Delete block
"y" = "yank_selection"   # Copy block
"c" = "change_selection" # Change block

# Mode transitions
"Escape" = "normal_mode" # Return to Normal mode
"Ctrl+v" = "normal_mode" # Toggle Visual Block off
"Alt+v" = "normal_mode"  # Toggle Visual Block off (alternative)
"v" = "visual_mode"      # Switch to Visual (character)

[replace_mode]
# Character replacement
"Char" = "replace_char"   # Replace character under cursor, then advance
"Escape" = "normal_mode"  # Return to Normal mode
"Ctrl+c" = "normal_mode"  # Return to Normal mode

[search_mode]
# Search input
"Char" = "append_search"   # Add character to search pattern
"Enter" = "execute_search" # Execute current search
"Escape" = "normal_mode"   # Cancel search
"Ctrl+c" = "normal_mode"   # Cancel search
"Backspace" = "delete_search_char" # Delete last search character

[operator_pending_mode]
# Compound operator sequences (when already in operator-pending mode)
"d" = "delete_line"         # dd - delete current line (second d when already in OP mode)
"c" = "change_entire_line"  # cc - change current line  
"y" = "yank_line"           # yy - yank current line

# Text objects for operators
# Word text objects
"iw" = "text_object_iw" # inner word
"aw" = "text_object_aw" # a word
"iW" = "text_object_iW" # inner WORD
"aW" = "text_object_aW" # a WORD

# Sentence and paragraph text objects
"is" = "text_object_is" # inner sentence
"as" = "text_object_as" # a sentence
"ip" = "text_object_ip" # inner paragraph
"ap" = "text_object_ap" # a paragraph

# Quote text objects
"i\"" = "text_object_i\"" # inner double quotes
"a\"" = "text_object_a\"" # a double quotes
"i'" = "text_object_i'"   # inner single quotes
"a'" = "text_object_a'"   # a single quotes
"i`" = "text_object_i`"   # inner backticks
"a`" = "text_object_a`"   # a backticks

# Bracket text objects
"i(" = "text_object_i(" # inner parentheses
"a(" = "text_object_a(" # a parentheses
"i)" = "text_object_i)" # inner parentheses (alternative)
"a)" = "text_object_a)" # a parentheses (alternative)
"ib" = "text_object_ib" # inner parentheses (vim style)
"ab" = "text_object_ab" # a parentheses (vim style)

"i[" = "text_object_i[" # inner square brackets
"a[" = "text_object_a[" # a square brackets
"i]" = "text_object_i]" # inner square brackets (alternative)
"a]" = "text_object_a]" # a square brackets (alternative)

"i{" = "text_object_i{" # inner curly braces
"a{" = "text_object_a{" # a curly braces
"i}" = "text_object_i}" # inner curly braces (alternative)
"a}" = "text_object_a}" # a curly braces (alternative)
"iB" = "text_object_iB" # inner curly braces (vim style)
"aB" = "text_object_aB" # a curly braces (vim style)

"i<" = "text_object_i<" # inner angle brackets
"a<" = "text_object_a<" # a angle brackets
"i>" = "text_object_i>" # inner angle brackets (alternative)
"a>" = "text_object_a>" # a angle brackets (alternative)

# Tag text objects (HTML/XML)
"it" = "text_object_it" # inner tag
"at" = "text_object_at" # a tag

# Escape to cancel operator
"Escape" = "normal_mode" # Cancel operator
"Ctrl+c" = "normal_mode" # Cancel operator
```

### Theme Configuration (`themes.toml`)

The `plain_text` key controls the default color for text that has no specific syntax highlight.

```toml
# ###############################################################################
# Oxidized Theme Configuration
#
# This file enumerates all supported UI and syntax theme colors, with inline
# comments describing each key’s purpose. Define one or more themes under
# the [themes.<name>] tables and select the active theme via [theme].current
# (or by setting display.color_scheme in editor.toml / using :setp).
#
# Notes:
# - All keys are optional for a theme; unspecified colors fall back to
#   built-in defaults.
# - UI colors live under [themes.<name>.ui] (and nested tables like
#   [.ui.statusline], [.ui.mode]); syntax colors live under
#   [themes.<name>.tree_sitter].
# ###############################################################################

[theme]
current = "default"

[themes.default]
name = "Rust Theme"
description = "Rust-inspired color palette with warm oranges and earth tones"

[themes.default.ui]
background = "#1f1611"          # Editor background
status_bg = "#ce422b"           # Statusline background
status_fg = "#cccccc"           # Statusline foreground text
status_modified = "#f74c00"     # Statusline accent when buffer is modified
line_number = "#8c6239"         # Gutter line numbers
line_number_current = "#deb887" # Current line number highlight
mark_indicator = "#e6b422"      # Golden mark label in number column
cursor_line_bg = "#2d2318"      # Current cursor line background
empty_line = "#4a3728"          # Filler for lines past end-of-file
command_line_bg = "#1f1611"     # ':' command-line background
command_line_fg = "#deb887"     # ':' command-line text color
completion_key_fg = "#deb887"   # Completion popup: canonical key column (defaults to command_line_fg if omitted)
completion_alias_fg = "#cccccc" # Completion popup: alias column (defaults to command_line_fg if omitted)
completion_value_fg = "#ffe6c7" # Completion popup: value column (defaults to command_line_fg if omitted)
selection_bg = "#8c4a2b"        # Generic selection background (fallback)
visual_line_bg = "#8c4a2b"      # Line-wise visual selection background
visual_char_bg = "#7a3f28"      # Character-wise visual selection background  
visual_block_bg = "#9a5235"     # Block-wise visual selection background (future)
warning = "#ff8c00"             # Warning text/accent color
error = "#dc322f"               # Error text/accent color

[themes.default.ui.statusline]
left_bg = "#ce422b"   # Left segment background
left_fg = "#cccccc"   # Left segment text
mid_bg = "#ce422b"    # Middle segment background
mid_fg = "#cccccc"    # Middle segment text
right_bg = "#ce422b"  # Right segment background
right_fg = "#cccccc"  # Right segment text

[themes.default.ui.mode]
normal_bg = "#ce422b"       # Statusline mode badge background (Normal)
normal_fg = "#ffffff"       # Statusline mode badge text (Normal)
insert_bg = "#ce422b"       # Statusline mode badge background (Insert)
insert_fg = "#ffe6c7"       # Statusline mode badge text (Insert)
visual_bg = "#ce422b"       # Statusline mode badge background (Visual)
visual_fg = "#fff3da"       # Statusline mode badge text (Visual)
visual_line_bg = "#ce422b"  # Statusline mode badge background (Visual Line)
visual_line_fg = "#ffedd5"  # Statusline mode badge text (Visual Line)
visual_block_bg = "#ce422b" # Statusline mode badge background (Visual Block)
visual_block_fg = "#fffbeb" # Statusline mode badge text (Visual Block)
replace_bg = "#ce422b"      # Statusline mode badge background (Replace)
replace_fg = "#ffd1c1"      # Statusline mode badge text (Replace)
command_bg = "#ce422b"      # Statusline mode badge background (Command)
command_fg = "#ffffff"      # Statusline mode badge text (Command)

[themes.default.tree_sitter]
# Rust-inspired color scheme with warm earth tones
plain_text = "#deb887"     # Default text color (no specific syntax)
keyword = "#ce422b"        # Keywords (fn, let, pub, if, else, return)
function = "#b58900"       # Function and method names
type = "#268bd2"           # Types, structs, enums, traits
string = "#859900"         # String literals
number = "#d33682"         # Numeric literals
comment = "#93a1a1"        # Comments
# Everything else uses warm foreground color
identifier = "#deb887"     # Identifiers (generic)
variable = "#deb887"       # Variables and bindings
operator = "#cb4b16"       # Operators (+, -, *, /, =, ==, etc.)
punctuation = "#839496"    # Punctuation (commas, periods)
delimiter = "#839496"      # Delimiters (brackets, braces, parentheses)
character = "#859900"      # Character literals
documentation = "#586e75"  # Documentation comments
preprocessor = "#6c71c4"   # Preprocessor/attributes directives
macro = "#dc322f"          # Macro invocations/definitions
attribute = "#2aa198"      # Attributes/annotations
label = "#cb4b16"          # Labels and lifetimes
constant = "#d33682"       # Constants
```

## 🏗️ Architecture Overview

Developer docs and diagrams:

- [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) — high-level guide with inline ASCII diagrams

### Core Components

**Editor Engine:**

- **Modal System**: Complete implementation of Normal, Insert, Command, Visual, Replace, and Search modes
- **Buffer Management**: Multi-buffer support with efficient switching and state management
- **Window System**: Advanced window splitting, navigation, and resizing with independent viewports
- **Undo Engine**: Sophisticated multi-level undo/redo with operation tracking

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
- **Command Integration**: `:set` (session) and `:setp` (persistent) command pair for runtime configuration
- **Theme Engine**: Dynamic theme switching with semantic color schemes

### Performance Features

- **Efficient Rendering**: Minimized redraws and buffered terminal updates
- **Background Processing**: Syntax highlighting runs asynchronously via a
   worker thread and event-driven dispatcher
- **Versioned Results**: A monotonic token prevents stale highlight results
   from being applied after scroll/resize/theme changes
- **Bounded Cache**: A small in-memory LRU stores per-line highlights for fast
   reuse without unbounded growth
- **Memory Management**: Rust's ownership system ensures memory safety without
   garbage collection
- **Pragmatic Data Structures**: Efficient line-based model today; advanced
   gap/rope structures are planned
- **Fast Search Path (ASCII)**: Case-insensitive search uses an ASCII fast
   path to avoid per-line lowercase allocations when possible; Unicode-
   insensitive search preserves exact matching semantics

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
- Distinct ephemeral (`:set`) vs persistent (`:setp`) configuration updates
- Over 30 configurable editor settings

**Syntax Highlighting:**

- Async Tree-sitter worker with versioned results and a dispatcher thread
- Priority-based requests (Critical: cursor line, High: viewport, Medium: nearby)
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
- **Window Management**: Splitting, navigation, resizing, viewport handling

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

- **log + tracing-subscriber/appender**: Logging infrastructure (file-by-default in TTY, stderr otherwise)
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
- **Command System**: Ex-commands with completion (:w, :q, :set, :setp, etc.)
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
