// Configuration management
// This handles editor.toml parsing and settings management

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    pub display: DisplayConfig,
    pub behavior: BehaviorConfig,
    pub editing: EditingConfig,
    pub interface: InterfaceConfig,
    pub languages: LanguageConfig,
    #[serde(default)]
    pub statusline: StatusLineConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub show_line_numbers: bool,
    pub show_relative_numbers: bool,
    pub show_cursor_line: bool,
    pub color_scheme: String,
    pub syntax_highlighting: bool,
    /// Show a mark indicator in the gutter/number column for lines that have a mark
    #[serde(default = "default_true")]
    pub show_marks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    pub tab_width: usize,
    pub expand_tabs: bool,
    pub auto_indent: bool,
    pub ignore_case: bool,
    pub smart_case: bool,
    pub highlight_search: bool,
    pub incremental_search: bool,
    pub wrap_lines: bool,
    pub line_break: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditingConfig {
    pub undo_levels: usize,
    pub persistent_undo: bool,
    pub backup: bool,
    pub swap_file: bool,
    pub auto_save: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceConfig {
    pub show_status_line: bool,
    pub command_timeout: u64,
    pub show_command: bool,
    pub scroll_off: usize,
    pub side_scroll_off: usize,
    pub window_resize_amount: u16,
    pub completion_menu_height: u16,
    /// Enable '%' prefix in file path completion to root at current buffer directory
    #[serde(default = "default_true")]
    pub percent_path_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusLineConfig {
    #[serde(default = "default_true")]
    pub show_indent: bool,
    #[serde(default = "default_true")]
    pub show_eol: bool,
    #[serde(default = "default_true")]
    pub show_encoding: bool,
    #[serde(default = "default_true")]
    pub show_type: bool,
    #[serde(default = "default_true")]
    pub show_macro: bool,
    #[serde(default = "default_true")]
    pub show_search: bool,
    #[serde(default = "default_true")]
    pub show_progress: bool,
    /// Separator text used between right-side statusline segments
    #[serde(default = "default_separator")]
    pub separator: String,
}

impl Default for StatusLineConfig {
    fn default() -> Self {
        Self {
            show_indent: true,
            show_eol: true,
            show_encoding: true,
            show_type: true,
            show_macro: true,
            show_search: true,
            show_progress: true,
            separator: default_separator(),
        }
    }
}

fn default_separator() -> String {
    "  ".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LanguageConfig {
    /// Mapping from file extension (no dot) to language name
    #[serde(default)]
    pub extensions: HashMap<String, String>,
    /// Disabled feature; kept for backward-compat. Empty by default.
    #[serde(default)]
    pub content_patterns: HashMap<String, Vec<String>>,
    /// Default language when detection fails
    #[serde(default)]
    pub default_language: Option<String>,
}

impl LanguageConfig {
    /// Detect language from file extension
    pub fn detect_language_from_extension(&self, file_path: &str) -> Option<String> {
        use std::path::Path;

        let extension = Path::new(file_path).extension()?.to_str()?;
        self.extensions.get(extension).cloned()
    }

    /// Detect language from file content patterns for unnamed files
    pub fn detect_language_from_content(&self, content: &str) -> Option<String> {
        for (language, patterns) in &self.content_patterns {
            let match_count = patterns
                .iter()
                .filter(|pattern| content.contains(*pattern))
                .count();

            // If we find at least 2 patterns for a language, consider it a match
            if match_count >= 2 {
                return Some(language.clone());
            }
        }
        None
    }

    /// Get all supported file extensions
    pub fn get_supported_extensions(&self) -> Vec<&String> {
        self.extensions.keys().collect()
    }

    /// Get all supported languages
    pub fn get_supported_languages(&self) -> Vec<&String> {
        self.extensions
            .values()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Check if any languages are configured
    pub fn has_language_support(&self) -> bool {
        !self.extensions.is_empty()
    }

    /// Get a fallback language when no specific language is detected
    pub fn get_fallback_language(&self) -> Option<String> {
        // Use explicit default_language if configured, otherwise fall back to first extension
        self.default_language
            .clone()
            .or_else(|| self.extensions.values().next().cloned())
    }
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            display: DisplayConfig {
                show_line_numbers: true,
                show_relative_numbers: false,
                show_cursor_line: false,
                color_scheme: "default".to_string(), // Match the theme in themes.toml
                syntax_highlighting: true,
                show_marks: true,
            },
            behavior: BehaviorConfig {
                tab_width: 4,
                expand_tabs: false,
                auto_indent: true,
                ignore_case: false,
                smart_case: false,
                highlight_search: true,
                incremental_search: true,
                wrap_lines: false,
                line_break: false,
            },
            editing: EditingConfig {
                undo_levels: 1000,
                persistent_undo: false,
                backup: false,
                swap_file: false,
                auto_save: false,
            },
            interface: InterfaceConfig {
                show_status_line: true,
                command_timeout: 1000,
                show_command: true,
                scroll_off: 0,
                side_scroll_off: 0,
                window_resize_amount: 1,
                completion_menu_height: 8,
                percent_path_root: true,
            },
            statusline: StatusLineConfig {
                show_indent: true,
                show_eol: true,
                show_encoding: true,
                show_type: true,
                show_macro: true,
                show_search: true,
                show_progress: true,
                separator: default_separator(),
            },
            languages: LanguageConfig::default(),
        }
    }
}

fn default_true() -> bool {
    true
}

impl EditorConfig {
    pub fn load() -> Self {
        debug!("Loading editor configuration");
        // Try to load from editor.toml file, fall back to defaults if not found
        if let Ok(config_content) = fs::read_to_string("editor.toml") {
            debug!("Found editor.toml file, attempting to parse");
            if let Ok(config) = toml::from_str(&config_content) {
                info!("Loaded editor configuration from editor.toml");
                return config;
            } else {
                warn!("Failed to parse editor.toml, falling back to default configuration");
            }
        } else {
            debug!("editor.toml not found, using default configuration");
        }

        // Fallback to default configuration
        debug!("Using default editor configuration");
        Self::default()
    }

    /// Persist a single setting to editor.toml by editing the file in-place, preserving comments/layout.
    /// Expects `setting` to be any accepted long or short name (e.g., "number" or "nu").
    pub fn persist_setting_in_place(
        &self,
        setting: &str,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::io::{Read, Write};
        use std::path::Path;

        // Map setting alias to (section, key, kind, normalized_value_string)
        #[derive(Copy, Clone)]
        enum Kind {
            Bool,
            Int,
            Str,
        }

        fn map(setting: &str, val: &str) -> Option<(&'static str, &'static str, Kind, String)> {
            // Normalize booleans to true/false; ints kept; strings kept
            let b = |s: &str| {
                s.eq_ignore_ascii_case("true") || s == "1" || s.eq_ignore_ascii_case("on")
            };
            match setting {
                // Display
                "number" | "nu" => Some((
                    "display",
                    "show_line_numbers",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "relativenumber" | "rnu" => Some((
                    "display",
                    "show_relative_numbers",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "cursorline" | "cul" => Some((
                    "display",
                    "show_cursor_line",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "showmarks" | "smk" => Some((
                    "display",
                    "show_marks",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "colorscheme" | "colo" => {
                    Some(("display", "color_scheme", Kind::Str, val.to_string()))
                }
                "syntax" | "syn" => Some((
                    "display",
                    "syntax_highlighting",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),

                // Behavior
                "tabstop" | "ts" => Some(("behavior", "tab_width", Kind::Int, val.to_string())),
                "expandtab" | "et" => Some((
                    "behavior",
                    "expand_tabs",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "autoindent" | "ai" => Some((
                    "behavior",
                    "auto_indent",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "ignorecase" | "ic" => Some((
                    "behavior",
                    "ignore_case",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "smartcase" | "scs" => Some((
                    "behavior",
                    "smart_case",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "hlsearch" | "hls" => Some((
                    "behavior",
                    "highlight_search",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "incsearch" | "is" => Some((
                    "behavior",
                    "incremental_search",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "wrap" => Some((
                    "behavior",
                    "wrap_lines",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "linebreak" | "lbr" => Some((
                    "behavior",
                    "line_break",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),

                // Editing
                "undolevels" | "ul" => Some(("editing", "undo_levels", Kind::Int, val.to_string())),
                "undofile" | "udf" => Some((
                    "editing",
                    "persistent_undo",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "backup" | "bk" => Some((
                    "editing",
                    "backup",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "swapfile" | "swf" => Some((
                    "editing",
                    "swap_file",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "autosave" | "aw" => Some((
                    "editing",
                    "auto_save",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),

                // Interface
                "laststatus" | "ls" => Some((
                    "interface",
                    "show_status_line",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "showcmd" | "sc" => Some((
                    "interface",
                    "show_command",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),
                "scrolloff" | "so" => Some(("interface", "scroll_off", Kind::Int, val.to_string())),
                "sidescrolloff" | "siso" => {
                    Some(("interface", "side_scroll_off", Kind::Int, val.to_string()))
                }
                "timeoutlen" | "tm" => {
                    Some(("interface", "command_timeout", Kind::Int, val.to_string()))
                }
                "percentpathroot" | "ppr" => Some((
                    "interface",
                    "percent_path_root",
                    Kind::Bool,
                    if b(val) { "true" } else { "false" }.to_string(),
                )),

                _ => None,
            }
        }

        // Determine target (section, key, kind, normalized value)
        let Some((section, key, kind, normalized)) = map(setting, value) else {
            // Unknown, nothing to persist
            return Ok(());
        };

        // Read or create file
        let path = Path::new("editor.toml");
        if !path.exists() {
            // Create minimal file with just the target section/key
            let value_literal = match kind {
                Kind::Bool | Kind::Int => normalized,
                Kind::Str => format!("\"{}\"", normalized),
            };
            let mut file = std::fs::File::create(path)?;
            let mut out = String::new();
            out.push_str(&format!("[{}]\n", section));
            out.push_str(&format!("{} = {}\n", key, value_literal));
            file.write_all(out.as_bytes())?;
            return Ok(());
        }

        let mut content = String::new();
        std::fs::File::open(path)?.read_to_string(&mut content)?;

        // Helper: find first '#' not inside quotes
        fn first_unquoted_hash(s: &str) -> Option<usize> {
            let mut in_str = false;
            let mut prev = '\0';
            for (i, ch) in s.char_indices() {
                if ch == '"' && prev != '\\' {
                    in_str = !in_str;
                }
                if ch == '#' && !in_str {
                    return Some(i);
                }
                prev = ch;
            }
            None
        }

        let value_literal = match kind {
            Kind::Bool | Kind::Int => normalized,
            Kind::Str => format!("\"{}\"", normalized),
        };

        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        let mut found_line_idx: Option<usize> = None;
        let mut section_found = false;
        let mut insert_idx_after_section: Option<usize> = None;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let name = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
                if section_found && found_line_idx.is_none() {
                    // We hit the start of a new section after the target section without finding the key
                    insert_idx_after_section = Some(i);
                    break;
                }
                if name == section {
                    section_found = true;
                }
                continue;
            }

            if section_found {
                let trimmed_start = line.trim_start();
                if trimmed_start.starts_with('#') {
                    continue;
                }
                if let Some(eq_pos) = trimmed_start.find('=') {
                    let lhs = trimmed_start[..eq_pos].trim();
                    if lhs == key {
                        found_line_idx = Some(i);
                        break;
                    }
                }
            }
        }

        if let Some(i) = found_line_idx {
            // Replace existing line preserving leading whitespace and trailing comment
            let original = &lines[i];
            let trimmed_start = original.trim_start();
            let leading_len = original.len() - trimmed_start.len();
            let leading = &original[..leading_len];
            let comment_part = if let Some(hash_i) = first_unquoted_hash(trimmed_start) {
                let abs_i = leading_len + hash_i;
                Some(original[abs_i..].to_string())
            } else {
                None
            };
            let new_core = format!("{} = {}", key, &value_literal);
            let new_line = match comment_part {
                Some(c) => format!("{}{} {}", leading, new_core, c),
                None => format!("{}{}", leading, new_core),
            };
            lines[i] = new_line;
        } else {
            // Need to insert the key
            if !section_found {
                // Section not present; add at end
                lines.push(String::new());
                lines.push(format!("[{}]", section));
                lines.push(format!("{} = {}", key, value_literal));
            } else if let Some(insert_at) = insert_idx_after_section {
                lines.insert(insert_at, format!("{} = {}", key, value_literal));
            } else {
                // Section is the last one; append at end
                lines.push(format!("{} = {}", key, value_literal));
            }
        }

        // Write back
        let mut file = std::fs::File::create(path)?;
        let out = lines.join("\n");
        file.write_all(out.as_bytes())?;
        Ok(())
    }

    /// Update a setting and return success status
    pub fn set_setting(&mut self, setting: &str, value: &str) -> Result<String, String> {
        debug!("Setting configuration: '{}' = '{}'", setting, value);
        match setting {
            // Display settings
            "number" | "nu" => {
                self.display.show_line_numbers = value.parse().unwrap_or(true);
                info!(
                    "Line numbers setting changed to: {}",
                    self.display.show_line_numbers
                );
                Ok(format!(
                    "Line numbers: {}",
                    if self.display.show_line_numbers {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "relativenumber" | "rnu" => {
                self.display.show_relative_numbers = value.parse().unwrap_or(false);
                Ok(format!(
                    "Relative line numbers: {}",
                    if self.display.show_relative_numbers {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "cursorline" | "cul" => {
                self.display.show_cursor_line = value.parse().unwrap_or(false);
                Ok(format!(
                    "Cursor line: {}",
                    if self.display.show_cursor_line {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }

            // Behavior settings
            "ignorecase" | "ic" => {
                self.behavior.ignore_case = value.parse().unwrap_or(false);
                Ok(format!(
                    "Ignore case: {}",
                    if self.behavior.ignore_case {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "smartcase" | "scs" => {
                self.behavior.smart_case = value.parse().unwrap_or(false);
                Ok(format!(
                    "Smart case: {}",
                    if self.behavior.smart_case {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "hlsearch" | "hls" => {
                self.behavior.highlight_search = value.parse().unwrap_or(true);
                Ok(format!(
                    "Search highlighting: {}",
                    if self.behavior.highlight_search {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "expandtab" | "et" => {
                self.behavior.expand_tabs = value.parse().unwrap_or(false);
                Ok(format!(
                    "Expand tabs: {}",
                    if self.behavior.expand_tabs {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "tabstop" | "ts" => {
                if let Ok(width) = value.parse::<usize>() {
                    self.behavior.tab_width = width;
                    Ok(format!("Tab width: {}", width))
                } else {
                    Err("Invalid tab width".to_string())
                }
            }
            "autoindent" | "ai" => {
                self.behavior.auto_indent = value.parse().unwrap_or(true);
                Ok(format!(
                    "Auto indent: {}",
                    if self.behavior.auto_indent {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "incsearch" | "is" => {
                self.behavior.incremental_search = value.parse().unwrap_or(true);
                Ok(format!(
                    "Incremental search: {}",
                    if self.behavior.incremental_search {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "wrap" => {
                self.behavior.wrap_lines = value.parse().unwrap_or(false);
                Ok(format!(
                    "Line wrap: {}",
                    if self.behavior.wrap_lines {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "linebreak" | "lbr" => {
                self.behavior.line_break = value.parse().unwrap_or(false);
                Ok(format!(
                    "Line break: {}",
                    if self.behavior.line_break {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }

            // Editing settings
            "undolevels" | "ul" => {
                if let Ok(levels) = value.parse::<usize>() {
                    self.editing.undo_levels = levels;
                    Ok(format!("Undo levels: {}", levels))
                } else {
                    Err("Invalid undo levels".to_string())
                }
            }
            "undofile" | "udf" => {
                self.editing.persistent_undo = value.parse().unwrap_or(false);
                Ok(format!(
                    "Persistent undo: {}",
                    if self.editing.persistent_undo {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "backup" | "bk" => {
                self.editing.backup = value.parse().unwrap_or(false);
                Ok(format!(
                    "Backup files: {}",
                    if self.editing.backup {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "swapfile" | "swf" => {
                self.editing.swap_file = value.parse().unwrap_or(false);
                Ok(format!(
                    "Swap file: {}",
                    if self.editing.swap_file {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "autosave" | "aw" => {
                self.editing.auto_save = value.parse().unwrap_or(false);
                Ok(format!(
                    "Auto save: {}",
                    if self.editing.auto_save {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }

            // Interface settings
            "laststatus" | "ls" => {
                self.interface.show_status_line = value.parse().unwrap_or(true);
                Ok(format!(
                    "Status line: {}",
                    if self.interface.show_status_line {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "showcmd" | "sc" => {
                self.interface.show_command = value.parse().unwrap_or(true);
                Ok(format!(
                    "Show command: {}",
                    if self.interface.show_command {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "scrolloff" | "so" => {
                if let Ok(lines) = value.parse::<usize>() {
                    self.interface.scroll_off = lines;
                    Ok(format!("Scroll offset: {}", lines))
                } else {
                    Err("Invalid scroll offset".to_string())
                }
            }
            "sidescrolloff" | "siso" => {
                if let Ok(cols) = value.parse::<usize>() {
                    self.interface.side_scroll_off = cols;
                    Ok(format!("Side scroll offset: {}", cols))
                } else {
                    Err("Invalid side scroll offset".to_string())
                }
            }
            "timeoutlen" | "tm" => {
                if let Ok(timeout) = value.parse::<u64>() {
                    self.interface.command_timeout = timeout;
                    Ok(format!("Command timeout: {} ms", timeout))
                } else {
                    Err("Invalid timeout value".to_string())
                }
            }
            "percentpathroot" | "ppr" => {
                self.interface.percent_path_root = value.parse().unwrap_or(true);
                Ok(format!(
                    "Percent path root: {}",
                    if self.interface.percent_path_root {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }

            // Display settings
            "colorscheme" | "colo" => {
                self.display.color_scheme = value.to_string();
                Ok(format!("Color scheme: {}", value))
            }
            "syntax" | "syn" => {
                self.display.syntax_highlighting = value.parse().unwrap_or(true);
                Ok(format!(
                    "Syntax highlighting: {}",
                    if self.display.syntax_highlighting {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }
            "showmarks" | "smk" => {
                self.display.show_marks = value.parse().unwrap_or(true);
                Ok(format!(
                    "Marks in gutter: {}",
                    if self.display.show_marks {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ))
            }

            // Language settings (informational only - no modification via :set)
            "languages" | "lang" => {
                let extensions: Vec<String> = self
                    .languages
                    .extensions
                    .iter()
                    .map(|(ext, lang)| format!(".{} -> {}", ext, lang))
                    .collect();
                Ok(format!("Supported languages: {}", extensions.join(", ")))
            }

            _ => Err(format!("Unknown setting: {}", setting)),
        }
    }
}

// Legacy config types for backwards compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub settings: HashMap<String, ConfigValue>,
    pub keymaps: HashMap<String, String>,
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigValue {
    Bool(bool),
    Int(i64),
    String(String),
    List(Vec<String>),
}
