use super::engine::{CompletionContext, CompletionItem};
use crate::config::theme::ThemeConfig;

// Provider trait and registry to decouple the engine from concrete sources.
pub trait CompletionProvider {
    fn static_items(&self) -> Vec<CompletionItem>;
    fn dynamic_items(&self, ctx: Option<&CompletionContext>, input: &str) -> Vec<CompletionItem>;
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: Vec<Box<dyn CompletionProvider + Send + Sync>>,
}

impl ProviderRegistry {
    pub fn new_with_defaults() -> Self {
        let mut reg = Self {
            providers: Vec::new(),
        };
        // Focused providers; dynamic responsibilities are partitioned to avoid duplicates
        reg.register(Box::new(ExCommandsProvider));
        reg.register(Box::new(SetOptionsProvider));
        reg.register(Box::new(BuffersProvider));
        reg.register(Box::new(FilesProvider));
        reg.register(Box::new(ThemesProvider));
        reg.register(Box::new(NumericHintsProvider));
        reg.register(Box::new(PercentBoolProvider));
        reg
    }

    pub fn register(&mut self, provider: Box<dyn CompletionProvider + Send + Sync>) {
        self.providers.push(provider);
    }

    pub fn static_items(&self) -> Vec<CompletionItem> {
        let mut all = Vec::new();
        for p in &self.providers {
            all.extend(p.static_items());
        }
        all
    }

    pub fn dynamic_items(
        &self,
        ctx: Option<&CompletionContext>,
        input: &str,
    ) -> Vec<CompletionItem> {
        let mut all = Vec::new();
        for p in &self.providers {
            all.extend(p.dynamic_items(ctx, input));
        }
        all
    }
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field("providers_len", &self.providers.len())
            .finish()
    }
}

/// Static commands provider: returns the full catalog.
pub fn static_commands() -> Vec<CompletionItem> {
    let mut commands = Vec::new();

    // Basic ex commands
    commands.extend(vec![
        CompletionItem {
            text: "quit".into(),
            description: "Quit editor".into(),
            category: "file".into(),
        },
        CompletionItem {
            text: "q".into(),
            description: "Quit editor (short)".into(),
            category: "file".into(),
        },
        CompletionItem {
            text: "quit!".into(),
            description: "Force quit without saving".into(),
            category: "file".into(),
        },
        CompletionItem {
            text: "q!".into(),
            description: "Force quit without saving (short)".into(),
            category: "file".into(),
        },
        CompletionItem {
            text: "write".into(),
            description: "Save current file".into(),
            category: "file".into(),
        },
        CompletionItem {
            text: "w".into(),
            description: "Save current file (short)".into(),
            category: "file".into(),
        },
        CompletionItem {
            text: "wq".into(),
            description: "Save and quit".into(),
            category: "file".into(),
        },
        CompletionItem {
            text: "x".into(),
            description: "Save and quit (short)".into(),
            category: "file".into(),
        },
        CompletionItem {
            text: "edit".into(),
            description: "Edit file in new buffer".into(),
            category: "file".into(),
        },
        CompletionItem {
            text: "e".into(),
            description: "Edit file in new buffer (short)".into(),
            category: "file".into(),
        },
    ]);

    // Buffer management
    commands.extend(vec![
        CompletionItem {
            text: "buffer".into(),
            description: "Switch to buffer".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "b".into(),
            description: "Switch to buffer (short)".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "bnext".into(),
            description: "Switch to next buffer".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "bn".into(),
            description: "Switch to next buffer (short)".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "bprevious".into(),
            description: "Switch to previous buffer".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "bp".into(),
            description: "Switch to previous buffer (short)".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "bprev".into(),
            description: "Switch to previous buffer".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "bdelete".into(),
            description: "Delete current buffer".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "bd".into(),
            description: "Delete current buffer (short)".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "bdelete!".into(),
            description: "Force delete current buffer".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "bd!".into(),
            description: "Force delete current buffer (short)".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "ls".into(),
            description: "List all buffers".into(),
            category: "buffer".into(),
        },
        CompletionItem {
            text: "buffers".into(),
            description: "List all buffers".into(),
            category: "buffer".into(),
        },
    ]);

    // Window/split
    commands.extend(vec![
        CompletionItem {
            text: "split".into(),
            description: "Create horizontal split".into(),
            category: "window".into(),
        },
        CompletionItem {
            text: "sp".into(),
            description: "Create horizontal split (short)".into(),
            category: "window".into(),
        },
        CompletionItem {
            text: "vsplit".into(),
            description: "Create vertical split".into(),
            category: "window".into(),
        },
        CompletionItem {
            text: "vsp".into(),
            description: "Create vertical split (short)".into(),
            category: "window".into(),
        },
        CompletionItem {
            text: "close".into(),
            description: "Close current window".into(),
            category: "window".into(),
        },
    ]);

    // Info/tools
    commands.extend(vec![
        CompletionItem {
            text: "registers".into(),
            description: "Show registers view".into(),
            category: "info".into(),
        },
        CompletionItem {
            text: "reg".into(),
            description: "Show registers view (short)".into(),
            category: "info".into(),
        },
    ]);

    // Markdown preview Ex commands
    commands.extend(vec![
        CompletionItem {
            text: "MarkdownPreviewOpen".into(),
            description: "Open Markdown preview split".into(),
            category: "preview".into(),
        },
        CompletionItem {
            text: "MarkdownPreviewClose".into(),
            description: "Close Markdown preview split".into(),
            category: "preview".into(),
        },
        CompletionItem {
            text: "MarkdownPreviewToggle".into(),
            description: "Toggle Markdown preview split".into(),
            category: "preview".into(),
        },
        CompletionItem {
            text: "MarkdownPreviewRefresh".into(),
            description: "Refresh Markdown preview content".into(),
            category: "preview".into(),
        },
    ]);

    // Set commands - display
    commands.extend(vec![
        CompletionItem {
            text: "set number".into(),
            description: "Show line numbers".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nu".into(),
            description: "Show line numbers (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nonumber".into(),
            description: "Hide line numbers".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nonu".into(),
            description: "Hide line numbers (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set relativenumber".into(),
            description: "Show relative line numbers".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set rnu".into(),
            description: "Show relative line numbers (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set norelativenumber".into(),
            description: "Hide relative line numbers".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nornu".into(),
            description: "Hide relative line numbers (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set cursorline".into(),
            description: "Highlight cursor line".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set cul".into(),
            description: "Highlight cursor line (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nocursorline".into(),
            description: "Disable cursor line highlight".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nocul".into(),
            description: "Disable cursor line highlight (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set showmarks".into(),
            description: "Show marks in gutter/number column".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set smk".into(),
            description: "Show marks (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noshowmarks".into(),
            description: "Hide marks in gutter/number column".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nosmk".into(),
            description: "Hide marks (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set wrap".into(),
            description: "Enable soft line wrapping".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nowrap".into(),
            description: "Disable soft line wrapping".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set linebreak".into(),
            description: "Prefer breaking at word boundaries when wrapping".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set lbr".into(),
            description: "Prefer word-boundary breaks (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nolinebreak".into(),
            description: "Disable word-boundary preference".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nolbr".into(),
            description: "Disable word-boundary preference (short)".into(),
            category: "set".into(),
        },
    ]);

    // Set commands - search/navigation
    commands.extend(vec![
        CompletionItem {
            text: "set ignorecase".into(),
            description: "Case-insensitive search".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set ic".into(),
            description: "Case-insensitive search (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noignorecase".into(),
            description: "Case-sensitive search".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noic".into(),
            description: "Case-sensitive search (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set smartcase".into(),
            description: "Smart case matching".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set scs".into(),
            description: "Smart case matching (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nosmartcase".into(),
            description: "Disable smart case".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noscs".into(),
            description: "Disable smart case (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set hlsearch".into(),
            description: "Highlight search results".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set hls".into(),
            description: "Highlight search results (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nohlsearch".into(),
            description: "Disable search highlighting".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nohls".into(),
            description: "Disable search highlighting (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set incsearch".into(),
            description: "Incremental search".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set is".into(),
            description: "Incremental search (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noincsearch".into(),
            description: "Disable incremental search".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nois".into(),
            description: "Disable incremental search (short)".into(),
            category: "set".into(),
        },
    ]);

    // Set commands - editing toggles
    commands.extend(vec![
        CompletionItem {
            text: "set expandtab".into(),
            description: "Insert spaces for tabs".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set et".into(),
            description: "Insert spaces for tabs (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noexpandtab".into(),
            description: "Use hard tab characters".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noet".into(),
            description: "Use hard tab characters (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set autoindent".into(),
            description: "Enable automatic indentation".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set ai".into(),
            description: "Enable automatic indentation (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noautoindent".into(),
            description: "Disable automatic indentation".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noai".into(),
            description: "Disable automatic indentation (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set smartindent".into(),
            description: "Enable smart block indentation".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set si".into(),
            description: "Enable smart block indentation (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nosmartindent".into(),
            description: "Disable smart block indentation".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nosi".into(),
            description: "Disable smart block indentation (short)".into(),
            category: "set".into(),
        },
    ]);

    // Set commands - files/persistence
    commands.extend(vec![
        CompletionItem {
            text: "set undofile".into(),
            description: "Enable persistent undo".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set udf".into(),
            description: "Enable persistent undo (short)".into(),
            category: "set".into(),
        },
        // Markdown preview related :set keys
        CompletionItem {
            text: "set mdpreview.update".into(),
            description: "Markdown preview refresh policy".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set mdpreview.debounce_ms".into(),
            description: "Markdown preview debounce in ms".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set mdpreview.scrollsync".into(),
            description: "Sync preview scroll with source".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nomdpreview.scrollsync".into(),
            description: "Disable preview scroll sync".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set mdpreview.math".into(),
            description: "Math rendering mode".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set mdpreview.large_file_mode".into(),
            description: "Strategy for very large markdown files".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noundofile".into(),
            description: "Disable persistent undo".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noudf".into(),
            description: "Disable persistent undo (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set backup".into(),
            description: "Enable backup files".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set bk".into(),
            description: "Enable backup files (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nobackup".into(),
            description: "Disable backup files".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nobk".into(),
            description: "Disable backup files (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set swapfile".into(),
            description: "Enable swap file".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set swf".into(),
            description: "Enable swap file (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noswapfile".into(),
            description: "Disable swap file".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noswf".into(),
            description: "Disable swap file (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set autosave".into(),
            description: "Enable auto save".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set aw".into(),
            description: "Enable auto save (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noautosave".into(),
            description: "Disable auto save".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noaw".into(),
            description: "Disable auto save (short)".into(),
            category: "set".into(),
        },
    ]);

    // Set commands - UI/syntax
    commands.extend(vec![
        CompletionItem {
            text: "set laststatus".into(),
            description: "Show status line".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set ls".into(),
            description: "Show status line (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nolaststatus".into(),
            description: "Hide status line".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nols".into(),
            description: "Hide status line (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set showcmd".into(),
            description: "Show command in status area".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set sc".into(),
            description: "Show command (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noshowcmd".into(),
            description: "Hide command in status area".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nosc".into(),
            description: "Hide command (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set syntax".into(),
            description: "Enable syntax highlighting".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set syn".into(),
            description: "Enable syntax highlighting (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nosyntax".into(),
            description: "Disable syntax highlighting".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nosyn".into(),
            description: "Disable syntax highlighting (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set percentpathroot".into(),
            description: "Enable '%' root in path completion".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set nopercentpathroot".into(),
            description: "Disable '%' root in path completion".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set percentpathroot ".into(),
            description: "Set '%' root behavior (true/false)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set ppr".into(),
            description: "Enable '%' root in path completion (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set noppr".into(),
            description: "Disable '%' root in path completion (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set ppr ".into(),
            description: "Set '%' root behavior (short)".into(),
            category: "set".into(),
        },
    ]);

    // Positional values (no '=')
    commands.extend(vec![
        CompletionItem {
            text: "set tabstop ".into(),
            description: "Set tab width".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set ts ".into(),
            description: "Set tab width (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set scrolloff ".into(),
            description: "Lines to keep around cursor".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set so ".into(),
            description: "Lines to keep around cursor (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set sidescrolloff ".into(),
            description: "Columns to keep around cursor".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set siso ".into(),
            description: "Columns to keep around cursor (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set colorscheme ".into(),
            description: "Change color scheme".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set colo ".into(),
            description: "Change color scheme (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set timeoutlen ".into(),
            description: "Set command timeout (ms)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set tm ".into(),
            description: "Set command timeout (ms) (short)".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set undolevels ".into(),
            description: "Set number of undo levels".into(),
            category: "set".into(),
        },
        CompletionItem {
            text: "set ul ".into(),
            description: "Set undo levels (short)".into(),
            category: "set".into(),
        },
    ]);

    commands
}

/// Dynamic matches provider: depends on input and context.
pub fn dynamic_matches(ctx: Option<&CompletionContext>, input: &str) -> Vec<CompletionItem> {
    let trimmed = input.trim_start();
    let lower = trimmed.to_lowercase();
    let mut out: Vec<CompletionItem> = Vec::new();
    let set_prefix: &str = if lower.starts_with("setp ") {
        "setp "
    } else {
        "set "
    };

    // Buffers: b <...> or buffer <...>
    if (lower.starts_with("b ") || lower.starts_with("buffer "))
        && ctx.is_some()
        && let Some(ctx) = ctx
    {
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
                out.push(CompletionItem {
                    text: format!("b {}", b.id),
                    description: format!(
                        "Buffer {}: {}{}",
                        b.id,
                        b.name,
                        if b.modified { " [+]" } else { "" }
                    ),
                    category: "buffer".into(),
                });
                if b.name != "[No Name]" {
                    out.push(CompletionItem {
                        text: format!("b {}", b.name),
                        description: format!(
                            "Buffer {}: {}{}",
                            b.id,
                            b.name,
                            if b.modified { " [+]" } else { "" }
                        ),
                        category: "buffer".into(),
                    });
                }
            }
        }
    }

    // Helper: value prefix after last space
    fn value_prefix_space(s: &str) -> &str {
        s.rsplit_once(' ').map(|(_, v)| v).unwrap_or("")
    }

    // colorscheme
    if lower.starts_with(&format!("{set_prefix}colorscheme "))
        || lower.starts_with(&format!("{set_prefix}colo "))
    {
        let val_pref = value_prefix_space(trimmed);
        let cfg = ThemeConfig::load();
        for (name, theme) in cfg.themes.iter() {
            if val_pref.is_empty() || name.to_lowercase().starts_with(&val_pref.to_lowercase()) {
                out.push(CompletionItem {
                    text: format!("{}colorscheme {}", set_prefix, name),
                    description: format!("Theme: {}", theme.description),
                    category: "set".into(),
                });
            }
        }
    }

    // percentpathroot bool positional
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
                description: "Boolean".into(),
                category: "set".into(),
            });
        }
    }

    // Numeric suggestions helper
    let add_numeric = |out: &mut Vec<CompletionItem>, key: &str, vals: &[&str]| {
        for v in vals {
            out.push(CompletionItem {
                text: format!("{}{} {}", set_prefix, key, v),
                description: "Value".into(),
                category: "set".into(),
            });
        }
    };

    if lower.starts_with(&format!("{set_prefix}tabstop "))
        || lower.starts_with(&format!("{set_prefix}ts "))
    {
        add_numeric(
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
        add_numeric(
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
        add_numeric(
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
        add_numeric(
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
        add_numeric(
            &mut out,
            if lower.contains(" ul ") {
                "ul"
            } else {
                "undolevels"
            },
            &["100", "200", "1000"],
        );
    }

    // mdpreview.debounce_ms numeric suggestions
    if lower.starts_with(&format!("{set_prefix}mdpreview.debounce_ms ")) {
        add_numeric(
            &mut out,
            "mdpreview.debounce_ms",
            &["0", "50", "100", "200", "500"],
        );
    }

    // mdpreview.update enum suggestions
    if lower.starts_with(&format!("{set_prefix}mdpreview.update ")) {
        for v in ["manual", "on_save", "live"] {
            out.push(CompletionItem {
                text: format!("{}mdpreview.update {}", set_prefix, v),
                description: "Value".into(),
                category: "set".into(),
            });
        }
    }

    // mdpreview.math enum suggestions
    if lower.starts_with(&format!("{set_prefix}mdpreview.math ")) {
        for v in ["off", "inline", "block"] {
            out.push(CompletionItem {
                text: format!("{}mdpreview.math {}", set_prefix, v),
                description: "Value".into(),
                category: "set".into(),
            });
        }
    }

    // mdpreview.large_file_mode enum suggestions
    if lower.starts_with(&format!("{set_prefix}mdpreview.large_file_mode ")) {
        for v in ["truncate", "disable"] {
            out.push(CompletionItem {
                text: format!("{}mdpreview.large_file_mode {}", set_prefix, v),
                description: "Value".into(),
                category: "set".into(),
            });
        }
    }

    // File path completion for :e and :w commands
    if (lower.starts_with("e ")
        || lower.starts_with("edit ")
        || lower.starts_with("w ")
        || lower.starts_with("write "))
        && ctx.is_some()
        && let Some(ctx) = ctx
    {
        let raw = trimmed
            .strip_prefix("e ")
            .or_else(|| trimmed.strip_prefix("edit "))
            .or_else(|| trimmed.strip_prefix("w "))
            .or_else(|| trimmed.strip_prefix("write "))
            .unwrap_or_default();
        let mut input_path = raw.trim_start();

        let use_buf_root = input_path.starts_with('%') && ctx.allow_percent_path_root;
        if use_buf_root {
            input_path = &input_path[1..];
            while input_path.starts_with('/') || input_path.starts_with('\\') {
                input_path = &input_path[1..];
            }
        }

        use std::fs;
        use std::path::{Path, PathBuf};
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
                if !text.ends_with(std::path::MAIN_SEPARATOR) {
                    text.push(std::path::MAIN_SEPARATOR);
                }
                out.push(CompletionItem {
                    text,
                    description: "Parent Directory".into(),
                    category: "file".into(),
                });
            }
            for entry in read_dir.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if !filter.is_empty() && !name.to_lowercase().starts_with(&filter.to_lowercase()) {
                    continue;
                }
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
                if !input_path.is_empty() {
                    let suggested = if Path::new(input_path).is_absolute() {
                        dir.join(&name)
                    } else {
                        let abs = dir.join(&name);
                        abs.strip_prefix(&base_root).unwrap_or(&abs).to_path_buf()
                    };
                    text.push_str(&suggested.to_string_lossy());
                } else {
                    let rel = path.strip_prefix(&base_root).unwrap_or(&path).to_path_buf();
                    text.push_str(&rel.to_string_lossy());
                }
                if path.is_dir() && !text.ends_with(std::path::MAIN_SEPARATOR) {
                    text.push(std::path::MAIN_SEPARATOR);
                }
                let desc = if path.is_dir() { "Directory" } else { "File" };
                out.push(CompletionItem {
                    text,
                    description: desc.into(),
                    category: "file".into(),
                });
            }
        }
    }

    out
}

// Focused provider implementations

pub struct ExCommandsProvider;
impl CompletionProvider for ExCommandsProvider {
    fn static_items(&self) -> Vec<CompletionItem> {
        let mut v = static_commands();
        v.retain(|c| c.category != "set");
        v
    }
    fn dynamic_items(&self, _ctx: Option<&CompletionContext>, _input: &str) -> Vec<CompletionItem> {
        Vec::new()
    }
}

pub struct SetOptionsProvider;
impl CompletionProvider for SetOptionsProvider {
    fn static_items(&self) -> Vec<CompletionItem> {
        let mut v = static_commands();
        v.retain(|c| c.category == "set");
        v
    }
    fn dynamic_items(&self, ctx: Option<&CompletionContext>, input: &str) -> Vec<CompletionItem> {
        // Forward dynamic matches for ':set' space/value completions
        let mut v = dynamic_matches(ctx, input);
        v.retain(|c| c.category == "set");
        v
    }
}

pub struct BuffersProvider;
impl CompletionProvider for BuffersProvider {
    fn static_items(&self) -> Vec<CompletionItem> {
        Vec::new()
    }
    fn dynamic_items(&self, ctx: Option<&CompletionContext>, input: &str) -> Vec<CompletionItem> {
        let mut v = dynamic_matches(ctx, input);
        v.retain(|c| c.category == "buffer");
        v
    }
}

pub struct FilesProvider;
impl CompletionProvider for FilesProvider {
    fn static_items(&self) -> Vec<CompletionItem> {
        Vec::new()
    }
    fn dynamic_items(&self, ctx: Option<&CompletionContext>, input: &str) -> Vec<CompletionItem> {
        let mut v = dynamic_matches(ctx, input);
        v.retain(|c| c.category == "file");
        v
    }
}

pub struct ThemesProvider;
impl CompletionProvider for ThemesProvider {
    fn static_items(&self) -> Vec<CompletionItem> {
        Vec::new()
    }
    fn dynamic_items(&self, ctx: Option<&CompletionContext>, input: &str) -> Vec<CompletionItem> {
        let mut v = dynamic_matches(ctx, input);
        v.retain(|c| c.category == "set" && c.text.contains("colorscheme "));
        v
    }
}

pub struct NumericHintsProvider;
impl CompletionProvider for NumericHintsProvider {
    fn static_items(&self) -> Vec<CompletionItem> {
        Vec::new()
    }
    fn dynamic_items(&self, ctx: Option<&CompletionContext>, input: &str) -> Vec<CompletionItem> {
        let mut v = dynamic_matches(ctx, input);
        v.retain(|c| {
            if c.category != "set" {
                return false;
            }
            let t = c.text.to_lowercase();
            t.contains(" tabstop ")
                || t.contains(" ts ")
                || t.contains(" scrolloff ")
                || t.contains(" so ")
                || t.contains(" sidescrolloff ")
                || t.contains(" siso ")
                || t.contains(" timeoutlen ")
                || t.contains(" tm ")
                || t.contains(" undolevels ")
                || t.contains(" ul ")
        });
        v
    }
}

pub struct PercentBoolProvider;
impl CompletionProvider for PercentBoolProvider {
    fn static_items(&self) -> Vec<CompletionItem> {
        Vec::new()
    }
    fn dynamic_items(&self, ctx: Option<&CompletionContext>, input: &str) -> Vec<CompletionItem> {
        let mut v = dynamic_matches(ctx, input);
        v.retain(|c| {
            c.category == "set"
                && (c.text.contains(" percentpathroot true")
                    || c.text.contains(" percentpathroot false")
                    || c.text.contains(" ppr true")
                    || c.text.contains(" ppr false"))
        });
        v
    }
}
