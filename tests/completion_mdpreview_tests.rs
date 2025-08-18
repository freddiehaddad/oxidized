#[test]
fn markdown_preview_ex_commands_appear() {
    let mut cc = oxidized::features::completion::CommandCompletionBuilder::new().build();
    // No special context needed for Ex command completions
    cc.start_completion("Ma");
    assert!(cc.should_show());
    let items: Vec<String> = cc.matches.iter().map(|i| i.text.clone()).collect();
    for cmd in [
        "MarkdownPreviewOpen",
        "MarkdownPreviewClose",
        "MarkdownPreviewToggle",
        "MarkdownPreviewRefresh",
    ] {
        assert!(
            items.iter().any(|t| t == cmd),
            "missing completion for {}",
            cmd
        );
    }
}

#[test]
fn set_mdpreview_keys_appear() {
    let mut cc = oxidized::features::completion::CommandCompletionBuilder::new().build();
    cc.start_completion("set mdpreview.");
    assert!(cc.should_show());
    let items: Vec<String> = cc.matches.iter().map(|i| i.text.clone()).collect();
    for key in [
        "set mdpreview.update",
        "set mdpreview.debounce_ms",
        "set mdpreview.scrollsync",
        "set mdpreview.math",
        "set mdpreview.large_file_mode",
    ] {
        assert!(
            items.iter().any(|t| t == key),
            "missing completion for {}",
            key
        );
    }
}

#[test]
fn setp_mdpreview_keys_appear_with_setp_prefix() {
    // Presenter should normalize suggestions to the setp prefix when typed
    let mut cc = oxidized::features::completion::CommandCompletionBuilder::new().build();
    cc.start_completion("setp mdpreview.");
    assert!(cc.should_show());
    let items: Vec<String> = cc.matches.iter().map(|i| i.text.clone()).collect();
    for key in [
        "setp mdpreview.update",
        "setp mdpreview.debounce_ms",
        "setp mdpreview.scrollsync",
        "setp mdpreview.math",
        "setp mdpreview.large_file_mode",
    ] {
        assert!(
            items.iter().any(|t| t == key),
            "missing completion for {}",
            key
        );
    }
}

#[test]
fn dynamic_values_for_mdpreview_options_are_suggested() {
    let mut cc = oxidized::features::completion::CommandCompletionBuilder::new().build();
    // update values
    cc.start_completion("set mdpreview.update ");
    assert!(cc.should_show());
    let items: Vec<String> = cc.matches.iter().map(|i| i.text.clone()).collect();
    for v in [
        "set mdpreview.update manual",
        "set mdpreview.update on_save",
        "set mdpreview.update live",
    ] {
        assert!(items.contains(&v.to_string()), "missing {}", v);
    }

    // math values
    cc.start_completion("set mdpreview.math ");
    assert!(cc.should_show());
    let items: Vec<String> = cc.matches.iter().map(|i| i.text.clone()).collect();
    for v in [
        "set mdpreview.math off",
        "set mdpreview.math inline",
        "set mdpreview.math block",
    ] {
        assert!(items.contains(&v.to_string()), "missing {}", v);
    }

    // large_file_mode values
    cc.start_completion("set mdpreview.large_file_mode ");
    assert!(cc.should_show());
    let items: Vec<String> = cc.matches.iter().map(|i| i.text.clone()).collect();
    for v in [
        "set mdpreview.large_file_mode truncate",
        "set mdpreview.large_file_mode disable",
    ] {
        assert!(items.contains(&v.to_string()), "missing {}", v);
    }
}

#[test]
fn numeric_suggestions_for_mdpreview_debounce_ms() {
    let mut cc = oxidized::features::completion::CommandCompletionBuilder::new().build();
    cc.start_completion("set mdpreview.debounce_ms ");
    assert!(cc.should_show());
    let items: Vec<String> = cc.matches.iter().map(|i| i.text.clone()).collect();
    // We only assert a couple of representative values
    assert!(
        items.contains(&"set mdpreview.debounce_ms 0".to_string())
            || items.contains(&"set mdpreview.debounce_ms 50".to_string())
    );
}
