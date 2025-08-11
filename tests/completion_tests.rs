use oxidized::features::completion::{BufferSummary, CommandCompletion, CompletionContext};

#[test]
fn completes_directories_with_trailing_separator_and_parent() {
    let mut cc = CommandCompletion::new();
    let cwd = std::env::current_dir().unwrap();
    cc.set_context(CompletionContext {
        cwd: cwd.clone(),
        buffers: vec![],
        current_buffer_dir: None,
        allow_percent_path_root: true,
    });
    cc.start_completion("e ");
    assert!(cc.should_show());
    let items = cc.matches.clone();
    assert!(
        items
            .iter()
            .any(|i| i.category == "file" && i.text.ends_with(std::path::MAIN_SEPARATOR))
    );
    assert!(
        items
            .iter()
            .any(|i| i.description.contains("Parent Directory"))
    );
}

#[test]
fn supports_percent_root_for_current_buffer_dir() {
    let mut cc = CommandCompletion::new();
    let cwd = std::env::current_dir().unwrap();
    let tmp_dir = tempfile::tempdir().unwrap();
    cc.set_context(CompletionContext {
        cwd,
        buffers: vec![],
        current_buffer_dir: Some(tmp_dir.path().to_path_buf()),
        allow_percent_path_root: true,
    });
    cc.start_completion("e %/");
    assert!(cc.should_show());
}

#[test]
fn buffer_and_numeric_hints_present() {
    let mut cc = CommandCompletion::new();
    let cwd = std::env::current_dir().unwrap();
    cc.set_context(CompletionContext {
        cwd,
        buffers: vec![BufferSummary {
            id: 1,
            name: "test.txt".into(),
            modified: false,
        }],
        current_buffer_dir: None,
        allow_percent_path_root: true,
    });
    cc.start_completion("b ");
    // buffers completion may produce items; then check numeric hints
    cc.start_completion("set ts=");
    assert!(cc.should_show());
    let items = cc.matches.clone();
    assert!(
        items
            .iter()
            .any(|i| i.text.starts_with("set ts=") || i.text.starts_with("set tabstop="))
    );
}

#[test]
fn setp_completes_like_set_with_proper_prefix() {
    let mut cc = CommandCompletion::new();
    // No special context needed for set completions
    cc.start_completion("setp ");
    assert!(cc.should_show());
    // Ensure we don't see mixed prefixes
    assert!(
        cc.matches
            .iter()
            .all(|i| !i.text.starts_with("set ") || !i.text.contains(' '))
    );
    // And that we do see setp-based suggestions
    assert!(cc.matches.iter().any(|i| i.text.starts_with("setp ")));
}

#[test]
fn setp_dynamic_numeric_suggestions_present() {
    let mut cc = CommandCompletion::new();
    cc.start_completion("setp ts=");
    assert!(cc.should_show());
    // Should suggest common numbers with setp prefix
    let items: Vec<String> = cc.matches.iter().map(|i| i.text.clone()).collect();
    assert!(items.iter().any(|t| t == "setp tabstop=2"));
    assert!(items.iter().any(|t| t == "setp tabstop=4"));
}
