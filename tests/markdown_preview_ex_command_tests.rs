use oxidized::core::Editor;
use oxidized::utils::command::{execute_ex_command, handle_set_command};

#[test]
fn mdpreview_ex_commands_open_refresh_close_toggle() {
    let mut editor = Editor::new().expect("editor");

    // Prepare a markdown source buffer
    let src_id = editor.create_buffer(None).expect("buffer");
    {
        let buf = editor.current_buffer_mut().expect("cur buf");
        buf.file_path = Some(std::path::PathBuf::from("test.md"));
        buf.lines = vec!["Hello".into(), "World".into()];
        buf.modified = false;
    }

    // :MarkdownPreviewOpen
    execute_ex_command(&mut editor, "MarkdownPreviewOpen");
    assert!(editor.status_message().to_lowercase().contains("opened"));
    assert_eq!(editor.window_manager.all_windows().len(), 2);

    // :MarkdownPreviewRefresh
    execute_ex_command(&mut editor, "MarkdownPreviewRefresh");
    assert!(editor.status_message().to_lowercase().contains("refresh"));

    // :MarkdownPreviewClose
    execute_ex_command(&mut editor, "MarkdownPreviewClose");
    assert!(editor.status_message().to_lowercase().contains("closed"));
    assert_eq!(editor.window_manager.all_windows().len(), 1);

    // :MarkdownPreviewToggle opens when closed
    execute_ex_command(&mut editor, "MarkdownPreviewToggle");
    assert!(editor.status_message().to_lowercase().contains("opened"));
    assert_eq!(editor.window_manager.all_windows().len(), 2);

    // :MarkdownPreviewToggle closes when open
    execute_ex_command(&mut editor, "MarkdownPreviewToggle");
    assert!(editor.status_message().to_lowercase().contains("closed"));
    assert_eq!(editor.window_manager.all_windows().len(), 1);

    // Ensure source window still shows the source buffer
    let current_id = editor
        .window_manager
        .current_window()
        .and_then(|w| w.buffer_id)
        .unwrap();
    assert_eq!(current_id, src_id);
}

#[test]
fn mdpreview_set_commands_update_debounce_scrollsync_math_largefile() {
    let mut editor = Editor::new().expect("editor");

    // Use :set via execute_ex_command for strings and integers
    execute_ex_command(&mut editor, "set mdpreview.update live");
    assert_eq!(
        editor.get_config_value("mdpreview.update").as_deref(),
        Some("live")
    );

    execute_ex_command(&mut editor, "set mdpreview.debounce_ms 200");
    assert_eq!(
        editor.get_config_value("mdpreview.debounce_ms").as_deref(),
        Some("200")
    );

    // Boolean toggle using :set and :set no...
    execute_ex_command(&mut editor, "set mdpreview.scrollsync");
    assert_eq!(
        editor.get_config_value("mdpreview.scrollsync").as_deref(),
        Some("true")
    );
    execute_ex_command(&mut editor, "set nomdpreview.scrollsync");
    assert_eq!(
        editor.get_config_value("mdpreview.scrollsync").as_deref(),
        Some("false")
    );

    // Math mode
    execute_ex_command(&mut editor, "set mdpreview.math inline");
    assert_eq!(
        editor.get_config_value("mdpreview.math").as_deref(),
        Some("inline")
    );

    // Large file mode
    execute_ex_command(&mut editor, "set mdpreview.large_file_mode truncate");
    assert_eq!(
        editor
            .get_config_value("mdpreview.large_file_mode")
            .as_deref(),
        Some("truncate")
    );

    // Also try centralized handler in ephemeral mode
    handle_set_command(&mut editor, "mdpreview.update on_save", false);
    assert_eq!(
        editor.get_config_value("mdpreview.update").as_deref(),
        Some("on_save")
    );
}

#[test]
fn mdpreview_live_debounced_refresh_updates_preview() {
    let mut editor = Editor::new().expect("editor");

    // Source markdown buffer
    editor.create_buffer(None).expect("buffer");
    {
        let buf = editor.current_buffer_mut().expect("cur buf");
        buf.file_path = Some(std::path::PathBuf::from("live.md"));
        buf.lines = vec!["first".into()];
        buf.modified = false;
    }

    // Configure live updates with zero debounce
    execute_ex_command(&mut editor, "set mdpreview.update live");
    execute_ex_command(&mut editor, "set mdpreview.debounce_ms 0");

    // Open preview
    execute_ex_command(&mut editor, "MarkdownPreviewOpen");
    assert_eq!(editor.window_manager.all_windows().len(), 2);

    // Change source and trigger debounced refresh
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.lines = vec!["updated".into()];
        buf.modified = true;
    }
    // directly invoke the debounced helper used by key events
    editor.debounced_maybe_refresh_markdown_preview();

    // Move right and inspect preview content
    assert!(editor.move_to_window_right());
    let cur = editor.current_buffer().unwrap();
    assert!(cur.lines.iter().any(|l| l == "updated"));
}
