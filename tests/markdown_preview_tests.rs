use oxidized::core::Editor;

#[test]
fn markdown_preview_opens_in_right_split_and_restores_left() {
    let mut editor = Editor::new().expect("editor");

    // Create a source buffer and add some content
    let src_id = editor.create_buffer(None).expect("buffer");
    {
        let buf = editor.current_buffer_mut().expect("cur buf");
        buf.file_path = Some(std::path::PathBuf::from("README.md"));
        buf.lines = vec!["# Title".into(), "body".into()];
        buf.modified = false;
    }

    // Sanity: single window initially
    assert_eq!(editor.window_manager.all_windows().len(), 1);
    let left_before = editor.window_manager.current_window_id().unwrap();
    let left_buf_before = editor
        .window_manager
        .get_window(left_before)
        .unwrap()
        .buffer_id;
    assert_eq!(left_buf_before, Some(src_id));

    // Open preview
    let msg = editor.open_markdown_preview();
    assert!(msg.to_lowercase().contains("opened"));

    // Now there should be two windows
    assert_eq!(editor.window_manager.all_windows().len(), 2);
    let left_after = editor.window_manager.current_window_id().unwrap();
    assert_eq!(
        left_after, left_before,
        "Left/source window focus should be restored"
    );

    // Identify the right window (the one that's not current)
    let right_id = editor
        .window_manager
        .all_windows()
        .keys()
        .copied()
        .find(|&id| id != left_after)
        .expect("right window id");

    // Left window should still show the source buffer
    let left_win = editor.window_manager.get_window(left_after).unwrap();
    assert_eq!(left_win.buffer_id, Some(src_id));

    // Right window should show a different buffer (the preview buffer)
    let right_win = editor.window_manager.get_window(right_id).unwrap();
    let preview_buf_id = right_win.buffer_id.expect("preview buffer id");
    assert_ne!(
        preview_buf_id, src_id,
        "Preview must not reuse source buffer"
    );

    // Move focus to right window using editor API (updates current_buffer_id)
    assert!(
        editor.move_to_window_right(),
        "Can move focus to right window"
    );
    let cur = editor.current_buffer().expect("current buffer on right");
    assert!(!cur.lines.is_empty());

    // Move back to left; buffer should be source again
    assert!(editor.move_to_window_left());
    let cur_left = editor.current_buffer().expect("current buffer on left");
    assert_eq!(
        cur_left.file_path.as_ref().unwrap().file_name().unwrap(),
        "README.md"
    );
}

#[test]
fn markdown_preview_refresh_updates_right_only() {
    let mut editor = Editor::new().expect("editor");

    // Source markdown buffer
    let src_id = editor.create_buffer(None).expect("buffer");
    {
        let buf = editor.current_buffer_mut().expect("cur buf");
        buf.file_path = Some(std::path::PathBuf::from("notes.md"));
        buf.lines = vec!["first".into()];
        buf.modified = false;
    }

    // Open preview
    let _ = editor.open_markdown_preview();
    assert_eq!(editor.window_manager.all_windows().len(), 2);

    // Capture right window id
    let left_id = editor.window_manager.current_window_id().unwrap();
    // Right window exists but we don't need its id explicitly here
    let _ = editor
        .window_manager
        .all_windows()
        .keys()
        .copied()
        .find(|&id| id != left_id)
        .unwrap();

    // Edit source content and refresh preview
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.lines = vec!["changed".into()];
        buf.modified = true;
    }
    let msg = editor.refresh_markdown_preview_now();
    assert!(msg.to_lowercase().contains("preview"));

    // Left window should still show the source buffer id
    let left_win = editor.window_manager.get_window(left_id).unwrap();
    assert_eq!(left_win.buffer_id, Some(src_id));

    // Right window still points at preview buffer
    assert!(editor.move_to_window_right());
    let cur = editor.current_buffer().unwrap();
    assert!(cur.lines.iter().any(|l| l == "changed"));

    // Return to left and confirm it's still the source file
    assert!(editor.move_to_window_left());
    let cur_left = editor.current_buffer().unwrap();
    assert_eq!(
        cur_left.file_path.as_ref().unwrap().file_name().unwrap(),
        "notes.md"
    );
}
