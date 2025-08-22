use crate::config::theme::{SyntaxTheme, ThemeConfig, UITheme};
use crate::core::buffer::LineEnding;
use crate::core::editor::EditorRenderState;
use crate::core::mode::{Mode, Position};
use crate::features::syntax::HighlightRange;
use crate::ui::terminal::Terminal;
use log::{debug, warn};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Context for rendering a line of text
struct LineRenderContext {
    line_number: usize,
    is_cursor_line: bool,
    selection: Option<crate::core::mode::Selection>,
    editor_mode: crate::core::mode::Mode,
    /// Horizontal base offset into the logical line for rendering
    base_offset: usize,
    /// Total character count of the full logical line (for selection math)
    total_line_chars: usize,
}

// Cache metadata for completion popup to avoid redundant redraws
#[derive(Clone)]
pub struct PopupCache {
    pub row: u16,
    pub col: u16,
    pub width: u16,
    pub height: u16,
    pub hash: u64,
}

pub struct UI {
    /// Top row of the current viewport
    viewport_top: usize,
    /// Show absolute line numbers
    pub show_line_numbers: bool,
    /// Show relative line numbers
    pub show_relative_numbers: bool,
    /// Highlight the current cursor line
    pub show_cursor_line: bool,
    /// Show mark indicator in the gutter/number column when a line has a mark
    pub show_marks: bool,
    /// Current UI theme from themes.toml
    theme: UITheme,
    /// Current syntax theme from themes.toml
    syntax_theme: SyntaxTheme,
    popup_cache: Option<PopupCache>,
}

impl Default for UI {
    fn default() -> Self {
        Self::new()
    }
}

impl UI {
    /// Compute the hanging indent columns for a rendered Markdown preview line.
    /// This prefers aligning wrapped continuations under the text following any
    /// blockquote prefix (▎ ), unordered list bullet (• ), or ordered list marker
    /// (e.g., `10. `). Returns the display column width to indent continuation rows.
    fn preview_hanging_indent_cols(&self, line: &str) -> usize {
        let mut i = 0usize;
        let mut cols = 0usize;
        let mut saw_blockquote = false;

        // Consume any number of blockquote prefixes: "▎ "
        while i < line.len() {
            let s = &line[i..];
            if s.starts_with("▎ ") {
                cols += UnicodeWidthStr::width("▎ ");
                i += "▎ ".len();
                saw_blockquote = true;
            } else {
                break;
            }
        }

        // Unordered list bullet used by preview: "• "
        if i < line.len() {
            let s = &line[i..];
            if s.starts_with("• ") {
                cols += UnicodeWidthStr::width("• ");
                return cols;
            }
        }

        // Ordered list: one or more ASCII digits, then ". "
        let digits_start = i;
        while i < line.len() {
            let b = line.as_bytes()[i];
            if b.is_ascii_digit() {
                cols += 1; // ASCII digit width
                i += 1;
            } else {
                break;
            }
        }
        if i > digits_start {
            let s = &line[i..];
            if s.starts_with(". ") {
                cols += UnicodeWidthStr::width(". ");
                return cols;
            }
        }

        // If we saw only blockquote prefix(es) return their width so continuation rows can
        // re-render the prefix instead of shifting text left.
        if saw_blockquote { cols } else { 0 }
    }

    pub fn new() -> Self {
        // Load theme configuration from themes.toml
        let theme_config = ThemeConfig::load();
        let current_theme = theme_config.get_current_theme();

        Self {
            viewport_top: 0,
            show_line_numbers: true,            // Enable by default like Vim
            show_relative_numbers: false,       // Disabled by default
            show_cursor_line: true,             // Enable by default
            show_marks: true,                   // Default to on per request
            theme: current_theme.ui,            // Use theme from themes.toml
            syntax_theme: current_theme.syntax, // Use syntax theme from themes.toml
            popup_cache: None,
        }
    }

    /// Compute the left gutter width. When absolute or relative line numbers are enabled,
    /// reserve enough space for the largest line number plus a space. If only marks are
    /// enabled, reserve a small fixed gutter so marks can be shown without numbers.
    pub fn compute_gutter_width(&self, total_lines: usize) -> usize {
        if self.show_line_numbers || self.show_relative_numbers {
            let width = total_lines.max(1).to_string().len();
            (width + 1).max(4) // At least 4 chars wide, +1 for space
        } else if self.show_marks {
            2 // Minimal gutter to display a single mark glyph/letter
        } else {
            0
        }
    }

    /// Compute a three-part status line (left/middle/right) within given width.
    pub fn compute_status_line_text(
        &self,
        editor_state: &crate::core::editor::EditorRenderState,
        width: u16,
    ) -> String {
        let layout = self.compute_status_line_layout(editor_state, width);
        let mut s = String::with_capacity(width as usize);
        s.push_str(&layout.left);
        s.push_str(&" ".repeat(layout.left_gap));
        s.push_str(&layout.mid);
        s.push_str(&" ".repeat(layout.right_gap));
        s.push_str(&layout.right);
        if s.len() < width as usize {
            s.push_str(&" ".repeat((width as usize) - s.len()));
        }
        if s.len() > width as usize {
            s.truncate(width as usize);
        }
        s
    }

    /// Internal: build status line parts and spacing so renderer can color segments.
    fn compute_status_line_layout(
        &self,
        editor_state: &crate::core::editor::EditorRenderState,
        width: u16,
    ) -> StatusLineLayout {
        let total = width as usize;

        // Left: mode, filename, modified
        let mut left = String::new();
        left.push_str(&format!(" {} ", editor_state.mode));
        if let Some(buffer) = &editor_state.current_buffer {
            let name = buffer
                .file_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "[No Name]".to_string());
            left.push_str(&format!(" {} ", name));
            if buffer.modified {
                left.push_str("[+] ");
            }
        } else {
            left.push_str(" [No Name] ");
        }

        // Middle: status message
        let mut mid = String::new();
        if !editor_state.status_message.is_empty() {
            mid.push_str(&editor_state.status_message);
        }

        // Right: cursor, indent, encoding, type, macro, search, progress
        let mut right = String::new();
        let sep = editor_state.config.statusline.separator.as_str();
        if let Some(buffer) = &editor_state.current_buffer {
            right.push_str(&format!(
                "Ln {}, Col {}",
                buffer.cursor.row + 1,
                buffer.cursor.col + 1
            ));

            // Indentation (from behavior config)
            if editor_state.config.statusline.show_indent {
                let spaces = editor_state.config.behavior.expand_tabs;
                let width = editor_state.config.behavior.tab_width;
                right.push_str(&format!(
                    "{}{}: {}",
                    sep,
                    if spaces { "Spaces" } else { "Tabs" },
                    width
                ));
            }

            // Encoding (ASCII vs UTF-8 heuristic)
            if editor_state.config.statusline.show_encoding {
                let is_ascii = buffer.lines.iter().all(|l| l.is_ascii());
                right.push_str(&format!(
                    "{}{}",
                    sep,
                    if is_ascii { "ASCII" } else { "UTF-8" }
                ));
            }

            if editor_state.config.statusline.show_eol {
                right.push_str(sep);
                let eol = match buffer.eol {
                    LineEnding::LF => "LF",
                    LineEnding::CRLF => "CRLF",
                    LineEnding::CR => "CR",
                };
                right.push_str(eol);
            }

            // File type
            if editor_state.config.statusline.show_type && editor_state.filetype.is_some() {
                right.push_str(sep);
                right.push_str(editor_state.filetype.as_deref().unwrap());
            }
        }

        // Macro recording
        if editor_state.config.statusline.show_macro && editor_state.macro_recording.is_some() {
            right.push_str(sep);
            right.push_str(&format!("REC @{}", editor_state.macro_recording.unwrap()));
        }

        // Search status
        if editor_state.config.statusline.show_search && editor_state.search_total > 0 {
            right.push_str(sep);
            match editor_state.search_index {
                Some(i) => right.push_str(&format!("{}/{}", i + 1, editor_state.search_total)),
                None => right.push_str(&format!("{} matches", editor_state.search_total)),
            }
        }

        // Progress
        if editor_state.config.statusline.show_progress && editor_state.current_buffer.is_some() {
            let buffer = editor_state.current_buffer.as_ref().unwrap();
            let total_lines = buffer.lines.len().max(1);
            let percent = ((buffer.cursor.row + 1) * 100) / total_lines;
            right.push_str(sep);
            right.push_str(&format!("{}%", percent));
        }

        // Measure and fit into total width with priorities and truncation
        let left_w = unicode_width::UnicodeWidthStr::width(left.as_str());
        let mid_w = unicode_width::UnicodeWidthStr::width(mid.as_str());
        let right_w = unicode_width::UnicodeWidthStr::width(right.as_str());

        // If everything fits, compute gaps for centered middle and right-aligned right

        // Helper to truncate end of a string by columns
        let trunc_end = |s: &mut String, target_cols: usize| {
            use unicode_segmentation::UnicodeSegmentation;
            let mut cols = 0usize;
            let mut out = String::new();
            for g in s.graphemes(true) {
                let w = unicode_width::UnicodeWidthStr::width(g);
                if cols + w > target_cols {
                    break;
                }
                out.push_str(g);
                cols += w;
            }
            *s = out;
        };

        // Shrink until it fits: drop mid first, then truncate left filename, then right details
        if left_w + mid_w + right_w > total {
            // Drop middle first
            mid.clear();
            // middle cleared
        }
        if left_w + right_w > total {
            // Truncate left to make room for right; keep mode and some filename
            trunc_end(&mut left, total.saturating_sub(right_w));
            // recompute if needed later
        }
        if left_w + right_w > total {
            // Still too big: truncate right as last resort
            trunc_end(&mut right, total.saturating_sub(left_w));
            // recompute if needed later
        }

        // Recompute widths after potential truncations
        let l_w = unicode_width::UnicodeWidthStr::width(left.as_str());
        let m_w = unicode_width::UnicodeWidthStr::width(mid.as_str());
        let r_w = unicode_width::UnicodeWidthStr::width(right.as_str());
        let rem = total.saturating_sub(l_w + r_w);
        let mid_space = rem;
        let left_gap = mid_space.saturating_sub(m_w) / 2;
        let right_gap = mid_space.saturating_sub(m_w) - left_gap;

        StatusLineLayout {
            left,
            mid,
            right,
            left_gap,
            right_gap,
        }
    }

    // --- UTF-8 helpers ---
    /// Convert a character position to a byte index by walking char_indices.
    #[doc(hidden)]
    pub fn floor_char_boundary(s: &str, char_pos: usize) -> usize {
        if char_pos == 0 {
            return 0;
        }
        for (count, (b, _)) in s.char_indices().enumerate() {
            if count == char_pos {
                return b;
            }
        }
        s.len()
    }

    /// Compute the end byte index for a wrapped segment of at most `width` columns
    /// starting at `start_byte`. If `word_break` is true, prefer the last whitespace
    /// break within the segment (including that whitespace grapheme).
    #[doc(hidden)]
    pub fn wrap_next_end_byte(
        &self,
        s: &str,
        start_byte: usize,
        width: usize,
        word_break: bool,
    ) -> (usize, usize) {
        if start_byte >= s.len() || width == 0 {
            return (start_byte, 0);
        }
        let slice = &s[start_byte..];
        let mut cols = 0usize;
        let mut last_break_end_rel: Option<usize> = None;
        let mut last_good_rel = 0usize;
        let mut seg_graphemes = 0usize;
        for (rel, g) in slice.grapheme_indices(true) {
            let g_end = rel + g.len();
            let g_cols = UnicodeWidthStr::width(g);
            if g.trim().is_empty() {
                last_break_end_rel = Some(g_end);
            }
            if cols + g_cols > width {
                if word_break && let Some(b) = last_break_end_rel {
                    let end = start_byte + b;
                    let seg_count = slice[..b].graphemes(true).count();
                    return (end, seg_count);
                }
                return (start_byte + last_good_rel, seg_graphemes);
            }
            cols += g_cols;
            last_good_rel = g_end;
            seg_graphemes += 1;
            if cols == width {
                if word_break && let Some(b) = last_break_end_rel {
                    let end = start_byte + b;
                    let seg_count = slice[..b].graphemes(true).count();
                    return (end, seg_count);
                }
                return (start_byte + last_good_rel, seg_graphemes);
            }
        }
        let end = start_byte + slice.len();
        let seg_count = slice.graphemes(true).count();
        (end, seg_count)
    }

    /// Set the UI theme by loading from themes.toml
    pub fn set_theme(&mut self, theme_name: &str) {
        debug!("Setting UI theme to: '{}'", theme_name);
        let theme_config = ThemeConfig::load();
        if let Some(complete_theme) = theme_config.get_theme(theme_name) {
            debug!("Successfully loaded theme: '{}'", theme_name);
            self.theme = complete_theme.ui;
            self.syntax_theme = complete_theme.syntax;
            // Invalidate popup cache so colors refresh under new theme
            self.popup_cache = None;
        } else {
            warn!("Theme '{}' not found, using default theme", theme_name);
            // Fallback to default theme if theme not found
            let default_theme = theme_config.get_current_theme();
            self.theme = default_theme.ui;
            self.syntax_theme = default_theme.syntax;
            self.popup_cache = None;
        }
    }

    /// Get current theme name
    pub fn theme_name(&self) -> &'static str {
        // Load current theme from config
        let theme_config = ThemeConfig::load();
        // For now, return the current theme name - could be enhanced to track theme state
        if theme_config.theme.current == "dark" {
            "dark"
        } else if theme_config.theme.current == "light" {
            "light"
        } else if theme_config.theme.current == "ferris" {
            "ferris"
        } else {
            "default"
        }
    }

    /// Get the appropriate visual selection background color based on the editor mode
    fn get_visual_selection_bg(&self, mode: crate::core::mode::Mode) -> crossterm::style::Color {
        use crate::core::mode::Mode;
        match mode {
            Mode::Visual => self.theme.visual_char_bg,
            Mode::VisualLine => self.theme.visual_line_bg,
            Mode::VisualBlock => self.theme.visual_block_bg,
            Mode::Select => self.theme.select_char_bg,
            Mode::SelectLine => self.theme.select_line_bg,
            _ => self.theme.selection_bg, // Fallback for other cases
        }
    }

    /// Calculate line selection range for visual selection using core helper
    fn calculate_line_selection_range(
        &self,
        line_number: usize,
        selection: Option<crate::core::mode::Selection>,
        base_offset: usize,
        total_line_chars: usize,
    ) -> Option<(usize, usize)> {
        selection
            .and_then(|sel| sel.highlight_span_for_line(line_number, total_line_chars))
            .map(|(start, end)| {
                (
                    start.saturating_sub(base_offset),
                    end.saturating_sub(base_offset),
                )
            })
    }

    pub fn render(
        &mut self,
        terminal: &mut Terminal,
        editor_state: &EditorRenderState,
        full_redraw: bool,
    ) -> io::Result<()> {
        // Attempt scroll optimization: if a single primary window scrolled by small delta, shift prev frame rows.
        if !full_redraw && let Some(win) = editor_state.window_manager.current_window() {
            // Use UI's stored viewport_top (for current active view) as previous value
            let old_top = self.viewport_top;
            let new_top = win.viewport_top;
            let delta = new_top as i32 - old_top as i32;
            if delta != 0 && delta.unsigned_abs() <= (win.height / 2) as u32 {
                let top = win.y; // already u16
                let height = win.content_height() as u16;
                terminal.scroll_prev_frame_region(
                    top,
                    height,
                    delta as i16,
                    Some(self.theme.background),
                );
            }
            // Update stored viewport_top for next frame
            self.viewport_top = new_top;
        }

        // Start shadow frame capture (double buffering) with background color AFTER possible scroll shift.
        terminal.begin_frame(self.theme.background);
        // Refresh terminal size to reflect any recent resize events
        terminal.update_size()?;
        let (width, height) = terminal.size();

        // Start double buffering - queue all operations without immediate display
        terminal.queue_hide_cursor()?;

        // Force a full repaint on demand by invalidating previous frame instead of issuing a terminal clear.
        if full_redraw {
            terminal.invalidate_previous_frame();
        }

        // Determine reserved rows from config flags
        let reserved_rows: u16 = (editor_state.config.interface.show_status_line as u16)
            + (editor_state.config.interface.show_command as u16);

        // Popup area is no longer proactively cleared; underlying window content is always rendered
        // earlier in the frame, and diffing restores it automatically when the popup disappears.
        // (popup visibility computed on demand where needed; previous proactive clearing removed)

        // Render all windows
        self.render_windows(terminal, editor_state, reserved_rows)?;

        // Render status line if enabled in config
        if editor_state.config.interface.show_status_line {
            self.render_status_line(terminal, editor_state, width, height)?;
        }

        // Render command line if enabled and in command or search mode
        if editor_state.config.interface.show_command
            && (editor_state.mode == Mode::Command || editor_state.mode == Mode::Search)
        {
            self.render_command_line(terminal, editor_state, width, height)?;
        } else if editor_state.config.interface.show_command {
            // Draw an empty command line row without issuing clear_line; we fully overwrite the row.
            let command_row = height.saturating_sub(1);
            terminal.queue_move_cursor(Position::new(command_row as usize, 0))?;
            terminal.queue_set_bg_color(self.theme.background)?;
            terminal.queue_print(&" ".repeat(width as usize))?;
            terminal.queue_reset_color()?;
        }

        // Render command completion popup if enabled and active
        if editor_state.config.interface.show_command
            && editor_state.mode == Mode::Command
            && editor_state.command_completion.should_show()
        {
            self.render_completion_popup(terminal, editor_state, width, height)?;
        }

        // Position cursor and show it
        self.position_cursor(terminal, editor_state)?;

        terminal.queue_show_cursor()?;

        // End double buffering - flush all queued operations at once
        // This eliminates flicker by making all changes appear atomically
        terminal.flush_frame()?;

        Ok(())
    }

    fn render_windows(
        &self,
        terminal: &mut Terminal,
        editor_state: &EditorRenderState,
        reserved_rows: u16,
    ) -> io::Result<()> {
        // Render each window
        for window in editor_state.window_manager.all_windows().values() {
            // Get the buffer for this window
            let buffer = if let Some(buffer_id) = window.buffer_id {
                editor_state.all_buffers.get(&buffer_id)
            } else {
                continue;
            };

            if let Some(buffer) = buffer {
                self.render_window_buffer(terminal, window, buffer, editor_state)?;

                // Draw window border if there are multiple windows
                if editor_state.window_manager.window_count() > 1 {
                    self.render_window_border(
                        terminal,
                        window,
                        editor_state.current_window_id == Some(window.id),
                        reserved_rows,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn render_window_buffer(
        &self,
        terminal: &mut Terminal,
        window: &crate::core::window::Window,
        buffer: &crate::core::buffer::Buffer,
        editor_state: &EditorRenderState,
    ) -> io::Result<()> {
        let content_height = window.content_height();

        // Calculate gutter width for this window (numbers or marks)
        let line_number_width = self.compute_gutter_width(buffer.lines.len());

        let text_start_col = window.x as usize + line_number_width;
        let text_width = window.width.saturating_sub(line_number_width as u16) as usize;

        // Wrapping flags: use preview-specific setting if this window shows the preview buffer
        let wrap_enabled = if let Some(prev_id) = editor_state.markdown_preview_buffer_id {
            if Some(prev_id) == window.buffer_id {
                editor_state.config.markdown_preview.wrap_lines
            } else {
                editor_state.config.behavior.wrap_lines
            }
        } else {
            editor_state.config.behavior.wrap_lines
        };
        // Use word-boundary wrapping for the markdown preview window to keep words intact
        let word_break = if let Some(prev_id) = editor_state.markdown_preview_buffer_id {
            if Some(prev_id) == window.buffer_id {
                true
            } else {
                editor_state.config.behavior.line_break
            }
        } else {
            editor_state.config.behavior.line_break
        };

        if wrap_enabled {
            // Render using soft wrapping: multiple visual rows per logical line
            let mut screen_rows_rendered = 0usize;
            let mut buf_row = window.viewport_top;
            let is_active_window = editor_state.current_window_id == Some(window.id);

            while screen_rows_rendered < content_height {
                let screen_row = window.y as usize + screen_rows_rendered;

                // Move cursor to the start of this line within the window
                terminal.queue_move_cursor(Position::new(screen_row, window.x as usize))?;

                // Determine if this visual row corresponds to the cursor line
                let is_cursor_line =
                    self.show_cursor_line && is_active_window && buf_row == buffer.cursor.row;

                // Set background color for this line (cursor line background or normal background)
                if is_cursor_line {
                    terminal.queue_set_bg_color(self.theme.cursor_line_bg)?;
                } else {
                    terminal.queue_set_bg_color(self.theme.background)?;
                }

                // Clear the window area with the background color set
                let spaces = " ".repeat(window.width as usize);
                terminal.queue_print(&spaces)?;

                // Move back to the start of the window for actual content rendering
                terminal.queue_move_cursor(Position::new(screen_row, window.x as usize))?;

                if buf_row < buffer.lines.len() {
                    // Render gutter (numbers or marks) only for first visual row
                    if line_number_width > 0 {
                        self.render_line_number(
                            terminal,
                            buffer,
                            buf_row,
                            line_number_width,
                            is_cursor_line,
                            is_active_window,
                        )?;
                    }

                    // Move to text area within the window
                    terminal.queue_move_cursor(Position::new(screen_row, text_start_col))?;

                    // Render as many wrapped segments from this logical line as fit in remaining rows
                    let line = &buffer.lines[buf_row];
                    // For preview, compute a hanging indent so continuation rows align under content
                    let hanging_indent_cols =
                        if let Some(prev_id) = editor_state.markdown_preview_buffer_id {
                            if Some(prev_id) == window.buffer_id {
                                self.preview_hanging_indent_cols(line).min(text_width)
                            } else {
                                0
                            }
                        } else {
                            0
                        };
                    // Capture blockquote prefix string (one or more "▎ ") for possible re-render
                    let mut bqi = 0usize; // byte index after blockquote prefixes
                    while bqi < line.len() && line[bqi..].starts_with("▎ ") {
                        bqi += "▎ ".len();
                    }
                    let blockquote_prefix = &line[..bqi];
                    let line_body = &line[bqi..];
                    let quote_color = self
                        .syntax_theme
                        .tree_sitter_mappings
                        .get("comment")
                        .copied()
                        .unwrap_or_else(|| self.syntax_theme.get_default_text_color());
                    // Determine if only blockquote (no bullet / number marker) so we repeat prefix
                    let remainder = &line[bqi..];
                    let blockquote_only =
                        !(blockquote_prefix.is_empty() || remainder.starts_with("• ") || {
                            // ordered list detection: digits + ". "
                            let bytes = remainder.as_bytes();
                            let mut j = 0;
                            let mut any_digit = false;
                            while j < bytes.len() && bytes[j].is_ascii_digit() {
                                any_digit = true;
                                j += 1;
                            }
                            any_digit && remainder[j..].starts_with(". ")
                        });
                    let mut start = 0usize;
                    loop {
                        // Determine the end of this segment using UTF-8 safe wrapping
                        // Continuations reserve space for the hanging indent in preview
                        let avail_width = if start == 0 {
                            text_width
                        } else {
                            text_width.saturating_sub(hanging_indent_cols)
                        };
                        let (end, _seg_count) =
                            self.wrap_next_end_byte(line_body, start, avail_width, word_break);
                        let display_slice = &line_body[start..end];
                        // Measure visual columns of this slice for correct padding
                        let display_slice_cols = UnicodeWidthStr::width(display_slice);

                        // Compute base offset in character columns for selection math up to start
                        let base_offset_chars = line_body[..start].chars().count();

                        if let Some(highlights) =
                            editor_state.syntax_highlights.get(&(buffer.id, buf_row))
                        {
                            // Shift highlights for this slice
                            let shifted: Vec<HighlightRange> = highlights
                                .iter()
                                .map(|h| HighlightRange {
                                    start: h.start.saturating_sub(start),
                                    end: h.end.saturating_sub(start),
                                    style: h.style.clone(),
                                })
                                .collect();
                            let context = LineRenderContext {
                                line_number: buf_row,
                                is_cursor_line,
                                selection: buffer.get_selection(),
                                editor_mode: editor_state.mode,
                                base_offset: base_offset_chars,
                                total_line_chars: line.chars().count(),
                            };
                            // Print hanging indent for continuation rows (preview only)
                            // Always print prefix (first or continuation) for blockquote-only lines
                            if blockquote_only {
                                terminal.queue_reset_color()?;
                                terminal.queue_set_fg_color(quote_color)?;
                                self.set_background_color(terminal, is_cursor_line)?;
                                terminal.queue_print(blockquote_prefix)?;
                            } else if start > 0 && hanging_indent_cols > 0 {
                                // non-blockquote indent
                                terminal.queue_print(&" ".repeat(hanging_indent_cols))?;
                            }
                            let _rendered_cols = self.render_highlighted_line(
                                terminal,
                                display_slice,
                                &shifted,
                                &context,
                            )?;
                            let padded_width = text_width.saturating_sub(0); // prefix printed separately
                            if display_slice_cols < padded_width {
                                // Ensure filler uses the row background, not selection bg
                                if is_cursor_line && self.show_cursor_line {
                                    terminal.queue_set_bg_color(self.theme.cursor_line_bg)?;
                                } else {
                                    terminal.queue_set_bg_color(self.theme.background)?;
                                }
                                let filler = " ".repeat(padded_width - display_slice_cols);
                                terminal.queue_print(&filler)?;
                            }
                        } else {
                            // Print hanging indent for continuation rows (preview only)
                            if blockquote_only {
                                terminal.queue_reset_color()?;
                                terminal.queue_set_fg_color(quote_color)?;
                                self.set_background_color(terminal, is_cursor_line)?;
                                terminal.queue_print(blockquote_prefix)?;
                            } else if start > 0 && hanging_indent_cols > 0 {
                                terminal.queue_print(&" ".repeat(hanging_indent_cols))?;
                            }
                            self.render_plain_text_line(
                                terminal,
                                display_slice,
                                buf_row,
                                is_cursor_line,
                                buffer.get_selection(),
                                editor_state.mode,
                                base_offset_chars,
                                line.chars().count(),
                            )?;
                            let padded_width = text_width;
                            if display_slice_cols < padded_width {
                                // Ensure filler uses the row background, not selection bg
                                if is_cursor_line && self.show_cursor_line {
                                    terminal.queue_set_bg_color(self.theme.cursor_line_bg)?;
                                } else {
                                    terminal.queue_set_bg_color(self.theme.background)?;
                                }
                                let filler = " ".repeat(padded_width - display_slice_cols);
                                terminal.queue_print(&filler)?;
                            }
                        }

                        // Finished one visual row
                        screen_rows_rendered += 1;
                        if screen_rows_rendered >= content_height {
                            return Ok(());
                        }

                        // If end of line reached, advance to next buffer line
                        if end >= line_body.len() {
                            buf_row += 1;
                            break;
                        }

                        // Otherwise, continue with next wrapped segment on the next screen row
                        start = end; // continue within line_body

                        // Prepare next visual row: move to beginning of the next row area
                        let next_screen_row = window.y as usize + screen_rows_rendered;
                        terminal
                            .queue_move_cursor(Position::new(next_screen_row, window.x as usize))?;
                        // Clear row background (do not carry selection bg)
                        terminal.queue_set_bg_color(self.theme.background)?;
                        let spaces = " ".repeat(window.width as usize);
                        terminal.queue_print(&spaces)?;
                        terminal
                            .queue_move_cursor(Position::new(next_screen_row, window.x as usize))?;

                        // In wrapped continuation rows, gutter is blank
                        if line_number_width > 0 {
                            let blanks = " ".repeat(line_number_width);
                            terminal.queue_print(&blanks)?;
                        }

                        // Move to text start for the next segment
                        terminal
                            .queue_move_cursor(Position::new(next_screen_row, text_start_col))?;
                    }
                } else {
                    // Beyond buffer end: draw gutter then empty line indicator
                    if line_number_width > 0 {
                        self.render_line_number(
                            terminal,
                            buffer,
                            buf_row,
                            line_number_width,
                            is_cursor_line,
                            is_active_window,
                        )?;
                    }
                    terminal.queue_move_cursor(Position::new(screen_row, text_start_col))?;
                    terminal.queue_set_fg_color(self.theme.empty_line)?;
                    terminal.queue_print("~")?;
                    if text_width > 1 {
                        let filler = " ".repeat(text_width - 1);
                        terminal.queue_print(&filler)?;
                    }
                    screen_rows_rendered += 1;
                }

                // Reset colors after each line
                terminal.queue_reset_color()?;
            }

            return Ok(());
        }

        // No wrapping: existing single-row-per-line rendering with horizontal offset
        for row in 0..content_height {
            let buffer_row = window.viewport_top + row;
            let screen_row = window.y as usize + row;

            // Move cursor to the start of this line within the window
            terminal.queue_move_cursor(Position::new(screen_row, window.x as usize))?;

            // Check if this is the cursor line for highlighting (only in the active window)
            let is_active_window = editor_state.current_window_id == Some(window.id);
            let is_cursor_line =
                self.show_cursor_line && is_active_window && buffer_row == buffer.cursor.row;

            // Set background color for this line (cursor line background or normal background)
            if is_cursor_line {
                terminal.queue_set_bg_color(self.theme.cursor_line_bg)?;
            } else {
                terminal.queue_set_bg_color(self.theme.background)?;
            }

            // Clear the window area with the background color set
            let spaces = " ".repeat(window.width as usize);
            terminal.queue_print(&spaces)?;

            // Move back to the start of the window for actual content rendering
            terminal.queue_move_cursor(Position::new(screen_row, window.x as usize))?;

            if buffer_row < buffer.lines.len() {
                // Render gutter (numbers or marks) if enabled
                if line_number_width > 0 {
                    self.render_line_number(
                        terminal,
                        buffer,
                        buffer_row,
                        line_number_width,
                        is_cursor_line,
                        is_active_window,
                    )?;
                }

                // Move to text area within the window
                terminal.queue_move_cursor(Position::new(screen_row, text_start_col))?;

                let line = &buffer.lines[buffer_row];

                // Track how much content we've rendered for cursor line filling
                let content_rendered = if let Some(highlights) =
                    editor_state.syntax_highlights.get(&(buffer.id, buffer_row))
                {
                    // Debug: Show we have highlights
                    if log::log_enabled!(log::Level::Debug) && !highlights.is_empty() {
                        debug!(
                            "UI: Rendering line {} with {} highlights: '{}'",
                            buffer_row,
                            highlights.len(),
                            line.chars().take(30).collect::<String>()
                        );

                        // Show what's actually being highlighted (UTF-8 safe)
                        for (i, highlight) in highlights.iter().enumerate() {
                            let bytes = line.as_bytes();
                            let s = highlight.start.min(bytes.len());
                            let e = highlight.end.min(bytes.len());
                            let highlighted_text =
                                std::str::from_utf8(&bytes[s..e]).unwrap_or("<invalid-utf8-range>");
                            debug!(
                                "  Highlight {}: '{}' ({}-{}) color: {:?}",
                                i,
                                highlighted_text,
                                highlight.start,
                                highlight.end,
                                highlight.style.fg_color
                            );
                        }
                    }
                    // Compute base offsets safely for horizontal slicing (UTF-8 aware)
                    let base_offset_bytes = Self::floor_char_boundary(line, window.horiz_offset);
                    let base_offset_chars = window.horiz_offset;

                    let context = LineRenderContext {
                        line_number: buffer_row,
                        is_cursor_line,
                        selection: buffer.get_selection(),
                        editor_mode: editor_state.mode,
                        base_offset: base_offset_chars,
                        total_line_chars: line.chars().count(),
                    };
                    // Apply horizontal offset and clamp to available width in character columns
                    let start_byte = base_offset_bytes;
                    let mut chars_seen = 0usize;
                    let mut end_byte = start_byte;
                    if start_byte < line.len() {
                        for (b, ch) in line[start_byte..].char_indices() {
                            let next_end = start_byte + b + ch.len_utf8();
                            if chars_seen + 1 > text_width {
                                break;
                            }
                            chars_seen += 1;
                            end_byte = next_end;
                            if chars_seen == text_width {
                                break;
                            }
                        }
                    }
                    if end_byte < start_byte {
                        end_byte = start_byte;
                    }
                    let display_slice = &line[start_byte..end_byte];
                    // Shift highlight ranges to match the sliced view (byte-based); clamped in renderer
                    let shifted: Vec<HighlightRange> = highlights
                        .iter()
                        .map(|h| HighlightRange {
                            start: h.start.saturating_sub(start_byte),
                            end: h.end.saturating_sub(start_byte),
                            style: h.style.clone(),
                        })
                        .collect();
                    // Render the clamped slice
                    let _ =
                        self.render_highlighted_line(terminal, display_slice, &shifted, &context)?;
                    // We rendered exactly `chars_seen` columns
                    chars_seen
                } else {
                    // Debug: Show we're missing highlights
                    if log::log_enabled!(log::Level::Debug) {
                        debug!(
                            "UI: No highlights for line {} (buffer {}): '{}'",
                            buffer_row,
                            buffer.id,
                            line.chars().take(30).collect::<String>()
                        );
                    }
                    // Render line without syntax highlighting but with visual selection support
                    // Apply horizontal offset and clamp to available width (UTF-8 safe)
                    let start_byte = Self::floor_char_boundary(line, window.horiz_offset);
                    // Determine end byte that fits within text_width characters
                    let mut chars_seen = 0usize;
                    let mut end_byte = start_byte;
                    for (b, ch) in line[start_byte..].char_indices() {
                        let next_end = start_byte + b + ch.len_utf8();
                        if chars_seen + 1 > text_width {
                            break;
                        }
                        chars_seen += 1;
                        end_byte = next_end;
                        if chars_seen == text_width {
                            break;
                        }
                    }
                    if end_byte < start_byte {
                        end_byte = start_byte;
                    }
                    let display_line = &line[start_byte..end_byte];
                    // Compute base offset in character columns for selection math
                    let base_offset_chars = line[..start_byte].chars().count();
                    self.render_plain_text_line(
                        terminal,
                        display_line,
                        buffer_row,
                        is_cursor_line,
                        buffer.get_selection(),
                        editor_state.mode,
                        base_offset_chars,
                        line.chars().count(),
                    )?;
                    chars_seen
                };

                // Fill remaining width with appropriate background (never selection)
                if content_rendered < text_width {
                    let remaining_width = text_width - content_rendered;
                    if is_cursor_line && self.show_cursor_line {
                        terminal.queue_set_bg_color(self.theme.cursor_line_bg)?;
                    } else {
                        terminal.queue_set_bg_color(self.theme.background)?;
                    }
                    let filler = " ".repeat(remaining_width);
                    terminal.queue_print(&filler)?;
                }
            } else {
                // Empty line - render gutter if enabled
                if line_number_width > 0 {
                    self.render_line_number(
                        terminal,
                        buffer,
                        buffer_row,
                        line_number_width,
                        is_cursor_line,
                        is_active_window,
                    )?;
                }

                // Move to text area and show tilde for empty lines (like Vim)
                terminal.queue_move_cursor(Position::new(screen_row, text_start_col))?;
                if !is_cursor_line {
                    terminal.queue_set_fg_color(self.theme.empty_line)?;
                }
                terminal.queue_print("~")?;

                // Fill remaining width with appropriate background
                let remaining_width = text_width - 1; // -1 for the tilde character
                if remaining_width > 0 {
                    let filler = " ".repeat(remaining_width);
                    terminal.queue_print(&filler)?;
                }
            }

            // Reset colors after each line
            terminal.queue_reset_color()?;
        }

        Ok(())
    }

    fn render_window_border(
        &self,
        terminal: &mut Terminal,
        window: &crate::core::window::Window,
        is_active: bool,
        reserved_rows: u16,
    ) -> io::Result<()> {
        // Draw border around the window (simple ASCII border)
        let border_char = if is_active { '|' } else { '│' };

        // Right border
        if window.x + window.width < terminal.size().0 {
            for y in window.y..window.y + window.height {
                terminal.queue_move_cursor(Position::new(
                    y as usize,
                    (window.x + window.width) as usize,
                ))?;
                terminal.queue_print(&border_char.to_string())?;
            }
        }

        // Bottom border
        if window.y + window.height < terminal.size().1.saturating_sub(reserved_rows) {
            terminal.queue_move_cursor(Position::new(
                (window.y + window.height) as usize,
                window.x as usize,
            ))?;
            let border = "─".repeat(window.width as usize);
            terminal.queue_print(&border)?;
        }

        Ok(())
    }

    fn position_cursor(
        &self,
        terminal: &mut Terminal,
        editor_state: &EditorRenderState,
    ) -> io::Result<()> {
        // Set cursor shape based on current mode
        match editor_state.mode {
            Mode::Insert => {
                terminal.queue_cursor_line()?;
            }
            Mode::Replace => {
                terminal.queue_cursor_underline()?;
            }
            Mode::Normal
            | Mode::Visual
            | Mode::VisualLine
            | Mode::VisualBlock
            | Mode::Select
            | Mode::SelectLine
            | Mode::Command
            | Mode::Search
            | Mode::OperatorPending => {
                terminal.queue_cursor_block()?;
            }
        }

        if let Some(current_window) = editor_state.window_manager.current_window() {
            // Get the buffer for the current window
            let buffer = if let Some(buffer_id) = current_window.buffer_id {
                editor_state.all_buffers.get(&buffer_id)
            } else {
                return Ok(());
            };

            if let Some(buffer) = buffer {
                let content_height = current_window.content_height();

                // Calculate gutter width
                let line_number_width = self.compute_gutter_width(buffer.lines.len());

                let wrap_enabled = if let Some(prev_id) = editor_state.markdown_preview_buffer_id {
                    if Some(prev_id) == current_window.buffer_id {
                        editor_state.config.markdown_preview.wrap_lines
                    } else {
                        editor_state.config.behavior.wrap_lines
                    }
                } else {
                    editor_state.config.behavior.wrap_lines
                };
                // Use word-boundary wrapping for the markdown preview window to keep words intact
                let word_break = if let Some(prev_id) = editor_state.markdown_preview_buffer_id {
                    if Some(prev_id) == current_window.buffer_id {
                        true
                    } else {
                        editor_state.config.behavior.line_break
                    }
                } else {
                    editor_state.config.behavior.line_break
                };

                if wrap_enabled {
                    // Compute the visual row of the cursor by simulating wrapping from viewport_top
                    let text_width = current_window
                        .width
                        .saturating_sub(line_number_width as u16)
                        as usize;
                    if text_width == 0 {
                        return Ok(());
                    }

                    let mut visual_rows = 0usize;
                    // Count wrapped rows for all lines before the cursor line within the viewport
                    for row in current_window.viewport_top..buffer.cursor.row {
                        if row >= buffer.lines.len() {
                            visual_rows += 1; // empty visual line for beyond-EOF
                            continue;
                        }
                        let line = &buffer.lines[row];
                        if line.is_empty() {
                            visual_rows += 1;
                            continue;
                        }
                        let hanging_indent_cols =
                            if let Some(prev_id) = editor_state.markdown_preview_buffer_id {
                                if Some(prev_id) == current_window.buffer_id {
                                    self.preview_hanging_indent_cols(line).min(text_width)
                                } else {
                                    0
                                }
                            } else {
                                0
                            };
                        let mut start = 0usize;
                        loop {
                            let avail_width = if start == 0 {
                                text_width
                            } else {
                                text_width.saturating_sub(hanging_indent_cols)
                            };
                            let (end, _c) =
                                self.wrap_next_end_byte(line, start, avail_width, word_break);
                            visual_rows += 1;
                            if end >= line.len() {
                                break;
                            }
                            start = end;
                        }
                    }

                    // Determine which wrapped segment within the cursor line the cursor is in
                    let (segment_index, segment_start) = if buffer.cursor.row < buffer.lines.len() {
                        let line = &buffer.lines[buffer.cursor.row];
                        let hanging_indent_cols =
                            if let Some(prev_id) = editor_state.markdown_preview_buffer_id {
                                if Some(prev_id) == current_window.buffer_id {
                                    self.preview_hanging_indent_cols(line).min(text_width)
                                } else {
                                    0
                                }
                            } else {
                                0
                            };
                        let mut start = 0usize;
                        let mut seg_idx = 0usize;
                        loop {
                            let avail_width = if start == 0 {
                                text_width
                            } else {
                                text_width.saturating_sub(hanging_indent_cols)
                            };
                            let (end, _c) =
                                self.wrap_next_end_byte(line, start, avail_width, word_break);
                            // Treat a cursor at a segment boundary as belonging to the NEXT segment,
                            // except when this is the final segment (end-of-line).
                            if buffer.cursor.col < end {
                                break (seg_idx, start);
                            }
                            if end >= line.len() {
                                break (seg_idx, start);
                            }
                            start = end;
                            seg_idx += 1;
                        }
                    } else {
                        (0usize, 0usize)
                    };

                    let screen_row = visual_rows + segment_index;
                    if screen_row < content_height {
                        use unicode_width::UnicodeWidthStr;
                        let line = if buffer.cursor.row < buffer.lines.len() {
                            &buffer.lines[buffer.cursor.row]
                        } else {
                            ""
                        };
                        let start = segment_start.min(line.len());
                        let end = buffer.cursor.col.min(line.len());
                        let slice = if start < end { &line[start..end] } else { "" };
                        let within_segment_cols = UnicodeWidthStr::width(slice);
                        let final_row = current_window.y as usize + screen_row;
                        // Include hanging indent for continuation segments in preview
                        let hanging_indent_cols =
                            if let Some(prev_id) = editor_state.markdown_preview_buffer_id {
                                if Some(prev_id) == current_window.buffer_id && segment_start > 0 {
                                    self.preview_hanging_indent_cols(line).min(text_width)
                                } else {
                                    0
                                }
                            } else {
                                0
                            };
                        let final_col = current_window.x as usize
                            + line_number_width
                            + hanging_indent_cols
                            + within_segment_cols;
                        terminal.queue_move_cursor(Position::new(final_row, final_col))?;
                    }
                } else {
                    // Calculate screen cursor position relative to the current window
                    let screen_row = buffer
                        .cursor
                        .row
                        .saturating_sub(current_window.viewport_top);

                    // Compute visual column width from horiz_offset to cursor using Unicode width
                    let line = if buffer.cursor.row < buffer.lines.len() {
                        &buffer.lines[buffer.cursor.row]
                    } else {
                        ""
                    };
                    let start_byte = Self::floor_char_boundary(line, current_window.horiz_offset);
                    let cur_byte = buffer.cursor.col.min(line.len());
                    let cur_byte = if cur_byte <= line.len() {
                        cur_byte
                    } else {
                        line.len()
                    };
                    let cur_byte = if cur_byte >= start_byte {
                        cur_byte
                    } else {
                        start_byte
                    };
                    let slice = &line[start_byte..cur_byte];
                    let rel_cols = UnicodeWidthStr::width(slice);
                    let screen_col = line_number_width + rel_cols;

                    // Ensure cursor is within window bounds
                    if screen_row < content_height {
                        let final_row = current_window.y as usize + screen_row;
                        let final_col = current_window.x as usize + screen_col;
                        terminal.queue_move_cursor(Position::new(final_row, final_col))?;
                    }
                }
            }
        }
        Ok(())
    }

    fn render_highlighted_line(
        &self,
        terminal: &mut Terminal,
        line: &str,
        highlights: &[HighlightRange],
        context: &LineRenderContext,
    ) -> io::Result<usize> {
        let line_bytes = line.as_bytes();
        let mut current_pos = 0;

        // Use full slice length; caller already constrained by wrap/width.
        // Avoid mixing byte length with column width to prevent truncation.
        let display_len = line.len();

        // Determine if this line has visual selection and what range
        let line_selection_range = self.calculate_line_selection_range(
            context.line_number,
            context.selection,
            context.base_offset,
            context.total_line_chars,
        );

        for highlight in highlights {
            let start = std::cmp::min(highlight.start, display_len);
            let end = std::cmp::min(highlight.end, display_len);

            if start >= display_len {
                break;
            }

            // Print any text before this highlight
            if current_pos < start {
                self.render_text_segment(
                    terminal,
                    &line_bytes[current_pos..start],
                    current_pos,
                    context.is_cursor_line,
                    line_selection_range,
                    context.editor_mode,
                )?;
            }

            // Apply highlight style and print highlighted text
            self.render_highlighted_segment(
                terminal,
                &line_bytes[start..end],
                start,
                highlight,
                context.is_cursor_line,
                line_selection_range,
            )?;

            current_pos = end;
        }

        // Print any remaining text after the last highlight
        if current_pos < display_len {
            self.render_text_segment(
                terminal,
                &line_bytes[current_pos..display_len],
                current_pos,
                context.is_cursor_line,
                line_selection_range,
                context.editor_mode,
            )?;
        }

        Ok(display_len)
    }

    /// Helper method to render a text segment with proper visual selection highlighting
    fn render_text_segment(
        &self,
        terminal: &mut Terminal,
        text_bytes: &[u8],
        start_col: usize,
        is_cursor_line: bool,
        selection_range: Option<(usize, usize)>,
        editor_mode: crate::core::mode::Mode,
    ) -> io::Result<()> {
        let text = std::str::from_utf8(text_bytes).unwrap_or("");
        let char_count = text.chars().count();
        let end_col = start_col + char_count;

        if let Some((sel_start, sel_end)) = selection_range {
            // Check if this text segment overlaps with the selection
            if start_col < sel_end && end_col > sel_start {
                // There's an overlap, we need to split the segment
                let overlap_start = std::cmp::max(start_col, sel_start);
                let overlap_end = std::cmp::min(end_col, sel_end);

                // Convert to character indices for safe string slicing
                let text_chars: Vec<char> = text.chars().collect();

                // Render text before selection (if any)
                if start_col < overlap_start {
                    terminal.queue_set_fg_color(self.syntax_theme.get_default_text_color())?;
                    self.set_background_color(terminal, is_cursor_line)?;
                    let before_len = overlap_start - start_col;
                    let before_text: String = text_chars[0..before_len].iter().collect();
                    terminal.queue_print(&before_text)?;
                }

                // Render selected text
                terminal.queue_set_fg_color(self.syntax_theme.get_default_text_color())?;
                terminal.queue_set_bg_color(self.get_visual_selection_bg(editor_mode))?;
                let selected_start = overlap_start - start_col;
                let selected_end = overlap_end - start_col;
                let selected_text: String =
                    text_chars[selected_start..selected_end].iter().collect();
                terminal.queue_print(&selected_text)?;

                // Render text after selection (if any)
                if end_col > overlap_end {
                    terminal.queue_set_fg_color(self.syntax_theme.get_default_text_color())?;
                    self.set_background_color(terminal, is_cursor_line)?;
                    let after_start = overlap_end - start_col;
                    let after_text: String = text_chars[after_start..].iter().collect();
                    terminal.queue_print(&after_text)?;
                }
            } else {
                // No selection overlap, render normally
                terminal.queue_set_fg_color(self.syntax_theme.get_default_text_color())?;
                self.set_background_color(terminal, is_cursor_line)?;
                terminal.queue_print(text)?;
            }
        } else {
            // No selection, render normally
            terminal.queue_set_fg_color(self.syntax_theme.get_default_text_color())?;
            self.set_background_color(terminal, is_cursor_line)?;
            terminal.queue_print(text)?;
        }

        Ok(())
    }

    /// Helper method to render a highlighted segment with potential visual selection overlay
    fn render_highlighted_segment(
        &self,
        terminal: &mut Terminal,
        text_bytes: &[u8],
        start_col: usize,
        highlight: &HighlightRange,
        is_cursor_line: bool,
        selection_range: Option<(usize, usize)>,
    ) -> io::Result<()> {
        let text = std::str::from_utf8(text_bytes).unwrap_or("");
        let char_count = text.chars().count();
        let end_col = start_col + char_count;

        if let Some((sel_start, sel_end)) = selection_range {
            // Check if this highlighted segment overlaps with the selection
            if start_col < sel_end && end_col > sel_start {
                // Selection overrides syntax highlighting
                terminal.queue_set_fg_color(self.syntax_theme.get_default_text_color())?;
                terminal.queue_set_bg_color(self.theme.selection_bg)?;
                terminal.queue_print(text)?;
            } else {
                // No selection, use normal syntax highlighting
                if let Some(color) = highlight.style.to_color() {
                    terminal.queue_set_fg_color(color)?;
                }
                self.set_background_color(terminal, is_cursor_line)?;
                terminal.queue_print(text)?;
            }
        } else {
            // No selection, use normal syntax highlighting
            if let Some(color) = highlight.style.to_color() {
                terminal.queue_set_fg_color(color)?;
            }
            self.set_background_color(terminal, is_cursor_line)?;
            terminal.queue_print(text)?;
        }

        Ok(())
    }

    /// Helper method to set the background color appropriately
    fn set_background_color(
        &self,
        terminal: &mut Terminal,
        is_cursor_line: bool,
    ) -> io::Result<()> {
        if is_cursor_line && self.show_cursor_line {
            terminal.queue_set_bg_color(self.theme.cursor_line_bg)?;
        } else {
            terminal.queue_set_bg_color(self.theme.background)?;
        }
        Ok(())
    }

    /// Render a plain text line with visual selection support
    #[allow(clippy::too_many_arguments)]
    fn render_plain_text_line(
        &self,
        terminal: &mut Terminal,
        line: &str,
        line_number: usize,
        is_cursor_line: bool,
        selection: Option<crate::core::mode::Selection>,
        editor_mode: crate::core::mode::Mode,
        base_offset: usize,
        total_line_chars: usize,
    ) -> io::Result<()> {
        // Determine if this line has visual selection and what range
        let line_selection_range = self.calculate_line_selection_range(
            line_number,
            selection,
            base_offset,
            total_line_chars,
        );

        // Render the entire line with selection highlighting
        self.render_text_segment(
            terminal,
            line.as_bytes(),
            0,
            is_cursor_line,
            line_selection_range,
            editor_mode,
        )?;

        Ok(())
    }

    fn render_line_number(
        &self,
        terminal: &mut Terminal,
        buffer: &crate::core::buffer::Buffer,
        buffer_row: usize,
        width: usize,
        is_cursor_line: bool,
        is_active_window: bool,
    ) -> io::Result<()> {
        // Set line number colors using theme - highlight current line number if on cursor line
        if is_cursor_line && self.show_cursor_line {
            terminal.queue_set_fg_color(self.theme.line_number_current)?;
            terminal.queue_set_bg_color(self.theme.cursor_line_bg)?;
        } else {
            terminal.queue_set_fg_color(self.theme.line_number)?;
            terminal.queue_set_bg_color(self.theme.background)?;
        }

        if buffer_row < buffer.lines.len() {
            let numbers_enabled = self.show_line_numbers || self.show_relative_numbers;
            let line_num = if self.show_relative_numbers && is_active_window {
                // Only show relative numbers in the active window
                let current_line = buffer.cursor.row;
                if buffer_row == current_line {
                    // Show absolute line number for current line
                    buffer_row + 1
                } else {
                    // Show relative distance
                    buffer_row.abs_diff(current_line)
                }
            } else {
                // Show absolute line numbers (for inactive windows or when relative numbers are disabled)
                buffer_row + 1
            };

            // Build gutter text: numbers (right-aligned) or just padding when numbers are off
            let num_str = if numbers_enabled {
                format!("{:>width$}", line_num, width = width.saturating_sub(1))
            } else {
                " ".repeat(width.saturating_sub(1))
            };
            let trailing_space = " ";

            if self.show_marks {
                // Check if this line has any mark, and pick a deterministic one to display (smallest char)
                let mark_char = buffer
                    .marks
                    .iter()
                    .filter(|(_, pos)| pos.row == buffer_row)
                    .map(|(ch, _)| *ch)
                    .min();
                if let Some(mark_ch) = mark_char {
                    // Save current number fg to restore after painting the bullet
                    let saved_fg = if is_cursor_line && self.show_cursor_line {
                        self.theme.line_number_current
                    } else {
                        self.theme.line_number
                    };

                    // If there is left padding, replace the first padding space with a bullet.
                    let has_left_padding = num_str.as_bytes().first() == Some(&b' ');
                    if has_left_padding {
                        // Print mark letter in mark color, then the rest of the padded number, then trailing space
                        terminal.queue_set_fg_color(self.theme.mark_indicator)?;
                        let mut buf = [0u8; 4];
                        let s = mark_ch.encode_utf8(&mut buf);
                        terminal.queue_print(s)?;
                        terminal.queue_set_fg_color(saved_fg)?;
                        if num_str.len() > 1 {
                            terminal.queue_print(&num_str[1..])?;
                        }
                        terminal.queue_print(trailing_space)?;
                    } else {
                        // No padding (wide line numbers). Print number, then mark letter instead of trailing space.
                        terminal.queue_set_fg_color(saved_fg)?;
                        terminal.queue_print(&num_str)?;
                        terminal.queue_set_fg_color(self.theme.mark_indicator)?;
                        let mut buf = [0u8; 4];
                        let s = mark_ch.encode_utf8(&mut buf);
                        terminal.queue_print(s)?;
                    }
                    return Ok(());
                }
            }

            // Default: print gutter text and trailing space
            terminal.queue_print(&num_str)?;
            terminal.queue_print(trailing_space)?;
        } else {
            // Empty line - just print spaces
            let spaces = " ".repeat(width);
            terminal.queue_print(&spaces)?;
        }

        // Don't reset color here - let the caller handle it
        Ok(())
    }

    fn render_status_line(
        &self,
        terminal: &mut Terminal,
        editor_state: &EditorRenderState,
        width: u16,
        height: u16,
    ) -> io::Result<()> {
        let reserved_cmd = if editor_state.config.interface.show_command {
            1
        } else {
            0
        };
        let status_row = height.saturating_sub(reserved_cmd + 1);
        terminal.queue_move_cursor(Position::new(status_row as usize, 0))?;

        // No explicit clear: we overwrite the entire status line each frame.

        // Build layout parts for colorized output
        let layout = self.compute_status_line_layout(editor_state, width);

        // If buffer is modified, keep legacy behavior: use status_modified bg for whole bar
        let is_modified = editor_state
            .current_buffer
            .as_ref()
            .is_some_and(|b| b.modified);
        if is_modified {
            terminal.queue_set_bg_color(self.theme.status_modified)?;
            terminal.queue_set_fg_color(self.theme.status_fg)?;
            let mut s = String::with_capacity(width as usize);
            s.push_str(&layout.left);
            s.push_str(&" ".repeat(layout.left_gap));
            s.push_str(&layout.mid);
            s.push_str(&" ".repeat(layout.right_gap));
            s.push_str(&layout.right);
            if s.len() < width as usize {
                s.push_str(&" ".repeat((width as usize) - s.len()));
            }
            if s.len() > width as usize {
                s.truncate(width as usize);
            }
            terminal.queue_print(&s)?;
            terminal.queue_reset_color()?;
            return Ok(());
        }

        // Determine mode colors for the left-most mode token
        let (mode_fg, mode_bg) = match editor_state.mode {
            Mode::Insert => (
                self.theme.mode_colors.insert_fg,
                self.theme.mode_colors.insert_bg,
            ),
            Mode::Visual => (
                self.theme.mode_colors.visual_fg,
                self.theme.mode_colors.visual_bg,
            ),
            Mode::VisualLine => (
                self.theme.mode_colors.visual_line_fg,
                self.theme.mode_colors.visual_line_bg,
            ),
            Mode::VisualBlock => (
                self.theme.mode_colors.visual_block_fg,
                self.theme.mode_colors.visual_block_bg,
            ),
            Mode::Select => (
                self.theme.mode_colors.select_fg,
                self.theme.mode_colors.select_bg,
            ),
            Mode::SelectLine => (
                self.theme.mode_colors.select_line_fg,
                self.theme.mode_colors.select_line_bg,
            ),
            Mode::Replace => (
                self.theme.mode_colors.replace_fg,
                self.theme.mode_colors.replace_bg,
            ),
            Mode::Command => (
                self.theme.mode_colors.command_fg,
                self.theme.mode_colors.command_bg,
            ),
            _ => (
                self.theme.mode_colors.normal_fg,
                self.theme.mode_colors.normal_bg,
            ),
        };

        // Split left into mode token and rest (best-effort; if truncated, color all as mode)
        let mode_token = format!(" {} ", editor_state.mode);
        let (left_mode_part, left_rest_part) = if layout.left.starts_with(&mode_token) {
            (&mode_token[..], &layout.left[mode_token.len()..])
        } else {
            (layout.left.as_str(), "")
        };

        // 1) Left segment: print mode token with mode colors
        terminal.queue_set_bg_color(mode_bg)?;
        terminal.queue_set_fg_color(mode_fg)?;
        terminal.queue_print(left_mode_part)?;
        // then rest of left with left segment colors
        if !left_rest_part.is_empty() {
            terminal.queue_set_bg_color(self.theme.status_left_bg)?;
            terminal.queue_set_fg_color(self.theme.status_left_fg)?;
            terminal.queue_print(left_rest_part)?;
        }

        // 2) Left gap spaces (mid segment bg)
        if layout.left_gap > 0 {
            terminal.queue_set_bg_color(self.theme.status_mid_bg)?;
            terminal.queue_set_fg_color(self.theme.status_mid_fg)?;
            terminal.queue_print(&" ".repeat(layout.left_gap))?;
        }

        // 3) Middle text
        if !layout.mid.is_empty() {
            terminal.queue_set_bg_color(self.theme.status_mid_bg)?;
            terminal.queue_set_fg_color(self.theme.status_mid_fg)?;
            terminal.queue_print(&layout.mid)?;
        }

        // 4) Right gap spaces
        if layout.right_gap > 0 {
            terminal.queue_set_bg_color(self.theme.status_mid_bg)?;
            terminal.queue_set_fg_color(self.theme.status_mid_fg)?;
            terminal.queue_print(&" ".repeat(layout.right_gap))?;
        }

        // 5) Right text
        if !layout.right.is_empty() {
            terminal.queue_set_bg_color(self.theme.status_right_bg)?;
            terminal.queue_set_fg_color(self.theme.status_right_fg)?;
            terminal.queue_print(&layout.right)?;
        }

        terminal.queue_reset_color()?;

        Ok(())
    }

    fn render_command_line(
        &self,
        terminal: &mut Terminal,
        editor_state: &EditorRenderState,
        width: u16,
        height: u16,
    ) -> io::Result<()> {
        let command_row = height.saturating_sub(1);
        terminal.queue_move_cursor(Position::new(command_row as usize, 0))?;

        // Overwrite the entire command line (no explicit clear needed)
        terminal.queue_set_bg_color(self.theme.command_line_bg)?;
        terminal.queue_set_fg_color(self.theme.command_line_fg)?;

        let command_text = &editor_state.command_line;

        // Truncate if too long
        let display_text = if command_text.len() > width as usize {
            &command_text[..width as usize]
        } else {
            command_text
        };

        terminal.queue_print(display_text)?;
        terminal.queue_reset_color()?;

        Ok(())
    }

    fn render_completion_popup(
        &mut self,
        terminal: &mut Terminal,
        editor_state: &EditorRenderState,
        width: u16,
        height: u16,
    ) -> io::Result<()> {
        if !editor_state.command_completion.should_show() {
            return Ok(());
        }

        // Compute completion menu dimensions dynamically based on content
        const MIN_MENU_HEIGHT: usize = 3;
        let max_menu_height = editor_state
            .config
            .interface
            .completion_menu_height
            .max(MIN_MENU_HEIGHT as u16) as usize;
        let menu_height = editor_state
            .command_completion
            .matches
            .len()
            .min(max_menu_height)
            .min((height - 3) as usize); // Reserve space for status and command line

        if menu_height == 0 {
            return Ok(());
        }

        // Position the popup above the command line
        let popup_row = height.saturating_sub(2); // One line above command line
        let popup_col = 0; // Start at left edge

        // Get visible completion items
        let visible_items = editor_state.command_completion.visible_items(menu_height);
        let selected_index = editor_state
            .command_completion
            .visible_selected_index(menu_height);

        // Precompute aligned columns for visible items; include a description column for all items
        #[derive(Default, Clone)]
        struct RowColumns {
            key: String,
            alias: String,
            value: String, // for set: [value]
            desc: String,  // human-friendly description for all items
            is_columnar: bool,
        }

        // Helper: parse a set item into canonical, alias list, and current value
        let parse_set_item = |text: &str| -> (String, Vec<&'static str>, Option<String>) {
            // Extract key
            let mut key = if let Some(rest) = text.strip_prefix("setp ") {
                rest
            } else if let Some(rest) = text.strip_prefix("set ") {
                rest
            } else {
                text
            };
            if let Some((k, _)) = key.split_once('=') {
                key = k;
            }
            if let Some(k) = key.strip_suffix('?') {
                key = k;
            }
            if let Some(k) = key.strip_prefix("no") {
                key = k;
            }
            // Trim any trailing/leading whitespace from the key (positional forms end with a space)
            let key = key.trim();

            let canonical: String = match key {
                "nu" => "number".to_string(),
                "rnu" => "relativenumber".to_string(),
                "cul" => "cursorline".to_string(),
                "smk" => "showmarks".to_string(),
                "et" => "expandtab".to_string(),
                "ai" => "autoindent".to_string(),
                "ic" => "ignorecase".to_string(),
                "scs" => "smartcase".to_string(),
                "hls" => "hlsearch".to_string(),
                "is" => "incsearch".to_string(),
                "lbr" => "linebreak".to_string(),
                "udf" => "undofile".to_string(),
                "bk" => "backup".to_string(),
                "swf" => "swapfile".to_string(),
                "aw" => "autosave".to_string(),
                "ls" => "laststatus".to_string(),
                "sc" => "showcmd".to_string(),
                "ppr" => "percentpathroot".to_string(),
                "syn" => "syntax".to_string(),
                "ts" => "tabstop".to_string(),
                "ul" => "undolevels".to_string(),
                "so" => "scrolloff".to_string(),
                "siso" => "sidescrolloff".to_string(),
                "tm" => "timeoutlen".to_string(),
                "colo" => "colorscheme".to_string(),
                other => other.to_string(),
            };

            let aliases: &[&str] = match canonical.as_str() {
                "number" => &["nu"],
                "relativenumber" => &["rnu"],
                "cursorline" => &["cul"],
                "showmarks" => &["smk"],
                "tabstop" => &["ts"],
                "expandtab" => &["et"],
                "autoindent" => &["ai"],
                "ignorecase" => &["ic"],
                "smartcase" => &["scs"],
                "hlsearch" => &["hls"],
                "incsearch" => &["is"],
                "wrap" => &[],
                "linebreak" => &["lbr"],
                "undolevels" => &["ul"],
                "undofile" => &["udf"],
                "backup" => &["bk"],
                "swapfile" => &["swf"],
                "autosave" => &["aw"],
                "laststatus" => &["ls"],
                "showcmd" => &["sc"],
                "scrolloff" => &["so"],
                "sidescrolloff" => &["siso"],
                "timeoutlen" => &["tm"],
                "percentpathroot" => &["ppr"],
                "colorscheme" => &["colo"],
                "syntax" => &["syn"],
                "mdpreview.wrap" => &[],
                _ => &[],
            };

            // Current value from config
            let current = match canonical.as_str() {
                // Display
                "number" => Some(editor_state.config.display.show_line_numbers.to_string()),
                "relativenumber" => Some(
                    editor_state
                        .config
                        .display
                        .show_relative_numbers
                        .to_string(),
                ),
                "cursorline" => Some(editor_state.config.display.show_cursor_line.to_string()),
                "showmarks" => Some(editor_state.config.display.show_marks.to_string()),
                "colorscheme" => Some(editor_state.config.display.color_scheme.clone()),
                "syntax" => Some(editor_state.config.display.syntax_highlighting.to_string()),
                // Behavior
                "tabstop" => Some(editor_state.config.behavior.tab_width.to_string()),
                "expandtab" => Some(editor_state.config.behavior.expand_tabs.to_string()),
                "autoindent" => Some(editor_state.config.behavior.auto_indent.to_string()),
                "ignorecase" => Some(editor_state.config.behavior.ignore_case.to_string()),
                "smartcase" => Some(editor_state.config.behavior.smart_case.to_string()),
                "hlsearch" => Some(editor_state.config.behavior.highlight_search.to_string()),
                "incsearch" => Some(editor_state.config.behavior.incremental_search.to_string()),
                "wrap" => Some(editor_state.config.behavior.wrap_lines.to_string()),
                "mdpreview.wrap" => {
                    Some(editor_state.config.markdown_preview.wrap_lines.to_string())
                }
                "linebreak" => Some(editor_state.config.behavior.line_break.to_string()),
                // Editing
                "undolevels" => Some(editor_state.config.editing.undo_levels.to_string()),
                "undofile" => Some(editor_state.config.editing.persistent_undo.to_string()),
                "backup" => Some(editor_state.config.editing.backup.to_string()),
                "swapfile" => Some(editor_state.config.editing.swap_file.to_string()),
                "autosave" => Some(editor_state.config.editing.auto_save.to_string()),
                // Interface
                "laststatus" => Some(editor_state.config.interface.show_status_line.to_string()),
                "showcmd" => Some(editor_state.config.interface.show_command.to_string()),
                "scrolloff" => Some(editor_state.config.interface.scroll_off.to_string()),
                "sidescrolloff" => Some(editor_state.config.interface.side_scroll_off.to_string()),
                "timeoutlen" => Some(editor_state.config.interface.command_timeout.to_string()),
                "percentpathroot" => {
                    Some(editor_state.config.interface.percent_path_root.to_string())
                }
                _ => None,
            };

            (canonical, aliases.to_vec(), current)
        };

        // Helper: parse an Ex command into canonical name and aliases
        let parse_ex_item = |text: &str| -> (String, Vec<&'static str>) {
            let t = text.trim_end();
            match t {
                // File/quit family
                "write" | "w" => ("write".to_string(), vec!["w"]),
                "quit" | "q" => ("quit".to_string(), vec!["q"]),
                "quit!" | "q!" => ("quit!".to_string(), vec!["q!"]),
                "wq" | "x" => ("wq".to_string(), vec!["x"]),
                "edit" | "e" => ("edit".to_string(), vec!["e"]),
                // Buffer commands
                "buffer" | "b" => ("buffer".to_string(), vec!["b"]),
                "bnext" | "bn" => ("bnext".to_string(), vec!["bn"]),
                "bprevious" | "bp" | "bprev" => ("bprevious".to_string(), vec!["bp", "bprev"]),
                "bdelete" | "bd" => ("bdelete".to_string(), vec!["bd"]),
                "bdelete!" | "bd!" => ("bdelete!".to_string(), vec!["bd!"]),
                "buffers" | "ls" => ("buffers".to_string(), vec!["ls"]),
                // Window/split
                "split" | "sp" => ("split".to_string(), vec!["sp"]),
                "vsplit" | "vsp" => ("vsplit".to_string(), vec!["vsp"]),
                "close" => ("close".to_string(), vec![]),
                // Info/tools
                "registers" | "reg" => ("registers".to_string(), vec!["reg"]),
                other => (other.to_string(), vec![]),
            }
        };

        // Prepare rows and measure widths
        let mut rows: Vec<RowColumns> = Vec::with_capacity(visible_items.len());
        let mut key_w_max = 0usize;
        let mut alias_w_max = 0usize;
        let mut value_w_max = 0usize;
        let mut desc_w_max = 0usize;
        for item in visible_items {
            if item.category == "set" {
                let (canonical, aliases, current) = parse_set_item(&item.text);
                // Preserve negative form display if the suggestion itself is negative (e.g., 'set nowrap')
                let is_neg_form = if let Some(rest) = item
                    .text
                    .strip_prefix("setp ")
                    .or_else(|| item.text.strip_prefix("set "))
                {
                    rest.trim().starts_with("no")
                } else {
                    false
                };
                // Build alias display, and for booleans show the opposite form (toggle hint)
                let is_boolean_key = matches!(
                    canonical.as_str(),
                    "number"
                        | "relativenumber"
                        | "cursorline"
                        | "showmarks"
                        | "expandtab"
                        | "autoindent"
                        | "ignorecase"
                        | "smartcase"
                        | "hlsearch"
                        | "incsearch"
                        | "wrap"
                        | "mdpreview.wrap"
                        | "linebreak"
                        | "undofile"
                        | "backup"
                        | "swapfile"
                        | "autosave"
                        | "laststatus"
                        | "showcmd"
                        | "syntax"
                        | "percentpathroot"
                );
                let mut alias_parts: Vec<String> = if aliases.is_empty() {
                    Vec::new()
                } else {
                    aliases.iter().map(|s| s.to_string()).collect()
                };
                if is_boolean_key {
                    // Determine current boolean value directly from config
                    let cur_true = match canonical.as_str() {
                        "number" => editor_state.config.display.show_line_numbers,
                        "relativenumber" => editor_state.config.display.show_relative_numbers,
                        "cursorline" => editor_state.config.display.show_cursor_line,
                        "showmarks" => editor_state.config.display.show_marks,
                        "expandtab" => editor_state.config.behavior.expand_tabs,
                        "autoindent" => editor_state.config.behavior.auto_indent,
                        "ignorecase" => editor_state.config.behavior.ignore_case,
                        "smartcase" => editor_state.config.behavior.smart_case,
                        "hlsearch" => editor_state.config.behavior.highlight_search,
                        "incsearch" => editor_state.config.behavior.incremental_search,
                        "wrap" => editor_state.config.behavior.wrap_lines,
                        "mdpreview.wrap" => editor_state.config.markdown_preview.wrap_lines,
                        "linebreak" => editor_state.config.behavior.line_break,
                        "undofile" => editor_state.config.editing.persistent_undo,
                        "backup" => editor_state.config.editing.backup,
                        "swapfile" => editor_state.config.editing.swap_file,
                        "autosave" => editor_state.config.editing.auto_save,
                        "laststatus" => editor_state.config.interface.show_status_line,
                        "showcmd" => editor_state.config.interface.show_command,
                        "syntax" => editor_state.config.display.syntax_highlighting,
                        "percentpathroot" => editor_state.config.interface.percent_path_root,
                        _ => false,
                    };
                    // In the alias column, show the opposite of what we'll display as the key
                    if cur_true {
                        // key will display "no<key>", so alias hints the positive form
                        alias_parts.push(canonical.clone());
                    } else {
                        // key will display "<key>", so alias hints the negative form
                        alias_parts.push(format!("no{}", canonical));
                    }
                }
                let alias_str = alias_parts.join(", ");
                let value_str = current
                    .as_ref()
                    .map(|v| format!("[{}]", v))
                    .unwrap_or_default();
                // Display the action that would toggle the current state: if true, show negative, else positive
                let key_str = if is_boolean_key {
                    let cur_true = match canonical.as_str() {
                        "number" => editor_state.config.display.show_line_numbers,
                        "relativenumber" => editor_state.config.display.show_relative_numbers,
                        "cursorline" => editor_state.config.display.show_cursor_line,
                        "showmarks" => editor_state.config.display.show_marks,
                        "expandtab" => editor_state.config.behavior.expand_tabs,
                        "autoindent" => editor_state.config.behavior.auto_indent,
                        "ignorecase" => editor_state.config.behavior.ignore_case,
                        "smartcase" => editor_state.config.behavior.smart_case,
                        "hlsearch" => editor_state.config.behavior.highlight_search,
                        "incsearch" => editor_state.config.behavior.incremental_search,
                        "wrap" => editor_state.config.behavior.wrap_lines,
                        "mdpreview.wrap" => editor_state.config.markdown_preview.wrap_lines,
                        "linebreak" => editor_state.config.behavior.line_break,
                        "undofile" => editor_state.config.editing.persistent_undo,
                        "backup" => editor_state.config.editing.backup,
                        "swapfile" => editor_state.config.editing.swap_file,
                        "autosave" => editor_state.config.editing.auto_save,
                        "laststatus" => editor_state.config.interface.show_status_line,
                        "showcmd" => editor_state.config.interface.show_command,
                        "syntax" => editor_state.config.display.syntax_highlighting,
                        "percentpathroot" => editor_state.config.interface.percent_path_root,
                        _ => false,
                    };
                    if cur_true {
                        format!("no{}", canonical)
                    } else {
                        canonical.clone()
                    }
                } else if is_neg_form {
                    format!("no{}", canonical)
                } else {
                    canonical.clone()
                };
                // Choose description matching the action shown in key column for booleans
                let desc_str = if is_boolean_key {
                    let (pos_desc, neg_desc) = match canonical.as_str() {
                        // Display
                        "number" => ("Show line numbers", "Hide line numbers"),
                        "relativenumber" => {
                            ("Show relative line numbers", "Hide relative line numbers")
                        }
                        "cursorline" => ("Highlight cursor line", "Disable cursor line highlight"),
                        "showmarks" => (
                            "Show marks in gutter/number column",
                            "Hide marks in gutter/number column",
                        ),
                        // Behavior
                        "expandtab" => ("Insert spaces for tabs", "Use hard tab characters"),
                        "autoindent" => (
                            "Enable automatic indentation",
                            "Disable automatic indentation",
                        ),
                        "ignorecase" => ("Case-insensitive search", "Case-sensitive search"),
                        "smartcase" => ("Smart case matching", "Disable smart case"),
                        "hlsearch" => ("Highlight search results", "Disable search highlighting"),
                        "incsearch" => ("Incremental search", "Disable incremental search"),
                        "wrap" => ("Enable soft line wrapping", "Disable soft line wrapping"),
                        "linebreak" => (
                            "Prefer breaking at word boundaries when wrapping",
                            "Disable word-boundary preference",
                        ),
                        // Editing
                        "undofile" => ("Enable persistent undo", "Disable persistent undo"),
                        "backup" => ("Enable backup files", "Disable backup files"),
                        "swapfile" => ("Enable swap file", "Disable swap file"),
                        "autosave" => ("Enable auto save", "Disable auto save"),
                        // Interface
                        "laststatus" => ("Show status line", "Hide status line"),
                        "showcmd" => ("Show command in status area", "Hide command in status area"),
                        // Syntax/UI
                        "syntax" => ("Enable syntax highlighting", "Disable syntax highlighting"),
                        // Path behavior
                        "percentpathroot" => (
                            "Enable '%' root in path completion",
                            "Disable '%' root in path completion",
                        ),
                        _ => (item.description.as_str(), item.description.as_str()),
                    };
                    let cur_true = match canonical.as_str() {
                        "number" => editor_state.config.display.show_line_numbers,
                        "relativenumber" => editor_state.config.display.show_relative_numbers,
                        "cursorline" => editor_state.config.display.show_cursor_line,
                        "showmarks" => editor_state.config.display.show_marks,
                        "expandtab" => editor_state.config.behavior.expand_tabs,
                        "autoindent" => editor_state.config.behavior.auto_indent,
                        "ignorecase" => editor_state.config.behavior.ignore_case,
                        "smartcase" => editor_state.config.behavior.smart_case,
                        "hlsearch" => editor_state.config.behavior.highlight_search,
                        "incsearch" => editor_state.config.behavior.incremental_search,
                        "wrap" => editor_state.config.behavior.wrap_lines,
                        "linebreak" => editor_state.config.behavior.line_break,
                        "undofile" => editor_state.config.editing.persistent_undo,
                        "backup" => editor_state.config.editing.backup,
                        "swapfile" => editor_state.config.editing.swap_file,
                        "autosave" => editor_state.config.editing.auto_save,
                        "laststatus" => editor_state.config.interface.show_status_line,
                        "showcmd" => editor_state.config.interface.show_command,
                        "syntax" => editor_state.config.display.syntax_highlighting,
                        "percentpathroot" => editor_state.config.interface.percent_path_root,
                        _ => false,
                    };
                    if cur_true {
                        neg_desc.to_string()
                    } else {
                        pos_desc.to_string()
                    }
                } else {
                    item.description.clone()
                };
                let key_w = UnicodeWidthStr::width(key_str.as_str());
                let alias_w = UnicodeWidthStr::width(alias_str.as_str());
                key_w_max = key_w_max.max(key_w);
                alias_w_max = alias_w_max.max(alias_w);
                let row = RowColumns {
                    key: key_str,
                    alias: alias_str,
                    value: value_str,
                    desc: desc_str,
                    is_columnar: true,
                };
                value_w_max = value_w_max.max(UnicodeWidthStr::width(row.value.as_str()));
                desc_w_max = desc_w_max.max(UnicodeWidthStr::width(row.desc.as_str()));
                rows.push(row);
            } else if !item.text.contains(' ') {
                // Ex command without arguments: show canonical, aliases, description
                let (canonical, aliases) = parse_ex_item(&item.text);
                let alias_str = if aliases.is_empty() {
                    String::new()
                } else {
                    aliases.join(", ")
                };
                let value_str = String::new();
                let key_str = canonical;
                let key_w = UnicodeWidthStr::width(key_str.as_str());
                let alias_w = UnicodeWidthStr::width(alias_str.as_str());
                key_w_max = key_w_max.max(key_w);
                alias_w_max = alias_w_max.max(alias_w);
                let row = RowColumns {
                    key: key_str,
                    alias: alias_str,
                    value: value_str,
                    desc: item.description.clone(),
                    is_columnar: true,
                };
                value_w_max = value_w_max.max(UnicodeWidthStr::width(row.value.as_str()));
                desc_w_max = desc_w_max.max(UnicodeWidthStr::width(row.desc.as_str()));
                rows.push(row);
            } else {
                // Items with arguments (paths, buffer refs, etc.) also include description
                let row = RowColumns {
                    key: item.text.clone(),
                    alias: String::new(),
                    value: String::new(),
                    desc: item.description.clone(),
                    is_columnar: true,
                };
                value_w_max = value_w_max.max(UnicodeWidthStr::width(row.value.as_str()));
                desc_w_max = desc_w_max.max(UnicodeWidthStr::width(row.desc.as_str()));
                rows.push(row);
            }
        }

        // Spacing between columns
        let gap = 2usize;
        // Choose column width caps to keep desc readable and columns aligned
        const KEY_CAP: usize = 24;
        const ALIAS_CAP: usize = 16;
        const VALUE_CAP: usize = 12; // [true], [1000], etc.
        const MIN_DESC: usize = 16;

        let mut key_w_used = key_w_max.min(KEY_CAP);
        let mut alias_w_used = alias_w_max.min(ALIAS_CAP);
        let value_w_used = value_w_max.min(VALUE_CAP);

        // Compute menu width based on used widths and desired desc width
        let term_cap = (width.saturating_sub(2)) as usize; // keep a small margin
        let mut menu_width =
            1 + key_w_used + gap + alias_w_used + gap + value_w_used + gap + desc_w_max;
        if menu_width > term_cap {
            menu_width = term_cap;
        }

        // Assign descriptor width from remaining space
        let fixed = 1 + key_w_used + gap + alias_w_used + gap + value_w_used;
        let mut desc_w_used = menu_width.saturating_sub(fixed + gap);
        // Ensure a minimum description width by trimming alias, then key if needed
        if desc_w_used < MIN_DESC && fixed + gap + MIN_DESC <= term_cap {
            let needed = MIN_DESC - desc_w_used;
            // First take from alias
            let take_alias = needed.min(alias_w_used);
            alias_w_used -= take_alias;
            let fixed_now = 1 + key_w_used + gap + alias_w_used + gap + value_w_used;
            desc_w_used = menu_width.saturating_sub(fixed_now + gap);
            if desc_w_used < MIN_DESC {
                let needed2 = MIN_DESC - desc_w_used;
                let take_key = needed2.min(key_w_used);
                key_w_used -= take_key;
                let fixed_now2 = 1 + key_w_used + gap + alias_w_used + gap + value_w_used;
                desc_w_used = menu_width.saturating_sub(fixed_now2 + gap);
            }
        }
        // If there's effectively no desc, drop the gap
        let include_desc = desc_w_used > 0;

        // Helper to truncate a string to a maximum display width (columns)
        let truncate_to_width = |s: &str, max_cols: usize| -> String {
            if max_cols == 0 {
                return String::new();
            }
            let mut cols = 0usize;
            let mut out = String::new();
            for g in UnicodeSegmentation::graphemes(s, true) {
                let w = UnicodeWidthStr::width(g);
                if cols + w > max_cols {
                    break;
                }
                out.push_str(g);
                cols += w;
            }
            out
        };

        // Compute content hash for redraw minimization (rows already contain derived display content)
        let mut hasher = DefaultHasher::new();
        for r in &rows {
            r.key.hash(&mut hasher);
            r.alias.hash(&mut hasher);
            r.value.hash(&mut hasher);
            r.desc.hash(&mut hasher);
            r.is_columnar.hash(&mut hasher);
        }
        selected_index.hash(&mut hasher);
        key_w_used.hash(&mut hasher);
        alias_w_used.hash(&mut hasher);
        value_w_used.hash(&mut hasher);
        desc_w_used.hash(&mut hasher);
        let content_hash = hasher.finish();

        // Early exit if geometry and content unchanged
        if let Some(cache) = &self.popup_cache {
            let same_pos = cache.row == popup_row && cache.col == popup_col;
            let same_size =
                cache.width as usize == menu_width && cache.height as usize == menu_height;
            let same_hash = cache.hash == content_hash;
            if same_pos && same_size && same_hash {
                return Ok(());
            }
        }

        // Render the popup background and rows
        for i in 0..menu_height {
            let row = popup_row.saturating_sub(menu_height as u16) + i as u16;
            terminal.queue_move_cursor(Position::new(row as usize, popup_col as usize))?;

            if i < rows.len() {
                let cols = &rows[i];
                let is_selected = i == selected_index;

                // Set colors based on selection
                if is_selected {
                    terminal.queue_set_bg_color(self.theme.completion_selected_bg)?;
                } else {
                    terminal.queue_set_bg_color(self.theme.completion_menu_bg)?;
                }
                if cols.is_columnar {
                    // Compose aligned columns with colors
                    // Leading space + key
                    let key_text = truncate_to_width(&cols.key, key_w_used);
                    let alias_text = truncate_to_width(&cols.alias, alias_w_used);
                    let value_text = truncate_to_width(&cols.value, value_w_used);
                    let desc_text = truncate_to_width(&cols.desc, desc_w_used);

                    // 1) leading space
                    terminal.queue_set_fg_color(self.theme.completion_key_fg)?;
                    terminal.queue_print(" ")?;
                    // 2) key padded
                    let key_padded = format!("{:<w$}", key_text, w = key_w_used);
                    terminal.queue_print(&key_padded)?;
                    // 3) gap
                    terminal.queue_print(&" ".repeat(gap))?;
                    // 4) alias padded
                    terminal.queue_set_fg_color(self.theme.completion_alias_fg)?;
                    let alias_padded = format!("{:<w$}", alias_text, w = alias_w_used);
                    terminal.queue_print(&alias_padded)?;
                    // 5) gap
                    terminal.queue_print(&" ".repeat(gap))?;
                    // 6) value (fixed width)
                    terminal.queue_set_fg_color(self.theme.completion_value_fg)?;
                    let value_padded = format!("{:<w$}", value_text, w = value_w_used);
                    terminal.queue_print(&value_padded)?;

                    // 7) optional gap and description (fixed start column)
                    let mut printed_width =
                        1 + key_w_used + gap + alias_w_used + gap + value_w_used;
                    if include_desc {
                        terminal.queue_print(&" ".repeat(gap))?;
                        terminal.queue_set_fg_color(self.theme.completion_desc_fg)?;
                        terminal.queue_print(&desc_text)?;
                        printed_width += gap + UnicodeWidthStr::width(desc_text.as_str());
                    }

                    // Pad any remaining to full width
                    if printed_width < menu_width {
                        let pad = menu_width - printed_width;
                        terminal.queue_print(&" ".repeat(pad))?;
                    }
                } else {
                    // Fallback: clear row (should not be hit in normal flow)
                    terminal.queue_set_bg_color(self.theme.command_line_bg)?;
                    terminal.queue_set_fg_color(self.theme.command_line_fg)?;
                    let empty_line = " ".repeat(menu_width);
                    terminal.queue_print(&empty_line)?;
                }

                // Reset colors immediately after printing each line
                terminal.queue_reset_color()?;
            } else {
                // Empty row - set background color and fill with spaces
                terminal.queue_set_bg_color(self.theme.command_line_bg)?;
                terminal.queue_set_fg_color(self.theme.command_line_fg)?;
                let empty_line = " ".repeat(menu_width);
                terminal.queue_print(&empty_line)?;

                // Reset colors immediately after printing each line
                terminal.queue_reset_color()?;
            }
        }

        // Update popup cache
        self.popup_cache = Some(PopupCache {
            row: popup_row,
            col: popup_col,
            width: menu_width as u16,
            height: menu_height as u16,
            hash: content_hash,
        });

        // Final color reset to ensure no artifacts
        terminal.queue_reset_color()?;

        Ok(())
    }

    /// Get the current viewport top position
    pub fn viewport_top(&self) -> usize {
        self.viewport_top
    }

    /// Get the current viewport range
    pub fn viewport_range(&self, height: u16) -> (usize, usize) {
        // Try to infer reserved rows from theme-agnostic UI flags
        // Fallback to 2 if we can't access config here; Renderer calls pass height consistently.
        // Since UI::renderer has access to EditorRenderState in render paths, content rendering
        // already respects borders; this method is used for cursor/selection math.
        let default_reserved = 2u16; // conservative fallback
        let content_height = height.saturating_sub(default_reserved) as usize;
        (self.viewport_top, content_height)
    }

    pub fn set_viewport_top(&mut self, viewport_top: usize) {
        self.viewport_top = viewport_top;
    }
}

// Note on test location:
// These unit tests are colocated in renderer.rs on purpose. They exercise the
// private helper `preview_hanging_indent_cols` used by the Markdown preview
// wrapping logic. Keeping them here allows direct access to the private
// function without changing its visibility or introducing a public seam just
// for testing. End-to-end preview behavior remains covered by integration
// tests under `tests/`.
#[cfg(test)]
mod preview_hanging_indent_tests {
    use super::UI;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn plain_text_has_no_hanging_indent() {
        let ui = UI::new();
        assert_eq!(ui.preview_hanging_indent_cols("hello world"), 0);
        assert_eq!(ui.preview_hanging_indent_cols(""), 0);
    }

    #[test]
    fn blockquote_only_indents_and_accumulates_with_list() {
        let ui = UI::new();
        let q = "▎ ";
        let w_q = UnicodeWidthStr::width(q);
        // Quote prefix alone now triggers hanging indent equal to its width so continuation
        // rows re-render the prefix glyph(s).
        assert_eq!(ui.preview_hanging_indent_cols("▎ hello"), w_q);
        // Multiple nested blockquotes accumulate width
        assert_eq!(ui.preview_hanging_indent_cols("▎ ▎ nested"), w_q * 2);
        // Blockquote followed by bullet accumulates both widths
        let w_bullet = UnicodeWidthStr::width("• ");
        assert_eq!(ui.preview_hanging_indent_cols("▎ • item"), w_q + w_bullet);
    }

    #[test]
    fn unordered_list_bullet_contributes_width() {
        let ui = UI::new();
        let w = UnicodeWidthStr::width("• ");
        assert_eq!(ui.preview_hanging_indent_cols("• item"), w);
        // Bullet must be at start (after optional quotes); mid-line bullet is ignored
        assert_eq!(ui.preview_hanging_indent_cols("x • item"), 0);
    }

    #[test]
    fn ordered_list_marker_counts_digits_and_dot_space() {
        let ui = UI::new();
        let dot_space = UnicodeWidthStr::width(". ");
        // Single digit: 1 digit + ". "
        assert_eq!(ui.preview_hanging_indent_cols("9. item"), 1 + dot_space);
        // Multiple digits
        assert_eq!(ui.preview_hanging_indent_cols("10. item"), 2 + dot_space);
        assert_eq!(ui.preview_hanging_indent_cols("123. item"), 3 + dot_space);

        // Requires exact ". " after digits; otherwise no indent
        assert_eq!(ui.preview_hanging_indent_cols("1.x item"), 0);
        assert_eq!(ui.preview_hanging_indent_cols("1 item"), 0);
    }

    #[test]
    fn mixed_blockquote_and_ordered_list_accumulate() {
        let ui = UI::new();
        let w_q = UnicodeWidthStr::width("▎ ");
        let dot_space = UnicodeWidthStr::width(". ");
        // ▎ 10. item -> quote width + 2 digits + ". "
        assert_eq!(
            ui.preview_hanging_indent_cols("▎ 10. item"),
            w_q + 2 + dot_space
        );
        // ▎ ▎ 3. item -> two quotes + 1 digit + ". "
        assert_eq!(
            ui.preview_hanging_indent_cols("▎ ▎ 3. item"),
            (w_q * 2) + 1 + dot_space
        );
    }

    #[test]
    fn unrelated_prefixes_do_not_trigger_indent() {
        let ui = UI::new();
        assert_eq!(ui.preview_hanging_indent_cols("> not our quote style"), 0);
        assert_eq!(ui.preview_hanging_indent_cols("- item"), 0);
        assert_eq!(ui.preview_hanging_indent_cols("•item (no space)"), 0);
        // Embedded symbols not at start are ignored
        assert_eq!(ui.preview_hanging_indent_cols("text ▎ quoted later"), 0);
    }
}

#[derive(Debug, Clone)]
struct StatusLineLayout {
    left: String,
    mid: String,
    right: String,
    left_gap: usize,
    right_gap: usize,
}
