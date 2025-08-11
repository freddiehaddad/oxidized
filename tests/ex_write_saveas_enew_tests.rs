use oxidized::Editor;

// Helpers
fn feed_cmd(editor: &mut Editor, cmd: &str) {
    editor.set_command_line(cmd.to_string());
    oxidized::utils::command::execute_ex_command(editor, cmd);
}

#[test]
fn test_write_to_arbitrary_file_and_saveas() {
    let mut editor = Editor::new().expect("editor");
    editor.create_buffer(None).expect("make buf");
    // Start with empty buffer, insert some text directly
    {
        let buf = editor.current_buffer_mut().expect("buf");
        buf.lines = vec!["alpha".into(), "beta".into()];
        buf.modified = true;
    }

    // Write to a temp file path without renaming buffer
    let dir = tempfile::tempdir().unwrap();
    let write_path = dir.path().join("out1.txt");
    let write_str = write_path.to_string_lossy().to_string();
    feed_cmd(&mut editor, &format!("w {}", write_str));

    // Ensure file exists and content correct
    let content = std::fs::read_to_string(&write_path).unwrap();
    assert_eq!(content, "alpha\nbeta");

    // Saveas to a new file, buffer should now have a path and be unmodified
    let saveas_path = dir.path().join("named.txt");
    let saveas_str = saveas_path.to_string_lossy().to_string();
    feed_cmd(&mut editor, &format!("saveas {}", saveas_str));

    // Check file content and buffer state
    let content2 = std::fs::read_to_string(&saveas_path).unwrap();
    assert_eq!(content2, "alpha\nbeta");
    let buf = editor.current_buffer().unwrap();
    assert_eq!(
        buf.file_path
            .as_ref()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy(),
        "named.txt"
    );
    assert!(!buf.modified);
}

#[test]
fn test_enew_creates_empty_buffer() {
    let mut editor = Editor::new().expect("editor");
    editor.create_buffer(None).expect("make buf");
    // Put some content so we can distinguish
    {
        let buf = editor.current_buffer_mut().expect("buf");
        buf.lines = vec!["hello".into()];
        buf.modified = true;
    }

    // enew should create a new empty buffer and switch to it
    feed_cmd(&mut editor, "enew");

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines.len(), 1);
    assert_eq!(buf.lines[0], "");
    assert!(buf.file_path.is_none());
}
