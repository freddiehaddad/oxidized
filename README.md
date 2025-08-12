# Oxidized: A High-Performance Vim Clone in Rust

**Oxidized** is a modern terminal-based text editor that brings Vim's powerful modal editing to the 21st century. Built from the ground up in Rust, it combines Vim's time-tested editing philosophy with cutting-edge architecture, delivering exceptional performance, memory safety, and extensibility.

> Note: This project is under active development. Features, behavior, and APIs may change.

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
  - [Logging quick reference](#quick-reference)
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
[display]
show_line_numbers = false
show_relative_numbers = true
show_cursor_line = true
color_scheme = "default"
syntax_highlighting = true
show_marks = true  # Show a mark's letter in the gutter/number column

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
completion_menu_width = 36
completion_menu_height = 8
percent_path_root = true

[languages]
default_language = "text"

[languages.extensions]
"rs" = "rust"
"toml" = "toml"
"md" = "markdown"
"txt" = "text"
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
- Value suggestions (after `=`) appear for numeric and boolean options (`tabstop=`, `scrolloff=`, `timeoutlen=`, `undolevels=`, etc.) and for enumerated / dynamic lists like `colorscheme=` (populated from `themes.toml`).
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
# Default Keymaps Configuration
# This file defines all keybindings for the editor
# Format: key = "action" or key = { action = "command", args = ["arg1", "arg2"] }

[normal_mode]
# Movement
"h" = "cursor_left"
"j" = "cursor_down"
"k" = "cursor_up"
"l" = "cursor_right"
"Left" = "cursor_left"
"Down" = "cursor_down"
"Up" = "cursor_up"
"Right" = "cursor_right"

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
"I" = "insert_line_start"
"a" = "insert_after"
"A" = "insert_line_end"
"o" = "insert_line_below"
"O" = "insert_line_above"
"v" = "visual_mode"
"V" = "visual_line_mode"
"Ctrl+v" = "visual_block_mode"
"Alt+v" = "visual_block_mode" # Alternative for terminals that intercept Ctrl+V (e.g., VS Code)
"R" = "replace_mode"

# Search
"/" = "search_forward"
"?" = "search_backward"
"n" = "search_next"
"N" = "search_previous"

# Character navigation (find/till)
"f" = "start_find_char_forward"
"F" = "start_find_char_backward"
"t" = "start_till_char_forward"
"T" = "start_till_char_backward"
";" = "repeat_char_search"
"," = "repeat_char_search_reverse"

# Commands
":" = "command_mode"

# Delete operations
"x" = "delete_char_at_cursor"
"X" = "delete_char_before_cursor"
"dd" = "delete_line"
"D" = "delete_to_end_of_line"

# Line operations
"J" = "join_lines"
"C" = "change_to_end_of_line"
"S" = "change_entire_line"
"s" = "substitute_char"

# Bracket matching
"%" = "bracket_match"

# Paragraph movement
"{" = "paragraph_backward"
"}" = "paragraph_forward"

# Sentence movement
"(" = "sentence_backward"

")" = "sentence_forward"

# Section movement
"[[" = "section_backward"

"]]" = "section_forward"

# Repeat operations
"." = "repeat_last_change"

# Operators (enter operator-pending mode)
"d" = "operator_delete"
"c" = "operator_change"
"y" = "operator_yank"
">" = "operator_indent"
"<" = "operator_unindent"
"~" = "operator_toggle_case"

# Yank (copy) operations
"yy" = "yank_line"
"yw" = "yank_word"
"y$" = "yank_to_end_of_line"

# Put (paste) operations
"p" = "put_after"
"P" = "put_before"

# File operations
"Ctrl+s" = "save_file"

# Macro operations
"q" = "start_macro_recording"  # q{register} - start/stop recording
"@" = "execute_macro"          # @{register} - execute macro

# Marks
"m" = "mark_set_start"         # m{register} - set mark
"'" = "mark_jump_line"         # '{register} - jump to line of mark
"`" = "mark_jump_exact"        # `{register} - jump to exact position of mark

# Undo/Redo
"u" = "undo"
"Ctrl+r" = "redo"

# Buffer management
"Ctrl+n" = "buffer_next"
"Ctrl+p" = "buffer_previous"

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
"z." = "center_cursor"     # Center current line (alternative)
"z-" = "cursor_to_bottom"  # Move current line to bottom (alternative)
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
"Home" = "line_start"
"End" = "line_end"
"PageUp" = "scroll_up_page"     # Map PageUp to scroll up page
"PageDown" = "scroll_down_page" # Map PageDown to scroll down page

# Function keys for common operations
"F1" = "command_mode"   # Help/command mode
"F2" = "save_file"      # Quick save
"F3" = "search_forward" # Find
"F10" = "quit"          # Exit

[insert_mode]
# Basic editing
"Char" = "insert_char"
"Enter" = "new_line"
"Backspace" = "delete_char"
"Delete" = "delete_char_forward"
"Tab" = "insert_tab"

# Movement in insert mode
"Left" = "cursor_left"
"Right" = "cursor_right"
"Up" = "cursor_up"
"Down" = "cursor_down"

# Mode transitions
"Escape" = "normal_mode"
"Ctrl+c" = "normal_mode"

# Additional navigation in insert mode
"Home" = "line_start"
"End" = "line_end"
"Ctrl+a" = "line_start"
"Ctrl+e" = "line_end"
"Ctrl+w" = "delete_word_backward"

[command_mode]
# Command execution
"Enter" = "execute_command"
"Escape" = "normal_mode"
"Ctrl+c" = "normal_mode"

# Editing command line
"Char" = "append_command"
"Backspace" = "delete_command_char"

# Command completion
"Tab" = "command_complete"
"Ctrl+n" = "completion_next"
"Ctrl+p" = "completion_previous"
"Ctrl+y" = "completion_accept"

[visual_mode]
# Movement (inherits from normal mode)
"h" = "cursor_left"
"j" = "cursor_down"
"k" = "cursor_up"
"l" = "cursor_right"
"w" = "word_forward"
"b" = "word_backward"
"0" = "line_start"
"$" = "line_end"

# Actions
"d" = "delete_selection"
"y" = "yank_selection"
"c" = "change_selection"

# Mode transitions
"Escape" = "normal_mode"
"v" = "normal_mode"

[visual_line_mode]
# Movement (inherits from normal mode)
"h" = "cursor_left"
"j" = "cursor_down"
"k" = "cursor_up"
"l" = "cursor_right"
"w" = "word_forward"
"b" = "word_backward"
"0" = "line_start"
"$" = "line_end"

# Actions
"d" = "delete_selection"
"y" = "yank_selection"
"c" = "change_selection"

# Mode transitions
"Escape" = "normal_mode"
"V" = "normal_mode"
"v" = "visual_mode"

[visual_block_mode]
# Movement (inherits from normal mode)
"h" = "cursor_left"
"j" = "cursor_down"
"k" = "cursor_up"
"l" = "cursor_right"
"w" = "word_forward"
"b" = "word_backward"
"0" = "line_start"
"$" = "line_end"

# Actions
"d" = "delete_selection"
"y" = "yank_selection"
"c" = "change_selection"

# Mode transitions
"Escape" = "normal_mode"
"Ctrl+v" = "normal_mode"
"Alt+v" = "normal_mode" # Alternative exit for Visual Block
"v" = "visual_mode"

[replace_mode]
# Character replacement
"Char" = "replace_char"
"Escape" = "normal_mode"
"Ctrl+c" = "normal_mode"

[search_mode]
# Search input
"Char" = "append_search"
"Enter" = "execute_search"
"Escape" = "normal_mode"
"Ctrl+c" = "normal_mode"
"Backspace" = "delete_search_char"

[operator_pending_mode]
# Compound operator sequences (when already in operator-pending mode)
"d" = "delete_line"     # dd - delete current line (second d when already in OP mode)
"c" = "change_entire_line" # cc - change current line  
"y" = "yank_line"       # yy - yank current line

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
"Escape" = "normal_mode"
"Ctrl+c" = "normal_mode"
```

### Theme Configuration (`themes.toml`)

The `plain_text` key controls the default color for text that has no specific syntax highlight.

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
status_fg = "#cccccc"
status_modified = "#f74c00"
line_number = "#8c6239"
line_number_current = "#deb887"
mark_indicator = "#e6b422"   # Color of the mark letter shown in gutter/number column
cursor_line_bg = "#2d2318"
empty_line = "#4a3728"
command_line_bg = "#1f1611"
command_line_fg = "#deb887"
selection_bg = "#8c4a2b"
visual_line_bg = "#8c4a2b"      # Line-wise visual selection background
visual_char_bg = "#7a3f28"      # Character-wise visual selection background
visual_block_bg = "#9a5235"     # Block-wise visual selection background
warning = "#ff8c00"
error = "#dc322f"

# Optional: granular status line colors per segment (fallbacks to status_bg/status_fg)
[themes.default.ui.statusline]
left_bg = "#ce422b"
left_fg = "#cccccc"
mid_bg = "#ce422b"
mid_fg = "#cccccc"
right_bg = "#ce422b"
right_fg = "#cccccc"

# Optional: per-mode colors for the mode token in the status line
[themes.default.ui.mode]
normal_bg = "#ce422b"
normal_fg = "#ffffff"
insert_bg = "#ce422b"
insert_fg = "#ffe6c7"
visual_bg = "#ce422b"
visual_fg = "#fff3da"
visual_line_bg = "#ce422b"
visual_line_fg = "#ffedd5"
visual_block_bg = "#ce422b"
visual_block_fg = "#fffbeb"
replace_bg = "#ce422b"
replace_fg = "#ffd1c1"
command_bg = "#ce422b"
command_fg = "#ffffff"

[themes.default.tree_sitter]
# Rust-inspired color scheme with warm earth tones
plain_text = "#deb887"   # Default text color when no syntax mapping applies
keyword = "#ce422b"  # Rust orange for keywords (fn, let, pub, etc.)
function = "#b58900" # Golden brown for function names
type = "#268bd2"     # Steel blue for types (keeps contrast)
string = "#859900"   # Olive green for strings
number = "#d33682"   # Magenta for numbers (good contrast)
comment = "#93a1a1"  # Light gray for comments
# Everything else uses warm foreground color
identifier = "#deb887"    # Burlywood for identifiers
variable = "#deb887"      # Burlywood for variables
operator = "#cb4b16"      # Orange-red for operators
punctuation = "#839496"   # Gray for punctuation
delimiter = "#839496"     # Gray for delimiters
character = "#859900"     # Same as strings
documentation = "#586e75" # Darker gray for docs
preprocessor = "#6c71c4"  # Purple for preprocessor
macro = "#dc322f"         # Red for macros
attribute = "#2aa198"     # Cyan for attributes
label = "#cb4b16"         # Orange for labels
constant = "#d33682"      # Same as numbers
```

## 🏗️ Architecture Overview

Developer docs and diagrams:

- docs/ARCHITECTURE.md — high-level guide with inline ASCII diagrams

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
- **Command Integration**: `:set` (session) and `:setp` (persistent) command pair for runtime configuration
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
- Distinct ephemeral (`:set`) vs persistent (`:setp`) configuration updates
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

Quick link: [Logging quick reference](#quick-reference)

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

Troubleshooting (Windows): if `cargo test` fails to remove `target\\debug\\oxy.exe`
with "Access is denied (os error 5)", ensure no running editor instance is
holding a file lock (close the editor or kill the process) and retry.
```

**Release Build:**

```powershell
# Windows - Optimized release build
cargo build --release

# Run with custom log level (TTY: logs go to file by default)
$env:RUST_LOG="debug"; .\target\release\oxy.exe filename.txt
# To force stderr instead of file:
# $env:OXY_LOG_DEST="stderr"; $env:RUST_LOG="debug"; .\target\release\oxy.exe filename.txt
```

```bash
# Linux/macOS - Optimized release build
cargo build --release

# Run with custom log level (TTY: logs go to file by default)
RUST_LOG=debug ./target/release/oxy filename.txt
# To force stderr instead of file:
# OXY_LOG_DEST=stderr RUST_LOG=debug ./target/release/oxy filename.txt
```

### 📊 Comprehensive Logging System

Oxidized provides extensive logging for development, debugging, and performance.
To avoid corrupting the terminal UI, when running in an interactive terminal
(TTY) logs are written to a file by default. In non-TTY contexts (e.g., CI),
logs go to stderr. Warnings and errors are mirrored to stderr even when file
logging is active.

#### Quick reference

```powershell
# Windows (PowerShell)
# 1) Default: logs go to oxidized.log when running in a terminal
cargo run filename.txt
Get-Content oxidized.log -Wait -Tail 50

# 2) Module-focused logging
$env:RUST_LOG="oxidized=info,oxidized::editor=debug"; cargo run

# 3) Force stderr logging instead of file
$env:OXY_LOG_DEST="stderr"; $env:RUST_LOG="debug"; cargo run
```

```bash
# Linux/macOS (Bash)
# 1) Default: logs go to oxidized.log when running in a terminal
cargo run filename.txt & tail -f oxidized.log

# 2) Module-focused logging
RUST_LOG="oxidized=info,oxidized::editor=debug" cargo run

# 3) Force stderr logging instead of file
OXY_LOG_DEST=stderr RUST_LOG=debug cargo run
```

#### **Log Levels and Usage**

**Available Log Levels** (in order of verbosity):

- `error` - Critical errors only
- `warn` - Warnings and errors
- `info` - General information, warnings, and errors
- `debug` - Detailed debugging information (**default for debug builds**)
- `trace` - Ultra-verbose tracing (development only)

> Note: Debug builds default to `debug` logging. Release builds respect `RUST_LOG` and are minimal by default.

#### **Environment Variables**

```powershell
# Windows PowerShell - Set logging level
$env:RUST_LOG="debug"                    # Debug level for all modules
$env:RUST_LOG="oxidized=trace"           # Trace level for oxidized only
$env:RUST_LOG="oxidized::editor=debug"   # Debug level for editor module only
$env:OXY_LOG_DEST="file|stderr|off"      # Destination override
$env:OXY_LOG_FILE="custom.log"           # File path (default: oxidized.log)

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
export OXY_LOG_DEST=file|stderr|off      # Destination override
export OXY_LOG_FILE=custom.log           # File path (default: oxidized.log)

# Multiple modules with different levels
export RUST_LOG="oxidized::buffer=trace,oxidized::syntax=debug,warn"

# Run with custom logging
cargo run filename.txt
```

#### **Viewing Logs**

Default behavior:

- Interactive TTY: logs are appended to `oxidized.log` in the working
   directory.
- Non-TTY (CI/headless): logs are written to stderr.
- Warnings and errors are mirrored to stderr when file logging is active.

```powershell
# Windows PowerShell
# Follow the default log file
cargo run filename.txt
Get-Content oxidized.log -Wait -Tail 50

# Force stderr output instead of file
$env:OXY_LOG_DEST="stderr"; $env:RUST_LOG="debug"; cargo run filename.txt
```

```bash
# Linux/macOS (Bash)
# Follow the default log file
cargo run filename.txt & tail -f oxidized.log

# Force stderr output instead of file
OXY_LOG_DEST=stderr RUST_LOG=debug cargo run filename.txt
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
