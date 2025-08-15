use crate::config::watcher::ConfigWatcher;
use crossterm::style::Color;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

/// Complete theme configuration with UI and syntax colors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub theme: ThemeSelection,
    pub themes: HashMap<String, Theme>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSelection {
    pub current: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub description: String,
    pub ui: UIColors,
    pub tree_sitter: HashMap<String, String>, // Direct node type -> color mappings (now required)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatusLineColors {
    pub left_bg: String,
    pub left_fg: String,
    pub mid_bg: String,
    pub mid_fg: String,
    pub right_bg: String,
    pub right_fg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModeColors {
    pub normal_bg: String,
    pub normal_fg: String,
    pub insert_bg: String,
    pub insert_fg: String,
    pub visual_bg: String,
    pub visual_fg: String,
    pub visual_line_bg: String,
    pub visual_line_fg: String,
    pub visual_block_bg: String,
    pub visual_block_fg: String,
    pub select_bg: String,
    pub select_fg: String,
    pub select_line_bg: String,
    pub select_line_fg: String,
    pub replace_bg: String,
    pub replace_fg: String,
    pub command_bg: String,
    pub command_fg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIColors {
    pub background: String,
    pub status_bg: String,
    pub status_fg: String,
    pub status_modified: String,
    pub line_number: String,
    pub line_number_current: String,
    /// Color of the mark indicator rendered in the number column
    #[serde(default = "default_mark_color")]
    pub mark_indicator: String,
    pub cursor_line_bg: String,
    pub empty_line: String,
    pub command_line_bg: String,
    pub command_line_fg: String,
    /// Completion popup key column foreground
    #[serde(default = "default_completion_key_fg")]
    pub completion_key_fg: String,
    /// Completion popup alias column foreground
    #[serde(default = "default_completion_alias_fg")]
    pub completion_alias_fg: String,
    /// Completion popup value column foreground
    #[serde(default = "default_completion_value_fg")]
    pub completion_value_fg: String,
    pub selection_bg: String,
    pub visual_line_bg: String,  // Line-wise visual selection background
    pub visual_char_bg: String,  // Character-wise visual selection background
    pub visual_block_bg: String, // Block-wise visual selection background (future)
    pub select_char_bg: String,  // Character-wise select mode background
    pub select_line_bg: String,  // Line-wise select mode background
    pub warning: String,
    pub error: String,
    /// Optional: granular status line colors per segment
    #[serde(default)]
    pub statusline: Option<StatusLineColors>,
    /// Optional: per-mode colors for mode indicator in status line
    #[serde(default)]
    pub mode: Option<ModeColors>,
}

// Removed SyntaxColors and RustSpecificColors - using only tree_sitter node mappings now

/// UI theme that uses colors from themes.toml
#[derive(Debug, Clone)]
pub struct UITheme {
    pub background: Color,
    pub status_bg: Color,
    pub status_fg: Color,
    pub status_modified: Color,
    // Optional per-segment status line colors
    pub status_left_bg: Color,
    pub status_left_fg: Color,
    pub status_mid_bg: Color,
    pub status_mid_fg: Color,
    pub status_right_bg: Color,
    pub status_right_fg: Color,
    pub line_number: Color,
    pub line_number_current: Color,
    /// Color used for the mark indicator rendered in the number column
    pub mark_indicator: Color,
    pub cursor_line_bg: Color,
    pub empty_line: Color,
    pub command_line_bg: Color,
    pub command_line_fg: Color,
    pub completion_key_fg: Color,
    pub completion_alias_fg: Color,
    pub completion_value_fg: Color,
    pub selection_bg: Color,
    pub visual_line_bg: Color,  // Line-wise visual selection background
    pub visual_char_bg: Color,  // Character-wise visual selection background
    pub visual_block_bg: Color, // Block-wise visual selection background (future)
    pub select_char_bg: Color,  // Character-wise select mode background
    pub select_line_bg: Color,  // Line-wise select mode background
    pub warning: Color,
    pub error: Color,
    pub mode_colors: ModeThemeColors,
}

/// Syntax theme that uses only tree-sitter node type mappings
#[derive(Debug, Clone)]
pub struct SyntaxTheme {
    // Tree-sitter node type mappings - the only source of syntax colors
    pub tree_sitter_mappings: HashMap<String, Color>,
}

/// Combined theme with both UI and syntax colors
#[derive(Debug, Clone)]
pub struct CompleteTheme {
    pub name: String,
    pub description: String,
    pub ui: UITheme,
    pub syntax: SyntaxTheme,
}

impl ThemeConfig {
    /// Load theme configuration from themes.toml
    /// Load theme configuration from themes.toml with fallback to editor.toml default
    pub fn load() -> Self {
        Self::load_with_default_theme("default") // Default fallback if editor.toml is not available
    }

    /// Load theme configuration with a specific default theme name
    pub fn load_with_default_theme(default_theme: &str) -> Self {
        if let Ok(config_content) = fs::read_to_string("themes.toml") {
            if let Ok(mut config) = toml::from_str::<ThemeConfig>(&config_content) {
                // Ensure the current theme exists, if not use the default from editor.toml
                if !config.themes.contains_key(&config.theme.current) {
                    log::warn!(
                        "Current theme '{}' not found in themes.toml",
                        config.theme.current
                    );

                    // Try to use the default theme from editor.toml
                    if config.themes.contains_key(default_theme) {
                        log::info!("Switching to default theme '{}'", default_theme);
                        config.theme.current = default_theme.to_string();
                    } else if let Some(first_theme_name) = config.themes.keys().next().cloned() {
                        log::warn!(
                            "Default theme '{}' not found, using first available theme '{}'",
                            default_theme,
                            first_theme_name
                        );
                        config.theme.current = first_theme_name;
                    } else {
                        log::error!("No themes found in themes.toml!");
                        return Self::create_emergency_config();
                    }
                }
                return config;
            } else {
                log::error!("Failed to parse themes.toml - invalid TOML format");
            }
        } else {
            log::error!("Failed to read themes.toml file");
        }

        // If we can't load themes.toml, create an emergency minimal config
        log::error!("Creating emergency theme configuration - please check themes.toml");
        Self::create_emergency_config()
    }

    /// Create minimal emergency configuration when themes.toml is missing or invalid
    pub fn create_emergency_config() -> Self {
        log::warn!(
            "themes.toml missing/invalid; using built-in 'default' theme to match repository settings"
        );
        let mut themes = HashMap::new();

        // Clone the repository's default theme so fallback matches exactly
        let default_ui = UIColors {
            background: "#1f1611".to_string(),
            status_bg: "#ce422b".to_string(),
            status_fg: "#cccccc".to_string(),
            status_modified: "#f74c00".to_string(),
            line_number: "#8c6239".to_string(),
            line_number_current: "#deb887".to_string(),
            mark_indicator: "#e6b422".to_string(),
            cursor_line_bg: "#2d2318".to_string(),
            empty_line: "#4a3728".to_string(),
            command_line_bg: "#1f1611".to_string(),
            command_line_fg: "#deb887".to_string(),
            completion_key_fg: "#deb887".to_string(),
            completion_alias_fg: "#cccccc".to_string(),
            completion_value_fg: "#ffe6c7".to_string(),
            selection_bg: "#8c4a2b".to_string(),
            visual_line_bg: "#8c4a2b".to_string(),
            visual_char_bg: "#7a3f28".to_string(),
            visual_block_bg: "#9a5235".to_string(),
            select_char_bg: "#7d4a30".to_string(),
            select_line_bg: "#8a5236".to_string(),
            warning: "#ff8c00".to_string(),
            error: "#dc322f".to_string(),
            statusline: Some(StatusLineColors {
                left_bg: "#ce422b".to_string(),
                left_fg: "#cccccc".to_string(),
                mid_bg: "#ce422b".to_string(),
                mid_fg: "#cccccc".to_string(),
                right_bg: "#ce422b".to_string(),
                right_fg: "#cccccc".to_string(),
            }),
            mode: Some(ModeColors {
                normal_bg: "#ce422b".to_string(),
                normal_fg: "#ffffff".to_string(),
                insert_bg: "#ce422b".to_string(),
                insert_fg: "#ffe6c7".to_string(),
                visual_bg: "#ce422b".to_string(),
                visual_fg: "#fff3da".to_string(),
                visual_line_bg: "#ce422b".to_string(),
                visual_line_fg: "#ffedd5".to_string(),
                visual_block_bg: "#ce422b".to_string(),
                visual_block_fg: "#fffbeb".to_string(),
                select_bg: "#ce422b".to_string(),
                select_fg: "#ffefd5".to_string(),
                select_line_bg: "#ce422b".to_string(),
                select_line_fg: "#ffe8c7".to_string(),
                replace_bg: "#ce422b".to_string(),
                replace_fg: "#ffd1c1".to_string(),
                command_bg: "#ce422b".to_string(),
                command_fg: "#ffffff".to_string(),
            }),
        };

        let default_ts: HashMap<String, String> = HashMap::from([
            ("plain_text".to_string(), "#deb887".to_string()),
            ("keyword".to_string(), "#ce422b".to_string()),
            ("function".to_string(), "#b58900".to_string()),
            ("type".to_string(), "#268bd2".to_string()),
            ("string".to_string(), "#859900".to_string()),
            ("number".to_string(), "#d33682".to_string()),
            ("comment".to_string(), "#93a1a1".to_string()),
            ("identifier".to_string(), "#deb887".to_string()),
            ("variable".to_string(), "#deb887".to_string()),
            ("operator".to_string(), "#cb4b16".to_string()),
            ("punctuation".to_string(), "#839496".to_string()),
            ("delimiter".to_string(), "#839496".to_string()),
            ("character".to_string(), "#859900".to_string()),
            ("documentation".to_string(), "#586e75".to_string()),
            ("preprocessor".to_string(), "#6c71c4".to_string()),
            ("macro".to_string(), "#dc322f".to_string()),
            ("attribute".to_string(), "#2aa198".to_string()),
            ("label".to_string(), "#cb4b16".to_string()),
            ("constant".to_string(), "#d33682".to_string()),
        ]);

        themes.insert(
            "default".to_string(),
            Theme {
                name: "Rust Theme".to_string(),
                description: "Rust-inspired color palette with warm oranges and earth tones"
                    .to_string(),
                ui: default_ui,
                tree_sitter: default_ts,
            },
        );

        Self {
            theme: ThemeSelection {
                current: "default".to_string(),
            },
            themes,
        }
    }

    // Intentionally no save() for ThemeConfig: themes.toml is user-managed only.

    /// Get the current active theme as a CompleteTheme
    pub fn get_current_theme(&self) -> CompleteTheme {
        let theme_name = &self.theme.current;
        if let Some(theme) = self.themes.get(theme_name) {
            CompleteTheme {
                name: theme.name.clone(),
                description: theme.description.clone(),
                ui: UITheme::from_colors(&theme.ui),
                syntax: SyntaxTheme::from_tree_sitter(&theme.tree_sitter),
            }
        } else {
            // If current theme doesn't exist, use first available theme
            if let Some((first_name, first_theme)) = self.themes.iter().next() {
                log::warn!(
                    "Current theme '{}' not found, using '{}'",
                    theme_name,
                    first_name
                );
                CompleteTheme {
                    name: first_theme.name.clone(),
                    description: first_theme.description.clone(),
                    ui: UITheme::from_colors(&first_theme.ui),
                    syntax: SyntaxTheme::from_tree_sitter(&first_theme.tree_sitter),
                }
            } else {
                // This should never happen as we ensure at least one theme exists
                log::error!("No themes available! This should not happen.");
                self.create_emergency_theme()
            }
        }
    }

    /// Create emergency theme if no themes are available (should rarely happen)
    fn create_emergency_theme(&self) -> CompleteTheme {
        // Build from the same built-in default theme used by create_emergency_config,
        // ensuring perfect parity with themes.toml's default.
        let cfg = ThemeConfig::create_emergency_config();
        cfg.get_current_theme()
    }

    /// Get a specific theme by name
    pub fn get_theme(&self, theme_name: &str) -> Option<CompleteTheme> {
        self.themes.get(theme_name).map(|theme| CompleteTheme {
            name: theme.name.clone(),
            description: theme.description.clone(),
            ui: UITheme::from_colors(&theme.ui),
            syntax: SyntaxTheme::from_tree_sitter(&theme.tree_sitter),
        })
    }

    /// Set the current active theme
    pub fn set_current_theme(&mut self, theme_name: &str) {
        if self.themes.contains_key(theme_name) {
            self.theme.current = theme_name.to_string();
        }
    }

    /// List all available theme names
    pub fn list_themes(&self) -> Vec<&String> {
        self.themes.keys().collect()
    }

    /// Get the current active theme name
    pub fn current_theme_name(&self) -> &str {
        &self.theme.current
    }

    /// Reload themes from themes.toml and return true if anything changed
    pub fn reload(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        // Get the current theme name to preserve as default if possible
        let current_theme = self.theme.current.clone();
        let new_config = Self::load_with_default_theme(&current_theme);

        // Check if anything has changed - theme name, theme count, or theme content
        let theme_changed = self.theme.current != new_config.theme.current;
        let theme_count_changed = self.themes.len() != new_config.themes.len();

        // Check if the content of any theme has changed by comparing the serialized data
        let content_changed = {
            // Convert both configs to strings and compare
            if let (Ok(old_toml), Ok(new_toml)) =
                (toml::to_string(self), toml::to_string(&new_config))
            {
                old_toml != new_toml
            } else {
                // If we can't serialize, assume it changed to be safe
                true
            }
        };

        let any_change = theme_changed || theme_count_changed || content_changed;

        if any_change {
            log::info!(
                "Theme configuration changed (theme: {}, count: {}, content: {})",
                theme_changed,
                theme_count_changed,
                content_changed
            );
        }

        *self = new_config;
        Ok(any_change)
    }

    /// Check for theme file changes using the provided watcher and reload if necessary
    pub fn check_and_reload(
        &mut self,
        watcher: &ConfigWatcher,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        log::trace!("Checking for theme file changes...");
        if watcher.check_for_theme_changes() {
            log::debug!("Theme changes detected, reloading...");
            return self.reload();
        }
        Ok(false)
    }
}

impl UITheme {
    /// Create UITheme from color strings in themes.toml
    pub fn from_colors(colors: &UIColors) -> Self {
        // Base status fg/bg
        let base_bg = parse_color(&colors.status_bg);
        let base_fg = parse_color(&colors.status_fg);

        // Segment colors with fallback to base
        let (left_bg, left_fg, mid_bg, mid_fg, right_bg, right_fg) =
            if let Some(sl) = &colors.statusline {
                (
                    parse_color(&sl.left_bg),
                    parse_color(&sl.left_fg),
                    parse_color(&sl.mid_bg),
                    parse_color(&sl.mid_fg),
                    parse_color(&sl.right_bg),
                    parse_color(&sl.right_fg),
                )
            } else {
                (base_bg, base_fg, base_bg, base_fg, base_bg, base_fg)
            };

        // Per-mode colors with fallback to base fg/bg
        let mode_colors = ModeThemeColors::from_mode_colors(colors.mode.as_ref(), base_fg, base_bg);

        // Completion columns (explicit fields with defaults)
        let comp_key_fg = parse_color(&colors.completion_key_fg);
        let comp_alias_fg = parse_color(&colors.completion_alias_fg);
        let comp_value_fg = parse_color(&colors.completion_value_fg);

        Self {
            background: parse_color(&colors.background),
            status_bg: base_bg,
            status_fg: base_fg,
            status_modified: parse_color(&colors.status_modified),
            status_left_bg: left_bg,
            status_left_fg: left_fg,
            status_mid_bg: mid_bg,
            status_mid_fg: mid_fg,
            status_right_bg: right_bg,
            status_right_fg: right_fg,
            line_number: parse_color(&colors.line_number),
            line_number_current: parse_color(&colors.line_number_current),
            mark_indicator: parse_color(&colors.mark_indicator),
            cursor_line_bg: parse_color(&colors.cursor_line_bg),
            empty_line: parse_color(&colors.empty_line),
            command_line_bg: parse_color(&colors.command_line_bg),
            command_line_fg: parse_color(&colors.command_line_fg),
            completion_key_fg: comp_key_fg,
            completion_alias_fg: comp_alias_fg,
            completion_value_fg: comp_value_fg,
            selection_bg: parse_color(&colors.selection_bg),
            visual_line_bg: parse_color(&colors.visual_line_bg),
            visual_char_bg: parse_color(&colors.visual_char_bg),
            visual_block_bg: parse_color(&colors.visual_block_bg),
            select_char_bg: parse_color(&colors.select_char_bg),
            select_line_bg: parse_color(&colors.select_line_bg),
            warning: parse_color(&colors.warning),
            error: parse_color(&colors.error),
            mode_colors,
        }
    }
}

fn default_mark_color() -> String {
    "#e6b422".to_string()
}

fn default_completion_key_fg() -> String {
    "#deb887".to_string()
}

fn default_completion_alias_fg() -> String {
    "#cccccc".to_string()
}

fn default_completion_value_fg() -> String {
    "#ffe6c7".to_string()
}

#[derive(Debug, Clone)]
pub struct ModeThemeColors {
    pub normal_fg: Color,
    pub normal_bg: Color,
    pub insert_fg: Color,
    pub insert_bg: Color,
    pub visual_fg: Color,
    pub visual_bg: Color,
    pub visual_line_fg: Color,
    pub visual_line_bg: Color,
    pub visual_block_fg: Color,
    pub visual_block_bg: Color,
    pub select_fg: Color,
    pub select_bg: Color,
    pub select_line_fg: Color,
    pub select_line_bg: Color,
    pub replace_fg: Color,
    pub replace_bg: Color,
    pub command_fg: Color,
    pub command_bg: Color,
}

impl ModeThemeColors {
    pub fn from_mode_colors(
        src: Option<&ModeColors>,
        default_fg: Color,
        default_bg: Color,
    ) -> Self {
        let get = |s: Option<&String>| s.map(|x| parse_color(x)).unwrap_or(default_fg);
        let get_bg = |s: Option<&String>| s.map(|x| parse_color(x)).unwrap_or(default_bg);
        if let Some(m) = src {
            Self {
                normal_fg: get(Some(&m.normal_fg)),
                normal_bg: get_bg(Some(&m.normal_bg)),
                insert_fg: get(Some(&m.insert_fg)),
                insert_bg: get_bg(Some(&m.insert_bg)),
                visual_fg: get(Some(&m.visual_fg)),
                visual_bg: get_bg(Some(&m.visual_bg)),
                visual_line_fg: get(Some(&m.visual_line_fg)),
                visual_line_bg: get_bg(Some(&m.visual_line_bg)),
                visual_block_fg: get(Some(&m.visual_block_fg)),
                visual_block_bg: get_bg(Some(&m.visual_block_bg)),
                select_fg: get(Some(&m.select_fg)),
                select_bg: get_bg(Some(&m.select_bg)),
                select_line_fg: get(Some(&m.select_line_fg)),
                select_line_bg: get_bg(Some(&m.select_line_bg)),
                replace_fg: get(Some(&m.replace_fg)),
                replace_bg: get_bg(Some(&m.replace_bg)),
                command_fg: get(Some(&m.command_fg)),
                command_bg: get_bg(Some(&m.command_bg)),
            }
        } else {
            // Default to the base fg/bg when per-mode colors are not provided
            Self {
                normal_fg: default_fg,
                normal_bg: default_bg,
                insert_fg: default_fg,
                insert_bg: default_bg,
                visual_fg: default_fg,
                visual_bg: default_bg,
                visual_line_fg: default_fg,
                visual_line_bg: default_bg,
                visual_block_fg: default_fg,
                visual_block_bg: default_bg,
                select_fg: default_fg,
                select_bg: default_bg,
                select_line_fg: default_fg,
                select_line_bg: default_bg,
                replace_fg: default_fg,
                replace_bg: default_bg,
                command_fg: default_fg,
                command_bg: default_bg,
            }
        }
    }
}

impl SyntaxTheme {
    /// Create SyntaxTheme from tree-sitter mappings in themes.toml
    pub fn from_tree_sitter(tree_sitter: &HashMap<String, String>) -> Self {
        // Build tree-sitter mappings
        let mut tree_sitter_mappings = HashMap::new();
        for (node_type, color_str) in tree_sitter {
            tree_sitter_mappings.insert(node_type.clone(), parse_color(color_str));
        }

        Self {
            tree_sitter_mappings,
        }
    }

    /// Get default text color used when no syntax highlight applies
    /// Priority order:
    /// 1) plain_text (preferred)
    /// 2) identifier (reasonable fallback often used for names)
    /// 3) White (final fallback)
    pub fn get_default_text_color(&self) -> crossterm::style::Color {
        if let Some(c) = self.tree_sitter_mappings.get("plain_text") {
            return *c;
        }
        if let Some(c) = self.tree_sitter_mappings.get("identifier") {
            return *c;
        }
        crossterm::style::Color::White
    }
}

/// Parse a hex color string to crossterm Color
pub fn parse_color(color_str: &str) -> Color {
    if let Some(stripped) = color_str.strip_prefix('#')
        && stripped.len() == 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&stripped[0..2], 16),
            u8::from_str_radix(&stripped[2..4], 16),
            u8::from_str_radix(&stripped[4..6], 16),
        )
    {
        return Color::Rgb { r, g, b };
    }

    // Fallback to white if parsing fails - this should rarely happen
    // since we now ensure themes.toml always exists
    log::warn!(
        "Failed to parse color '{}', using white fallback",
        color_str
    );
    Color::White
}
