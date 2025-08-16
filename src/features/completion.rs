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

        // Information and utility commands
        commands.extend(vec![
            CompletionItem {
                text: "registers".to_string(),
                description: "Show registers view".to_string(),
                category: "info".to_string(),
            },
            CompletionItem {
                text: "reg".to_string(),
                description: "Show registers view (short)".to_string(),
                category: "info".to_string(),
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
            CompletionItem {
                text: "set showmarks".to_string(),
                description: "Show marks in gutter/number column".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set smk".to_string(),
                description: "Show marks (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noshowmarks".to_string(),
                description: "Hide marks in gutter/number column".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nosmk".to_string(),
                description: "Hide marks (short)".to_string(),
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
            // Percent path root toggle for completion behavior
            CompletionItem {
                text: "set percentpathroot".to_string(),
                description: "Enable '%' root in path completion".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set nopercentpathroot".to_string(),
                description: "Disable '%' root in path completion".to_string(),
                category: "set".to_string(),
            },
            // Positional boolean argument form (no '=')
            CompletionItem {
                text: "set percentpathroot ".to_string(),
                description: "Set '%' root behavior (true/false)".to_string(),
                category: "set".to_string(),
            },
            // Short alias
            CompletionItem {
                text: "set ppr".to_string(),
                description: "Enable '%' root in path completion (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set noppr".to_string(),
                description: "Disable '%' root in path completion (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set ppr ".to_string(),
                description: "Set '%' root behavior (short)".to_string(),
                category: "set".to_string(),
            },
        ]);

        // Set commands with positional values (no '=')
        commands.extend(vec![
            CompletionItem {
                text: "set tabstop ".to_string(),
                description: "Set tab width".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set ts ".to_string(),
                description: "Set tab width (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set scrolloff ".to_string(),
                description: "Lines to keep around cursor".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set so ".to_string(),
                description: "Lines to keep around cursor (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set sidescrolloff ".to_string(),
                description: "Columns to keep around cursor".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set siso ".to_string(),
                description: "Columns to keep around cursor (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set colorscheme ".to_string(),
                description: "Change color scheme".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set colo ".to_string(),
                description: "Change color scheme (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set timeoutlen ".to_string(),
                description: "Set command timeout (ms)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set tm ".to_string(),
                description: "Set command timeout (ms) (short)".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set undolevels ".to_string(),
                description: "Set number of undo levels".to_string(),
                category: "set".to_string(),
            },
            CompletionItem {
                text: "set ul ".to_string(),
                description: "Set undo levels (short)".to_string(),
                category: "set".to_string(),
            },
        ]);

        // Note: we intentionally omit query-style suggestions like `set <opt>?` because the
        // completion popup already shows the current value inline for each option.

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
        // Detect whether the user typed `setp` and normalize to `set` for matching
        let input_lower = input.to_lowercase();
        let (normalized_for_match, desired_set_prefix): (String, Option<&'static str>) =
            if let Some(rest) = input_lower.strip_prefix("setp ") {
                (format!("set {}", rest), Some("setp "))
            } else if input_lower == "setp" || input_lower.starts_with("setp") {
                // Handle `setp` without trailing space
                ("set".to_string(), Some("setp "))
            } else if input_lower.starts_with("set ") || input_lower == "set" {
                (input_lower.clone(), Some("set "))
            } else {
                (input_lower.clone(), None)
            };

        // Static matches from the predefined command list
        let mut combined: Vec<CompletionItem> = self
            .commands
            .iter()
            .filter(|cmd| cmd.text.to_lowercase().starts_with(&normalized_for_match))
            .cloned()
            .map(|mut item| {
                // If the user typed `setp`, remap suggestions from `set ...` to `setp ...`
                if let Some(prefix) = desired_set_prefix
                    && prefix == "setp "
                    && item.text.starts_with("set ")
                {
                    item.text = format!("setp {}", &item.text[4..]);
                }
                item
            })
            .collect();

        // Dynamic matches based on current input (e.g., values after '=')
        let dynamic = self.dynamic_matches(input);
        combined.extend(dynamic);

        // Deduplicate Ex command aliases (e.g., quit/q, registers/reg) for no-arg items.
        // Keep the canonical long form and use its richer description when available.
        fn canonical_ex_name(s: &str) -> (String, bool) {
            match s {
                // file/quit
                "w" | "write" => ("write".to_string(), true),
                "q" | "quit" => ("quit".to_string(), true),
                "q!" | "quit!" => ("quit!".to_string(), true),
                "x" | "wq" => ("wq".to_string(), true),
                "e" | "edit" => ("edit".to_string(), true),
                // buffer family
                "b" | "buffer" => ("buffer".to_string(), true),
                "bn" | "bnext" => ("bnext".to_string(), true),
                "bp" | "bprev" | "bprevious" => ("bprevious".to_string(), true),
                "bd" | "bdelete" => ("bdelete".to_string(), true),
                "bd!" | "bdelete!" => ("bdelete!".to_string(), true),
                "ls" | "buffers" => ("buffers".to_string(), true),
                // window/split
                "sp" | "split" => ("split".to_string(), true),
                "vsp" | "vsplit" => ("vsplit".to_string(), true),
                "close" => ("close".to_string(), true),
                // info/tools
                "reg" | "registers" => ("registers".to_string(), true),
                _ => (s.to_string(), false),
            }
        }

        // Partition: collapse duplicates among non-set commands that have no args (no spaces)
        use std::collections::HashMap;
        let mut ex_map: HashMap<String, CompletionItem> = HashMap::new();
        let mut others: Vec<CompletionItem> = Vec::new();
        for item in combined.into_iter() {
            let has_space = item.text.contains(' ');
            if item.category != "set" && !has_space {
                let (canon, is_known) = canonical_ex_name(&item.text);
                if is_known {
                    let entry = ex_map.entry(canon.clone()).or_insert_with(|| item.clone());
                    // Prefer canonical long text and non-short description
                    let entry_is_alias = entry.text != canon;
                    let item_is_canon = item.text == canon;
                    let entry_short = entry.description.to_lowercase().contains("(short)");
                    let item_short = item.description.to_lowercase().contains("(short)");
                    if item_is_canon || (entry_is_alias && !item_short && entry_short) {
                        entry.text = canon;
                        entry.description = item.description.clone();
                        entry.category = item.category.clone();
                    }
                } else {
                    others.push(item);
                }
            } else {
                others.push(item);
            }
        }
        let mut combined: Vec<CompletionItem> = others;
        combined.extend(ex_map.into_values());

        // Build a map of canonical ':set' texts to their non-alias descriptions for reuse
        let mut set_canonical_desc: HashMap<String, String> = HashMap::new();
        for c in &self.commands {
            if c.category == "set" && !c.description.to_lowercase().contains("(short)") {
                // Normalize to use 'set ' prefix for keys
                let key = if let Some(rest) = c.text.strip_prefix("setp ") {
                    format!("set {}", rest)
                } else {
                    c.text.clone()
                };
                set_canonical_desc
                    .entry(key)
                    .or_insert_with(|| c.description.clone());
            }
        }

        // Normalize ':set' items so aliases (e.g., ts, ppr) are presented as canonical names
        // Preserve the user's typed prefix: if they started with 'setp' (with or without
        // a trailing space), we should emit suggestions with the 'setp ' prefix.
        let prefer_setp = input_lower.starts_with("setp");
        let desired_set_prefix: &str = if prefer_setp { "setp " } else { "set " };
        fn map_set_alias(key: &str) -> &str {
            match key {
                "nu" => "number",
                "rnu" => "relativenumber",
                "cul" => "cursorline",
                "smk" => "showmarks",
                "et" => "expandtab",
                "ai" => "autoindent",
                "ic" => "ignorecase",
                "scs" => "smartcase",
                "hls" => "hlsearch",
                "is" => "incsearch",
                "lbr" => "linebreak",
                "udf" => "undofile",
                "bk" => "backup",
                "swf" => "swapfile",
                "aw" => "autosave",
                "ls" => "laststatus",
                "sc" => "showcmd",
                "ppr" => "percentpathroot",
                "syn" => "syntax",
                "ts" => "tabstop",
                "ul" => "undolevels",
                "so" => "scrolloff",
                "siso" => "sidescrolloff",
                "tm" => "timeoutlen",
                "colo" => "colorscheme",
                other => other,
            }
        }
        let mut transformed: Vec<CompletionItem> = Vec::with_capacity(combined.len());
        for mut item in combined.into_iter() {
            if item.category != "set" {
                transformed.push(item);
                continue;
            }
            let after_prefix = item
                .text
                .strip_prefix("setp ")
                .or_else(|| item.text.strip_prefix("set "))
                .unwrap_or(&item.text);
            let mut key = after_prefix;
            let mut tail = "";
            if let Some((k, t)) = after_prefix.split_once(' ') {
                key = k;
                tail = t;
            }
            let is_neg = key.starts_with("no");
            let base = if is_neg { &key[2..] } else { key };
            let canonical = map_set_alias(base);
            let new_key = if is_neg {
                format!("no{}", canonical)
            } else {
                canonical.to_string()
            };
            let new_text = if tail.is_empty() {
                format!("{}{}", desired_set_prefix, new_key)
            } else {
                format!("{}{} {}", desired_set_prefix, new_key, tail)
            };
            // Prefer canonical description if available
            let desc_lookup_key = if tail.is_empty() {
                format!("set {}", new_key)
            } else {
                // For value/positional forms, try the base with trailing space
                format!("set {} ", new_key)
            };
            if let Some(desc) = set_canonical_desc.get(&desc_lookup_key) {
                item.description = desc.clone();
            }
            item.text = new_text;
            transformed.push(item);
        }
        let mut combined: Vec<CompletionItem> = transformed;

        // Deduplicate by text while preferring shorter description (or first seen)
        combined.sort_by(|a, b| a.text.cmp(&b.text));
        combined.dedup_by(|a, b| a.text == b.text);

        // Additional deduplication for :set/:setp option toggles so that aliases and full names
        // don't both appear as separate rows (e.g. `set showmarks` + `set smk`). We only keep
        // one positive form and one negative form per canonical option depending on context.
        // We do NOT collapse value suggestions containing '=' nor query forms ending with '?'.
        // Logic mirrors (must stay in sync with) renderer canonicalization.
        let input_is_negative = normalized_for_match.starts_with("set no");
        if normalized_for_match.starts_with("set ") || normalized_for_match.starts_with("setp ") {
            use std::collections::HashSet;
            let mut seen: HashSet<String> = HashSet::new();
            combined.retain(|item| {
                if item.category != "set" {
                    return true;
                }
                // Skip value and query forms from this alias collapsing
                if item.text.contains('=') || item.text.ends_with('?') {
                    return true;
                }
                // If the user didn't start with 'set no', hide negative forms entirely
                let is_negative_item = item
                    .text
                    .strip_prefix("setp ")
                    .or_else(|| item.text.strip_prefix("set "))
                    .map(|s| s.trim_start().starts_with("no"))
                    .unwrap_or(false);
                if !input_is_negative && is_negative_item {
                    return false;
                }
                // Extract the raw key portion after set/setp
                let raw_key = item
                    .text
                    .strip_prefix("setp ")
                    .or_else(|| item.text.strip_prefix("set "))
                    .unwrap_or(&item.text);
                let raw_key = raw_key.trim();
                // Determine negation
                let (is_neg, remainder) = if let Some(rest) = raw_key.strip_prefix("no") {
                    (true, rest)
                } else {
                    (false, raw_key)
                };
                // Map short aliases to canonical positive names
                let canonical_pos = match remainder {
                    "nu" => "number",
                    "rnu" => "relativenumber",
                    "cul" => "cursorline",
                    "smk" => "showmarks",
                    "et" => "expandtab",
                    "ai" => "autoindent",
                    "ic" => "ignorecase",
                    "scs" => "smartcase",
                    "hls" => "hlsearch",
                    "is" => "incsearch",
                    "lbr" => "linebreak",
                    "udf" => "undofile",
                    "bk" => "backup",
                    "swf" => "swapfile",
                    "aw" => "autosave",
                    "ls" => "laststatus",
                    "sc" => "showcmd",
                    "ppr" => "percentpathroot",
                    "syn" => "syntax",
                    "ts" => "tabstop",
                    "ul" => "undolevels",
                    "so" => "scrolloff",
                    "siso" => "sidescrolloff",
                    "tm" => "timeoutlen",
                    "colo" => "colorscheme",
                    other => other,
                };
                // Build a dedup key; separate positive/negative namespaces so a user typing
                // an explicit negative prefix still sees the negative form (but only once).
                // When the user did not begin with "set no", we suppress duplicates of the
                // negative forms entirely (keep only positive) by treating negative variants
                // as the same key as the positive.
                let key = if is_neg {
                    if input_is_negative {
                        format!("neg:{}", canonical_pos)
                    } else {
                        format!("pos:{}", canonical_pos)
                    }
                } else {
                    format!("pos:{}", canonical_pos)
                };
                if seen.contains(&key) {
                    false
                } else {
                    seen.insert(key);
                    true
                }
            });

            // Secondary filtering: if both a query form (set <opt>?) and a plain form (set <opt>)
            // would appear, hide the query form unless user actually typed a trailing '?'. This
            // prevents visually indistinguishable duplicates after renderer canonicalization.
            if !input_lower.ends_with('?') {
                // Collect canonical names from non-query items
                let mut plain_canon: HashSet<String> = HashSet::new();
                for item in combined
                    .iter()
                    .filter(|c| c.category == "set" && !c.text.ends_with('?'))
                {
                    if let Some(raw) = item
                        .text
                        .strip_prefix("setp ")
                        .or_else(|| item.text.strip_prefix("set "))
                    {
                        let raw = raw.trim();
                        let is_neg = raw.starts_with("no");
                        let mut key_part = if is_neg { &raw[2..] } else { raw };
                        if let Some((k, _)) = key_part.split_once('=') {
                            key_part = k;
                        }
                        // Map aliases to canonical (reuse subset of mapping)
                        let canonical = match key_part {
                            "nu" => "number",
                            "rnu" => "relativenumber",
                            "cul" => "cursorline",
                            "smk" => "showmarks",
                            "et" => "expandtab",
                            "ai" => "autoindent",
                            "ic" => "ignorecase",
                            "scs" => "smartcase",
                            "hls" => "hlsearch",
                            "is" => "incsearch",
                            "lbr" => "linebreak",
                            "udf" => "undofile",
                            "bk" => "backup",
                            "swf" => "swapfile",
                            "aw" => "autosave",
                            "ls" => "laststatus",
                            "sc" => "showcmd",
                            "ppr" => "percentpathroot",
                            "syn" => "syntax",
                            "ts" => "tabstop",
                            "ul" => "undolevels",
                            "so" => "scrolloff",
                            "siso" => "sidescrolloff",
                            "tm" => "timeoutlen",
                            "colo" => "colorscheme",
                            other => other,
                        };
                        // Namespace negative so queries of neg and pos both can exist theoretically
                        let canon_key = if is_neg {
                            format!("neg:{}", canonical)
                        } else {
                            format!("pos:{}", canonical)
                        };
                        plain_canon.insert(canon_key);
                    }
                }
                combined.retain(|c| {
                    if c.category != "set" || !c.text.ends_with('?') {
                        return true;
                    }
                    if let Some(raw) = c
                        .text
                        .strip_prefix("setp ")
                        .or_else(|| c.text.strip_prefix("set "))
                    {
                        let raw = raw.trim_end_matches('?').trim();
                        let is_neg = raw.starts_with("no");
                        let mut key_part = if is_neg { &raw[2..] } else { raw };
                        if let Some((k, _)) = key_part.split_once('=') {
                            key_part = k;
                        }
                        let canonical = match key_part {
                            "nu" => "number",
                            "rnu" => "relativenumber",
                            "cul" => "cursorline",
                            "smk" => "showmarks",
                            "et" => "expandtab",
                            "ai" => "autoindent",
                            "ic" => "ignorecase",
                            "scs" => "smartcase",
                            "hls" => "hlsearch",
                            "is" => "incsearch",
                            "lbr" => "linebreak",
                            "udf" => "undofile",
                            "bk" => "backup",
                            "swf" => "swapfile",
                            "aw" => "autosave",
                            "ls" => "laststatus",
                            "sc" => "showcmd",
                            "ppr" => "percentpathroot",
                            "syn" => "syntax",
                            "ts" => "tabstop",
                            "ul" => "undolevels",
                            "so" => "scrolloff",
                            "siso" => "sidescrolloff",
                            "tm" => "timeoutlen",
                            "colo" => "colorscheme",
                            other => other,
                        };
                        let canon_key = if is_neg {
                            format!("neg:{}", canonical)
                        } else {
                            format!("pos:{}", canonical)
                        };
                        // Drop query if we already have plain version
                        return !plain_canon.contains(&canon_key);
                    }
                    true
                });
            }
        }

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
        let set_prefix: &str = if lower.starts_with("setp ") {
            "setp "
        } else {
            "set "
        };
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

        // Helper: get the substring after the last space (value prefix for positional args)
        fn value_prefix_space(s: &str) -> &str {
            s.rsplit_once(' ').map(|(_, v)| v).unwrap_or("")
        }

        // colorscheme / colo dynamic values from themes.toml
        if lower.starts_with(&format!("{set_prefix}colorscheme "))
            || lower.starts_with(&format!("{set_prefix}colo "))
        {
            let val_pref = value_prefix_space(trimmed);
            let cfg = ThemeConfig::load();
            // Build list of (name, description)
            for (name, theme) in cfg.themes.iter() {
                if val_pref.is_empty() || name.to_lowercase().starts_with(&val_pref.to_lowercase())
                {
                    out.push(CompletionItem {
                        text: format!("{}colorscheme {}", set_prefix, name),
                        description: format!("Theme: {}", theme.description),
                        category: "set".to_string(),
                    });
                }
            }
        }

        // Boolean positional suggestions: percentpathroot / ppr true|false
        if lower.starts_with(&format!("{set_prefix}percentpathroot "))
            || lower.starts_with(&format!("{set_prefix}ppr "))
        {
            for val in ["true", "false"] {
                out.push(CompletionItem {
                    text: if lower.contains(" ppr ") {
                        format!("{}ppr {}", set_prefix, val)
                    } else {
                        format!("{}percentpathroot {}", set_prefix, val)
                    },
                    description: "Boolean".to_string(),
                    category: "set".to_string(),
                });
            }
        }

        // Numeric positional suggestions (common values)
        let add_numeric_suggestions = |out: &mut Vec<CompletionItem>, key: &str, vals: &[&str]| {
            for v in vals {
                out.push(CompletionItem {
                    text: format!("{}{} {}", set_prefix, key, v),
                    description: "Value".to_string(),
                    category: "set".to_string(),
                });
            }
        };

        if lower.starts_with(&format!("{set_prefix}tabstop "))
            || lower.starts_with(&format!("{set_prefix}ts "))
        {
            add_numeric_suggestions(
                &mut out,
                if lower.contains(" ts ") {
                    "ts"
                } else {
                    "tabstop"
                },
                &["2", "4", "8"],
            );
        }
        if lower.starts_with(&format!("{set_prefix}scrolloff "))
            || lower.starts_with(&format!("{set_prefix}so "))
        {
            add_numeric_suggestions(
                &mut out,
                if lower.contains(" so ") {
                    "so"
                } else {
                    "scrolloff"
                },
                &["0", "3", "5"],
            );
        }
        if lower.starts_with(&format!("{set_prefix}sidescrolloff "))
            || lower.starts_with(&format!("{set_prefix}siso "))
        {
            add_numeric_suggestions(
                &mut out,
                if lower.contains(" siso ") {
                    "siso"
                } else {
                    "sidescrolloff"
                },
                &["0", "5", "10"],
            );
        }
        if lower.starts_with(&format!("{set_prefix}timeoutlen "))
            || lower.starts_with(&format!("{set_prefix}tm "))
        {
            add_numeric_suggestions(
                &mut out,
                if lower.contains(" tm ") {
                    "tm"
                } else {
                    "timeoutlen"
                },
                &["500", "750", "1000"],
            );
        }
        if lower.starts_with(&format!("{set_prefix}undolevels "))
            || lower.starts_with(&format!("{set_prefix}ul "))
        {
            add_numeric_suggestions(
                &mut out,
                if lower.contains(" ul ") {
                    "ul"
                } else {
                    "undolevels"
                },
                &["100", "200", "1000"],
            );
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
            let use_buf_root = input_path.starts_with('%') && ctx.allow_percent_path_root;
            if use_buf_root {
                input_path = &input_path[1..];
                // Treat a leading path separator as relative to the buffer root
                while input_path.starts_with('/') || input_path.starts_with('\\') {
                    input_path = &input_path[1..];
                }
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

        // Remove legacy '=' based suggestions; replaced by positional logic above

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
            let mut completed = selected.text.clone();
            // Smart toggle for ':set' booleans: if the selected is a boolean key without an
            // explicit value, emit the form that switches the current setting.
            if (selected.text.starts_with("set ") || selected.text.starts_with("setp "))
                && let Some(ctx) = &self.context
            {
                let after = selected
                    .text
                    .strip_prefix("setp ")
                    .or_else(|| selected.text.strip_prefix("set "))
                    .unwrap_or(&selected.text);
                let key = after.trim();
                // Only handle plain boolean keys (no spaces/values)
                if !key.contains(' ') && !key.contains('=') {
                    // Normalize negative prefix
                    let base = if let Some(rest) = key.strip_prefix("no") {
                        rest
                    } else {
                        key
                    };
                    // Determine if base is a known boolean and current value
                    let cur_true = match base {
                        "number" => ctx.number,
                        "relativenumber" => ctx.relativenumber,
                        "cursorline" => ctx.cursorline,
                        "showmarks" => ctx.showmarks,
                        "expandtab" => ctx.expandtab,
                        "autoindent" => ctx.autoindent,
                        "ignorecase" => ctx.ignorecase,
                        "smartcase" => ctx.smartcase,
                        "hlsearch" => ctx.hlsearch,
                        "incsearch" => ctx.incsearch,
                        "wrap" => ctx.wrap,
                        "linebreak" => ctx.linebreak,
                        "undofile" => ctx.undofile,
                        "backup" => ctx.backup,
                        "swapfile" => ctx.swapfile,
                        "autosave" => ctx.autosave,
                        "laststatus" => ctx.laststatus,
                        "showcmd" => ctx.showcmd,
                        "syntax" => ctx.syntax,
                        "percentpathroot" => ctx.percentpathroot,
                        _ => false,
                    };
                    // If it's a known boolean, emit the opposite of current regardless of the
                    // form displayed in the row (keeps action consistent)
                    if base != key { /* no-op, handled below */ }
                    if [
                        "number",
                        "relativenumber",
                        "cursorline",
                        "showmarks",
                        "expandtab",
                        "autoindent",
                        "ignorecase",
                        "smartcase",
                        "hlsearch",
                        "incsearch",
                        "wrap",
                        "linebreak",
                        "undofile",
                        "backup",
                        "swapfile",
                        "autosave",
                        "laststatus",
                        "showcmd",
                        "syntax",
                        "percentpathroot",
                    ]
                    .contains(&base)
                    {
                        let prefix = if selected.text.starts_with("setp ") {
                            "setp"
                        } else {
                            "set"
                        };
                        if cur_true {
                            completed = format!("{} no{}", prefix, base);
                        } else {
                            completed = format!("{} {}", prefix, base);
                        }
                    }
                }
            }
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
    /// Whether '%' path rooting is enabled by config
    pub allow_percent_path_root: bool,
    // Current boolean option values for ':set' toggles
    pub number: bool,
    pub relativenumber: bool,
    pub cursorline: bool,
    pub showmarks: bool,
    pub expandtab: bool,
    pub autoindent: bool,
    pub ignorecase: bool,
    pub smartcase: bool,
    pub hlsearch: bool,
    pub incsearch: bool,
    pub wrap: bool,
    pub linebreak: bool,
    pub undofile: bool,
    pub backup: bool,
    pub swapfile: bool,
    pub autosave: bool,
    pub laststatus: bool,
    pub showcmd: bool,
    pub syntax: bool,
    pub percentpathroot: bool,
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
