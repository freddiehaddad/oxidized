use crate::config::theme::ThemeConfig;
/// Command completion system for Vim-style commands
use log::{debug, info};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CommandCompletion {
    /// Available commands for completion
    commands: Vec<CompletionItem>,
    /// Current completion state
    pub active: bool,
    /// Current matches based on input
    pub matches: Vec<CompletionItem>,
    /// Currently selected index
    pub selected_index: usize,
    /// The text that triggered completion
    pub completion_prefix: String,
    /// Optional dynamic context (cwd, buffers, etc.)
    pub context: Option<CompletionContext>,
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The command text to complete
    pub text: String,
    /// Description of the command
    pub description: String,
    /// Category of command (set, buffer, file, etc.)
    pub category: String,
}

impl CommandCompletion {
    pub fn new() -> Self {
        let commands = Self::build_command_list();
        info!(
            "Initialized command completion with {} commands",
            commands.len()
        );
        Self {
            commands,
            active: false,
            matches: Vec::new(),
            selected_index: 0,
            completion_prefix: String::new(),
            context: None,
        }
    }

    /// Build the complete list of available commands
    fn build_command_list() -> Vec<CompletionItem> {
        let mut commands = Vec::new();

        // Basic ex commands
        commands.extend(vec![
            CompletionItem {
                text: "quit".to_string(),
                description: "Quit editor".to_string(),
                category: "file".to_string(),
            },
            CompletionItem {
                text: "q".to_string(),
                description: "Quit editor (short)".to_string(),
                category: "file".to_string(),
            },
            CompletionItem {
                text: "quit!".to_string(),
                description: "Force quit without saving".to_string(),
                category: "file".to_string(),
            },
            CompletionItem {
                text: "q!".to_string(),
                description: "Force quit without saving (short)".to_string(),
                category: "file".to_string(),
            },
            CompletionItem {
                text: "write".to_string(),
                description: "Save current file".to_string(),
                category: "file".to_string(),
            },
            CompletionItem {
                text: "w".to_string(),
                description: "Save current file (short)".to_string(),
                category: "file".to_string(),
            },
            CompletionItem {
                text: "wq".to_string(),
                description: "Save and quit".to_string(),
                category: "file".to_string(),
            },
            CompletionItem {
                text: "x".to_string(),
                description: "Save and quit (short)".to_string(),
                category: "file".to_string(),
            },
            CompletionItem {
                text: "edit".to_string(),
                description: "Edit file in new buffer".to_string(),
                category: "file".to_string(),
            },
            CompletionItem {
                text: "e".to_string(),
                description: "Edit file in new buffer (short)".to_string(),
                category: "file".to_string(),
            },
        ]);

        // Buffer management commands
        commands.extend(vec![
            CompletionItem {
                text: "buffer".to_string(),
                description: "Switch to buffer".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "b".to_string(),
                description: "Switch to buffer (short)".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "bnext".to_string(),
                description: "Switch to next buffer".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "bn".to_string(),
                description: "Switch to next buffer (short)".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "bprevious".to_string(),
                description: "Switch to previous buffer".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "bp".to_string(),
                description: "Switch to previous buffer (short)".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "bprev".to_string(),
                description: "Switch to previous buffer".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "bdelete".to_string(),
                description: "Delete current buffer".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "bd".to_string(),
                description: "Delete current buffer (short)".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "bdelete!".to_string(),
                description: "Force delete current buffer".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "bd!".to_string(),
                description: "Force delete current buffer (short)".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "ls".to_string(),
                description: "List all buffers".to_string(),
                category: "buffer".to_string(),
            },
            CompletionItem {
                text: "buffers".to_string(),
                description: "List all buffers".to_string(),
                category: "buffer".to_string(),
            },
        ]);

        // Window/split commands
        commands.extend(vec![
            CompletionItem {
                text: "split".to_string(),
                description: "Create horizontal split".to_string(),
                category: "window".to_string(),
            },
            CompletionItem {
                text: "sp".to_string(),
                description: "Create horizontal split (short)".to_string(),
                category: "window".to_string(),
            },
            CompletionItem {
                text: "vsplit".to_string(),
                description: "Create vertical split".to_string(),
                category: "window".to_string(),
            },
            CompletionItem {
                text: "vsp".to_string(),
                description: "Create vertical split (short)".to_string(),
                category: "window".to_string(),
            },
            CompletionItem {
                text: "close".to_string(),
                description: "Close current window".to_string(),
                category: "window".to_string(),
            },
        ]);

        // Set commands - display settings
        commands.extend(vec![
            CompletionItem {
                text: "set number".to_string(),
                description: "Show line numbers".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nu".to_string(),
                description: "Show line numbers (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nonumber".to_string(),
                description: "Hide line numbers".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nonu".to_string(),
                description: "Hide line numbers (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set relativenumber".to_string(),
                description: "Show relative line numbers".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set rnu".to_string(),
                description: "Show relative line numbers (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set norelativenumber".to_string(),
                description: "Hide relative line numbers".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nornu".to_string(),
                description: "Hide relative line numbers (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set cursorline".to_string(),
                description: "Highlight cursor line".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set cul".to_string(),
                description: "Highlight cursor line (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nocursorline".to_string(),
                description: "Disable cursor line highlight".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nocul".to_string(),
                description: "Disable cursor line highlight (short)".to_string(),
                category: "set".to_string(),
            },
            // Wrapping and line break
            CompletionItem {
                text: "set wrap".to_string(),
                description: "Enable soft line wrapping".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nowrap".to_string(),
                description: "Disable soft line wrapping".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set linebreak".to_string(),
                description: "Prefer breaking at word boundaries when wrapping".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set lbr".to_string(),
                description: "Prefer word-boundary breaks (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nolinebreak".to_string(),
                description: "Disable word-boundary preference".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nolbr".to_string(),
                description: "Disable word-boundary preference (short)".to_string(),
                category: "set".to_string(),
            },
        ]);

        // Set commands - search and navigation
        commands.extend(vec![
            CompletionItem {
                text: "set ignorecase".to_string(),
                description: "Case-insensitive search".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set ic".to_string(),
                description: "Case-insensitive search (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noignorecase".to_string(),
                description: "Case-sensitive search".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noic".to_string(),
                description: "Case-sensitive search (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set smartcase".to_string(),
                description: "Smart case matching".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set scs".to_string(),
                description: "Smart case matching (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nosmartcase".to_string(),
                description: "Disable smart case".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noscs".to_string(),
                description: "Disable smart case (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set hlsearch".to_string(),
                description: "Highlight search results".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set hls".to_string(),
                description: "Highlight search results (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nohlsearch".to_string(),
                description: "Disable search highlighting".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nohls".to_string(),
                description: "Disable search highlighting (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set incsearch".to_string(),
                description: "Incremental search".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set is".to_string(),
                description: "Incremental search (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noincsearch".to_string(),
                description: "Disable incremental search".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nois".to_string(),
                description: "Disable incremental search (short)".to_string(),
                category: "set".to_string(),
            },
        ]);

        // Set commands - editing toggles
        commands.extend(vec![
            CompletionItem {
                text: "set expandtab".to_string(),
                description: "Insert spaces for tabs".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set et".to_string(),
                description: "Insert spaces for tabs (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noexpandtab".to_string(),
                description: "Use hard tab characters".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noet".to_string(),
                description: "Use hard tab characters (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set autoindent".to_string(),
                description: "Enable automatic indentation".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set ai".to_string(),
                description: "Enable automatic indentation (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noautoindent".to_string(),
                description: "Disable automatic indentation".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noai".to_string(),
                description: "Disable automatic indentation (short)".to_string(),
                category: "set".to_string(),
            },
        ]);

        // Set commands - files and persistence
        commands.extend(vec![
            CompletionItem {
                text: "set undofile".to_string(),
                description: "Enable persistent undo".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set udf".to_string(),
                description: "Enable persistent undo (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noundofile".to_string(),
                description: "Disable persistent undo".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noudf".to_string(),
                description: "Disable persistent undo (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set backup".to_string(),
                description: "Enable backup files".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set bk".to_string(),
                description: "Enable backup files (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nobackup".to_string(),
                description: "Disable backup files".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nobk".to_string(),
                description: "Disable backup files (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set swapfile".to_string(),
                description: "Enable swap file".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set swf".to_string(),
                description: "Enable swap file (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noswapfile".to_string(),
                description: "Disable swap file".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noswf".to_string(),
                description: "Disable swap file (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set autosave".to_string(),
                description: "Enable auto save".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set aw".to_string(),
                description: "Enable auto save (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noautosave".to_string(),
                description: "Disable auto save".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noaw".to_string(),
                description: "Disable auto save (short)".to_string(),
                category: "set".to_string(),
            },
        ]);

        // Set commands - UI toggles and syntax
        commands.extend(vec![
            CompletionItem {
                text: "set laststatus".to_string(),
                description: "Show status line".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set ls".to_string(),
                description: "Show status line (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nolaststatus".to_string(),
                description: "Hide status line".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nols".to_string(),
                description: "Hide status line (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set showcmd".to_string(),
                description: "Show command in status area".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set sc".to_string(),
                description: "Show command (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noshowcmd".to_string(),
                description: "Hide command in status area".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nosc".to_string(),
                description: "Hide command (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set syntax".to_string(),
                description: "Enable syntax highlighting".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set syn".to_string(),
                description: "Enable syntax highlighting (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nosyntax".to_string(),
                description: "Disable syntax highlighting".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nosyn".to_string(),
                description: "Disable syntax highlighting (short)".to_string(),
                category: "set".to_string(),
            },
        ]);

        // Set commands with values
        commands.extend(vec![
            CompletionItem {
                text: "set tabstop=".to_string(),
                description: "Set tab width".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set ts=".to_string(),
                description: "Set tab width (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set scrolloff=".to_string(),
                description: "Lines to keep around cursor".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set so=".to_string(),
                description: "Lines to keep around cursor (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set sidescrolloff=".to_string(),
                description: "Columns to keep around cursor".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set siso=".to_string(),
                description: "Columns to keep around cursor (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set colorscheme=".to_string(),
                description: "Change color scheme".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set colo=".to_string(),
                description: "Change color scheme (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set timeoutlen=".to_string(),
                description: "Set command timeout (ms)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set tm=".to_string(),
                description: "Set command timeout (ms) (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set undolevels=".to_string(),
                description: "Set number of undo levels".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set ul=".to_string(),
                description: "Set undo levels (short)".to_string(),
                category: "set".to_string(),
            },
        ]);

        // Set queries for booleans and values
        let query_items = vec![
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
            "colorscheme",
            "syntax",
        ];
        for key in query_items {
            commands.push(CompletionItem {
                text: format!("set {}?", key),
                description: format!("Query '{}' value", key),
                category: "set".to_string(),
            });
        }

        commands
    }

    /// Start completion for the given input
    pub fn start_completion(&mut self, input: &str) {
        debug!("Starting command completion for input: '{}'", input);
        self.active = true;
        self.completion_prefix = input.to_string();
        self.update_matches(input);
        self.selected_index = 0;
        debug!("Found {} completion matches", self.matches.len());
    }

    /// Update matches based on input
    fn update_matches(&mut self, input: &str) {
        let input_lower = input.to_lowercase();

        // Static matches from the predefined command list
        let mut combined: Vec<CompletionItem> = self
            .commands
            .iter()
            .filter(|cmd| cmd.text.to_lowercase().starts_with(&input_lower))
            .cloned()
            .collect();

        // Dynamic matches based on current input (e.g., values after '=')
        let dynamic = self.dynamic_matches(input);
        combined.extend(dynamic);

        // Deduplicate by text while preferring shorter description (or first seen)
        combined.sort_by(|a, b| a.text.cmp(&b.text));
        combined.dedup_by(|a, b| a.text == b.text);

        // Sort matches by length (shorter matches first) and then alphabetically
        combined.sort_by(|a, b| {
            a.text
                .len()
                .cmp(&b.text.len())
                .then_with(|| a.text.cmp(&b.text))
        });

        self.matches = combined;
    }

    /// Produce dynamic completion items for :set value forms
    fn dynamic_matches(&self, input: &str) -> Vec<CompletionItem> {
        let trimmed = input.trim_start();
        let lower = trimmed.to_lowercase();
        let mut out: Vec<CompletionItem> = Vec::new();
        // Buffer name/id completion: b <...> or buffer <...>
        if (lower.starts_with("b ") || lower.starts_with("buffer ")) && self.context.is_some() {
            let ctx = self.context.as_ref().unwrap();
            let prefix = trimmed
                .strip_prefix("b ")
                .or_else(|| trimmed.strip_prefix("buffer "))
                .unwrap_or_default()
                .trim_start();

            let prefix_lower = prefix.to_lowercase();
            for b in &ctx.buffers {
                let id_str = b.id.to_string();
                let name_lower = b.name.to_lowercase();
                let matches = prefix.is_empty()
                    || id_str.starts_with(prefix)
                    || name_lower.contains(&prefix_lower);
                if matches {
                    // Suggest by id
                    out.push(CompletionItem {
                        text: format!("b {}", b.id),
                        description: format!(
                            "Buffer {}: {}{}",
                            b.id,
                            b.name,
                            if b.modified { " [+]" } else { "" }
                        ),
                        category: "buffer".to_string(),
                    });
                    // Suggest by name (only if it isn't "[No Name]")
                    if b.name != "[No Name]" {
                        out.push(CompletionItem {
                            text: format!("b {}", b.name),
                            description: format!(
                                "Buffer {}: {}{}",
                                b.id,
                                b.name,
                                if b.modified { " [+]" } else { "" }
                            ),
                            category: "buffer".to_string(),
                        });
                    }
                }
            }
        }

        // Helper to filter suggestions by current value prefix after '='
        fn value_prefix(s: &str) -> &str {
            if let Some(idx) = s.find('=') {
                &s[idx + 1..]
            } else {
                ""
            }
        }

        // colorscheme / colo dynamic values from themes.toml
        if lower.starts_with("set colorscheme=") || lower.starts_with("set colo=") {
            let val_pref = value_prefix(trimmed);
            let cfg = ThemeConfig::load();
            // Build list of (name, description)
            for (name, theme) in cfg.themes.iter() {
                if val_pref.is_empty() || name.to_lowercase().starts_with(&val_pref.to_lowercase())
                {
                    out.push(CompletionItem {
                        text: format!("set colorscheme={}", name),
                        description: format!("Theme: {}", theme.description),
                        category: "set".to_string(),
                    });
                }
            }
        }

        // File path completion for :e and :w commands
        if (lower.starts_with("e ")
            || lower.starts_with("edit ")
            || lower.starts_with("w ")
            || lower.starts_with("write "))
            && self.context.is_some()
        {
            let ctx = self.context.as_ref().unwrap();
            let raw = trimmed
                .strip_prefix("e ")
                .or_else(|| trimmed.strip_prefix("edit "))
                .or_else(|| trimmed.strip_prefix("w "))
                .or_else(|| trimmed.strip_prefix("write "))
                .unwrap_or_default();
            let mut input_path = raw.trim_start();

            // Optional: support '%' as a shorthand to root at current buffer's directory
            // Example: :e %/src will complete under the current buffer directory
            let use_buf_root = input_path.starts_with('%');
            if use_buf_root {
                input_path = &input_path[1..];
            }

            use std::fs;
            use std::path::{Path, PathBuf};

            // Resolve base directory and prefix filter
            let base_root = if use_buf_root {
                ctx.current_buffer_dir.clone().unwrap_or(ctx.cwd.clone())
            } else {
                ctx.cwd.clone()
            };

            let (dir, filter) = if input_path.is_empty() {
                (base_root.clone(), String::new())
            } else {
                let p = Path::new(input_path);
                if p.is_absolute() {
                    if p.is_dir() {
                        (PathBuf::from(p), String::new())
                    } else if let Some(parent) = p.parent() {
                        (
                            parent.to_path_buf(),
                            p.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string(),
                        )
                    } else {
                        (ctx.cwd.clone(), input_path.to_string())
                    }
                } else {
                    let joined = base_root.join(p);
                    if joined.is_dir() {
                        (joined, String::new())
                    } else if let Some(parent) = joined.parent() {
                        (
                            parent.to_path_buf(),
                            joined
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string(),
                        )
                    } else {
                        (base_root.clone(), input_path.to_string())
                    }
                }
            };

            if let Ok(read_dir) = fs::read_dir(&dir) {
                // Include parent directory entry when not at filesystem root
                if let Some(parent) = dir.parent() {
                    let mut text = String::new();
                    if lower.starts_with("e ") {
                        text.push_str("e ");
                    } else if lower.starts_with("edit ") {
                        text.push_str("edit ");
                    } else if lower.starts_with("w ") {
                        text.push_str("w ");
                    } else {
                        text.push_str("write ");
                    }
                    // Preserve user-typed prefix
                    if !input_path.is_empty() {
                        let suggested = if Path::new(input_path).is_absolute() {
                            parent.join("..")
                        } else {
                            let abs = parent.join("..");
                            abs.strip_prefix(&base_root).unwrap_or(&abs).to_path_buf()
                        };
                        text.push_str(&suggested.to_string_lossy());
                    } else {
                        let rel = parent
                            .strip_prefix(&base_root)
                            .unwrap_or(parent)
                            .to_path_buf();
                        let display = if rel.as_os_str().is_empty() {
                            PathBuf::from("..")
                        } else {
                            rel.join("..")
                        };
                        text.push_str(&display.to_string_lossy());
                    }
                    // Ensure trailing separator for directories
                    if !text.ends_with(std::path::MAIN_SEPARATOR) {
                        text.push(std::path::MAIN_SEPARATOR);
                    }
                    out.push(CompletionItem {
                        text,
                        description: "Parent Directory".to_string(),
                        category: "file".to_string(),
                    });
                }
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !filter.is_empty()
                        && !name.to_lowercase().starts_with(&filter.to_lowercase())
                    {
                        continue;
                    }
                    let mut text = String::new();
                    // Preserve the command and prefix
                    if lower.starts_with("e ") {
                        text.push_str("e ");
                    } else if lower.starts_with("edit ") {
                        text.push_str("edit ");
                    } else if lower.starts_with("w ") {
                        text.push_str("w ");
                    } else {
                        text.push_str("write ");
                    }
                    // If the user typed a partial path, include it
                    if !input_path.is_empty() {
                        // Rebuild the suggestion keeping user-typed prefix up to dir
                        let suggested = if Path::new(input_path).is_absolute() {
                            dir.join(&name)
                        } else {
                            // Use a relative suggestion from cwd when possible
                            let abs = dir.join(&name);
                            abs.strip_prefix(&base_root).unwrap_or(&abs).to_path_buf()
                        };
                        text.push_str(&suggested.to_string_lossy());
                    } else {
                        // Base directory listing, relative to cwd when possible
                        let rel = path.strip_prefix(&base_root).unwrap_or(&path).to_path_buf();
                        text.push_str(&rel.to_string_lossy());
                    }

                    // Ensure trailing separator for directories
                    if path.is_dir() && !text.ends_with(std::path::MAIN_SEPARATOR) {
                        text.push(std::path::MAIN_SEPARATOR);
                    }

                    let desc = if path.is_dir() { "Directory" } else { "File" };
                    out.push(CompletionItem {
                        text,
                        description: desc.to_string(),
                        category: "file".to_string(),
                    });
                }
            }
        }

        // Numeric suggestions for common options
        // tabstop / ts
        if lower.starts_with("set tabstop=") || lower.starts_with("set ts=") {
            let val_pref = value_prefix(trimmed);
            let suggestions = ["2", "4", "8"]; // common tab widths
            for s in suggestions.iter() {
                if val_pref.is_empty() || s.starts_with(val_pref) {
                    out.push(CompletionItem {
                        text: format!("set tabstop={}", s),
                        description: "Set tab width".to_string(),
                        category: "set".to_string(),
                    });
                }
            }
        }

        // scrolloff / so
        if lower.starts_with("set scrolloff=") || lower.starts_with("set so=") {
            let val_pref = value_prefix(trimmed);
            let suggestions = ["0", "1", "2", "3", "5", "8", "10"];
            for s in suggestions.iter() {
                if val_pref.is_empty() || s.starts_with(val_pref) {
                    out.push(CompletionItem {
                        text: format!("set scrolloff={}", s),
                        description: "Lines to keep around cursor".to_string(),
                        category: "set".to_string(),
                    });
                }
            }
        }

        // sidescrolloff / siso
        if lower.starts_with("set sidescrolloff=") || lower.starts_with("set siso=") {
            let val_pref = value_prefix(trimmed);
            let suggestions = ["0", "1", "2", "3", "5", "8", "10"];
            for s in suggestions.iter() {
                if val_pref.is_empty() || s.starts_with(val_pref) {
                    out.push(CompletionItem {
                        text: format!("set sidescrolloff={}", s),
                        description: "Columns to keep around cursor".to_string(),
                        category: "set".to_string(),
                    });
                }
            }
        }

        // timeoutlen / tm
        if lower.starts_with("set timeoutlen=") || lower.starts_with("set tm=") {
            let val_pref = value_prefix(trimmed);
            let suggestions = ["200", "300", "500", "700", "1000"];
            for s in suggestions.iter() {
                if val_pref.is_empty() || s.starts_with(val_pref) {
                    out.push(CompletionItem {
                        text: format!("set timeoutlen={}", s),
                        description: "Command timeout in ms".to_string(),
                        category: "set".to_string(),
                    });
                }
            }
        }

        // undolevels / ul
        if lower.starts_with("set undolevels=") || lower.starts_with("set ul=") {
            let val_pref = value_prefix(trimmed);
            let suggestions = ["100", "1000"]; // sensible defaults
            for s in suggestions.iter() {
                if val_pref.is_empty() || s.starts_with(val_pref) {
                    out.push(CompletionItem {
                        text: format!("set undolevels={}", s),
                        description: "Number of undo levels".to_string(),
                        category: "set".to_string(),
                    });
                }
            }
        }

        out
    }

    /// Move to next completion
    pub fn next(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.matches.len();
        }
    }

    /// Move to previous completion
    pub fn previous(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.matches.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Get currently selected completion
    pub fn selected(&self) -> Option<&CompletionItem> {
        self.matches.get(self.selected_index)
    }

    /// Accept current completion and return the completed text
    pub fn accept(&mut self) -> Option<String> {
        if let Some(selected) = self.selected() {
            let completed = selected.text.clone();
            self.cancel();
            Some(completed)
        } else {
            None
        }
    }

    /// Cancel completion
    pub fn cancel(&mut self) {
        self.active = false;
        self.matches.clear();
        self.selected_index = 0;
        self.completion_prefix.clear();
    }

    /// Check if completion menu should be shown
    pub fn should_show(&self) -> bool {
        self.active && !self.matches.is_empty()
    }

    /// Get visible completion items (for rendering)
    pub fn visible_items(&self, max_items: usize) -> &[CompletionItem] {
        if self.matches.is_empty() {
            return &[];
        }

        let start_idx = if self.matches.len() <= max_items {
            0
        } else {
            // Center the selected item in the visible window
            let half_visible = max_items / 2;
            if self.selected_index < half_visible {
                0
            } else if self.selected_index >= self.matches.len() - half_visible {
                self.matches.len() - max_items
            } else {
                self.selected_index - half_visible
            }
        };

        let end_idx = (start_idx + max_items).min(self.matches.len());
        &self.matches[start_idx..end_idx]
    }

    /// Get the relative index of the selected item in the visible window
    pub fn visible_selected_index(&self, max_items: usize) -> usize {
        let visible_items = self.visible_items(max_items);
        if visible_items.is_empty() {
            return 0;
        }

        // Find the selected item in the visible window
        for (i, item) in visible_items.iter().enumerate() {
            if let Some(selected) = self.selected()
                && item.text == selected.text
            {
                return i;
            }
        }
        0
    }
}

#[derive(Debug, Clone)]
pub struct CompletionContext {
    pub cwd: PathBuf,
    pub buffers: Vec<BufferSummary>,
    /// Optional current buffer directory for '%'-rooted path completion
    pub current_buffer_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct BufferSummary {
    pub id: usize,
    pub name: String,
    pub modified: bool,
}

impl CommandCompletion {
    pub fn set_context(&mut self, ctx: CompletionContext) {
        self.context = Some(ctx);
    }
}

impl Default for CommandCompletion {
    fn default() -> Self {
        Self::new()
    }
}
