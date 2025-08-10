use crate::config::EditorConfig;
use crate::config::theme::ThemeConfig;
use crate::config::watcher::ConfigWatcher;
use crate::core::buffer::Buffer;
use crate::core::mode::Mode;
use crate::core::window::{SplitDirection, WindowManager};
use crate::features::completion::CommandCompletion;
use crate::features::search::{SearchEngine, SearchResult};
use crate::features::syntax::{AsyncSyntaxHighlighter, HighlightRange, Priority};
use crate::input::keymap::KeyHandler;
use crate::ui::UI;
use crate::ui::terminal::Terminal;
use anyhow::Result;
use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Represents an operator waiting for a text object or motion
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingOperator {
    Delete,     // d
    Change,     // c
    Yank,       // y
    Indent,     // >
    Unindent,   // <
    ToggleCase, // ~
}

impl PendingOperator {
    /// Get the character representation of this operator
    pub fn to_char(&self) -> char {
        match self {
            PendingOperator::Delete => 'd',
            PendingOperator::Change => 'c',
            PendingOperator::Yank => 'y',
            PendingOperator::Indent => '>',
            PendingOperator::Unindent => '<',
            PendingOperator::ToggleCase => '~',
        }
    }

    /// Parse operator from character
    pub fn from_char(ch: char) -> Option<Self> {
        match ch {
            'd' => Some(PendingOperator::Delete),
            'c' => Some(PendingOperator::Change),
            'y' => Some(PendingOperator::Yank),
            '>' => Some(PendingOperator::Indent),
            '<' => Some(PendingOperator::Unindent),
            '~' => Some(PendingOperator::ToggleCase),
            _ => None,
        }
    }
}

// Struct to hold editor state for rendering without borrowing issues
pub struct EditorRenderState {
    pub mode: Mode,
    pub current_buffer: Option<Buffer>,
    pub all_buffers: HashMap<usize, Buffer>,
    pub command_line: String,
    pub status_message: String,
    pub buffer_count: usize,
    pub current_buffer_id: Option<usize>,
    pub current_window_id: Option<usize>,
    pub window_manager: WindowManager,
    pub syntax_highlights: HashMap<(usize, usize), Vec<HighlightRange>>, // (buffer_id, line_index) -> highlights
    pub command_completion: CommandCompletion,
    pub config: EditorConfig,
    // --- Status line extras ---
    pub filetype: Option<String>,
    pub macro_recording: Option<char>,
    pub search_total: usize,
    pub search_index: Option<usize>,
}

pub struct Editor {
    /// All open buffers
    buffers: HashMap<usize, Buffer>,
    /// Currently active buffer ID
    pub current_buffer_id: Option<usize>,
    /// Next buffer ID to assign
    next_buffer_id: usize,
    /// Window management for splits
    pub window_manager: WindowManager,
    /// Current editor mode
    mode: Mode,
    /// Terminal interface
    terminal: Terminal,
    /// UI renderer
    ui: UI,
    /// Key handler for mode-specific input
    key_handler: KeyHandler,
    /// Editor configuration
    config: EditorConfig,
    /// Search engine for text search
    search_engine: SearchEngine,
    /// Current search results
    search_results: Vec<SearchResult>,
    /// Current search result index
    current_search_index: Option<usize>,
    /// Whether the editor should quit
    should_quit: bool,
    /// Command line content (for command mode)
    command_line: String,
    /// Status message
    status_message: String,
    /// Configuration file watcher for hot reloading
    pub config_watcher: Option<ConfigWatcher>,
    /// Theme configuration for hot reloading themes
    theme_config: ThemeConfig,
    /// Async syntax highlighter for background code highlighting
    async_syntax_highlighter: Option<AsyncSyntaxHighlighter>,
    /// Flag to trigger re-render when async syntax highlighting completes
    pub needs_syntax_refresh: Arc<AtomicBool>,
    /// Command completion system
    command_completion: CommandCompletion,
    /// Current pending operator (for operator + text object combinations)
    pending_operator: Option<PendingOperator>,
    /// Text object finder for parsing text objects
    text_object_finder: crate::features::text_objects::TextObjectFinder,
    /// Macro recording and playback system
    macro_recorder: crate::features::macros::MacroRecorder,
}

impl Editor {
    pub fn new() -> Result<Self> {
        info!("Initializing editor");

        let mut terminal = Terminal::new()?;

        // Set initial cursor shape for normal mode
        if let Err(e) = terminal.set_cursor_block() {
            warn!("Failed to set initial cursor shape: {}", e);
        }

        let config = EditorConfig::load();
        debug!(
            "Editor configuration loaded: color_scheme={}, line_numbers={}",
            config.display.color_scheme, config.display.show_line_numbers
        );

        // Get terminal size for window manager
        let (terminal_width, terminal_height) = terminal.size();
        debug!("Terminal size: {}x{}", terminal_width, terminal_height);

        // Initialize UI with config values
        let mut ui = UI::new();
        ui.show_line_numbers = config.display.show_line_numbers;
        ui.show_relative_numbers = config.display.show_relative_numbers;
        ui.show_cursor_line = config.display.show_cursor_line;
        ui.set_theme(&config.display.color_scheme);

        let key_handler = KeyHandler::new();

        // Initialize window manager
        let window_manager = WindowManager::new(terminal_width, terminal_height);

        // Initialize config watcher for hot reloading
        let config_watcher = match ConfigWatcher::new() {
            Ok(watcher) => {
                debug!("Configuration file watcher initialized");
                Some(watcher)
            }
            Err(e) => {
                warn!("Failed to initialize config watcher: {}", e);
                None
            }
        };

        // Initialize theme configuration for hot reloading themes using editor's color scheme
        let theme_config = ThemeConfig::load_with_default_theme(&config.display.color_scheme);

        // Initialize async syntax highlighter
        let async_syntax_highlighter = match AsyncSyntaxHighlighter::new() {
            Ok(highlighter) => {
                debug!("Async syntax highlighter initialized");
                Some(highlighter)
            }
            Err(e) => {
                warn!("Failed to initialize async syntax highlighter: {}", e);
                None
            }
        };

        info!("Editor initialized");
        let mut editor = Self {
            buffers: HashMap::new(),
            current_buffer_id: None,
            next_buffer_id: 1,
            window_manager,
            mode: Mode::Normal,
            terminal,
            ui,
            key_handler,
            config,
            search_engine: SearchEngine::new(),
            search_results: Vec::new(),
            current_search_index: None,
            should_quit: false,
            command_line: String::new(),
            status_message: String::new(),
            config_watcher,
            theme_config,
            async_syntax_highlighter,
            needs_syntax_refresh: Arc::new(AtomicBool::new(false)),
            command_completion: CommandCompletion::new(),
            pending_operator: None,
            text_object_finder: crate::features::text_objects::TextObjectFinder::new(),
            macro_recorder: crate::features::macros::MacroRecorder::new(),
        };
        // Initialize reserved rows for status/command lines based on config
        let reserved_rows = editor.reserved_rows_from_config();
        editor.window_manager.set_reserved_rows(reserved_rows);
        // Apply initial search settings from config
        editor.apply_search_settings();
        Ok(editor)
    }

    /// Get command timeout in milliseconds from configuration
    pub fn command_timeout_ms(&self) -> u64 {
        self.config.interface.command_timeout
    }

    fn reserved_rows_from_config(&self) -> u16 {
        let mut rows = 0u16;
        if self.config.interface.show_status_line {
            rows += 1;
        }
        if self.config.interface.show_command {
            rows += 1;
        }
        rows
    }

    pub fn create_buffer(&mut self, file_path: Option<PathBuf>) -> Result<usize> {
        let id = self.next_buffer_id;
        self.next_buffer_id += 1;

        let buffer = if let Some(path) = file_path {
            debug!("Creating buffer {} from file: {:?}", id, path);
            Buffer::from_file(id, path, self.config.editing.undo_levels)?
        } else {
            debug!("Creating empty buffer {}", id);
            Buffer::new(id, self.config.editing.undo_levels)
        };

        self.buffers.insert(id, buffer);
        self.current_buffer_id = Some(id);
        debug!("Buffer {} created and set as current", id);

        // Assign buffer to current window
        if let Some(current_window) = self.window_manager.current_window_mut() {
            current_window.set_buffer(id);
            // Initialize window cursor position from buffer's cursor position
            if let Some(buffer) = self.buffers.get(&id) {
                current_window.save_cursor_position(buffer.cursor.row, buffer.cursor.col);
                trace!(
                    "Window cursor position initialized: row={}, col={}",
                    buffer.cursor.row, buffer.cursor.col
                );
            }
        }

        // Request syntax highlighting for newly opened buffer
        self.clear_syntax_cache(); // Force fresh highlighting
        self.request_visible_line_highlighting();

        Ok(id)
    }

    pub fn current_buffer(&self) -> Option<&Buffer> {
        self.current_buffer_id.and_then(|id| self.buffers.get(&id))
    }

    pub fn current_buffer_mut(&mut self) -> Option<&mut Buffer> {
        self.current_buffer_id
            .and_then(|id| self.buffers.get_mut(&id))
    }

    pub fn switch_to_buffer(&mut self, id: usize) -> bool {
        if self.buffers.contains_key(&id) {
            debug!("Switching to buffer ID: {}", id);
            self.current_buffer_id = Some(id);
            true
        } else {
            warn!("Attempted to switch to non-existent buffer ID: {}", id);
            false
        }
    }

    pub fn close_buffer(&mut self, id: usize) -> Result<()> {
        debug!("Closing buffer ID: {}", id);

        if let Some(buffer) = self.buffers.get(&id)
            && buffer.modified
        {
            warn!("Buffer {} has unsaved changes, cannot close", id);
            // TODO: Handle unsaved changes
            self.status_message = "Buffer has unsaved changes!".to_string();
            return Ok(());
        }

        self.buffers.remove(&id);
        debug!("Successfully removed buffer ID: {}", id);

        // Switch to another buffer if we closed the current one
        if self.current_buffer_id == Some(id) {
            self.current_buffer_id = self.buffers.keys().next().copied();
            if let Some(new_id) = self.current_buffer_id {
                debug!(
                    "Switched to buffer ID: {} after closing current buffer",
                    new_id
                );
            } else {
                debug!("No buffers remaining after closing buffer {}", id);
            }
        }

        Ok(())
    }

    /// Open a file in a new buffer
    pub fn open_file(&mut self, filename: &str) -> Result<String> {
        let path = PathBuf::from(filename);
        let buffer_id = self.create_buffer(Some(path))?;
        Ok(format!("Opened '{}' in buffer {}", filename, buffer_id))
    }

    /// Switch to the next buffer in the list
    pub fn switch_to_next_buffer(&mut self) -> bool {
        if self.buffers.len() <= 1 {
            return false;
        }

        let buffer_ids: Vec<usize> = self.buffers.keys().copied().collect();
        let current_index = buffer_ids
            .iter()
            .position(|&id| Some(id) == self.current_buffer_id)
            .unwrap_or(0);

        let next_index = (current_index + 1) % buffer_ids.len();
        self.current_buffer_id = Some(buffer_ids[next_index]);
        true
    }

    /// Switch to the previous buffer in the list
    pub fn switch_to_previous_buffer(&mut self) -> bool {
        if self.buffers.len() <= 1 {
            return false;
        }

        let buffer_ids: Vec<usize> = self.buffers.keys().copied().collect();
        let current_index = buffer_ids
            .iter()
            .position(|&id| Some(id) == self.current_buffer_id)
            .unwrap_or(0);

        let prev_index = if current_index == 0 {
            buffer_ids.len() - 1
        } else {
            current_index - 1
        };
        self.current_buffer_id = Some(buffer_ids[prev_index]);
        true
    }

    /// Close the current buffer
    pub fn close_current_buffer(&mut self) -> Result<String> {
        if let Some(current_id) = self.current_buffer_id {
            if let Some(buffer) = self.buffers.get(&current_id)
                && buffer.modified
            {
                return Ok("Buffer has unsaved changes! Use :bd! to force close".to_string());
            }

            self.buffers.remove(&current_id);

            // Switch to another buffer or create a new one if this was the last
            if self.buffers.is_empty() {
                self.create_buffer(None)?;
                Ok("Closed buffer, created new empty buffer".to_string())
            } else {
                self.current_buffer_id = self.buffers.keys().next().copied();
                Ok("Buffer closed".to_string())
            }
        } else {
            Ok("No buffer to close".to_string())
        }
    }

    /// Force close the current buffer (ignore unsaved changes)
    pub fn force_close_current_buffer(&mut self) -> Result<String> {
        if let Some(current_id) = self.current_buffer_id {
            self.buffers.remove(&current_id);

            // Switch to another buffer or create a new one if this was the last
            if self.buffers.is_empty() {
                self.create_buffer(None)?;
                Ok("Closed buffer (discarded changes), created new empty buffer".to_string())
            } else {
                self.current_buffer_id = self.buffers.keys().next().copied();
                Ok("Buffer closed (discarded changes)".to_string())
            }
        } else {
            Ok("No buffer to close".to_string())
        }
    }

    /// Switch to buffer by name (partial matching)
    pub fn switch_to_buffer_by_name(&mut self, name: &str) -> bool {
        for (id, buffer) in &self.buffers {
            if let Some(file_path) = &buffer.file_path
                && let Some(filename) = file_path.file_name()
                && filename.to_string_lossy().contains(name)
            {
                self.current_buffer_id = Some(*id);
                return true;
            }
        }
        false
    }

    /// List all open buffers
    pub fn list_buffers(&self) -> String {
        if self.buffers.is_empty() {
            return "No buffers open".to_string();
        }

        let mut buffer_list = String::from("Buffers: ");
        for (id, buffer) in &self.buffers {
            let is_current = Some(*id) == self.current_buffer_id;
            let modified = if buffer.modified { "+" } else { "" };
            let name = buffer
                .file_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "[No Name]".to_string());

            buffer_list.push_str(&format!(
                "{}{}:{}{}{}",
                if is_current { "[" } else { "" },
                id,
                name,
                modified,
                if is_current { "]" } else { "" }
            ));

            buffer_list.push(' ');
        }

        buffer_list.trim_end().to_string()
    }

    pub fn render(&mut self) -> Result<()> {
        // Collect all needed data first
        let mode = self.mode;
        let current_buffer = self.current_buffer().cloned();
        let command_line = self.command_line.clone();
        let status_message = self.status_message.clone();

        // Update window viewport based on cursor position and scroll_off setting
        if let Some(buffer) = &current_buffer
            && let Some(current_window) = self.window_manager.current_window_mut()
        {
            let content_height = current_window.content_height();
            let cursor_row = buffer.cursor.row;
            let scroll_off = self.config.interface.scroll_off;

            // Calculate effective scroll boundaries considering scroll_off
            let scroll_off_top = current_window.viewport_top + scroll_off;
            let scroll_off_bottom =
                current_window.viewport_top + content_height.saturating_sub(scroll_off + 1);

            if cursor_row < scroll_off_top {
                // Cursor is too close to top of viewport - scroll up
                current_window.viewport_top = cursor_row.saturating_sub(scroll_off);
            } else if cursor_row > scroll_off_bottom {
                // Cursor is too close to bottom of viewport - scroll down
                current_window.viewport_top =
                    cursor_row.saturating_sub(content_height.saturating_sub(scroll_off + 1));
            }

            // Ensure viewport doesn't go below zero or beyond buffer end
            let max_viewport_top = buffer.lines.len().saturating_sub(content_height);
            current_window.viewport_top = current_window.viewport_top.min(max_viewport_top);

            // Maintain horizontal offset when wrapping is disabled
            if !self.config.behavior.wrap_lines {
                // Visible text columns within window (excluding line number column in UI)
                // We can't access UI's line number width here; use full window width as an approximation
                let text_width = current_window.width as usize;
                let col = buffer.cursor.col;
                let siso = self.config.interface.side_scroll_off;

                // Adjust horiz_offset to keep cursor within [offset+siso, offset+text_width-siso-1]
                if col < current_window.horiz_offset.saturating_add(siso) {
                    current_window.horiz_offset = col.saturating_sub(siso);
                } else if col
                    >= current_window
                        .horiz_offset
                        .saturating_add(text_width.saturating_sub(siso.max(1)))
                {
                    let target = col
                        .saturating_sub(text_width.saturating_sub(siso.max(1)))
                        .saturating_add(1);
                    current_window.horiz_offset = target;
                }
            } else {
                // Reset horizontal offset when wrap is enabled
                current_window.horiz_offset = 0;
            }
        }

        // Generate syntax highlights for all visible windows
        let mut syntax_highlights = HashMap::new();

        // First, collect all the lines that need highlighting from all windows
        let mut lines_to_highlight = HashMap::new(); // (buffer_id, line_index) -> (line_content, file_path)

        for window in self.window_manager.all_windows().values() {
            if let Some(buffer_id) = window.buffer_id
                && let Some(buffer) = self.buffers.get(&buffer_id)
            {
                let content_height = window.content_height();
                let viewport_top = window.viewport_top;

                // Only highlight visible lines + a small buffer for smooth scrolling
                let highlight_start = viewport_top;
                let highlight_end =
                    std::cmp::min(viewport_top + content_height + 10, buffer.lines.len()); // 10 line buffer

                for line_index in highlight_start..highlight_end {
                    let key = (buffer_id, line_index);
                    // Skip if we already have this line queued for highlighting
                    if lines_to_highlight.contains_key(&key) {
                        continue;
                    }

                    if let Some(line) = buffer.get_line(line_index) {
                        let file_path = buffer
                            .file_path
                            .as_ref()
                            .map(|p| p.to_string_lossy().to_string());
                        if let Some(path) = file_path {
                            lines_to_highlight.insert(key, (line.clone(), path));
                        }
                    }
                }
            }
        }

        // Now generate syntax highlights for all collected lines
        for (key, (line_content, file_path)) in lines_to_highlight {
            let (buffer_id, line_index) = key;
            let highlights =
                self.get_syntax_highlights(buffer_id, line_index, &line_content, Some(&file_path));
            // Store ALL highlights, even empty ones, so UI knows syntax highlighting was attempted
            syntax_highlights.insert(key, highlights);
        }

        // Create an optimized render state that only clones buffers currently displayed in windows
        // This avoids cloning ALL buffers while still providing the data needed for rendering
        let mut displayed_buffers = HashMap::new();

        // Only clone buffers that are actually displayed in windows
        for window in self.window_manager.all_windows().values() {
            if let Some(buffer_id) = window.buffer_id
                && let Some(buffer) = self.buffers.get(&buffer_id)
            {
                displayed_buffers.insert(buffer_id, buffer.clone());
            }
        }

        let editor_state = EditorRenderState {
            mode,
            current_buffer,
            all_buffers: displayed_buffers, // Only clone buffers that are visible
            command_line,
            status_message,
            buffer_count: self.buffers.len(),
            current_buffer_id: self.current_buffer_id,
            current_window_id: self.window_manager.current_window_id(),
            window_manager: self.window_manager.clone(), // Need the real window manager for layout
            syntax_highlights,
            command_completion: self.command_completion.clone(),
            config: self.config.clone(),
            filetype: self
                .current_buffer()
                .and_then(|b| b.file_path.as_ref())
                .and_then(|p| p.extension())
                .map(|e| e.to_string_lossy().to_string())
                .or_else(|| {
                    // fallback to language config if unnamed
                    if let Some(buf) = self.current_buffer() {
                        let content = buf.lines.join("\n");
                        self.config.languages.detect_language_from_content(&content)
                    } else {
                        None
                    }
                }),
            macro_recording: self.macro_recorder.recording_register(),
            search_total: self.search_results.len(),
            search_index: self.current_search_index,
        };

        // Use the existing UI render method but with optimized state
        self.ui.render(&mut self.terminal, &editor_state)?;
        Ok(())
    }

    // Getters for UI and other components
    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        debug!("Mode transition: {:?} -> {:?}", self.mode, mode);
        self.mode = mode;
        if mode != Mode::Command {
            self.command_line.clear();
            // Cancel completion when leaving command mode
            self.command_completion.cancel();
        }

        // Update cursor shape based on the new mode
        if let Err(e) = self.update_cursor_shape(mode) {
            warn!("Failed to update cursor shape: {}", e);
        }
    }

    /// Update cursor shape based on editor mode
    fn update_cursor_shape(&mut self, mode: Mode) -> Result<()> {
        match mode {
            Mode::Insert => {
                debug!("Setting cursor to line shape for insert mode");
                self.terminal.set_cursor_line()?;
            }
            Mode::Replace => {
                debug!("Setting cursor to underline shape for replace mode");
                self.terminal.set_cursor_underline()?;
            }
            Mode::Normal
            | Mode::Visual
            | Mode::VisualLine
            | Mode::VisualBlock
            | Mode::Command
            | Mode::Search
            | Mode::OperatorPending => {
                debug!("Setting cursor to block shape for {:?} mode", mode);
                self.terminal.set_cursor_block()?;
            }
        }
        Ok(())
    }

    pub fn command_line(&self) -> &str {
        &self.command_line
    }

    pub fn set_command_line(&mut self, text: String) {
        self.command_line = text;
    }

    pub fn status_message(&self) -> &str {
        &self.status_message
    }

    pub fn set_status_message(&mut self, message: String) {
        debug!("Status message updated: '{}'", message);
        self.status_message = message;
    }

    // Command completion methods
    pub fn start_command_completion(&mut self, input: &str) {
        // Build dynamic context for completion (cwd and buffers)
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let mut buffers = Vec::new();
        for (id, buf) in self.buffers.iter() {
            let name = buf
                .file_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "[No Name]".to_string());
            buffers.push(crate::features::completion::BufferSummary {
                id: *id,
                name,
                modified: buf.modified,
            });
        }
        let current_buffer_dir = self
            .current_buffer()
            .and_then(|b| b.file_path.as_ref())
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());

        let allow_percent_path_root = self.config.interface.percent_path_root;

        self.command_completion
            .set_context(crate::features::completion::CompletionContext {
                cwd,
                buffers,
                current_buffer_dir,
                allow_percent_path_root,
            });
        self.command_completion.start_completion(input);
    }

    pub fn is_completion_active(&self) -> bool {
        self.command_completion.active
    }

    /// Get the number of current completion matches (for UI redraw decisions)
    pub fn completion_matches_len(&self) -> usize {
        self.command_completion.matches.len()
    }

    /// Get the selected completion index (for UI redraw decisions)
    pub fn completion_selected_index(&self) -> usize {
        if self.command_completion.matches.is_empty() {
            0
        } else {
            self.command_completion.selected_index
        }
    }

    pub fn completion_next(&mut self) {
        if self.command_completion.active {
            self.command_completion.next();
        }
    }

    pub fn completion_previous(&mut self) {
        if self.command_completion.active {
            self.command_completion.previous();
        }
    }

    pub fn completion_accept(&mut self) -> Option<String> {
        if self.command_completion.active {
            self.command_completion.accept()
        } else {
            None
        }
    }

    pub fn completion_has_matches(&self) -> bool {
        !self.command_completion.matches.is_empty()
    }

    pub fn cancel_completion(&mut self) {
        self.command_completion.cancel();
    }

    // Operator and text object methods
    pub fn set_pending_operator(&mut self, operator: PendingOperator) {
        debug!("Set pending operator: {:?}", operator);
        self.pending_operator = Some(operator);
        self.set_mode(Mode::OperatorPending);
    }

    pub fn get_pending_operator(&self) -> Option<&PendingOperator> {
        self.pending_operator.as_ref()
    }

    pub fn clear_pending_operator(&mut self) {
        self.pending_operator = None;
        if self.mode == Mode::OperatorPending {
            self.set_mode(Mode::Normal);
        }
    }

    pub fn execute_operator_with_text_object(&mut self, text_object_str: &str) -> Result<bool> {
        let Some(operator) = self.pending_operator.clone() else {
            return Ok(false);
        };

        let Some((mode, object_type)) =
            crate::features::text_objects::parse_text_object(text_object_str)
        else {
            debug!("Invalid text object: {}", text_object_str);
            return Ok(false);
        };

        let Some(buffer) = self.current_buffer() else {
            return Ok(false);
        };

        let cursor = buffer.cursor;

        let text_object_range =
            self.text_object_finder
                .find_text_object(buffer, cursor, object_type, mode)?;

        if let Some(range) = text_object_range {
            debug!(
                "Found text object range: {:?} for operator {:?}",
                range, operator
            );
            self.execute_operator_on_range(operator, range)?;
            self.clear_pending_operator();
            return Ok(true);
        } else {
            debug!("No text object found for: {}", text_object_str);
        }

        Ok(false)
    }

    pub fn execute_operator_on_range(
        &mut self,
        operator: PendingOperator,
        range: crate::features::text_objects::TextObjectRange,
    ) -> Result<()> {
        let object_type = range.object_type; // Clone for logging

        match operator {
            PendingOperator::Delete => {
                self.delete_range(range)?;
                debug!("Deleted text object: {:?}", object_type);
            }
            PendingOperator::Yank => {
                self.yank_range(range)?;
                debug!("Yanked text object: {:?}", object_type);
            }
            PendingOperator::Change => {
                self.delete_range(range)?;
                self.set_mode(Mode::Insert);
                debug!("Changed text object: {:?}", object_type);
            }
            PendingOperator::Indent => {
                self.indent_range(range)?;
                debug!("Indented text object: {:?}", object_type);
            }
            PendingOperator::Unindent => {
                self.unindent_range(range)?;
                debug!("Unindented text object: {:?}", object_type);
            }
            PendingOperator::ToggleCase => {
                self.toggle_case_range(range)?;
                debug!("Toggled case for text object: {:?}", object_type);
            }
        }

        Ok(())
    }

    fn delete_range(
        &mut self,
        range: crate::features::text_objects::TextObjectRange,
    ) -> Result<()> {
        debug!("delete_range called with range: {:?}", range);
        let Some(buffer) = self.current_buffer_mut() else {
            debug!("No current buffer found");
            return Ok(());
        };

        debug!("Buffer has {} lines", buffer.lines.len());

        // Use the buffer's undo-aware delete_range method
        let deleted_text = buffer.delete_range(range.start, range.end);
        debug!("Text deleted: '{}'", deleted_text);

        // Store in clipboard
        buffer.clipboard.text = deleted_text.clone();
        buffer.clipboard.yank_type = if range.start.row != range.end.row {
            crate::core::buffer::YankType::Line
        } else {
            crate::core::buffer::YankType::Character
        };

        debug!("Buffer modified flag set to true");
        self.status_message = format!("Deleted text: '{}'", deleted_text);
        Ok(())
    }

    fn yank_range(&mut self, range: crate::features::text_objects::TextObjectRange) -> Result<()> {
        let Some(buffer) = self.current_buffer_mut() else {
            return Ok(());
        };

        let yanked_text = range.get_text(buffer);
        buffer.clipboard.text = yanked_text;
        buffer.clipboard.yank_type = if range.start.row != range.end.row {
            crate::core::buffer::YankType::Line
        } else {
            crate::core::buffer::YankType::Character
        };

        self.status_message = format!(
            "Yanked {} text object",
            match range.object_type {
                crate::features::text_objects::TextObjectType::Word => "word",
                crate::features::text_objects::TextObjectType::Word2 => "WORD",
                crate::features::text_objects::TextObjectType::Paragraph => "paragraph",
                crate::features::text_objects::TextObjectType::Sentence => "sentence",
                _ => "text",
            }
        );

        Ok(())
    }

    fn indent_range(
        &mut self,
        range: crate::features::text_objects::TextObjectRange,
    ) -> Result<()> {
        let Some(buffer) = self.current_buffer_mut() else {
            return Ok(());
        };

        for row in range.start.row..=range.end.row.min(buffer.lines.len().saturating_sub(1)) {
            let _ = buffer.indent_line(row);
        }

        Ok(())
    }

    fn unindent_range(
        &mut self,
        range: crate::features::text_objects::TextObjectRange,
    ) -> Result<()> {
        let Some(buffer) = self.current_buffer_mut() else {
            return Ok(());
        };

        for row in range.start.row..=range.end.row.min(buffer.lines.len().saturating_sub(1)) {
            let _ = buffer.unindent_line(row);
        }

        Ok(())
    }

    fn toggle_case_range(
        &mut self,
        range: crate::features::text_objects::TextObjectRange,
    ) -> Result<()> {
        let Some(buffer) = self.current_buffer_mut() else {
            return Ok(());
        };

        // Get the text in the range
        let text = buffer.get_text_in_range(range.start, range.end);

        // Toggle case of the text
        let toggled_text: String = text
            .chars()
            .map(|c| {
                if c.is_uppercase() {
                    c.to_lowercase().next().unwrap_or(c)
                } else if c.is_lowercase() {
                    c.to_uppercase().next().unwrap_or(c)
                } else {
                    c
                }
            })
            .collect();

        // Replace the range with the toggled text
        buffer.replace_range(range.start, range.end, &toggled_text);

        Ok(())
    }

    pub fn quit(&mut self) {
        // Check for unsaved buffers
        let unsaved = self.buffers.values().any(|b| b.modified);
        if unsaved {
            self.status_message = "Unsaved changes! Use :q! to force quit".to_string();
            return;
        }

        self.should_quit = true;
    }

    pub fn force_quit(&mut self) {
        self.should_quit = true;
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn save_current_buffer(&mut self) -> Result<()> {
        if let Some(buffer) = self.current_buffer_mut() {
            debug!("Saving buffer with file path: {:?}", buffer.file_path);
            match buffer.save() {
                Ok(_) => {
                    info!("Buffer saved");
                    self.status_message = "File saved".to_string();
                }
                Err(e) => {
                    error!("Failed to save buffer: {}", e);
                    self.status_message = format!("Error saving file: {}", e);
                    return Err(e);
                }
            }
        } else {
            warn!("No current buffer to save");
            self.status_message = "No file to save".to_string();
        }
        Ok(())
    }

    /// Perform a search in the current buffer
    pub fn search(&mut self, pattern: &str) -> bool {
        debug!("Performing search for pattern: '{}'", pattern);

        let lines = if let Some(buffer) = self.current_buffer() {
            buffer.lines.clone()
        } else {
            warn!("No current buffer for search");
            return false;
        };

        // Smart-case: if enabled and pattern has any uppercase, force case-sensitive for this search
        let base_case_sensitive = !self.config.behavior.ignore_case;
        let use_case_sensitive =
            if self.config.behavior.smart_case && pattern.chars().any(|c| c.is_uppercase()) {
                true
            } else {
                base_case_sensitive
            };
        // Temporarily set engine sensitivity for this search only
        self.search_engine.set_case_sensitive(use_case_sensitive);
        let search_results = self.search_engine.search(pattern, &lines);
        // Restore base sensitivity to avoid leaking state
        self.search_engine.set_case_sensitive(base_case_sensitive);
        self.search_results = search_results;

        if !self.search_results.is_empty() {
            debug!(
                "Search found {} matches for pattern: '{}'",
                self.search_results.len(),
                pattern
            );
            self.current_search_index = Some(0);
            self.move_to_search_result(0);
            self.status_message = format!("Found {} matches", self.search_results.len());
            true
        } else {
            info!("No search matches for pattern: '{}'", pattern);
            self.current_search_index = None;
            self.status_message = format!("Pattern not found: {}", pattern);
            false
        }
    }

    /// Move to the next search result
    pub fn search_next(&mut self) -> bool {
        if self.search_results.is_empty() {
            self.status_message = "No search results".to_string();
            return false;
        }

        if let Some(current_index) = self.current_search_index {
            let next_index = (current_index + 1) % self.search_results.len();
            self.current_search_index = Some(next_index);
            self.move_to_search_result(next_index);
            self.status_message =
                format!("Match {} of {}", next_index + 1, self.search_results.len());
            true
        } else {
            self.current_search_index = Some(0);
            self.move_to_search_result(0);
            true
        }
    }

    /// Move to the previous search result
    pub fn search_previous(&mut self) -> bool {
        if self.search_results.is_empty() {
            self.status_message = "No search results".to_string();
            return false;
        }

        if let Some(current_index) = self.current_search_index {
            let prev_index = if current_index == 0 {
                self.search_results.len() - 1
            } else {
                current_index - 1
            };
            self.current_search_index = Some(prev_index);
            self.move_to_search_result(prev_index);
            self.status_message =
                format!("Match {} of {}", prev_index + 1, self.search_results.len());
            true
        } else {
            self.current_search_index = Some(0);
            self.move_to_search_result(0);
            true
        }
    }

    /// Move cursor to a specific search result
    fn move_to_search_result(&mut self, index: usize) {
        if let Some(result) = self.search_results.get(index).cloned()
            && let Some(buffer) = self.current_buffer_mut()
        {
            buffer.cursor.row = result.line;
            buffer.cursor.col = result.start_col;
        }
    }

    /// Clear current search results
    pub fn clear_search(&mut self) {
        self.search_results.clear();
        self.current_search_index = None;
    }

    /// Toggle absolute line numbers
    pub fn toggle_line_numbers(&mut self) {
        self.ui.show_line_numbers = !self.ui.show_line_numbers;
        let status = if self.ui.show_line_numbers {
            "Line numbers enabled"
        } else {
            "Line numbers disabled"
        };
        self.status_message = status.to_string();
    }

    /// Toggle relative line numbers
    pub fn toggle_relative_numbers(&mut self) {
        self.ui.show_relative_numbers = !self.ui.show_relative_numbers;
        let status = if self.ui.show_relative_numbers {
            "Relative line numbers enabled"
        } else {
            "Relative line numbers disabled"
        };
        self.status_message = status.to_string();
    }

    /// Set line number display options
    pub fn set_line_numbers(&mut self, absolute: bool, relative: bool) {
        self.config.display.show_line_numbers = absolute;
        self.config.display.show_relative_numbers = relative;

        // Update UI to reflect config changes
        self.ui.show_line_numbers = absolute;
        self.ui.show_relative_numbers = relative;

        let status = match (absolute, relative) {
            (true, true) => "Hybrid line numbers enabled",
            (true, false) => "Absolute line numbers enabled",
            (false, true) => "Relative line numbers enabled",
            (false, false) => "Line numbers disabled",
        };
        self.status_message = status.to_string();

        // Save config changes
        let _ = self.config.save();
    }

    /// Toggle cursor line highlighting
    pub fn toggle_cursor_line(&mut self) {
        self.config.display.show_cursor_line = !self.config.display.show_cursor_line;
        self.ui.show_cursor_line = self.config.display.show_cursor_line;
        let status = if self.config.display.show_cursor_line {
            "Cursor line highlighting enabled"
        } else {
            "Cursor line highlighting disabled"
        };
        self.status_message = status.to_string();

        // Save config changes
        let _ = self.config.save();
    }

    /// Set cursor line highlighting
    pub fn set_cursor_line(&mut self, enabled: bool) {
        self.config.display.show_cursor_line = enabled;
        self.ui.show_cursor_line = enabled;

        let status = if enabled {
            "Cursor line highlighting enabled"
        } else {
            "Cursor line highlighting disabled"
        };
        self.status_message = status.to_string();

        // Save config changes
        let _ = self.config.save();
    }

    /// Set a configuration setting by name
    pub fn set_config_setting(&mut self, setting: &str, value: &str) {
        let _ = self.config.set_setting(setting, value);

        // Update UI to reflect config changes
        self.ui.show_line_numbers = self.config.display.show_line_numbers;
        self.ui.show_relative_numbers = self.config.display.show_relative_numbers;
        self.ui.show_cursor_line = self.config.display.show_cursor_line;
        self.ui.set_theme(&self.config.display.color_scheme);

        // Apply specific settings that need immediate effect
        match setting {
            "syntax" | "syn" => {
                if self.config.display.syntax_highlighting {
                    // Re-enable syntax highlighting
                    if self.async_syntax_highlighter.is_none() {
                        self.async_syntax_highlighter = AsyncSyntaxHighlighter::new().ok();
                    }
                } else {
                    // Disable syntax highlighting
                    self.async_syntax_highlighter = None;
                }
            }
            "colorscheme" | "colo" => {
                // Update theme configuration to match the new colorscheme
                self.theme_config.set_current_theme(value);
                if let Err(e) = self.theme_config.save() {
                    warn!("Failed to save theme configuration: {}", e);
                }
                self.ui.set_theme(value);

                // TODO: Update async syntax highlighter with new color scheme
                // This will require a new method on AsyncSyntaxHighlighter
            }
            "ignorecase" | "ic" | "smartcase" | "scs" => {
                self.apply_search_settings();
            }
            "autosave" | "aw" => {
                // Auto save setting changed, check if we should save now
                self.check_auto_save();
            }
            _ => {}
        }

        // Save config changes
        let _ = self.config.save();
    }

    /// Apply tab settings to current buffer
    pub fn apply_tab_settings(&mut self) {
        // Tab settings are handled at the editor level since Buffer doesn't store them
        // These settings affect how input is processed
    }

    /// Apply search settings
    pub fn apply_search_settings(&mut self) {
        // Update search engine case sensitivity
        self.search_engine
            .set_case_sensitive(!self.config.behavior.ignore_case);
        // Smart case logic would be implemented in search methods
    }

    /// Check if auto save is enabled and save if needed
    pub fn check_auto_save(&mut self) {
        if self.config.editing.auto_save
            && let Some(buffer) = self.current_buffer_mut()
            && buffer.modified
            && buffer.file_path.is_some()
        {
            let _ = buffer.save();
        }
    }

    /// Get configuration value for display  
    pub fn get_config_value(&self, setting: &str) -> Option<String> {
        match setting {
            "number" | "nu" => Some(self.config.display.show_line_numbers.to_string()),
            "relativenumber" | "rnu" => Some(self.config.display.show_relative_numbers.to_string()),
            "cursorline" | "cul" => Some(self.config.display.show_cursor_line.to_string()),
            "tabstop" | "ts" => Some(self.config.behavior.tab_width.to_string()),
            "expandtab" | "et" => Some(self.config.behavior.expand_tabs.to_string()),
            "autoindent" | "ai" => Some(self.config.behavior.auto_indent.to_string()),
            "ignorecase" | "ic" => Some(self.config.behavior.ignore_case.to_string()),
            "smartcase" | "scs" => Some(self.config.behavior.smart_case.to_string()),
            "hlsearch" | "hls" => Some(self.config.behavior.highlight_search.to_string()),
            "incsearch" | "is" => Some(self.config.behavior.incremental_search.to_string()),
            "wrap" => Some(self.config.behavior.wrap_lines.to_string()),
            "linebreak" | "lbr" => Some(self.config.behavior.line_break.to_string()),
            "undolevels" | "ul" => Some(self.config.editing.undo_levels.to_string()),
            "undofile" | "udf" => Some(self.config.editing.persistent_undo.to_string()),
            "backup" | "bk" => Some(self.config.editing.backup.to_string()),
            "swapfile" | "swf" => Some(self.config.editing.swap_file.to_string()),
            "autosave" | "aw" => Some(self.config.editing.auto_save.to_string()),
            "laststatus" | "ls" => Some(self.config.interface.show_status_line.to_string()),
            "showcmd" | "sc" => Some(self.config.interface.show_command.to_string()),
            "scrolloff" | "so" => Some(self.config.interface.scroll_off.to_string()),
            "sidescrolloff" | "siso" => Some(self.config.interface.side_scroll_off.to_string()),
            "timeoutlen" | "tm" => Some(self.config.interface.command_timeout.to_string()),
            "percentpathroot" | "ppr" => Some(self.config.interface.percent_path_root.to_string()),
            "colorscheme" | "colo" => Some(self.config.display.color_scheme.clone()),
            "syntax" | "syn" => Some(self.config.display.syntax_highlighting.to_string()),
            _ => None,
        }
    }

    /// Get the current value of a configuration setting
    pub fn get_line_number_state(&self) -> (bool, bool) {
        (
            self.config.display.show_line_numbers,
            self.config.display.show_relative_numbers,
        )
    }

    /// Get syntax highlights for a line of text (async version)
    pub fn get_syntax_highlights(
        &mut self,
        buffer_id: usize,
        line_index: usize,
        text: &str,
        file_path: Option<&str>,
    ) -> Vec<crate::features::syntax::HighlightRange> {
        // Debug: Log what we're trying to highlight
        log::debug!(
            "get_syntax_highlights: buffer={}, line={}, text='{}', path={:?}",
            buffer_id,
            line_index,
            text,
            file_path
        );

        // First check if we have an async syntax highlighter
        if let Some(ref highlighter) = self.async_syntax_highlighter {
            log::debug!("Async syntax highlighter is available");

            // Get the language from file extension using configuration
            let language = if let Some(path) = file_path {
                let detected = self.config.languages.detect_language_from_extension(path);
                log::debug!("Language detected from path '{}': {:?}", path, detected);
                detected
            } else {
                // For unnamed buffers, try to detect language from content
                let detected = self.config.languages.detect_language_from_content(text);
                log::debug!("Language detected from content: {:?}", detected);
                detected
            };

            if let Some(lang) = language {
                log::debug!("Using language: '{}'", lang);

                // Try to get cached highlights first - only for single line
                if let Some(cached) =
                    highlighter.get_cached_highlights(buffer_id, line_index, text, &lang)
                {
                    log::debug!("Found cached highlights: {} items", cached.len());
                    return cached;
                }

                log::debug!("No cached highlights, getting immediate highlights for single line");

                // Only highlight the current line, not the entire file
                // This prevents blocking the UI thread with expensive full-file parsing
                if let Some(immediate_highlights) = highlighter
                    .force_immediate_highlights_with_context(
                        buffer_id, line_index,
                        text, // Only pass the single line, not full_content
                        text, &lang,
                    )
                {
                    log::debug!(
                        "Got immediate highlights: {} items",
                        immediate_highlights.len()
                    );

                    return immediate_highlights;
                } else {
                    log::debug!(
                        "No immediate highlights available for line, using basic highlighting"
                    );
                }
            } else {
                log::warn!("No language detected for path: {:?}", file_path);
            }
        } else {
            log::warn!("No async syntax highlighter available");
        }

        // Fallback to empty highlights if no async highlighter or no language detected
        log::debug!("Returning empty highlights as fallback");
        Vec::new()
    }

    /// Request syntax highlighting for all visible lines in current window
    /// Uses full-file context for proper tree-sitter parsing
    pub fn request_visible_line_highlighting(&mut self) {
        if let Some(highlighter) = &mut self.async_syntax_highlighter
            && let Some(window) = self.window_manager.current_window()
            && let Some(buffer_id) = window.buffer_id
            && let Some(buffer) = self.buffers.get(&buffer_id)
        {
            let content_height = window.content_height();
            let viewport_top = window.viewport_top;

            // Get highlighting for visible lines immediately
            let visible_start = viewport_top;
            let visible_end = std::cmp::min(viewport_top + content_height, buffer.lines.len());

            // Request immediate highlighting for visible lines + buffer for scrolling
            let highlight_start = viewport_top;
            let highlight_end = std::cmp::min(
                viewport_top + content_height + 10, // 10 line buffer for smooth scrolling
                buffer.lines.len(),
            );

            if let Some(file_path) = &buffer.file_path {
                let path_str = file_path.to_string_lossy().to_string();

                // Determine language from file extension using configuration
                let language = self
                    .config
                    .languages
                    .detect_language_from_extension(&path_str)
                    .or_else(|| self.config.languages.get_fallback_language())
                    .unwrap_or_else(|| "text".to_string()); // Ultimate fallback

                // Get full buffer content for proper context-aware parsing
                let full_content = buffer.lines.join("\n");

                // Use full-file context highlighting for all visible lines
                for line_index in visible_start..visible_end {
                    if let Some(line) = buffer.get_line(line_index) {
                        // Use force_immediate_highlights_with_context to get proper full-file parsing
                        if let Some(_highlights) = highlighter
                            .force_immediate_highlights_with_context(
                                buffer_id,
                                line_index,
                                &full_content,
                                line,
                                &language,
                            )
                        {
                            // Results are automatically cached, no additional async request needed
                            log::trace!(
                                "Cached full-context highlights for buffer {} line {}",
                                buffer_id,
                                line_index
                            );
                        }
                    }
                }

                // Also cache highlights for buffer lines beyond visible area for smooth scrolling
                for line_index in highlight_start..highlight_end {
                    if (line_index < visible_start || line_index >= visible_end)
                        && let Some(line) = buffer.get_line(line_index)
                    {
                        // Use full-file context for buffer lines too
                        let _ = highlighter.force_immediate_highlights_with_context(
                            buffer_id,
                            line_index,
                            &full_content,
                            line,
                            &language,
                        );
                    }
                }
            }
        }
    }

    /// Get highlighted text for a specific line in the current buffer
    pub fn get_line_highlights(
        &mut self,
        line_index: usize,
    ) -> Vec<crate::features::syntax::HighlightRange> {
        // Get the necessary data first to avoid borrow conflicts
        let (buffer_id, line_text, file_path) = {
            if let Some(buffer) = self.current_buffer() {
                let line = buffer.get_line(line_index).map(|s| s.to_string());
                let path = buffer
                    .file_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string());
                (buffer.id, line, path)
            } else {
                (0, None, None)
            }
        };

        if let (Some(line), Some(path)) = (line_text, file_path) {
            self.get_syntax_highlights(buffer_id, line_index, &line, Some(&path))
        } else {
            Vec::new()
        }
    }

    /// Get syntax highlighting cache statistics
    pub fn get_cache_stats(&self) -> Option<(usize, usize)> {
        self.async_syntax_highlighter
            .as_ref()
            .map(|h| h.cache_stats())
    }

    /// Clear syntax highlighting cache (useful for debugging or memory management)
    pub fn clear_syntax_cache(&mut self) {
        if let Some(ref highlighter) = self.async_syntax_highlighter {
            highlighter.invalidate_buffer_cache(0); // Clear all cache for now
            self.status_message = "Syntax highlighting cache cleared".to_string();
        }
    }

    /// Reload editor configuration from editor.toml
    pub fn reload_editor_config(&mut self) {
        let new_config = EditorConfig::load();

        // Update UI to reflect new config values
        self.ui.show_line_numbers = new_config.display.show_line_numbers;
        self.ui.show_relative_numbers = new_config.display.show_relative_numbers;
        self.ui.show_cursor_line = new_config.display.show_cursor_line;

        self.config = new_config;
        self.status_message = "Editor configuration reloaded".to_string();
    }

    /// Reload keymap configuration from keymaps.toml
    pub fn reload_keymap_config(&mut self) {
        self.key_handler = KeyHandler::new(); // This will reload the keymaps.toml
        self.status_message = "Keymap configuration reloaded".to_string();
    }

    /// Reload UI theme from themes.toml
    pub fn reload_ui_theme(&mut self) {
        // Brief delay to ensure file write is complete
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Get the current theme name from the theme configuration
        let current_theme = self.theme_config.current_theme_name();

        log::info!("Reloading UI theme to: {}", current_theme);

        // Update the UI with the new theme
        self.ui.set_theme(current_theme);

        // Update the syntax highlighter with the new theme and clear cache
        if let Some(ref highlighter) = self.async_syntax_highlighter {
            if let Err(e) = highlighter.update_theme(current_theme) {
                log::warn!("Failed to update syntax highlighter theme: {}", e);
            } else {
                log::info!("Successfully updated syntax highlighter theme and cleared cache");
            }
        }

        self.status_message = format!("Theme '{}' reloaded", current_theme);

        // Force immediate re-highlighting of visible content with new theme
        self.refresh_visible_syntax_highlighting();
    }

    /// Force immediate re-highlighting of visible content (used after theme changes)
    fn refresh_visible_syntax_highlighting(&mut self) {
        if let Some(current_window) = self.window_manager.current_window()
            && let Some(buffer_id) = current_window.buffer_id
            && let Some(buffer) = self.buffers.get(&buffer_id)
            && let Some(ref highlighter) = self.async_syntax_highlighter
        {
            // Calculate visible range
            let viewport_top = current_window.viewport_top;
            let viewport_height = current_window.height as usize;
            let visible_start = viewport_top;
            let visible_end = (viewport_top + viewport_height).min(buffer.line_count());

            // Determine language
            let language = if let Some(ref path) = buffer.file_path {
                if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                    match extension {
                        "rs" => "rust",
                        "py" => "python",
                        "js" => "javascript",
                        "ts" => "typescript",
                        "c" => "c",
                        "cpp" | "cc" | "cxx" => "cpp",
                        _ => "rust",
                    }
                } else {
                    "rust"
                }
            } else {
                "rust"
            };

            log::debug!(
                "Refreshing syntax highlighting for visible lines {}-{}",
                visible_start,
                visible_end
            );

            // Force high-priority re-highlighting of all visible lines
            for line_index in visible_start..visible_end {
                if let Some(line) = buffer.get_line(line_index) {
                    highlighter.request_highlighting(
                        buffer_id,
                        line_index,
                        line.to_string(),
                        language.to_string(),
                        Priority::Critical,
                    );
                }
            }
        }
    }

    // Scrolling methods
    pub fn scroll_down_line(&mut self) {
        // Ctrl+e: Scroll down one line using current window viewport
        if let Some(current_window) = self.window_manager.current_window_mut() {
            current_window.viewport_top = current_window.viewport_top.saturating_add(1);
        }
        // Request highlighting for newly visible lines
        self.request_visible_line_highlighting();
    }

    pub fn scroll_up_line(&mut self) {
        // Ctrl+y: Scroll up one line using current window viewport
        if let Some(current_window) = self.window_manager.current_window_mut() {
            current_window.viewport_top = current_window.viewport_top.saturating_sub(1);
        }
        // Request highlighting for newly visible lines
        self.request_visible_line_highlighting();
    }

    pub fn scroll_down_page(&mut self) {
        // Ctrl+f: Scroll down one page using current window height
        let (old_viewport_top, new_viewport_top, content_height, scroll_off) = {
            if let Some(current_window) = self.window_manager.current_window_mut() {
                let page_size = current_window.content_height().saturating_sub(1); // Leave 1 line for overlap
                let old_viewport_top = current_window.viewport_top;
                current_window.viewport_top = current_window.viewport_top.saturating_add(page_size);
                let new_viewport_top = current_window.viewport_top;
                let content_height = current_window.content_height();
                (
                    old_viewport_top,
                    new_viewport_top,
                    content_height,
                    self.config.interface.scroll_off,
                )
            } else {
                return;
            }
        };

        // Move cursor down by the same amount as viewport
        if let Some(buffer) = self.current_buffer_mut() {
            let scroll_amount = new_viewport_top - old_viewport_top;
            buffer.cursor.row = buffer.cursor.row.saturating_add(scroll_amount);
            buffer.cursor.row = buffer.cursor.row.min(buffer.lines.len().saturating_sub(1));

            // Apply scroll_off to keep cursor within visible bounds
            let min_cursor_row = new_viewport_top + scroll_off;
            let max_cursor_row = new_viewport_top + content_height.saturating_sub(scroll_off + 1);

            if buffer.cursor.row < min_cursor_row {
                buffer.cursor.row = min_cursor_row.min(buffer.lines.len().saturating_sub(1));
            } else if buffer.cursor.row > max_cursor_row {
                buffer.cursor.row = max_cursor_row.min(buffer.lines.len().saturating_sub(1));
            }

            // Ensure cursor column is valid for the new line
            if let Some(line) = buffer.get_line(buffer.cursor.row) {
                buffer.cursor.col = buffer.cursor.col.min(line.len());
            }
        }

        // Request highlighting for newly visible lines after scrolling
        self.request_visible_line_highlighting();
    }

    pub fn scroll_up_page(&mut self) {
        // Ctrl+b: Scroll up one page using current window height
        let (old_viewport_top, new_viewport_top, content_height, scroll_off) = {
            if let Some(current_window) = self.window_manager.current_window_mut() {
                let page_size = current_window.content_height().saturating_sub(1); // Leave 1 line for overlap
                let old_viewport_top = current_window.viewport_top;
                current_window.viewport_top = current_window.viewport_top.saturating_sub(page_size);
                let new_viewport_top = current_window.viewport_top;
                let content_height = current_window.content_height();
                (
                    old_viewport_top,
                    new_viewport_top,
                    content_height,
                    self.config.interface.scroll_off,
                )
            } else {
                return;
            }
        };

        // Move cursor up by the same amount as viewport
        if let Some(buffer) = self.current_buffer_mut() {
            let scroll_amount = old_viewport_top - new_viewport_top;
            buffer.cursor.row = buffer.cursor.row.saturating_sub(scroll_amount);

            // Apply scroll_off to keep cursor within visible bounds
            let min_cursor_row = new_viewport_top + scroll_off;
            let max_cursor_row = new_viewport_top + content_height.saturating_sub(scroll_off + 1);

            if buffer.cursor.row < min_cursor_row {
                buffer.cursor.row = min_cursor_row.min(buffer.lines.len().saturating_sub(1));
            } else if buffer.cursor.row > max_cursor_row {
                buffer.cursor.row = max_cursor_row.min(buffer.lines.len().saturating_sub(1));
            }

            // Ensure cursor column is valid for the new line
            if let Some(line) = buffer.get_line(buffer.cursor.row) {
                buffer.cursor.col = buffer.cursor.col.min(line.len());
            }
        }

        // Request highlighting for newly visible lines after scrolling
        self.request_visible_line_highlighting();
    }

    pub fn scroll_down_half_page(&mut self) {
        // Ctrl+d: Scroll down half page using current window height
        let (old_viewport_top, new_viewport_top, content_height, scroll_off) = {
            if let Some(current_window) = self.window_manager.current_window_mut() {
                let half_page_size = (current_window.content_height() / 2).max(1);
                let old_viewport_top = current_window.viewport_top;
                current_window.viewport_top =
                    current_window.viewport_top.saturating_add(half_page_size);
                let new_viewport_top = current_window.viewport_top;
                let content_height = current_window.content_height();
                (
                    old_viewport_top,
                    new_viewport_top,
                    content_height,
                    self.config.interface.scroll_off,
                )
            } else {
                return;
            }
        };

        // Move cursor down by the same amount as viewport
        if let Some(buffer) = self.current_buffer_mut() {
            let scroll_amount = new_viewport_top - old_viewport_top;
            buffer.cursor.row = buffer.cursor.row.saturating_add(scroll_amount);
            buffer.cursor.row = buffer.cursor.row.min(buffer.lines.len().saturating_sub(1));

            // Apply scroll_off to keep cursor within visible bounds
            let min_cursor_row = new_viewport_top + scroll_off;
            let max_cursor_row = new_viewport_top + content_height.saturating_sub(scroll_off + 1);

            if buffer.cursor.row < min_cursor_row {
                buffer.cursor.row = min_cursor_row.min(buffer.lines.len().saturating_sub(1));
            } else if buffer.cursor.row > max_cursor_row {
                buffer.cursor.row = max_cursor_row.min(buffer.lines.len().saturating_sub(1));
            }

            // Ensure cursor column is valid for the new line
            if let Some(line) = buffer.get_line(buffer.cursor.row) {
                buffer.cursor.col = buffer.cursor.col.min(line.len());
            }
        }

        // Request highlighting for newly visible lines after scrolling
        self.request_visible_line_highlighting();
    }

    pub fn scroll_up_half_page(&mut self) {
        // Ctrl+u: Scroll up half page using current window height
        let (old_viewport_top, new_viewport_top, content_height, scroll_off) = {
            if let Some(current_window) = self.window_manager.current_window_mut() {
                let half_page_size = (current_window.content_height() / 2).max(1);
                let old_viewport_top = current_window.viewport_top;
                current_window.viewport_top =
                    current_window.viewport_top.saturating_sub(half_page_size);
                let new_viewport_top = current_window.viewport_top;
                let content_height = current_window.content_height();
                (
                    old_viewport_top,
                    new_viewport_top,
                    content_height,
                    self.config.interface.scroll_off,
                )
            } else {
                return;
            }
        };

        // Move cursor up by the same amount as viewport
        if let Some(buffer) = self.current_buffer_mut() {
            let scroll_amount = old_viewport_top - new_viewport_top;
            buffer.cursor.row = buffer.cursor.row.saturating_sub(scroll_amount);

            // Apply scroll_off to keep cursor within visible bounds
            let min_cursor_row = new_viewport_top + scroll_off;
            let max_cursor_row = new_viewport_top + content_height.saturating_sub(scroll_off + 1);

            if buffer.cursor.row < min_cursor_row {
                buffer.cursor.row = min_cursor_row.min(buffer.lines.len().saturating_sub(1));
            } else if buffer.cursor.row > max_cursor_row {
                buffer.cursor.row = max_cursor_row.min(buffer.lines.len().saturating_sub(1));
            }

            // Ensure cursor column is valid for the new line
            if let Some(line) = buffer.get_line(buffer.cursor.row) {
                buffer.cursor.col = buffer.cursor.col.min(line.len());
            }
        }

        // Request highlighting for newly visible lines after scrolling
        self.request_visible_line_highlighting();
    }

    // Centering methods (z commands in Vim)
    pub fn center_cursor(&mut self) {
        // zz: Center current line in viewport
        if let Some(buffer) = self.current_buffer() {
            let cursor_row = buffer.cursor.row;
            let buffer_lines_len = buffer.lines.len();

            if let Some(current_window) = self.window_manager.current_window_mut() {
                let content_height = current_window.content_height();
                let half_height = content_height / 2;

                // Set viewport so cursor line is in the middle
                current_window.viewport_top = cursor_row.saturating_sub(half_height);

                // Ensure we don't scroll past the end of the buffer
                let max_viewport_top = buffer_lines_len.saturating_sub(content_height);
                current_window.viewport_top = current_window.viewport_top.min(max_viewport_top);
            }
        }
    }

    pub fn cursor_to_top(&mut self) {
        // zt: Move current line to top of viewport
        if let Some(buffer) = self.current_buffer() {
            let cursor_row = buffer.cursor.row;
            if let Some(current_window) = self.window_manager.current_window_mut() {
                current_window.viewport_top = cursor_row;
            }
        }
    }

    pub fn cursor_to_bottom(&mut self) {
        // zb: Move current line to bottom of viewport
        if let Some(buffer) = self.current_buffer() {
            let cursor_row = buffer.cursor.row;
            if let Some(current_window) = self.window_manager.current_window_mut() {
                let content_height = current_window.content_height();

                // Set viewport so cursor line is at the bottom
                current_window.viewport_top =
                    cursor_row.saturating_sub(content_height.saturating_sub(1));
            }
        }
    }

    /// Helper method to set up a new window with buffer and cursor position
    fn setup_new_window(&mut self, new_window_id: usize) {
        if let Some(buffer_id) = self.current_buffer_id
            && let Some(buffer) = self.buffers.get(&buffer_id)
            && let Some(new_window) = self.window_manager.get_window_mut(new_window_id)
        {
            new_window.set_buffer(buffer_id);
            // Copy current cursor position to the new window
            new_window.save_cursor_position(buffer.cursor.row, buffer.cursor.col);
        }
    }

    // Split window methods
    pub fn split_horizontal(&mut self) -> String {
        if let Some(new_window_id) = self
            .window_manager
            .split_current_window(SplitDirection::Horizontal)
        {
            // Set up the new window with buffer and cursor position
            self.setup_new_window(new_window_id);
            format!("Created horizontal split (window {})", new_window_id)
        } else {
            "Failed to create horizontal split".to_string()
        }
    }

    pub fn split_vertical(&mut self) -> String {
        if let Some(new_window_id) = self
            .window_manager
            .split_current_window(SplitDirection::Vertical)
        {
            // Set up the new window with buffer and cursor position
            self.setup_new_window(new_window_id);
            format!("Created vertical split (window {})", new_window_id)
        } else {
            "Failed to create vertical split".to_string()
        }
    }

    pub fn split_horizontal_above(&mut self) -> String {
        if let Some(new_window_id) = self
            .window_manager
            .split_current_window(SplitDirection::HorizontalAbove)
        {
            // Set up the new window with buffer and cursor position
            self.setup_new_window(new_window_id);
            format!("Created horizontal split above (window {})", new_window_id)
        } else {
            "Failed to create horizontal split above".to_string()
        }
    }

    pub fn split_horizontal_below(&mut self) -> String {
        if let Some(new_window_id) = self
            .window_manager
            .split_current_window(SplitDirection::HorizontalBelow)
        {
            // Set up the new window with buffer and cursor position
            self.setup_new_window(new_window_id);
            format!("Created horizontal split below (window {})", new_window_id)
        } else {
            "Failed to create horizontal split below".to_string()
        }
    }

    pub fn split_vertical_left(&mut self) -> String {
        if let Some(new_window_id) = self
            .window_manager
            .split_current_window(SplitDirection::VerticalLeft)
        {
            // Set up the new window with buffer and cursor position
            self.setup_new_window(new_window_id);
            format!("Created vertical split left (window {})", new_window_id)
        } else {
            "Failed to create vertical split left".to_string()
        }
    }

    pub fn split_vertical_right(&mut self) -> String {
        if let Some(new_window_id) = self
            .window_manager
            .split_current_window(SplitDirection::VerticalRight)
        {
            // Set up the new window with buffer and cursor position
            self.setup_new_window(new_window_id);
            format!("Created vertical split right (window {})", new_window_id)
        } else {
            "Failed to create vertical split right".to_string()
        }
    }

    pub fn close_window(&mut self) -> String {
        if self.window_manager.close_current_window() {
            // Update current buffer based on new current window
            if let Some(current_window) = self.window_manager.current_window() {
                self.current_buffer_id = current_window.buffer_id;
            }
            "Window closed".to_string()
        } else {
            "Cannot close the last window".to_string()
        }
    }

    // Window navigation methods
    pub fn move_to_window_left(&mut self) -> bool {
        // Save current cursor position before switching
        self.save_current_cursor_to_window();

        let result = self.window_manager.move_to_window_left();
        if result {
            self.restore_cursor_from_current_window();
        }
        result
    }

    pub fn move_to_window_right(&mut self) -> bool {
        // Save current cursor position before switching
        self.save_current_cursor_to_window();

        let result = self.window_manager.move_to_window_right();
        if result {
            self.restore_cursor_from_current_window();
        }
        result
    }

    pub fn move_to_window_up(&mut self) -> bool {
        // Save current cursor position before switching
        self.save_current_cursor_to_window();

        let result = self.window_manager.move_to_window_up();
        if result {
            self.restore_cursor_from_current_window();
        }
        result
    }

    pub fn move_to_window_down(&mut self) -> bool {
        // Save current cursor position before switching
        self.save_current_cursor_to_window();

        let result = self.window_manager.move_to_window_down();
        if result {
            self.restore_cursor_from_current_window();
        }
        result
    }

    // Window resizing methods
    pub fn resize_window_wider(&mut self) -> String {
        let resize_amount = self.config.interface.window_resize_amount;
        if self
            .window_manager
            .resize_current_window_wider(resize_amount)
        {
            format!("Window resized wider by {} columns", resize_amount)
        } else {
            "Cannot resize window wider".to_string()
        }
    }

    pub fn resize_window_narrower(&mut self) -> String {
        let resize_amount = self.config.interface.window_resize_amount;
        if self
            .window_manager
            .resize_current_window_narrower(resize_amount)
        {
            format!("Window resized narrower by {} columns", resize_amount)
        } else {
            "Cannot resize window narrower".to_string()
        }
    }

    pub fn resize_window_taller(&mut self) -> String {
        let resize_amount = self.config.interface.window_resize_amount;
        if self
            .window_manager
            .resize_current_window_taller(resize_amount)
        {
            format!("Window resized taller by {} rows", resize_amount)
        } else {
            "Cannot resize window taller".to_string()
        }
    }

    pub fn resize_window_shorter(&mut self) -> String {
        let resize_amount = self.config.interface.window_resize_amount;
        if self
            .window_manager
            .resize_current_window_shorter(resize_amount)
        {
            format!("Window resized shorter by {} rows", resize_amount)
        } else {
            "Cannot resize window shorter".to_string()
        }
    }

    fn save_current_cursor_to_window(&mut self) {
        if let (Some(current_buffer_id), Some(current_window_id)) = (
            self.current_buffer_id,
            self.window_manager.current_window_id(),
        ) && let Some(current_buffer) = self.buffers.get(&current_buffer_id)
            && let Some(current_window) = self.window_manager.get_window_mut(current_window_id)
        {
            current_window
                .save_cursor_position(current_buffer.cursor.row, current_buffer.cursor.col);
        }
    }

    fn restore_cursor_from_current_window(&mut self) {
        // Switch to the new window's buffer
        if let Some(new_window) = self.window_manager.current_window() {
            self.current_buffer_id = new_window.buffer_id;

            // Restore cursor position from the new window
            if let Some(buffer_id) = new_window.buffer_id {
                let (cursor_row, cursor_col) = new_window.get_cursor_position();
                if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
                    buffer.move_cursor(crate::core::mode::Position::new(cursor_row, cursor_col));
                }
            }
        }
    }

    /// Handle a key event without borrowing conflicts that would reset KeyHandler state
    pub fn handle_key_event(&mut self, key_event: crossterm::event::KeyEvent) -> Result<()> {
        // We need to work around the borrow checker here. The KeyHandler needs &mut self (KeyHandler)
        // and &mut Editor, but KeyHandler is owned by Editor, creating a borrow conflict.
        //
        // Using unsafe to create a mutable reference to the KeyHandler while also having
        // a mutable reference to the Editor. This is safe because we know the KeyHandler
        // and Editor don't overlap in memory.

        debug!(
            "Handling key event: {:?} in mode: {:?}",
            key_event, self.mode
        );

        unsafe {
            let key_handler_ptr = &mut self.key_handler as *mut KeyHandler;
            let result = (*key_handler_ptr).handle_key(self, key_event);

            if let Err(ref e) = result {
                error!("Error handling key event {:?}: {}", key_event, e);
            }

            result
        }
    }

    // Macro system public methods
    pub fn is_macro_recording(&self) -> bool {
        self.macro_recorder.is_recording()
    }

    pub fn start_macro_recording(
        &mut self,
        register: char,
    ) -> Result<(), crate::features::macros::MacroError> {
        self.macro_recorder.start_recording(register)
    }

    pub fn stop_macro_recording(&mut self) -> Result<char, crate::features::macros::MacroError> {
        self.macro_recorder.stop_recording()
    }

    pub fn play_macro(
        &mut self,
        register: char,
    ) -> Result<Vec<crossterm::event::KeyEvent>, crate::features::macros::MacroError> {
        self.macro_recorder.play_macro(register)
    }

    pub fn play_last_macro(
        &mut self,
    ) -> Result<Vec<crossterm::event::KeyEvent>, crate::features::macros::MacroError> {
        self.macro_recorder.play_last_macro()
    }

    pub fn get_last_played_macro_register(&self) -> Option<char> {
        self.macro_recorder.get_last_played_register()
    }

    pub fn record_macro_event(&mut self, event: crossterm::event::KeyEvent) {
        self.macro_recorder.record_event(event);
    }
}
