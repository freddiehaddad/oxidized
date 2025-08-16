use log::debug;
use std::path::PathBuf;
use std::sync::Arc;

use super::presenter::{CompletionPresenter, DefaultPresenter};
use super::providers::ProviderRegistry;

#[derive(Clone)]
struct PresenterHandle(Arc<dyn CompletionPresenter>);

impl PresenterHandle {
    fn new<P: CompletionPresenter + 'static>(p: P) -> Self {
        Self(Arc::new(p))
    }
}

impl std::fmt::Debug for PresenterHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PresenterHandle").finish()
    }
}

#[derive(Debug, Clone)]
pub struct CommandCompletion {
    commands: Vec<CompletionItem>,
    providers: Arc<ProviderRegistry>,
    presenter: PresenterHandle,
    pub active: bool,
    pub matches: Vec<CompletionItem>,
    pub selected_index: usize,
    pub completion_prefix: String,
    pub context: Option<CompletionContext>,
}

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub text: String,
    pub description: String,
    pub category: String,
}

impl CommandCompletion {
    pub fn with_components<P: CompletionPresenter + 'static>(
        providers: ProviderRegistry,
        presenter: P,
    ) -> Self {
        let providers = Arc::new(providers);
        let commands = providers.static_items();
        let presenter = PresenterHandle::new(presenter);
        Self {
            commands,
            providers,
            presenter,
            active: false,
            matches: Vec::new(),
            selected_index: 0,
            completion_prefix: String::new(),
            context: None,
        }
    }
}

#[derive(Debug)]
pub struct CommandCompletionBuilder {
    registry: ProviderRegistry,
    presenter: PresenterHandle,
}

impl CommandCompletionBuilder {
    pub fn new() -> Self {
        Self {
            registry: ProviderRegistry::new_with_defaults(),
            presenter: PresenterHandle::new(DefaultPresenter),
        }
    }

    pub fn with_registry(mut self, registry: ProviderRegistry) -> Self {
        self.registry = registry;
        self
    }

    pub fn with_presenter<P: CompletionPresenter + 'static>(mut self, presenter: P) -> Self {
        self.presenter = PresenterHandle::new(presenter);
        self
    }

    pub fn add_provider(
        mut self,
        provider: Box<dyn super::providers::CompletionProvider + Send + Sync>,
    ) -> Self {
        self.registry.register(provider);
        self
    }

    pub fn build(self) -> CommandCompletion {
        let providers = Arc::new(self.registry);
        let commands = providers.static_items();
        CommandCompletion {
            commands,
            providers,
            presenter: self.presenter,
            active: false,
            matches: Vec::new(),
            selected_index: 0,
            completion_prefix: String::new(),
            context: None,
        }
    }
}

impl Default for CommandCompletionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandCompletion {
    pub fn set_context(&mut self, ctx: CompletionContext) {
        self.context = Some(ctx);
    }

    pub fn start_completion(&mut self, input: &str) {
        debug!("Starting command completion for input: '{}'", input);
        self.active = true;
        self.completion_prefix = input.to_string();
        self.update_matches(input);
        self.selected_index = 0;
        debug!("Found {} completion matches", self.matches.len());
    }

    fn update_matches(&mut self, input: &str) {
        let mut all = self.commands.clone();
        let dynamic = self.providers.dynamic_items(self.context.as_ref(), input);
        all.extend(dynamic);
        self.matches = self.presenter.0.present(all, input, self.context.as_ref());
    }

    pub fn next(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.matches.len();
        }
    }
    pub fn previous(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.matches.len() - 1
            } else {
                self.selected_index - 1
            };
        }
    }
    pub fn selected(&self) -> Option<&CompletionItem> {
        self.matches.get(self.selected_index)
    }

    pub fn accept(&mut self) -> Option<String> {
        if let Some(selected) = self.selected() {
            let mut completed = selected.text.clone();
            if (selected.text.starts_with("set ") || selected.text.starts_with("setp "))
                && let Some(ctx) = &self.context
            {
                let after = selected
                    .text
                    .strip_prefix("setp ")
                    .or_else(|| selected.text.strip_prefix("set "))
                    .unwrap_or(&selected.text);
                let key = after.trim();
                if !key.contains(' ') && !key.contains('=') {
                    let base = if let Some(rest) = key.strip_prefix("no") {
                        rest
                    } else {
                        key
                    };
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

    pub fn cancel(&mut self) {
        self.active = false;
        self.matches.clear();
        self.selected_index = 0;
        self.completion_prefix.clear();
    }

    pub fn should_show(&self) -> bool {
        self.active && !self.matches.is_empty()
    }

    pub fn visible_items(&self, max_items: usize) -> &[CompletionItem] {
        if self.matches.is_empty() {
            return &[];
        }
        let start_idx = if self.matches.len() <= max_items {
            0
        } else {
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

    pub fn visible_selected_index(&self, max_items: usize) -> usize {
        let visible_items = self.visible_items(max_items);
        if visible_items.is_empty() {
            return 0;
        }
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
    pub current_buffer_dir: Option<PathBuf>,
    pub allow_percent_path_root: bool,
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

impl Default for CommandCompletion {
    fn default() -> Self {
        CommandCompletionBuilder::new().build()
    }
}
