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
    // buffers completion may produce items; then check numeric hints (positional form)
    cc.start_completion("set ts ");
    assert!(cc.should_show());
    let items = cc.matches.clone();
    assert!(items.iter().any(|i| i.text.starts_with("set ts ")
        || i.text.starts_with("set tabstop ")
        || i.text == "set ts 2"
        || i.text == "set tabstop 2"));
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
    cc.start_completion("setp ts ");
    assert!(cc.should_show());
    // Should suggest common numbers with setp prefix
    let items: Vec<String> = cc.matches.iter().map(|i| i.text.clone()).collect();
    assert!(
        items
            .iter()
            .any(|t| t == "setp ts 2" || t == "setp tabstop 2")
    );
    assert!(
        items
            .iter()
            .any(|t| t == "setp ts 4" || t == "setp tabstop 4")
    );
}

#[test]
fn no_duplicate_set_alias_entries() {
    let mut cc = CommandCompletion::new();
    cc.start_completion("set sh"); // e.g. should not show both showmarks and smk duplicates
    assert!(cc.should_show());
    let positives: Vec<&String> = cc
        .matches
        .iter()
        .filter(|i| i.text.starts_with("set "))
        .map(|i| &i.text)
        .collect();
    // Ensure we don't have both the canonical and alias positive forms when both match prefix
    // Accept either `set showmarks` or `set smk` but not both
    let has_showmarks = positives.iter().any(|t| t.as_str() == "set showmarks");
    let has_smk = positives.iter().any(|t| t.as_str() == "set smk");
    assert!(
        !(has_showmarks && has_smk),
        "duplicate positive alias entries present"
    );

    // Now test negative prefix path: we should be able to see one negative form, but not duplicates
    cc.start_completion("set nosh");
    assert!(cc.should_show());
    let negatives: Vec<&String> = cc
        .matches
        .iter()
        .filter(|i| i.text.starts_with("set no"))
        .map(|i| &i.text)
        .collect();
    let has_noshowmarks = negatives.iter().any(|t| t.as_str() == "set noshowmarks");
    let has_nosmk = negatives.iter().any(|t| t.as_str() == "set nosmk");
    assert!(
        !(has_noshowmarks && has_nosmk),
        "duplicate negative alias entries present"
    );
}
