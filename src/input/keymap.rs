use crate::core::editor::Editor;
use crate::core::mode::{Mode, Position};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use log::{debug, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeymapConfig {
    pub normal_mode: HashMap<String, String>,
    pub insert_mode: HashMap<String, String>,
    pub command_mode: HashMap<String, String>,
    pub visual_mode: HashMap<String, String>,
    pub visual_line_mode: HashMap<String, String>,
    pub visual_block_mode: HashMap<String, String>,
    pub replace_mode: HashMap<String, String>,
    pub search_mode: HashMap<String, String>,
    pub operator_pending_mode: HashMap<String, String>,
}

#[derive(Clone)]
pub struct KeyHandler {
    keymap_config: KeymapConfig,
    pub pending_sequence: String,
    pub last_key_time: Option<std::time::Instant>,
    // Pending numeric count prefix (e.g., 10j, 3dd)
    pub pending_count: Option<usize>,
    // Character navigation state
    pub last_char_search: Option<CharSearchState>,
    pub pending_char_command: Option<PendingCharCommand>,
    // Repeat command state
    pub last_command: Option<RepeatableCommand>,
    // Macro recording register selection state (after pressing 'q')
    pub pending_macro_register: bool,
    // Macro execution register selection state (after pressing '@')
    pub pending_macro_execute: bool,
}

#[derive(Clone, Debug)]
pub struct RepeatableCommand {
    pub action: String,
    pub key: KeyEvent,
    pub count: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct CharSearchState {
    pub search_type: CharSearchType,
    pub character: char,
    pub forward: bool,
}

#[derive(Clone, Debug, PartialEq, Copy)]
pub struct PendingCharCommand {
    pub search_type: CharSearchType,
    pub forward: bool,
}

#[derive(Clone, Debug, PartialEq, Copy)]
pub enum CharSearchType {
    Find, // f/F - find character
    Till, // t/T - till character (stop before)
}

impl Default for KeyHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyHandler {
    pub fn new() -> Self {
        Self {
            keymap_config: Self::load_default_keymaps(),
            pending_sequence: String::new(),
            last_key_time: None,
            pending_count: None,
            last_char_search: None,
            pending_char_command: None,
            last_command: None,
            pending_macro_register: false,
            pending_macro_execute: false,
        }
    }

    fn load_default_keymaps() -> KeymapConfig {
        info!("Loading keymap configuration");
        // Try to load from keymaps.toml file
        if let Ok(config_content) = fs::read_to_string("keymaps.toml") {
            debug!("Found keymaps.toml file, attempting to parse");
            if let Ok(config) = toml::from_str(&config_content) {
                info!("Successfully loaded keymap configuration from keymaps.toml");
                return config;
            } else {
                warn!(
                    "Failed to parse keymaps.toml; no built-in keymaps will be used. Please fix keymaps.toml."
                );
            }
        } else {
            warn!(
                "keymaps.toml not found; no built-in keymaps will be used. Please add keymaps.toml."
            );
        }

        // Return empty keymaps to ensure all bindings come from keymaps.toml only
        Self::create_minimal_fallback()
    }

    fn create_minimal_fallback() -> KeymapConfig {
        // Return fully empty maps to avoid any hard-coded keybindings in code
        KeymapConfig {
            normal_mode: HashMap::new(),
            insert_mode: HashMap::new(),
            command_mode: HashMap::new(),
            visual_mode: HashMap::new(),
            visual_line_mode: HashMap::new(),
            visual_block_mode: HashMap::new(),
            replace_mode: HashMap::new(),
            search_mode: HashMap::new(),
            operator_pending_mode: HashMap::new(),
        }
    }

    pub fn handle_key(&mut self, editor: &mut Editor, key: KeyEvent) -> Result<()> {
        let key_string = Self::key_event_to_string(key);
        trace!(
            "Handling key: '{}' in mode: {:?}",
            key_string,
            editor.mode()
        );

        // If we're waiting for a macro register after 'q', consume the next char
        if self.pending_macro_register {
            match key.code {
                KeyCode::Char(register) => {
                    self.pending_macro_register = false;
                    // Start recording with the chosen register
                    if editor.start_macro_recording(register).is_ok() {
                        info!("Started macro recording for register '{}'", register);
                    }
                    // Do not process this key further or record it; it's only a selector
                    self.pending_sequence.clear();
                    return Ok(());
                }
                KeyCode::Esc => {
                    // Cancel pending register selection
                    self.pending_macro_register = false;
                    self.pending_sequence.clear();
                    return Ok(());
                }
                _ => {
                    // Any non-char cancels the pending state without starting
                    self.pending_macro_register = false;
                    self.pending_sequence.clear();
                    return Ok(());
                }
            }
        }

        // If we're waiting for a macro execute target after '@', handle next key
        if self.pending_macro_execute {
            match key.code {
                KeyCode::Char('@') => {
                    // Repeat last executed macro (@@)
                    self.pending_macro_execute = false;
                    self.pending_sequence.clear();
                    match editor.play_last_macro() {
                        Ok(events) => {
                            if let Some(last_register) = editor.get_last_played_macro_register() {
                                info!(
                                    "Repeating last macro from register '{}' with {} events",
                                    last_register,
                                    events.len()
                                );
                            }
                            // Replay events through the normal key handler so counts and state are honored
                            for key_event in events {
                                self.handle_key(editor, key_event)?;
                            }
                            // Mark playback finished
                            editor.finish_macro_playback();
                        }
                        Err(e) => {
                            warn!("Failed to repeat last macro: {}", e);
                        }
                    }
                    return Ok(());
                }
                KeyCode::Char(register) => {
                    self.pending_macro_execute = false;
                    self.pending_sequence.clear();
                    match editor.play_macro(register) {
                        Ok(events) => {
                            info!(
                                "Executing macro from register '{}' with {} events",
                                register,
                                events.len()
                            );
                            // Replay events through the normal key handler so counts and state are honored
                            for key_event in events {
                                self.handle_key(editor, key_event)?;
                            }
                            // Mark playback finished
                            editor.finish_macro_playback();
                        }
                        Err(e) => {
                            warn!(
                                "Failed to execute macro from register '{}': {}",
                                register, e
                            );
                        }
                    }
                    return Ok(());
                }
                KeyCode::Esc => {
                    // Cancel pending execute
                    self.pending_macro_execute = false;
                    self.pending_sequence.clear();
                    return Ok(());
                }
                _ => {
                    // Any non-char cancels
                    self.pending_macro_execute = false;
                    self.pending_sequence.clear();
                    return Ok(());
                }
            }
        }

        // Handle pending character navigation commands
        if let Some(pending_cmd) = self.pending_char_command {
            if let KeyCode::Char(ch) = key.code {
                debug!(
                    "Executing pending character command: {:?} for char '{}'",
                    pending_cmd, ch
                );
                self.pending_char_command = None; // Clear the pending command

                // Create a key event for the character and execute the appropriate action
                let char_key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::empty());
                match (pending_cmd.search_type, pending_cmd.forward) {
                    (CharSearchType::Find, true) => {
                        self.action_find_char_forward(editor, char_key)?;
                    }
                    (CharSearchType::Find, false) => {
                        self.action_find_char_backward(editor, char_key)?;
                    }
                    (CharSearchType::Till, true) => {
                        self.action_till_char_forward(editor, char_key)?;
                    }
                    (CharSearchType::Till, false) => {
                        self.action_till_char_backward(editor, char_key)?;
                    }
                }
                return Ok(());
            } else {
                // Non-character key pressed, cancel the pending command
                debug!("Non-character key pressed, canceling pending character command");
                self.pending_char_command = None;
            }
        }

        // Handle key sequences for normal mode and operator pending mode
        if matches!(editor.mode(), Mode::Normal | Mode::OperatorPending) {
            // Check for timeout (reset sequence if too much time passed)
            let now = Instant::now();
            if !self.pending_sequence.is_empty()
                && let Some(last_time) = self.last_key_time
                && now.duration_since(last_time).as_millis() > editor.command_timeout_ms() as u128
            {
                debug!(
                    "Key sequence '{}' timed out, clearing",
                    self.pending_sequence
                );
                self.pending_sequence.clear();
                // Also clear any pending count
                if self.pending_count.is_some() {
                    debug!("Pending count timed out, clearing");
                }
                self.pending_count = None;
            }
            self.last_key_time = Some(now);

            // Numeric count handling: accumulate digits as count prefix in Normal/OP modes
            if let KeyCode::Char(d) = key.code
                && d.is_ascii_digit()
            {
                // '0' is special: if no count started and no sequence, treat as line_start
                let is_zero_no_count =
                    d == '0' && self.pending_count.is_none() && self.pending_sequence.is_empty();
                if !is_zero_no_count {
                    let digit = (d as u8 - b'0') as usize;
                    let new_count = self
                        .pending_count
                        .unwrap_or(0)
                        .saturating_mul(10)
                        .saturating_add(digit);
                    self.pending_count = Some(new_count);
                    debug!("Accumulated count: {}", new_count);
                    // If recording a macro, also record the digit key so playback reproduces the count
                    if editor.is_macro_recording() {
                        editor.record_macro_event(key);
                    }
                    // Don't include digits in the key sequence; wait for next non-digit
                    return Ok(());
                }
            }

            // Add current key to sequence
            if !self.pending_sequence.is_empty() {
                // For single character keys after single character sequences, concatenate without space
                // Example: "g" + "g" = "gg", "]" + "]" = "]]", "[" + "[" = "[["
                if key_string.len() == 1
                    && self.pending_sequence.len() == 1
                    && (self
                        .pending_sequence
                        .chars()
                        .next()
                        .unwrap_or(' ')
                        .is_ascii_alphabetic()
                        || (self.pending_sequence == "]" && key_string == "]")
                        || (self.pending_sequence == "[" && key_string == "["))
                {
                    self.pending_sequence.push_str(&key_string);
                } else {
                    // For all other cases, add space between keys
                    // Example: "Ctrl+w" + "h" = "Ctrl+w h"
                    self.pending_sequence.push(' ');
                    self.pending_sequence.push_str(&key_string);
                }
            } else {
                self.pending_sequence = key_string.clone();
            }

            debug!("Current key sequence: '{}'", self.pending_sequence);

            // Check if sequence matches any command in the current mode
            let current_keymap = match editor.mode() {
                Mode::Normal => &self.keymap_config.normal_mode,
                Mode::OperatorPending => &self.keymap_config.operator_pending_mode,
                _ => unreachable!(), // We only enter this block for Normal and OperatorPending
            };

            if let Some(action) = current_keymap.get(&self.pending_sequence) {
                // Special handling for operators in Normal mode: they should execute immediately
                // even if there are longer potential matches (like 'd' vs 'dd')
                let is_operator = matches!(editor.mode(), Mode::Normal)
                    && matches!(
                        action.as_str(),
                        "operator_delete"
                            | "operator_change"
                            | "operator_yank"
                            | "operator_indent"
                            | "operator_unindent"
                            | "operator_toggle_case"
                    );

                if is_operator {
                    debug!(
                        "Executing operator '{}' immediately for key sequence '{}'",
                        action, self.pending_sequence
                    );
                    let action_clone = action.clone();
                    let action_result = self.execute_action(editor, &action_clone, key);
                    self.pending_sequence.clear();
                    return action_result;
                }

                // For non-operators, check if there's also a longer potential match.
                // If so, we wait. If not, we execute immediately.
                // Special handling: single character keys should not wait for unrelated keys
                let has_potential_match = if self.pending_sequence.len() == 1 {
                    // For single character sequences, only consider potential matches that are legitimate extensions
                    current_keymap.keys().any(|k| {
                        if k.starts_with(&self.pending_sequence) && k != &self.pending_sequence {
                            // Check what kind of potential match this is
                            let potential_suffix = &k[self.pending_sequence.len()..];

                            // Don't wait for function keys (F1, F2, etc.) when user presses F
                            if potential_suffix.chars().all(|c| c.is_ascii_digit()) {
                                return false;
                            }

                            // For single character prefixes, only wait for legitimate compound sequences
                            // Don't wait for completely different key types
                            match self.pending_sequence.as_str() {
                                "D" => {
                                    // D should only wait for sequences like "dd", not "Down"
                                    k.starts_with("d")
                                        && k.len() > 1
                                        && k.chars().nth(1).is_some_and(|c| c.is_ascii_lowercase())
                                }
                                "C" => {
                                    // C should only wait for sequences starting with "c", not "Ctrl+..."
                                    k.starts_with("c")
                                        && k.len() > 1
                                        && k.chars().nth(1).is_some_and(|c| c.is_ascii_lowercase())
                                }
                                _ => {
                                    // For other single characters, only wait for same-case extensions
                                    let first_char = self.pending_sequence.chars().next().unwrap();
                                    if first_char.is_ascii_uppercase() {
                                        // Uppercase letters should only wait for other uppercase sequences
                                        k.chars()
                                            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                                    } else if first_char.is_ascii_lowercase() {
                                        // Lowercase letters should wait for valid compound sequences
                                        true
                                    } else {
                                        // Other characters use default logic
                                        true
                                    }
                                }
                            }
                        } else {
                            false
                        }
                    })
                } else {
                    // For multi-character sequences, use the original logic
                    current_keymap.keys().any(|k| {
                        k.starts_with(&self.pending_sequence) && k != &self.pending_sequence
                    })
                };

                if !has_potential_match {
                    debug!(
                        "Executing action '{}' for key sequence '{}'",
                        action, self.pending_sequence
                    );
                    let action_clone = action.clone();
                    let action_result = self.execute_action(editor, &action_clone, key);
                    self.pending_sequence.clear();
                    return action_result;
                } else {
                    debug!(
                        "Key sequence '{}' has potential longer matches, waiting for next key",
                        self.pending_sequence
                    );
                }
                // If there is a potential longer match, we don't do anything yet, just wait for the next key.
                return Ok(());
            }

            // If we are here, the sequence did not match any command directly.
            // Check if it's a prefix of any command.
            let has_potential_match = current_keymap
                .keys()
                .any(|k| k.starts_with(&self.pending_sequence));

            if !has_potential_match {
                // No potential matches, clear the sequence.
                debug!(
                    "Key sequence '{}' has no potential matches, clearing sequence",
                    self.pending_sequence
                );
                self.pending_sequence.clear();
                // Clear pending count as well since the sequence is invalid
                if self.pending_count.is_some() {
                    debug!("Clearing pending count due to no match");
                }
                self.pending_count = None;
            }

            return Ok(());
        }

        // For modes other than Normal and OperatorPending, handle simple key mapping
        if !matches!(editor.mode(), Mode::Normal | Mode::OperatorPending) {
            let action = match editor.mode() {
                Mode::Insert => {
                    if let KeyCode::Char(_) = key.code {
                        if !key.modifiers.contains(KeyModifiers::CONTROL)
                            && !key.modifiers.contains(KeyModifiers::ALT)
                        {
                            self.keymap_config.insert_mode.get("Char")
                        } else {
                            self.keymap_config.insert_mode.get(&key_string)
                        }
                    } else {
                        self.keymap_config.insert_mode.get(&key_string)
                    }
                }
                Mode::Command => {
                    if let KeyCode::Char(_) = key.code {
                        if !key.modifiers.contains(KeyModifiers::CONTROL)
                            && !key.modifiers.contains(KeyModifiers::ALT)
                        {
                            self.keymap_config.command_mode.get("Char")
                        } else {
                            self.keymap_config.command_mode.get(&key_string)
                        }
                    } else {
                        self.keymap_config.command_mode.get(&key_string)
                    }
                }
                Mode::Visual => self.keymap_config.visual_mode.get(&key_string),
                Mode::VisualLine => self.keymap_config.visual_line_mode.get(&key_string),
                Mode::VisualBlock => self.keymap_config.visual_block_mode.get(&key_string),
                Mode::Replace => {
                    if let KeyCode::Char(_) = key.code {
                        self.keymap_config.replace_mode.get("Char")
                    } else {
                        self.keymap_config.replace_mode.get(&key_string)
                    }
                }
                Mode::Search => {
                    if let KeyCode::Char(_) = key.code {
                        if !key.modifiers.contains(KeyModifiers::CONTROL)
                            && !key.modifiers.contains(KeyModifiers::ALT)
                        {
                            self.keymap_config.search_mode.get("Char")
                        } else {
                            self.keymap_config.search_mode.get(&key_string)
                        }
                    } else {
                        self.keymap_config.search_mode.get(&key_string)
                    }
                }
                _ => None, // Should not reach here
            };

            if let Some(action_name) = action {
                let action_name = action_name.clone();
                self.execute_action(editor, &action_name, key)?;
            }
        }

        Ok(())
    }

    fn key_event_to_string(key: KeyEvent) -> String {
        let mut result = String::new();

        // Add modifiers
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            result.push_str("Ctrl+");
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            result.push_str("Alt+");
        }
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            result.push_str("Shift+");
        }

        // Add the key itself
        match key.code {
            KeyCode::Char(c) => {
                // Don't add Shift+ for uppercase letters as they're already shifted
                if key.modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_lowercase() {
                    result.truncate(result.len() - 6); // Remove "Shift+"
                    result.push(c.to_ascii_uppercase());
                } else if key.modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_uppercase() {
                    result.truncate(result.len() - 6); // Remove "Shift+"
                    result.push(c);
                } else if key.modifiers.contains(KeyModifiers::SHIFT)
                    && "!@#$%^&*()_+{}|:\"<>?~".contains(c)
                {
                    // For shifted special characters, remove Shift+ as the character itself represents the shifted version
                    result.truncate(result.len() - 6); // Remove "Shift+"
                    result.push(c);
                } else {
                    result.push(c);
                }
            }
            KeyCode::Enter => result.push_str("Enter"),
            KeyCode::Left => result.push_str("Left"),
            KeyCode::Right => result.push_str("Right"),
            KeyCode::Up => result.push_str("Up"),
            KeyCode::Down => result.push_str("Down"),
            KeyCode::Backspace => result.push_str("Backspace"),
            KeyCode::Esc => result.push_str("Escape"),
            KeyCode::Tab => result.push_str("Tab"),
            KeyCode::Delete => result.push_str("Delete"),
            KeyCode::Home => result.push_str("Home"),
            KeyCode::End => result.push_str("End"),
            KeyCode::PageUp => result.push_str("PageUp"),
            KeyCode::PageDown => result.push_str("PageDown"),
            KeyCode::Insert => result.push_str("Insert"),
            KeyCode::F(n) => result.push_str(&format!("F{}", n)),
            _ => result.push_str("Unknown"),
        }

        result
    }

    fn execute_action(&mut self, editor: &mut Editor, action: &str, key: KeyEvent) -> Result<()> {
        // Record repeatable commands before executing
        if self.is_repeatable_action(action) {
            self.record_command(action, key);
        }

        self.execute_action_internal(editor, action, key)
    }

    fn execute_action_without_recording(
        &mut self,
        editor: &mut Editor,
        action: &str,
        key: KeyEvent,
    ) -> Result<()> {
        self.execute_action_internal(editor, action, key)
    }

    fn execute_action_internal(
        &mut self,
        editor: &mut Editor,
        action: &str,
        key: KeyEvent,
    ) -> Result<()> {
        // Peek and apply pending numeric count (default = 1); clear only when used
        let count = self.pending_count.unwrap_or(1);
        let mut used_count = false;
        if count > 1 {
            debug!("Applying count {} to '{}' when applicable", count, action);
        }
        // Record key event if macro recording is active (but not if we're executing macro actions)
        if editor.is_macro_recording()
            && !matches!(action, "start_macro_recording" | "execute_macro")
        {
            editor.record_macro_event(key);
        }

        match action {
            // Movement actions
            "cursor_left" => {
                used_count = true;
                for _ in 0..count {
                    self.action_cursor_left(editor)?;
                }
            }
            "cursor_right" => {
                used_count = true;
                for _ in 0..count {
                    self.action_cursor_right(editor)?;
                }
            }
            "cursor_up" => {
                used_count = true;
                for _ in 0..count {
                    self.action_cursor_up(editor)?;
                }
            }
            "cursor_down" => {
                used_count = true;
                for _ in 0..count {
                    self.action_cursor_down(editor)?;
                }
            }

            // Word movement
            "word_forward" => {
                used_count = true;
                for _ in 0..count {
                    self.action_word_forward(editor)?;
                }
            }
            "word_backward" => {
                used_count = true;
                for _ in 0..count {
                    self.action_word_backward(editor)?;
                }
            }
            "word_end" => {
                used_count = true;
                for _ in 0..count {
                    self.action_word_end(editor)?;
                }
            }

            // Character navigation
            "start_find_char_forward" => self.action_start_find_char_forward(editor)?,
            "start_find_char_backward" => self.action_start_find_char_backward(editor)?,
            "start_till_char_forward" => self.action_start_till_char_forward(editor)?,
            "start_till_char_backward" => self.action_start_till_char_backward(editor)?,
            "find_char_forward" => self.action_find_char_forward(editor, key)?,
            "find_char_backward" => self.action_find_char_backward(editor, key)?,
            "till_char_forward" => self.action_till_char_forward(editor, key)?,
            "till_char_backward" => self.action_till_char_backward(editor, key)?,
            "repeat_char_search" => self.action_repeat_char_search(editor)?,
            "repeat_char_search_reverse" => self.action_repeat_char_search_reverse(editor)?,

            // Bracket matching
            "bracket_match" => self.action_bracket_match(editor)?,

            // Paragraph movement
            "paragraph_forward" => self.action_paragraph_forward(editor)?,
            "paragraph_backward" => self.action_paragraph_backward(editor)?,

            // Sentence movement
            "sentence_forward" => self.action_sentence_forward(editor)?,
            "sentence_backward" => self.action_sentence_backward(editor)?,

            // Section movement
            "section_forward" => self.action_section_forward(editor)?,
            "section_backward" => self.action_section_backward(editor)?,

            // Repeat operations
            "repeat_last_change" => self.action_repeat_last_change(editor)?,

            // Delete operations
            "delete_char_at_cursor" => {
                used_count = true;
                for _ in 0..count {
                    self.action_delete_char_at_cursor(editor)?;
                }
            }
            "delete_char_before_cursor" => {
                used_count = true;
                for _ in 0..count {
                    self.action_delete_char_before_cursor(editor)?;
                }
            }
            "delete_line" => {
                used_count = true;
                for _ in 0..count {
                    self.action_delete_line(editor)?;
                }
            }
            "delete_to_end_of_line" => self.action_delete_to_end_of_line(editor)?,

            // Line operations
            "join_lines" => self.action_join_lines(editor)?,
            "change_to_end_of_line" => self.action_change_to_end_of_line(editor)?,
            "change_entire_line" => self.action_change_entire_line(editor)?,
            "substitute_char" => self.action_substitute_char(editor)?,

            // Yank (copy) operations
            "yank_line" => {
                used_count = true;
                for _ in 0..count {
                    self.action_yank_line(editor)?;
                }
            }
            "yank_word" => self.action_yank_word(editor)?,
            "yank_to_end_of_line" => self.action_yank_to_end_of_line(editor)?,

            // Put (paste) operations
            "put_after" => self.action_put_after(editor)?,
            "put_before" => self.action_put_before(editor)?,

            // Line movement
            "line_start" => self.action_line_start(editor)?,
            "line_end" => self.action_line_end(editor)?,
            "line_first_char" => self.action_line_start(editor)?, // Temporary fallback

            // Buffer movement
            "buffer_start" => self.action_buffer_start(editor)?,
            "buffer_end" => self.action_buffer_end(editor)?,

            // Mode transitions
            "insert_mode" => self.action_insert_mode(editor)?,
            "insert_line_start" => self.action_insert_line_start(editor)?,
            "insert_after" => self.action_insert_after(editor)?,
            "insert_line_end" => self.action_insert_line_end(editor)?,
            "insert_line_below" => self.action_insert_line_below(editor)?,
            "insert_line_above" => self.action_insert_line_above(editor)?,
            "normal_mode" => self.action_normal_mode(editor)?,
            "command_mode" => self.action_command_mode(editor)?,
            "visual_mode" => self.action_visual_mode(editor)?,
            "visual_line_mode" => self.action_visual_line_mode(editor)?,
            "visual_block_mode" => self.action_visual_block_mode(editor)?,
            "replace_mode" => self.action_replace_mode(editor)?,
            "search_forward" => self.action_search_forward(editor)?,
            "search_backward" => self.action_search_backward(editor)?,
            "search_next" => self.action_search_next(editor)?,
            "search_previous" => self.action_search_previous(editor)?,

            // File operations
            "save_file" => self.action_save_file(editor)?,
            "quit" => self.action_quit(editor)?,

            // Macro operations
            "start_macro_recording" => self.action_start_macro_recording(editor, key)?,
            "execute_macro" => self.action_execute_macro(editor, key)?,

            // Undo/Redo
            "undo" => self.action_undo(editor)?,
            "redo" => self.action_redo(editor)?,

            // Buffer management actions
            "buffer_next" => self.action_buffer_next(editor)?,
            "buffer_previous" => self.action_buffer_previous(editor)?,

            // Insert mode actions
            "insert_char" => self.action_insert_char(editor, key)?,
            "new_line" => self.action_new_line(editor)?,
            "delete_char" => self.action_delete_char(editor)?,
            "delete_char_forward" => self.action_delete_char_forward(editor)?,
            "delete_word_backward" => self.action_delete_word_backward(editor)?,
            "insert_tab" => self.action_insert_tab(editor)?,

            // Command mode actions
            "append_command" => self.action_append_command(editor, key)?,
            "delete_command_char" => self.action_delete_command_char(editor)?,
            "execute_command" => self.action_execute_command(editor)?,
            "command_complete" => self.action_command_complete(editor)?,
            "completion_next" => self.action_completion_next(editor)?,
            "completion_previous" => self.action_completion_previous(editor)?,
            "completion_accept" => self.action_completion_accept(editor)?,

            // Search mode actions
            "append_search" => self.action_append_search(editor, key)?,
            "delete_search_char" => self.action_delete_search_char(editor)?,
            "execute_search" => self.action_execute_search(editor)?,

            // Visual mode actions
            "delete_selection" => self.action_delete_selection(editor)?,
            "yank_selection" => self.action_yank_selection(editor)?,
            "change_selection" => self.action_change_selection(editor)?,

            // Replace mode actions
            "replace_char" => self.action_replace_char(editor, key)?,

            // Operator actions
            "operator_delete" => self.action_operator_delete(editor)?,
            "operator_change" => self.action_operator_change(editor)?,
            "operator_yank" => self.action_operator_yank(editor)?,
            "operator_indent" => self.action_operator_indent(editor)?,
            "operator_unindent" => self.action_operator_unindent(editor)?,
            "operator_toggle_case" => self.action_operator_toggle_case(editor)?,

            // Text object actions (for operator-pending mode)
            action if action.starts_with("text_object_") => {
                let text_object_str = action.strip_prefix("text_object_").unwrap_or("");
                self.action_text_object(editor, text_object_str)?;
            }

            // Scrolling actions
            "scroll_down_line" => self.action_scroll_down_line(editor)?,
            "scroll_up_line" => self.action_scroll_up_line(editor)?,
            "scroll_down_page" => self.action_scroll_down_page(editor)?,
            "scroll_up_page" => self.action_scroll_up_page(editor)?,
            "scroll_down_half_page" => self.action_scroll_down_half_page(editor)?,
            "scroll_up_half_page" => self.action_scroll_up_half_page(editor)?,

            // Centering actions (z commands)
            "center_cursor" => self.action_center_cursor(editor)?,
            "cursor_to_top" => self.action_cursor_to_top(editor)?,
            "cursor_to_bottom" => self.action_cursor_to_bottom(editor)?,

            // Window/Split actions
            "split_horizontal" => self.action_split_horizontal(editor)?,
            "split_vertical" => self.action_split_vertical(editor)?,
            "split_horizontal_above" => self.action_split_horizontal_above(editor)?,
            "split_horizontal_below" => self.action_split_horizontal_below(editor)?,
            "split_vertical_left" => self.action_split_vertical_left(editor)?,
            "split_vertical_right" => self.action_split_vertical_right(editor)?,
            "close_window" => self.action_close_window(editor)?,
            "window_left" => self.action_window_left(editor)?,
            "window_right" => self.action_window_right(editor)?,
            "window_up" => self.action_window_up(editor)?,
            "window_down" => self.action_window_down(editor)?,

            // Window resizing actions
            "resize_window_wider" => self.action_resize_window_wider(editor)?,
            "resize_window_narrower" => self.action_resize_window_narrower(editor)?,
            "resize_window_taller" => self.action_resize_window_taller(editor)?,
            "resize_window_shorter" => self.action_resize_window_shorter(editor)?,

            _ => return Ok(()), // Unknown action, ignore
        }
        if used_count {
            self.pending_count = None;
        }
        Ok(())
    }

    // Action implementations
    fn action_cursor_left(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut()
            && buffer.cursor.col > 0
        {
            buffer.cursor.col -= 1;
            // Update visual selection if in any visual mode
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_cursor_right(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut()
            && let Some(line) = buffer.get_line(buffer.cursor.row)
            && buffer.cursor.col < line.len()
        {
            buffer.cursor.col += 1;
            // Update visual selection if in any visual mode
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_cursor_up(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut()
            && buffer.cursor.row > 0
        {
            buffer.cursor.row -= 1;
            if let Some(line) = buffer.get_line(buffer.cursor.row) {
                buffer.cursor.col = buffer.cursor.col.min(line.len());
            }
            // Update visual selection if in any visual mode
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_cursor_down(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut()
            && buffer.cursor.row < buffer.lines.len() - 1
        {
            buffer.cursor.row += 1;
            if let Some(line) = buffer.get_line(buffer.cursor.row) {
                buffer.cursor.col = buffer.cursor.col.min(line.len());
            }
            // Update visual selection if in any visual mode
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_insert_mode(&self, editor: &mut Editor) -> Result<()> {
        editor.set_mode(Mode::Insert);
        Ok(())
    }

    fn action_normal_mode(&self, editor: &mut Editor) -> Result<()> {
        // Clear visual selection when returning to normal mode
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.clear_visual_selection();
        }
        editor.set_mode(Mode::Normal);
        // Clear any pending operator when returning to normal mode
        editor.clear_pending_operator();
        Ok(())
    }

    fn action_command_mode(&self, editor: &mut Editor) -> Result<()> {
        // Explicitly clear command line and set mode
        editor.set_command_line(String::new());
        editor.set_mode(Mode::Command);
        editor.set_command_line(":".to_string());
        Ok(())
    }

    fn action_search_forward(&self, editor: &mut Editor) -> Result<()> {
        editor.set_mode(Mode::Search);
        editor.set_command_line("/".to_string());
        Ok(())
    }

    fn action_insert_char(&self, editor: &mut Editor, key: KeyEvent) -> Result<()> {
        if let KeyCode::Char(ch) = key.code
            && let Some(buffer) = editor.current_buffer_mut()
        {
            buffer.insert_char(ch);
        }
        Ok(())
    }

    fn action_new_line(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.insert_line_break();
        }
        Ok(())
    }

    fn action_delete_char(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.delete_char();
        }
        Ok(())
    }

    fn action_append_command(&self, editor: &mut Editor, key: KeyEvent) -> Result<()> {
        if let KeyCode::Char(ch) = key.code {
            let mut command = editor.command_line().to_string();
            command.push(ch);
            editor.set_command_line(command);

            // Cancel completion when user types to trigger new completion
            editor.cancel_completion();
        }
        Ok(())
    }

    fn action_delete_command_char(&self, editor: &mut Editor) -> Result<()> {
        let mut command = editor.command_line().to_string();
        if command.len() > 1 {
            command.pop();
            editor.set_command_line(command);

            // Cancel completion when user deletes to trigger new completion
            editor.cancel_completion();
        }
        Ok(())
    }

    fn action_execute_command(&self, editor: &mut Editor) -> Result<()> {
        let command = editor.command_line().trim_start_matches(':').to_string();

        match command.as_str() {
            // Quit commands
            "q" | "quit" => editor.quit(),
            "q!" | "quit!" => editor.force_quit(),

            // Write/save commands
            "w" | "write" => {
                if let Some(buffer) = editor.current_buffer_mut() {
                    match buffer.save() {
                        Ok(_) => editor.set_status_message("File saved".to_string()),
                        Err(e) => editor.set_status_message(format!("Error saving: {}", e)),
                    }
                }
            }
            // Write and quit
            "wq" | "x" => {
                if let Some(buffer) = editor.current_buffer_mut() {
                    match buffer.save() {
                        Ok(_) => {
                            editor.set_status_message("File saved".to_string());
                            editor.quit();
                        }
                        Err(e) => editor.set_status_message(format!("Error saving: {}", e)),
                    }
                }
            }

            // Line number commands
            "set nu" | "set number" => editor.set_line_numbers(true, false),
            "set nonu" | "set nonumber" => editor.set_line_numbers(false, false),
            "set rnu" | "set relativenumber" => editor.set_line_numbers(false, true),
            "set nornu" | "set norelativenumber" => editor.set_line_numbers(true, false),
            "set nu rnu" | "set number relativenumber" => editor.set_line_numbers(true, true),

            // Cursor line commands
            "set cul" | "set cursorline" => editor.set_cursor_line(true),
            "set nocul" | "set nocursorline" => editor.set_cursor_line(false),

            // Buffer management commands
            "bn" | "bnext" => {
                if editor.switch_to_next_buffer() {
                    editor.set_status_message("Switched to next buffer".to_string());
                } else {
                    editor.set_status_message("No next buffer".to_string());
                }
            }
            "bd" | "bdelete" => match editor.close_current_buffer() {
                Ok(msg) => editor.set_status_message(msg),
                Err(e) => editor.set_status_message(format!("Error: {}", e)),
            },
            "bd!" | "bdelete!" => match editor.force_close_current_buffer() {
                Ok(msg) => editor.set_status_message(msg),
                Err(e) => editor.set_status_message(format!("Error: {}", e)),
            },
            "ls" | "buffers" => {
                let buffer_list = editor.list_buffers();
                editor.set_status_message(buffer_list);
            }

            // Window/Split commands
            "split" | "sp" => {
                let message = editor.split_horizontal();
                editor.set_status_message(message);
            }
            "vsplit" | "vsp" => {
                let message = editor.split_vertical();
                editor.set_status_message(message);
            }
            "close" => {
                let message = editor.close_window();
                editor.set_status_message(message);
            }

            _ => {
                // Handle :e filename and :b commands
                if let Some(filename) = command.strip_prefix("e ") {
                    let filename = filename.trim();
                    match editor.open_file(filename) {
                        Ok(msg) => editor.set_status_message(msg),
                        Err(e) => editor.set_status_message(format!("Error opening file: {}", e)),
                    }
                } else if let Some(buffer_ref) = command.strip_prefix("b ") {
                    let buffer_ref = buffer_ref.trim();
                    if let Ok(buffer_id) = buffer_ref.parse::<usize>() {
                        if editor.switch_to_buffer(buffer_id) {
                            editor.set_status_message(format!("Switched to buffer {}", buffer_id));
                        } else {
                            editor.set_status_message(format!("No buffer with ID {}", buffer_id));
                        }
                    } else {
                        // Try to switch by filename
                        if editor.switch_to_buffer_by_name(buffer_ref) {
                            editor
                                .set_status_message(format!("Switched to buffer '{}'", buffer_ref));
                        } else {
                            editor
                                .set_status_message(format!("No buffer matching '{}'", buffer_ref));
                        }
                    }
                } else if let Some(set_args) = command.strip_prefix("set ") {
                    self.handle_set_command(editor, set_args);
                } else {
                    editor.set_status_message(format!("Unknown command: {}", command));
                }
            }
        }

        editor.set_mode(Mode::Normal);
        editor.set_command_line(String::new());
        Ok(())
    }

    fn action_command_complete(&self, editor: &mut Editor) -> Result<()> {
        // Extract the command part (without the ':' prefix)
        let command_line = editor.command_line().to_string();
        if let Some(command_part) = command_line.strip_prefix(':') {
            let command_part = command_part.to_string();

            // If completion is not active, start it
            if !editor.is_completion_active() {
                editor.start_command_completion(&command_part);
            }

            // If we have matches, move to the next one
            if editor.completion_has_matches() {
                editor.completion_next();
            }
        }
        Ok(())
    }

    fn action_completion_next(&self, editor: &mut Editor) -> Result<()> {
        editor.completion_next();
        Ok(())
    }

    fn action_completion_previous(&self, editor: &mut Editor) -> Result<()> {
        editor.completion_previous();
        Ok(())
    }

    fn action_completion_accept(&self, editor: &mut Editor) -> Result<()> {
        if let Some(completed_text) = editor.completion_accept() {
            // Set the command line to the completed command with ':' prefix
            editor.set_command_line(format!(":{}", completed_text));
        }
        Ok(())
    }

    fn action_append_search(&self, editor: &mut Editor, key: KeyEvent) -> Result<()> {
        if let KeyCode::Char(ch) = key.code {
            let mut search = editor.command_line().to_string();
            search.push(ch);
            editor.set_command_line(search);
        }
        Ok(())
    }

    fn action_delete_search_char(&self, editor: &mut Editor) -> Result<()> {
        let mut search = editor.command_line().to_string();
        if search.len() > 1 {
            search.pop();
            editor.set_command_line(search);
        }
        Ok(())
    }

    fn action_execute_search(&self, editor: &mut Editor) -> Result<()> {
        let search_term = editor.command_line()[1..].to_string();
        if !search_term.is_empty() {
            editor.search(&search_term);
        }
        editor.set_mode(Mode::Normal);
        editor.set_command_line(String::new());
        Ok(())
    }

    // Additional action implementations
    fn action_line_start(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.cursor.col = 0;
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_line_end(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut() {
            if let Some(line) = buffer.get_line(buffer.cursor.row) {
                buffer.cursor.col = line.len();
            }
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_buffer_start(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.cursor.row = 0;
            buffer.cursor.col = 0;
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_buffer_end(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.cursor.row = buffer.lines.len().saturating_sub(1);
            if let Some(line) = buffer.get_line(buffer.cursor.row) {
                buffer.cursor.col = line.len();
            }
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_insert_line_start(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.cursor.col = 0;
        }
        editor.set_mode(Mode::Insert);
        Ok(())
    }

    fn action_insert_after(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut()
            && let Some(line) = buffer.get_line(buffer.cursor.row)
            && buffer.cursor.col < line.len()
        {
            buffer.cursor.col += 1;
        }
        editor.set_mode(Mode::Insert);
        Ok(())
    }

    fn action_insert_line_end(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut()
            && let Some(line) = buffer.get_line(buffer.cursor.row)
        {
            buffer.cursor.col = line.len();
        }
        editor.set_mode(Mode::Insert);
        Ok(())
    }

    fn action_insert_line_below(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            let row = buffer.cursor.row;
            buffer.lines.insert(row + 1, String::new());
            buffer.cursor.row = row + 1;
            buffer.cursor.col = 0;
            buffer.modified = true;
        }
        editor.set_mode(Mode::Insert);
        Ok(())
    }

    fn action_insert_line_above(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            let row = buffer.cursor.row;
            buffer.lines.insert(row, String::new());
            buffer.cursor.col = 0;
            buffer.modified = true;
        }
        editor.set_mode(Mode::Insert);
        Ok(())
    }

    fn action_visual_mode(&self, editor: &mut Editor) -> Result<()> {
        // If we're already in visual mode, exit to normal mode
        if editor.mode() == Mode::Visual {
            if let Some(buffer) = editor.current_buffer_mut() {
                buffer.clear_visual_selection();
            }
            editor.set_mode(Mode::Normal);
            debug!("Visual mode: toggled off, returning to normal mode");
        } else {
            // Start visual selection at current cursor position
            if let Some(buffer) = editor.current_buffer_mut() {
                buffer.start_visual_selection();
            }
            editor.set_mode(Mode::Visual);
            debug!("Visual mode: started selection at cursor position");
        }
        Ok(())
    }

    fn action_visual_line_mode(&self, editor: &mut Editor) -> Result<()> {
        debug!("Entering visual line mode");
        editor.set_mode(Mode::VisualLine);

        // Start line-wise visual selection on the current buffer
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.start_visual_line_selection();
            debug!("Started line-wise visual selection");
        }

        Ok(())
    }

    fn action_visual_block_mode(&self, editor: &mut Editor) -> Result<()> {
        debug!("Entering visual block mode");
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.start_visual_block_selection();
        }
        editor.set_mode(Mode::VisualBlock);
        Ok(())
    }

    fn action_replace_mode(&self, editor: &mut Editor) -> Result<()> {
        editor.set_mode(Mode::Replace);
        Ok(())
    }

    fn action_search_backward(&self, editor: &mut Editor) -> Result<()> {
        editor.set_mode(Mode::Search);
        editor.set_command_line("?".to_string());
        Ok(())
    }

    fn action_save_file(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            match buffer.save() {
                Ok(_) => editor.set_status_message("File saved".to_string()),
                Err(e) => editor.set_status_message(format!("Error saving: {}", e)),
            }
        }
        Ok(())
    }

    fn action_quit(&self, editor: &mut Editor) -> Result<()> {
        editor.quit();
        Ok(())
    }

    fn action_undo(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.undo();
        }
        Ok(())
    }

    fn action_redo(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.redo();
        }
        Ok(())
    }

    fn action_buffer_next(&self, editor: &mut Editor) -> Result<()> {
        editor.switch_to_next_buffer();
        Ok(())
    }

    fn action_buffer_previous(&self, editor: &mut Editor) -> Result<()> {
        editor.switch_to_previous_buffer();
        Ok(())
    }

    fn action_delete_char_forward(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut()
            && let Some(line) = buffer.lines.get_mut(buffer.cursor.row)
            && buffer.cursor.col < line.len()
        {
            line.remove(buffer.cursor.col);
            buffer.modified = true;
        }
        Ok(())
    }

    fn action_delete_word_backward(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut()
            && let Some(line) = buffer.lines.get_mut(buffer.cursor.row)
            && buffer.cursor.col > 0
        {
            // Find the start of the current word or previous word
            let mut pos = buffer.cursor.col;

            // Skip any whitespace before the cursor
            while pos > 0 && line.chars().nth(pos - 1).unwrap_or(' ').is_whitespace() {
                pos -= 1;
            }

            // Delete the word characters
            while pos > 0 && !line.chars().nth(pos - 1).unwrap_or(' ').is_whitespace() {
                pos -= 1;
            }

            // Remove the characters from pos to cursor
            line.drain(pos..buffer.cursor.col);
            buffer.cursor.col = pos;
            buffer.modified = true;
        }
        Ok(())
    }

    fn action_insert_tab(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.insert_char('\t');
        }
        Ok(())
    }

    fn action_replace_char(&self, editor: &mut Editor, key: KeyEvent) -> Result<()> {
        if let KeyCode::Char(ch) = key.code
            && let Some(buffer) = editor.current_buffer_mut()
            && let Some(line) = buffer.lines.get_mut(buffer.cursor.row)
            && buffer.cursor.col < line.len()
        {
            line.replace_range(buffer.cursor.col..buffer.cursor.col + 1, &ch.to_string());
            if buffer.cursor.col < line.len() {
                buffer.cursor.col += 1;
            }
            buffer.modified = true;
        }
        Ok(())
    }

    fn action_delete_selection(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            if let Some(deleted_text) = buffer.delete_selection() {
                info!("Visual mode: deleted {} characters", deleted_text.len());
                editor.set_status_message(format!("Deleted {} characters", deleted_text.len()));
            } else {
                warn!("Visual mode: no selection to delete");
                editor.set_status_message("No selection".to_string());
            }
        }
        // Return to normal mode after delete operation
        editor.set_mode(crate::core::mode::Mode::Normal);
        Ok(())
    }

    fn action_yank_selection(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            if let Some(yanked_text) = buffer.yank_selection() {
                info!("Visual mode: yanked {} characters", yanked_text.len());
                editor.set_status_message(format!("Yanked {} characters", yanked_text.len()));
            } else {
                warn!("Visual mode: no selection to yank");
                editor.set_status_message("No selection".to_string());
            }
        }
        // Return to normal mode after yank operation
        editor.set_mode(crate::core::mode::Mode::Normal);
        Ok(())
    }

    fn action_change_selection(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            if let Some(deleted_text) = buffer.delete_selection() {
                info!("Visual mode: changed {} characters", deleted_text.len());
                editor.set_status_message(format!("Changed {} characters", deleted_text.len()));
                // Enter insert mode after deleting selection
                editor.set_mode(crate::core::mode::Mode::Insert);
            } else {
                warn!("Visual mode: no selection to change");
                editor.set_status_message("No selection".to_string());
                // Still return to normal mode
                editor.set_mode(crate::core::mode::Mode::Normal);
            }
        } else {
            editor.set_mode(crate::core::mode::Mode::Normal);
        }
        Ok(())
    }

    fn action_word_forward(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.move_to_next_word();
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_word_backward(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.move_to_previous_word();
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_word_end(&self, editor: &mut Editor) -> Result<()> {
        let is_visual_mode = matches!(
            editor.mode(),
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock
        );
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.move_to_word_end();
            if is_visual_mode {
                buffer.update_visual_selection(buffer.cursor);
            }
        }
        Ok(())
    }

    fn action_paragraph_forward(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            let mut current_row = buffer.cursor.row;
            let total_lines = buffer.lines.len();

            // Skip current paragraph (non-empty lines)
            while current_row < total_lines {
                if let Some(line) = buffer.get_line(current_row)
                    && line.trim().is_empty()
                {
                    break;
                }
                current_row += 1;
            }

            // Skip empty lines to find start of next paragraph
            while current_row < total_lines {
                if let Some(line) = buffer.get_line(current_row)
                    && !line.trim().is_empty()
                {
                    break;
                }
                current_row += 1;
            }

            // If we've reached the end, go to the last line
            if current_row >= total_lines {
                current_row = total_lines.saturating_sub(1);
            }

            buffer.cursor.row = current_row;
            buffer.cursor.col = 0;
        }
        Ok(())
    }

    fn action_paragraph_backward(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            let mut current_row = buffer.cursor.row;

            // Skip current paragraph (non-empty lines) going backward
            while current_row > 0 {
                current_row -= 1;
                if let Some(line) = buffer.get_line(current_row)
                    && line.trim().is_empty()
                {
                    break;
                }
            }

            // Skip empty lines to find start of previous paragraph
            while current_row > 0 {
                if let Some(line) = buffer.get_line(current_row)
                    && !line.trim().is_empty()
                {
                    break;
                }
                current_row -= 1;
            }

            // Find the start of this paragraph
            while current_row > 0 {
                if let Some(line) = buffer.get_line(current_row - 1)
                    && line.trim().is_empty()
                {
                    break;
                }
                current_row -= 1;
            }

            buffer.cursor.row = current_row;
            buffer.cursor.col = 0;
        }
        Ok(())
    }

    fn action_sentence_forward(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            let current_row = buffer.cursor.row;
            let current_col = buffer.cursor.col;

            debug!(
                "Moving to next sentence from {}:{}",
                current_row, current_col
            );

            // Convert buffer to string for comprehensive sentence detection
            let mut buffer_text = String::new();
            let mut row_col_map = Vec::new(); // Maps string positions to (row, col)

            for (row_idx, line) in buffer.lines.iter().enumerate() {
                for (col_idx, ch) in line.chars().enumerate() {
                    row_col_map.push((row_idx, col_idx));
                    buffer_text.push(ch);
                }
                // Add newline character and track its position
                row_col_map.push((row_idx, line.len()));
                buffer_text.push('\n');
            }

            // Find current position in string
            let mut current_pos = 0;
            for (pos, &(row, col)) in row_col_map.iter().enumerate() {
                if row == current_row && col == current_col {
                    current_pos = pos;
                    break;
                }
            }

            // Start searching from the position after current cursor
            let chars: Vec<char> = buffer_text.chars().collect();

            // Method 1: Try empty line detection first (for LICENSE-like files)
            let mut search_pos = current_pos + 1;

            while search_pos < chars.len() {
                let ch = chars[search_pos];

                if ch == '\n' {
                    // Check if this is an empty line (next char is also newline or end of text)
                    if search_pos + 1 >= chars.len() || chars[search_pos + 1] == '\n' {
                        search_pos += 1; // Skip the first newline

                        // Skip any additional empty lines
                        while search_pos < chars.len() && chars[search_pos] == '\n' {
                            search_pos += 1;
                        }

                        // Now find the start of the next non-empty line
                        while search_pos < chars.len()
                            && chars[search_pos].is_whitespace()
                            && chars[search_pos] != '\n'
                        {
                            search_pos += 1;
                        }

                        // If we found content after empty lines, this is our target
                        if search_pos < chars.len()
                            && search_pos < row_col_map.len()
                            && chars[search_pos] != '\n'
                        {
                            let (new_row, new_col) = row_col_map[search_pos];
                            buffer.cursor.row = new_row;
                            buffer.cursor.col = new_col;
                            info!(
                                "Moved to next sentence (empty line) at {}:{}",
                                new_row, new_col
                            );
                            return Ok(());
                        }
                    }
                }
                search_pos += 1;
            }

            // Method 2: Try traditional sentence endings (.!?) followed by whitespace
            search_pos = current_pos + 1;
            while search_pos < chars.len() {
                let ch = chars[search_pos];

                if ch == '.' || ch == '!' || ch == '?' {
                    // Look ahead for whitespace or end of text
                    let mut next_pos = search_pos + 1;

                    // Skip any additional punctuation
                    while next_pos < chars.len()
                        && (chars[next_pos] == '.'
                            || chars[next_pos] == '!'
                            || chars[next_pos] == '?')
                    {
                        next_pos += 1;
                    }

                    // Check if followed by whitespace
                    if next_pos >= chars.len() || chars[next_pos].is_whitespace() {
                        // Skip whitespace to find start of next sentence
                        while next_pos < chars.len() && chars[next_pos].is_whitespace() {
                            next_pos += 1;
                        }

                        if next_pos < chars.len() && next_pos < row_col_map.len() {
                            let (new_row, new_col) = row_col_map[next_pos];
                            buffer.cursor.row = new_row;
                            buffer.cursor.col = new_col;
                            info!(
                                "Moved to next sentence (punctuation) at {}:{}",
                                new_row, new_col
                            );
                            return Ok(());
                        }
                    }
                }
                search_pos += 1;
            }

            // Method 3: Look for double spaces (common in formatted text)
            search_pos = current_pos + 1;
            while search_pos + 1 < chars.len() {
                if chars[search_pos] == ' ' && chars[search_pos + 1] == ' ' {
                    // Found double space, look for next non-whitespace
                    let mut content_pos = search_pos + 2;
                    while content_pos < chars.len() && chars[content_pos].is_whitespace() {
                        content_pos += 1;
                    }

                    if content_pos < chars.len() && content_pos < row_col_map.len() {
                        let (new_row, new_col) = row_col_map[content_pos];
                        buffer.cursor.row = new_row;
                        buffer.cursor.col = new_col;
                        info!(
                            "Moved to next sentence (double space) at {}:{}",
                            new_row, new_col
                        );
                        return Ok(());
                    }
                }
                search_pos += 1;
            }

            // If no sentence found, go to end of buffer
            let total_lines = buffer.lines.len();
            if total_lines > 0 {
                buffer.cursor.row = total_lines - 1;
                if let Some(last_line) = buffer.get_line(total_lines - 1) {
                    buffer.cursor.col = last_line.len();
                }
            }
            info!("No next sentence found, moved to end of buffer");
        }
        Ok(())
    }

    fn action_sentence_backward(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            let current_row = buffer.cursor.row;
            let current_col = buffer.cursor.col;

            // Convert buffer to one big string for easier processing
            let mut all_text = String::new();
            let mut line_starts = vec![0]; // Track where each line starts in the big string

            for (i, line) in buffer.lines.iter().enumerate() {
                if i > 0 {
                    all_text.push('\n');
                    line_starts.push(all_text.len());
                }
                all_text.push_str(line);
            }

            // Calculate current position in the big string
            let current_pos = line_starts[current_row] + current_col;

            // Find all sentence starts in the text
            let mut sentence_starts = vec![0]; // Buffer always starts with a sentence
            let chars: Vec<char> = all_text.chars().collect();

            // Method 1: Find sentences ending with punctuation
            for i in 0..chars.len() {
                if chars[i] == '.' || chars[i] == '!' || chars[i] == '?' {
                    // Skip consecutive punctuation
                    let mut j = i + 1;
                    while j < chars.len() && (chars[j] == '.' || chars[j] == '!' || chars[j] == '?')
                    {
                        j += 1;
                    }

                    // Skip whitespace to find start of next sentence
                    while j < chars.len() && chars[j].is_whitespace() {
                        j += 1;
                    }

                    // If we found a non-whitespace character, it's a sentence start
                    if j < chars.len() {
                        sentence_starts.push(j);
                    }
                }
            }

            // Method 2: Find sentences separated by empty lines (for cases like LICENSE files)
            let mut i = 0;
            while i < chars.len() {
                if chars[i] == '\n' {
                    // Check if this is the start of an empty line
                    let mut j = i + 1;

                    // Skip whitespace on this line
                    while j < chars.len() && chars[j] != '\n' && chars[j].is_whitespace() {
                        j += 1;
                    }

                    // If we reach another newline, this was an empty (or whitespace-only) line
                    if j < chars.len() && chars[j] == '\n' {
                        // Now skip any additional empty lines
                        while j < chars.len() && chars[j] == '\n' {
                            j += 1;
                            // Skip whitespace on next line
                            while j < chars.len() && chars[j] != '\n' && chars[j].is_whitespace() {
                                j += 1;
                            }
                            // If this line has content, we found start of next sentence
                            if j < chars.len() && chars[j] != '\n' {
                                sentence_starts.push(j);
                                break;
                            }
                        }
                        i = j;
                        continue;
                    }
                }
                i += 1;
            }

            // Method 3: Find sentences with double spaces
            for i in 0..(chars.len().saturating_sub(2)) {
                if chars[i] == ' ' && chars[i + 1] == ' ' && !chars[i + 2].is_whitespace() {
                    sentence_starts.push(i + 2);
                }
            }

            // Remove duplicates and sort
            sentence_starts.sort();
            sentence_starts.dedup();

            // Find the sentence start to move to
            let mut target_pos = 0;
            for &start in sentence_starts.iter().rev() {
                if start < current_pos {
                    target_pos = start;
                    break;
                }
            }

            // Convert back to row/col coordinates
            let mut target_row = 0;
            let mut target_col = target_pos;

            for (i, &line_start) in line_starts.iter().enumerate() {
                if target_pos >= line_start {
                    target_row = i;
                    target_col = target_pos - line_start;
                } else {
                    break;
                }
            }

            buffer.cursor.row = target_row;
            buffer.cursor.col = target_col;
        }
        Ok(())
    }

    fn action_section_forward(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            let total_lines = buffer.lines.len();
            let mut current_row = buffer.cursor.row + 1; // Start from next line

            debug!("Moving to next section from line {}", buffer.cursor.row);

            // Look for section markers (lines starting with specific patterns)
            while current_row < total_lines {
                if let Some(line) = buffer.get_line(current_row) {
                    let trimmed = line.trim_start();

                    // Check for top-level section markers (prioritize broader structures)
                    if trimmed.starts_with("# ") ||        // Markdown header
                       trimmed.starts_with("## ") ||       // Markdown subheader
                       trimmed.starts_with("### ") ||      // Markdown subsubheader
                       trimmed.starts_with("class ") ||    // Class definition
                       trimmed.starts_with("impl ") ||     // Rust impl block
                       trimmed.starts_with("mod ") ||      // Rust module
                       trimmed.starts_with("pub struct ") || // Rust public struct
                       trimmed.starts_with("struct ") ||   // Struct definition
                       trimmed.starts_with("enum ") ||     // Enum definition
                       trimmed.starts_with("trait ") ||    // Rust trait
                       trimmed.starts_with("function ") || // JavaScript/TypeScript function
                       (trimmed.starts_with("fn ") && !line.starts_with("    ")) || // Top-level Rust function (not indented)
                       (trimmed.starts_with("pub fn ") && !line.starts_with("    ")) || // Top-level public function
                       (trimmed.starts_with("def ") && !line.starts_with("    ")) || // Top-level Python function
                       (line.starts_with('{') && line.trim().len() == 1)
                    // Opening brace alone
                    {
                        buffer.cursor.row = current_row;
                        buffer.cursor.col = 0;
                        info!("Moved to next section at line {}", current_row);
                        return Ok(());
                    }
                }
                current_row += 1;
            }

            // If no section found, go to end of buffer
            if total_lines > 0 {
                buffer.cursor.row = total_lines - 1;
                buffer.cursor.col = 0;
            }
            info!("No next section found, moved to end of buffer");
        }
        Ok(())
    }

    fn action_section_backward(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            let mut current_row = buffer.cursor.row;

            debug!("Moving to previous section from line {}", current_row);

            // Start from current line and go backwards
            loop {
                if current_row == 0 {
                    // Already at start, can't go further back
                    buffer.cursor.row = 0;
                    buffer.cursor.col = 0;
                    info!("Already at start of buffer");
                    return Ok(());
                }

                current_row -= 1;

                if let Some(line) = buffer.get_line(current_row) {
                    let trimmed = line.trim_start();

                    // Check for top-level section markers (prioritize broader structures)
                    if trimmed.starts_with("# ") ||        // Markdown header
                       trimmed.starts_with("## ") ||       // Markdown subheader
                       trimmed.starts_with("### ") ||      // Markdown subsubheader
                       trimmed.starts_with("class ") ||    // Class definition
                       trimmed.starts_with("impl ") ||     // Rust impl block
                       trimmed.starts_with("mod ") ||      // Rust module
                       trimmed.starts_with("pub struct ") || // Rust public struct
                       trimmed.starts_with("struct ") ||   // Struct definition
                       trimmed.starts_with("enum ") ||     // Enum definition
                       trimmed.starts_with("trait ") ||    // Rust trait
                       trimmed.starts_with("function ") || // JavaScript/TypeScript function
                       (trimmed.starts_with("fn ") && !line.starts_with("    ")) || // Top-level Rust function (not indented)
                       (trimmed.starts_with("pub fn ") && !line.starts_with("    ")) || // Top-level public function
                       (trimmed.starts_with("def ") && !line.starts_with("    ")) || // Top-level Python function
                       (line.starts_with('{') && line.trim().len() == 1)
                    // Opening brace alone
                    {
                        buffer.cursor.row = current_row;
                        buffer.cursor.col = 0;
                        info!("Moved to previous section at line {}", current_row);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    fn action_repeat_last_change(&mut self, editor: &mut Editor) -> Result<()> {
        if let Some(last_command) = &self.last_command.clone() {
            info!("Repeating last command: {}", last_command.action);
            // Execute the last command without recording it again to avoid infinite loops
            self.execute_action_without_recording(editor, &last_command.action, last_command.key)?;
            editor.set_status_message(format!("Repeated: {}", last_command.action));
        } else {
            editor.set_status_message("No command to repeat".to_string());
        }
        Ok(())
    }

    fn action_delete_char_at_cursor(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.delete_char_at_cursor();
        }
        Ok(())
    }

    fn action_delete_char_before_cursor(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.delete_char_before_cursor();
        }
        Ok(())
    }

    fn action_delete_line(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.delete_line();
        }
        // Clear pending operator and return to Normal mode after deleting line
        editor.clear_pending_operator();
        editor.set_mode(Mode::Normal);
        Ok(())
    }

    fn action_yank_line(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.yank_line();
            editor.set_status_message("Line yanked".to_string());
        }
        Ok(())
    }

    fn action_yank_word(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.yank_word();
            editor.set_status_message("Word yanked".to_string());
        }
        Ok(())
    }

    fn action_yank_to_end_of_line(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.yank_to_end_of_line();
            editor.set_status_message("Text yanked to end of line".to_string());
        }
        Ok(())
    }

    fn action_put_after(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.put_after();
            editor.set_status_message("Text pasted after cursor".to_string());
        }
        Ok(())
    }

    fn action_put_before(&self, editor: &mut Editor) -> Result<()> {
        if let Some(buffer) = editor.current_buffer_mut() {
            buffer.put_before();
            editor.set_status_message("Text pasted before cursor".to_string());
        }
        Ok(())
    }

    fn action_search_next(&self, editor: &mut Editor) -> Result<()> {
        editor.search_next();
        Ok(())
    }

    fn action_search_previous(&self, editor: &mut Editor) -> Result<()> {
        editor.search_previous();
        Ok(())
    }

    // Scrolling action implementations
    fn action_scroll_down_line(&self, editor: &mut Editor) -> Result<()> {
        editor.scroll_down_line();
        Ok(())
    }

    fn action_scroll_up_line(&self, editor: &mut Editor) -> Result<()> {
        editor.scroll_up_line();
        Ok(())
    }

    fn action_scroll_down_page(&self, editor: &mut Editor) -> Result<()> {
        editor.scroll_down_page();
        Ok(())
    }

    fn action_scroll_up_page(&self, editor: &mut Editor) -> Result<()> {
        editor.scroll_up_page();
        Ok(())
    }

    fn action_scroll_down_half_page(&self, editor: &mut Editor) -> Result<()> {
        editor.scroll_down_half_page();
        Ok(())
    }

    fn action_scroll_up_half_page(&self, editor: &mut Editor) -> Result<()> {
        editor.scroll_up_half_page();
        Ok(())
    }

    // Centering action implementations (z commands)
    fn action_center_cursor(&self, editor: &mut Editor) -> Result<()> {
        editor.center_cursor();
        Ok(())
    }

    fn action_cursor_to_top(&self, editor: &mut Editor) -> Result<()> {
        editor.cursor_to_top();
        Ok(())
    }

    fn action_cursor_to_bottom(&self, editor: &mut Editor) -> Result<()> {
        editor.cursor_to_bottom();
        Ok(())
    }

    // Window/Split action implementations
    fn action_split_horizontal(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.split_horizontal();
        editor.set_status_message(message);
        Ok(())
    }

    fn action_split_vertical(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.split_vertical();
        editor.set_status_message(message);
        Ok(())
    }

    fn action_split_horizontal_above(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.split_horizontal_above();
        editor.set_status_message(message);
        Ok(())
    }

    fn action_split_horizontal_below(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.split_horizontal_below();
        editor.set_status_message(message);
        Ok(())
    }

    fn action_split_vertical_left(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.split_vertical_left();
        editor.set_status_message(message);
        Ok(())
    }

    fn action_split_vertical_right(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.split_vertical_right();
        editor.set_status_message(message);
        Ok(())
    }

    fn action_close_window(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.close_window();
        editor.set_status_message(message);
        Ok(())
    }

    fn action_window_left(&self, editor: &mut Editor) -> Result<()> {
        if !editor.move_to_window_left() {
            editor.set_status_message("No window to the left".to_string());
        }
        Ok(())
    }

    fn action_window_right(&self, editor: &mut Editor) -> Result<()> {
        if !editor.move_to_window_right() {
            editor.set_status_message("No window to the right".to_string());
        }
        Ok(())
    }

    fn action_window_up(&self, editor: &mut Editor) -> Result<()> {
        if !editor.move_to_window_up() {
            editor.set_status_message("No window above".to_string());
        }
        Ok(())
    }

    fn action_window_down(&self, editor: &mut Editor) -> Result<()> {
        if !editor.move_to_window_down() {
            editor.set_status_message("No window below".to_string());
        }
        Ok(())
    }

    // Window resizing action implementations
    fn action_resize_window_wider(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.resize_window_wider();
        editor.set_status_message(message);
        Ok(())
    }

    fn action_resize_window_narrower(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.resize_window_narrower();
        editor.set_status_message(message);
        Ok(())
    }

    fn action_resize_window_taller(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.resize_window_taller();
        editor.set_status_message(message);
        Ok(())
    }

    fn action_resize_window_shorter(&self, editor: &mut Editor) -> Result<()> {
        let message = editor.resize_window_shorter();
        editor.set_status_message(message);
        Ok(())
    }

    /// Handle generic :set commands
    fn handle_set_command(&self, editor: &mut Editor, args: &str) {
        let args = args.trim();

        // Handle empty set command - show some basic settings
        if args.is_empty() {
            let mut settings = Vec::new();
            settings.push(format!(
                "number: {}",
                editor.get_config_value("number").unwrap_or_default()
            ));
            settings.push(format!(
                "relativenumber: {}",
                editor
                    .get_config_value("relativenumber")
                    .unwrap_or_default()
            ));
            settings.push(format!(
                "cursorline: {}",
                editor.get_config_value("cursorline").unwrap_or_default()
            ));
            settings.push(format!(
                "tabstop: {}",
                editor.get_config_value("tabstop").unwrap_or_default()
            ));
            settings.push(format!(
                "expandtab: {}",
                editor.get_config_value("expandtab").unwrap_or_default()
            ));
            settings.push(format!(
                "wrap: {}",
                editor.get_config_value("wrap").unwrap_or_default()
            ));
            settings.push(format!(
                "linebreak: {}",
                editor.get_config_value("linebreak").unwrap_or_default()
            ));
            editor.set_status_message(format!("Current settings: {}", settings.join(", ")));
            return;
        }

        // Handle :set all command
        if args == "all" {
            let all_settings = [
                "number",
                "relativenumber",
                "cursorline",
                "tabstop",
                "expandtab",
                "autoindent",
                "ignorecase",
                "smartcase",
                "hlsearch",
                "incsearch",
                "wrap",
                "linebreak",
                "undolevels",
                "undofile",
                "backup",
                "swapfile",
                "autosave",
                "laststatus",
                "showcmd",
                "scrolloff",
                "sidescrolloff",
                "timeoutlen",
                "percentpathroot",
                "ppr",
                "colorscheme",
                "syntax",
            ];

            let mut all_values = Vec::new();
            for setting in &all_settings {
                if let Some(value) = editor.get_config_value(setting) {
                    all_values.push(format!("{}={}", setting, value));
                }
            }

            editor.set_status_message(format!("All settings: {}", all_values.join(" | ")));
            return;
        }

        // Handle query for specific setting (e.g., "set number?")
        if let Some(setting) = args.strip_suffix('?') {
            if let Some(value) = editor.get_config_value(setting) {
                editor.set_status_message(format!("{}: {}", setting, value));
            } else {
                editor.set_status_message(format!("Unknown setting: {}", setting));
            }
            return;
        }

        // Handle "no" prefix for disabling options
        if let Some(setting) = args.strip_prefix("no") {
            match setting {
                "number" | "nu" => {
                    editor.set_config_setting("number", "false");
                    let (_, relative) = editor.get_line_number_state();
                    editor.set_line_numbers(false, relative);
                }
                "relativenumber" | "rnu" => {
                    editor.set_config_setting("relativenumber", "false");
                    let (absolute, _) = editor.get_line_number_state();
                    editor.set_line_numbers(absolute, false);
                }
                "cursorline" | "cul" => {
                    editor.set_config_setting("cursorline", "false");
                    editor.set_cursor_line(false);
                }
                "ignorecase" | "ic" => {
                    editor.set_config_setting("ignorecase", "false");
                }
                "smartcase" | "scs" => {
                    editor.set_config_setting("smartcase", "false");
                }
                "hlsearch" | "hls" => {
                    editor.set_config_setting("hlsearch", "false");
                }
                "expandtab" | "et" => {
                    editor.set_config_setting("expandtab", "false");
                }
                "autoindent" | "ai" => {
                    editor.set_config_setting("autoindent", "false");
                }
                "incsearch" | "is" => {
                    editor.set_config_setting("incsearch", "false");
                }
                "wrap" => {
                    editor.set_config_setting("wrap", "false");
                }
                "linebreak" | "lbr" => {
                    editor.set_config_setting("linebreak", "false");
                }
                "undofile" | "udf" => {
                    editor.set_config_setting("undofile", "false");
                }
                "backup" | "bk" => {
                    editor.set_config_setting("backup", "false");
                }
                "swapfile" | "swf" => {
                    editor.set_config_setting("swapfile", "false");
                }
                "autosave" | "aw" => {
                    editor.set_config_setting("autosave", "false");
                }
                "laststatus" | "ls" => {
                    editor.set_config_setting("laststatus", "false");
                }
                "showcmd" | "sc" => {
                    editor.set_config_setting("showcmd", "false");
                }
                "syntax" | "syn" => {
                    editor.set_config_setting("syntax", "false");
                }
                "percentpathroot" | "ppr" => {
                    editor.set_config_setting("percentpathroot", "false");
                }
                _ => editor.set_status_message(format!("Unknown option: no{}", setting)),
            }
            return;
        }

        // Handle setting with values (e.g., "tabstop=4")
        if let Some((setting, value)) = args.split_once('=') {
            match setting.trim() {
                "tabstop" | "ts" => {
                    if let Ok(_width) = value.parse::<usize>() {
                        editor.set_config_setting("tabstop", value);
                        editor.set_status_message(format!("Tab width set to {}", value));
                    } else {
                        editor.set_status_message("Invalid tab width value".to_string());
                    }
                }
                "undolevels" | "ul" => {
                    if let Ok(_levels) = value.parse::<usize>() {
                        editor.set_config_setting("undolevels", value);
                        editor.set_status_message(format!("Undo levels set to {}", value));
                    } else {
                        editor.set_status_message("Invalid undo levels value".to_string());
                    }
                }
                "scrolloff" | "so" => {
                    if let Ok(_lines) = value.parse::<usize>() {
                        editor.set_config_setting("scrolloff", value);
                        editor.set_status_message(format!("Scroll offset set to {}", value));
                    } else {
                        editor.set_status_message("Invalid scroll offset value".to_string());
                    }
                }
                "sidescrolloff" | "siso" => {
                    if let Ok(_cols) = value.parse::<usize>() {
                        editor.set_config_setting("sidescrolloff", value);
                        editor.set_status_message(format!("Side scroll offset set to {}", value));
                    } else {
                        editor.set_status_message("Invalid side scroll offset value".to_string());
                    }
                }
                "timeoutlen" | "tm" => {
                    if let Ok(_timeout) = value.parse::<u64>() {
                        editor.set_config_setting("timeoutlen", value);
                        editor.set_status_message(format!("Command timeout set to {} ms", value));
                    } else {
                        editor.set_status_message("Invalid timeout value".to_string());
                    }
                }
                "colorscheme" | "colo" => {
                    editor.set_config_setting("colorscheme", value);
                    editor.set_status_message(format!("Color scheme set to {}", value));
                }
                "percentpathroot" | "ppr" => {
                    if let Ok(_b) = value.parse::<bool>() {
                        editor.set_config_setting("percentpathroot", value);
                        editor.set_status_message(format!("Percent path root set to {}", value));
                    } else {
                        editor.set_status_message("Invalid boolean value".to_string());
                    }
                }
                _ => editor.set_status_message(format!("Unknown setting: {}", setting)),
            }
            return;
        }

        // Handle boolean options (enable)
        match args {
            "number" | "nu" => {
                editor.set_config_setting("number", "true");
                let (_, relative) = editor.get_line_number_state();
                editor.set_line_numbers(true, relative);
            }
            "relativenumber" | "rnu" => {
                editor.set_config_setting("relativenumber", "true");
                let (absolute, _) = editor.get_line_number_state();
                editor.set_line_numbers(absolute, true);
            }
            "cursorline" | "cul" => {
                editor.set_config_setting("cursorline", "true");
                editor.set_cursor_line(true);
            }
            "ignorecase" | "ic" => {
                editor.set_config_setting("ignorecase", "true");
                editor.set_status_message("Ignore case enabled".to_string());
            }
            "smartcase" | "scs" => {
                editor.set_config_setting("smartcase", "true");
                editor.set_status_message("Smart case enabled".to_string());
            }
            "hlsearch" | "hls" => {
                editor.set_config_setting("hlsearch", "true");
                editor.set_status_message("Search highlighting enabled".to_string());
            }
            "expandtab" | "et" => {
                editor.set_config_setting("expandtab", "true");
                editor.set_status_message("Expand tabs enabled".to_string());
            }
            "autoindent" | "ai" => {
                editor.set_config_setting("autoindent", "true");
                editor.set_status_message("Auto indent enabled".to_string());
            }
            "incsearch" | "is" => {
                editor.set_config_setting("incsearch", "true");
                editor.set_status_message("Incremental search enabled".to_string());
            }
            "wrap" => {
                editor.set_config_setting("wrap", "true");
                editor.set_status_message("Line wrap enabled".to_string());
            }
            "linebreak" | "lbr" => {
                editor.set_config_setting("linebreak", "true");
                editor.set_status_message("Line break enabled".to_string());
            }
            "undofile" | "udf" => {
                editor.set_config_setting("undofile", "true");
                editor.set_status_message("Persistent undo enabled".to_string());
            }
            "backup" | "bk" => {
                editor.set_config_setting("backup", "true");
                editor.set_status_message("Backup files enabled".to_string());
            }
            "swapfile" | "swf" => {
                editor.set_config_setting("swapfile", "true");
                editor.set_status_message("Swap file enabled".to_string());
            }
            "autosave" | "aw" => {
                editor.set_config_setting("autosave", "true");
                editor.set_status_message("Auto save enabled".to_string());
            }
            "laststatus" | "ls" => {
                editor.set_config_setting("laststatus", "true");
                editor.set_status_message("Status line enabled".to_string());
            }
            "showcmd" | "sc" => {
                editor.set_config_setting("showcmd", "true");
                editor.set_status_message("Show command enabled".to_string());
            }
            "syntax" | "syn" => {
                editor.set_config_setting("syntax", "true");
                editor.set_status_message("Syntax highlighting enabled".to_string());
            }
            "percentpathroot" | "ppr" => {
                editor.set_config_setting("percentpathroot", "true");
                editor.set_status_message("Percent path root enabled".to_string());
            }
            _ => editor.set_status_message(format!("Unknown option: {}", args)),
        }
    }

    // Operator action implementations
    fn action_operator_delete(&self, editor: &mut Editor) -> Result<()> {
        editor.set_pending_operator(crate::core::editor::PendingOperator::Delete);
        Ok(())
    }

    fn action_operator_change(&self, editor: &mut Editor) -> Result<()> {
        editor.set_pending_operator(crate::core::editor::PendingOperator::Change);
        Ok(())
    }

    fn action_operator_yank(&self, editor: &mut Editor) -> Result<()> {
        editor.set_pending_operator(crate::core::editor::PendingOperator::Yank);
        Ok(())
    }

    fn action_operator_indent(&self, editor: &mut Editor) -> Result<()> {
        editor.set_pending_operator(crate::core::editor::PendingOperator::Indent);
        Ok(())
    }

    fn action_operator_unindent(&self, editor: &mut Editor) -> Result<()> {
        editor.set_pending_operator(crate::core::editor::PendingOperator::Unindent);
        Ok(())
    }

    fn action_operator_toggle_case(&self, editor: &mut Editor) -> Result<()> {
        editor.set_pending_operator(crate::core::editor::PendingOperator::ToggleCase);
        Ok(())
    }

    fn action_text_object(&self, editor: &mut Editor, text_object_str: &str) -> Result<()> {
        debug!("action_text_object called with: {}", text_object_str);
        if editor.get_pending_operator().is_some() {
            debug!(
                "Found pending operator: {:?}",
                editor.get_pending_operator()
            );
            // Execute the operator with the text object
            if editor.execute_operator_with_text_object(text_object_str)? {
                debug!(
                    "Successfully executed operator with text object: {}",
                    text_object_str
                );
            } else {
                debug!(
                    "Failed to execute operator with text object: {}",
                    text_object_str
                );
                editor.clear_pending_operator();
            }
        } else {
            debug!("No pending operator for text object: {}", text_object_str);
        }

        Ok(())
    }

    // Character navigation start actions
    fn action_start_find_char_forward(&mut self, _editor: &mut Editor) -> Result<()> {
        debug!("Starting find character forward command");
        self.pending_char_command = Some(PendingCharCommand {
            search_type: CharSearchType::Find,
            forward: true,
        });
        Ok(())
    }

    fn action_start_find_char_backward(&mut self, _editor: &mut Editor) -> Result<()> {
        debug!("Starting find character backward command");
        self.pending_char_command = Some(PendingCharCommand {
            search_type: CharSearchType::Find,
            forward: false,
        });
        Ok(())
    }

    fn action_start_till_char_forward(&mut self, _editor: &mut Editor) -> Result<()> {
        debug!("Starting till character forward command");
        self.pending_char_command = Some(PendingCharCommand {
            search_type: CharSearchType::Till,
            forward: true,
        });
        Ok(())
    }

    fn action_start_till_char_backward(&mut self, _editor: &mut Editor) -> Result<()> {
        debug!("Starting till character backward command");
        self.pending_char_command = Some(PendingCharCommand {
            search_type: CharSearchType::Till,
            forward: false,
        });
        Ok(())
    }

    // Character navigation actions
    fn action_find_char_forward(&mut self, editor: &mut Editor, key: KeyEvent) -> Result<()> {
        if let KeyCode::Char(ch) = key.code {
            debug!("Finding character '{}' forward on current line", ch);
            if let Some(buffer) = editor.current_buffer_mut() {
                let cursor = buffer.cursor;
                if let Some(line_text) = buffer.get_line(cursor.row) {
                    // Search for character after current cursor position
                    if let Some(pos) = line_text.chars().skip(cursor.col + 1).position(|c| c == ch)
                    {
                        let new_column = cursor.col + 1 + pos;
                        buffer.cursor.col = new_column;

                        // Store this search for repeat operations
                        self.last_char_search = Some(CharSearchState {
                            search_type: CharSearchType::Find,
                            character: ch,
                            forward: true,
                        });

                        info!("Found character '{}' at column {}", ch, new_column);
                    } else {
                        debug!("Character '{}' not found forward on current line", ch);
                    }
                }
            }
        }
        Ok(())
    }

    fn action_find_char_backward(&mut self, editor: &mut Editor, key: KeyEvent) -> Result<()> {
        if let KeyCode::Char(ch) = key.code {
            debug!("Finding character '{}' backward on current line", ch);
            if let Some(buffer) = editor.current_buffer_mut() {
                let cursor = buffer.cursor;
                if let Some(line_text) = buffer.get_line(cursor.row) {
                    // Search for character before current cursor position
                    if let Some(pos) = line_text
                        .chars()
                        .take(cursor.col)
                        .collect::<String>()
                        .rfind(ch)
                    {
                        buffer.cursor.col = pos;

                        // Store this search for repeat operations
                        self.last_char_search = Some(CharSearchState {
                            search_type: CharSearchType::Find,
                            character: ch,
                            forward: false,
                        });

                        info!("Found character '{}' at column {}", ch, pos);
                    } else {
                        debug!("Character '{}' not found backward on current line", ch);
                    }
                }
            }
        }
        Ok(())
    }

    fn action_till_char_forward(&mut self, editor: &mut Editor, key: KeyEvent) -> Result<()> {
        if let KeyCode::Char(ch) = key.code {
            debug!("Finding till character '{}' forward on current line", ch);
            if let Some(buffer) = editor.current_buffer_mut() {
                let cursor = buffer.cursor;
                if let Some(line_text) = buffer.get_line(cursor.row) {
                    // Search for character after current cursor position
                    let search_start = cursor.col + 1;
                    if let Some(pos) = line_text.chars().skip(search_start).position(|c| c == ch) {
                        let target_char_position = search_start + pos;
                        let new_column = target_char_position.saturating_sub(1); // Stop before the character
                        buffer.cursor.col = new_column;

                        // Store this search for repeat operations
                        self.last_char_search = Some(CharSearchState {
                            search_type: CharSearchType::Till,
                            character: ch,
                            forward: true,
                        });

                        info!("Till character '{}', stopped at column {}", ch, new_column);
                    } else {
                        debug!("Character '{}' not found forward on current line", ch);
                    }
                }
            }
        }
        Ok(())
    }

    fn action_till_char_backward(&mut self, editor: &mut Editor, key: KeyEvent) -> Result<()> {
        if let KeyCode::Char(ch) = key.code {
            debug!("Finding till character '{}' backward on current line", ch);
            if let Some(buffer) = editor.current_buffer_mut() {
                let cursor = buffer.cursor;
                if let Some(line_text) = buffer.get_line(cursor.row) {
                    // Search for character before current cursor position
                    if let Some(pos) = line_text
                        .chars()
                        .take(cursor.col)
                        .collect::<String>()
                        .rfind(ch)
                    {
                        let new_column = pos + 1; // Stop after the character
                        if new_column < line_text.chars().count() {
                            buffer.cursor.col = new_column;

                            // Store this search for repeat operations
                            self.last_char_search = Some(CharSearchState {
                                search_type: CharSearchType::Till,
                                character: ch,
                                forward: false,
                            });

                            info!("Till character '{}', stopped at column {}", ch, new_column);
                        }
                    } else {
                        debug!("Character '{}' not found backward on current line", ch);
                    }
                }
            }
        }
        Ok(())
    }

    fn action_repeat_char_search(&mut self, editor: &mut Editor) -> Result<()> {
        if let Some(ref search_state) = self.last_char_search.clone() {
            debug!(
                "Repeating character search: {:?} '{}' forward: {}",
                search_state.search_type, search_state.character, search_state.forward
            );

            // For repeat operations, we need special logic to avoid finding the same character again
            match (search_state.search_type, search_state.forward) {
                (CharSearchType::Find, true) => {
                    self.action_find_char_forward_repeat(editor, search_state.character)?
                }
                (CharSearchType::Find, false) => {
                    self.action_find_char_backward_repeat(editor, search_state.character)?
                }
                (CharSearchType::Till, true) => {
                    self.action_till_char_forward_repeat(editor, search_state.character)?
                }
                (CharSearchType::Till, false) => {
                    self.action_till_char_backward_repeat(editor, search_state.character)?
                }
            }
        } else {
            debug!("No previous character search to repeat");
        }
        Ok(())
    }

    fn action_repeat_char_search_reverse(&mut self, editor: &mut Editor) -> Result<()> {
        if let Some(ref search_state) = self.last_char_search.clone() {
            debug!(
                "Repeating character search in reverse: {:?} '{}' forward: {}",
                search_state.search_type, search_state.character, !search_state.forward
            );

            // For reverse repeat operations, we also need special logic
            match (search_state.search_type, search_state.forward) {
                (CharSearchType::Find, true) => {
                    self.action_find_char_backward_repeat(editor, search_state.character)?
                }
                (CharSearchType::Find, false) => {
                    self.action_find_char_forward_repeat(editor, search_state.character)?
                }
                (CharSearchType::Till, true) => {
                    self.action_till_char_backward_repeat(editor, search_state.character)?
                }
                (CharSearchType::Till, false) => {
                    self.action_till_char_forward_repeat(editor, search_state.character)?
                }
            }
        } else {
            debug!("No previous character search to repeat in reverse");
        }
        Ok(())
    }

    // Specialized repeat methods that handle cursor positioning correctly
    fn action_find_char_forward_repeat(&mut self, editor: &mut Editor, ch: char) -> Result<()> {
        debug!("Repeating find character '{}' forward", ch);
        if let Some(buffer) = editor.current_buffer_mut() {
            let cursor = buffer.cursor;
            if let Some(line_text) = buffer.get_line(cursor.row) {
                // For repeat, start searching from next position
                if let Some(pos) = line_text.chars().skip(cursor.col + 1).position(|c| c == ch) {
                    let new_column = cursor.col + 1 + pos;
                    buffer.cursor.col = new_column;
                    info!("Found character '{}' at column {}", ch, new_column);
                } else {
                    debug!("Character '{}' not found forward on current line", ch);
                }
            }
        }
        Ok(())
    }

    fn action_find_char_backward_repeat(&mut self, editor: &mut Editor, ch: char) -> Result<()> {
        debug!("Repeating find character '{}' backward", ch);
        if let Some(buffer) = editor.current_buffer_mut() {
            let cursor = buffer.cursor;
            if let Some(line_text) = buffer.get_line(cursor.row) {
                // For backward repeat, start searching from cursor.col - 1 to skip past current character
                if cursor.col > 0 {
                    let search_end = cursor.col; // Search up to but not including current position
                    if let Some(pos) = line_text
                        .chars()
                        .take(search_end)
                        .collect::<String>()
                        .rfind(ch)
                    {
                        buffer.cursor.col = pos;
                        info!("Found character '{}' at column {}", ch, pos);
                    } else {
                        debug!("Character '{}' not found backward on current line", ch);
                    }
                }
            }
        }
        Ok(())
    }

    fn action_till_char_forward_repeat(&mut self, editor: &mut Editor, ch: char) -> Result<()> {
        debug!("Repeating till character '{}' forward", ch);
        if let Some(buffer) = editor.current_buffer_mut() {
            let cursor = buffer.cursor;
            if let Some(line_text) = buffer.get_line(cursor.row) {
                // For till repeat, we need to skip past the current target character
                // Start searching from cursor + 2 to skip past the character we're currently "till"
                let search_start = cursor.col + 2;
                if let Some(pos) = line_text.chars().skip(search_start).position(|c| c == ch) {
                    let target_char_position = search_start + pos;
                    let new_column = target_char_position.saturating_sub(1);
                    buffer.cursor.col = new_column;
                    info!("Till character '{}', stopped at column {}", ch, new_column);
                } else {
                    debug!("Character '{}' not found forward on current line", ch);
                }
            }
        }
        Ok(())
    }

    fn action_till_char_backward_repeat(&mut self, editor: &mut Editor, ch: char) -> Result<()> {
        debug!("Repeating till character '{}' backward", ch);
        if let Some(buffer) = editor.current_buffer_mut() {
            let cursor = buffer.cursor;
            if let Some(line_text) = buffer.get_line(cursor.row) {
                // For till backward repeat, we need to skip past the character we're currently "till"
                // Since we're positioned after the target character, we need to search before cursor.col - 1
                if cursor.col > 1 {
                    let search_end = cursor.col - 1; // Skip past the current target character
                    if let Some(pos) = line_text
                        .chars()
                        .take(search_end)
                        .collect::<String>()
                        .rfind(ch)
                    {
                        let new_column = pos + 1;
                        if new_column < line_text.chars().count() {
                            buffer.cursor.col = new_column;
                            info!("Till character '{}', stopped at column {}", ch, new_column);
                        }
                    } else {
                        debug!("Character '{}' not found backward on current line", ch);
                    }
                } else {
                    debug!("At beginning of line, cannot search backward");
                }
            }
        }
        Ok(())
    }

    // Line operation actions
    fn action_delete_to_end_of_line(&self, editor: &mut Editor) -> Result<()> {
        debug!("Deleting to end of line");
        if let Some(buffer) = editor.current_buffer_mut()
            && let Some(line) = buffer.get_line(buffer.cursor.row)
        {
            if buffer.cursor.col < line.len() {
                // Use buffer's delete_range method for proper undo support
                let start = buffer.cursor;
                let end = Position::new(buffer.cursor.row, line.len());
                let deleted_text = buffer.delete_range(start, end);
                info!("Deleted text to end of line: '{}'", deleted_text);
            } else {
                debug!("Already at end of line, nothing to delete");
            }
        }
        Ok(())
    }

    fn action_join_lines(&self, editor: &mut Editor) -> Result<()> {
        debug!("Joining current line with next line");
        if let Some(buffer) = editor.current_buffer_mut() {
            if buffer.cursor.row + 1 < buffer.lines.len() {
                let current_row = buffer.cursor.row;
                let current_line = buffer.lines[current_row].clone();
                let next_line = buffer.lines[current_row + 1].clone();

                // Create the joined line
                let joined_line = format!("{} {}", current_line.trim_end(), next_line.trim_start());

                // Replace the current line with the joined line
                let start = Position::new(current_row, 0);
                let end = Position::new(current_row + 1, next_line.len());
                buffer.replace_range(start, end, &joined_line);

                info!("Joined lines: '{}' and '{}'", current_line, next_line);
                editor.set_status_message("Lines joined".to_string());
            } else {
                debug!("Cannot join: already at last line");
            }
        }
        Ok(())
    }

    fn action_change_to_end_of_line(&self, editor: &mut Editor) -> Result<()> {
        debug!("Changing to end of line (C command)");
        // First delete to end of line
        self.action_delete_to_end_of_line(editor)?;
        // Then enter insert mode
        self.action_insert_mode(editor)?;
        Ok(())
    }

    fn action_change_entire_line(&self, editor: &mut Editor) -> Result<()> {
        debug!("Changing entire line (S command)");
        if let Some(buffer) = editor.current_buffer_mut() {
            let current_row = buffer.cursor.row;
            if let Some(line) = buffer.get_line(current_row)
                && !line.is_empty()
            {
                // Replace entire line with empty string
                let start = Position::new(current_row, 0);
                let end = Position::new(current_row, line.len());
                buffer.replace_range(start, end, "");
            }
            // Move cursor to beginning of line and enter insert mode
            buffer.cursor.col = 0;
            self.action_insert_mode(editor)?;
        }
        Ok(())
    }

    fn action_substitute_char(&self, editor: &mut Editor) -> Result<()> {
        debug!("Substituting character (s command)");
        // Delete character at cursor
        self.action_delete_char_at_cursor(editor)?;
        // Enter insert mode
        self.action_insert_mode(editor)?;
        Ok(())
    }

    fn action_bracket_match(&self, editor: &mut Editor) -> Result<()> {
        debug!("Finding matching bracket (% command)");

        if let Some(buffer) = editor.current_buffer_mut() {
            let current_pos = Position::new(buffer.cursor.row, buffer.cursor.col);

            // Get the character at the cursor
            if let Some(line) = buffer.get_line(current_pos.row)
                && current_pos.col < line.len()
            {
                let char_at_cursor = line.chars().nth(current_pos.col).unwrap_or(' ');

                // Check if it's a bracket we can match
                let target_char = match char_at_cursor {
                    '(' => Some(')'),
                    ')' => Some('('),
                    '[' => Some(']'),
                    ']' => Some('['),
                    '{' => Some('}'),
                    '}' => Some('{'),
                    '<' => Some('>'),
                    '>' => Some('<'),
                    _ => None,
                };

                if let Some(target) = target_char {
                    let is_opening = matches!(char_at_cursor, '(' | '[' | '{' | '<');

                    if let Some(match_pos) = self.find_matching_bracket(
                        &buffer.lines,
                        current_pos,
                        char_at_cursor,
                        target,
                        is_opening,
                    ) {
                        // Move cursor to the matching bracket
                        buffer.cursor.row = match_pos.row;
                        buffer.cursor.col = match_pos.col;

                        info!(
                            "Found matching bracket '{}' at {}:{}",
                            target,
                            match_pos.row + 1,
                            match_pos.col + 1
                        );
                        editor.set_status_message(format!("Jumped to matching '{}'", target));
                    } else {
                        debug!("No matching bracket found for '{}'", char_at_cursor);
                        editor.set_status_message("No matching bracket found".to_string());
                    }
                } else {
                    debug!("Character '{}' is not a bracket", char_at_cursor);
                    editor.set_status_message("Not on a bracket".to_string());
                }
            }
        }
        Ok(())
    }

    fn find_matching_bracket(
        &self,
        lines: &[String],
        start_pos: Position,
        start_char: char,
        target_char: char,
        is_opening: bool,
    ) -> Option<Position> {
        let mut stack_count = 1;

        if is_opening {
            // Search forward for closing bracket
            let mut row = start_pos.row;
            let mut col = start_pos.col + 1; // Start from next character

            while row < lines.len() {
                let line = &lines[row];

                while col < line.len() {
                    if let Some(ch) = line.chars().nth(col) {
                        if ch == start_char {
                            stack_count += 1;
                        } else if ch == target_char {
                            stack_count -= 1;
                            if stack_count == 0 {
                                return Some(Position::new(row, col));
                            }
                        }
                    }
                    col += 1;
                }

                row += 1;
                col = 0; // Reset column for next line
            }
        } else {
            // Search backward for opening bracket
            let mut row = start_pos.row;
            let mut col = if start_pos.col > 0 {
                start_pos.col - 1
            } else {
                // If we're at position 0, we need to go to the previous line
                if row > 0 {
                    row -= 1;
                    if !lines[row].is_empty() {
                        lines[row].len() - 1
                    } else {
                        0
                    }
                } else {
                    // We're at position 0 of line 0, nowhere to go backward
                    return None;
                }
            };

            loop {
                let line = &lines[row];

                loop {
                    if let Some(ch) = line.chars().nth(col) {
                        if ch == start_char {
                            stack_count += 1;
                        } else if ch == target_char {
                            stack_count -= 1;
                            if stack_count == 0 {
                                return Some(Position::new(row, col));
                            }
                        }
                    }

                    if col == 0 {
                        break;
                    }
                    col -= 1;
                }

                if row == 0 {
                    break;
                }
                row -= 1;
                col = if !lines[row].is_empty() {
                    lines[row].len() - 1
                } else {
                    0
                };
            }
        }

        None
    }

    // Macro action methods
    fn action_start_macro_recording(&mut self, editor: &mut Editor, _key: KeyEvent) -> Result<()> {
        // 'q' toggles stop if already recording; otherwise arm register-pending
        if editor.is_macro_recording() {
            if let Ok(stopped_register) = editor.stop_macro_recording() {
                info!(
                    "Stopped macro recording for register '{}'",
                    stopped_register
                );
            }
            // Clear any pending to be safe
            self.pending_macro_register = false;
            self.pending_sequence.clear();
        } else {
            // Arm the pending register selection; consume next char as register
            self.pending_macro_register = true;
            // Reset sequence so the next key (e.g., 'a') doesn't match normal mappings
            self.pending_sequence.clear();
        }
        Ok(())
    }

    fn action_execute_macro(&mut self, _editor: &mut Editor, key: KeyEvent) -> Result<()> {
        // Pressing '@' should arm pending macro execution; next key chooses register or '@' for repeat
        if let KeyCode::Char('@') = key.code {
            self.pending_macro_execute = true;
            self.pending_sequence.clear();
            debug!("Armed pending macro execution; awaiting register or '@'");
        } else {
            // If mapping is triggered with other key (unlikely), fall back to arming as well
            self.pending_macro_execute = true;
            self.pending_sequence.clear();
        }
        Ok(())
    }

    // Note: previously had a helper to map KeyEvent to action; no longer needed after macro
    // playback replays through handle_key. If needed in the future, restore a similar helper.

    fn is_repeatable_action(&self, action: &str) -> bool {
        matches!(
            action,
            // Character operations
            "delete_char_at_cursor" |
            "delete_char_before_cursor" |
            "substitute_char" |
            // Line operations
            "delete_line" |
            "delete_to_end_of_line" |
            "change_to_end_of_line" |
            "change_entire_line" |
            "join_lines" |
            // Insert operations (when entering insert mode with text)
            "insert_mode" |
            "insert_after" |
            "insert_line_start" |
            "insert_line_end" |
            "insert_line_below" |
            "insert_line_above" |
            // Put operations
            "put_after" |
            "put_before" |
            // Text case operations (future implementation)
            "toggle_case"
        )
    }

    fn record_command(&mut self, action: &str, key: KeyEvent) {
        self.last_command = Some(RepeatableCommand {
            action: action.to_string(),
            key,
            // Capture the active count if any; execution already consumed it
            count: self.pending_count,
        });
        debug!("Recorded command for repeat: {}", action);
    }
}
