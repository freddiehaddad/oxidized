use oxidized::features::completion::{
    CommandCompletionBuilder, CompletionContext, CompletionItem, CompletionProvider,
};

#[derive(Default)]
struct TestProvider;

impl CompletionProvider for TestProvider {
    fn static_items(&self) -> Vec<CompletionItem> {
        vec![CompletionItem {
            text: "zcustom".into(),
            description: "Custom provider item".into(),
            category: "custom".into(),
        }]
    }

    fn dynamic_items(
        &self,
        _ctx: Option<&oxidized::features::completion::CompletionContext>,
        _input: &str,
    ) -> Vec<CompletionItem> {
        Vec::new()
    }
}

#[test]
fn builder_can_register_custom_provider() {
    let mut cc = CommandCompletionBuilder::new()
        .add_provider(Box::new(TestProvider))
        .build();

    // Minimal context
    cc.set_context(CompletionContext {
        cwd: std::env::current_dir().unwrap(),
        buffers: vec![],
        current_buffer_dir: None,
        allow_percent_path_root: true,
        number: false,
        relativenumber: false,
        cursorline: false,
        showmarks: false,
        expandtab: false,
        autoindent: false,
        smartindent: false,
        ignorecase: false,
        smartcase: false,
        hlsearch: false,
        incsearch: false,
        wrap: false,
        linebreak: false,
        undofile: false,
        backup: false,
        swapfile: false,
        autosave: false,
        laststatus: false,
        showcmd: false,
        syntax: false,
        percentpathroot: true,
    });

    // Our custom item should be discoverable (inputs do not include a leading ':')
    cc.start_completion("z");
    assert!(cc.should_show());
    let has_custom = cc
        .matches
        .iter()
        .any(|i| i.category == "custom" && i.text == "zcustom");
    assert!(has_custom, "custom provider item missing");
}
