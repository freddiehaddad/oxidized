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

    // Open preview (first time)
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
    // Opening again should NOT create an extra window and should update content
    let again = editor.open_markdown_preview();
    assert!(again.to_lowercase().contains("preview"));
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

#[test]
fn markdown_preview_placeholder_when_no_markdown() {
    let mut editor = Editor::new().expect("editor");

    // Create a non-markdown buffer
    let _ = editor.create_buffer(None).expect("buf");
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.file_path = Some(std::path::PathBuf::from("main.rs"));
        buf.lines = vec!["fn main() {}".into()];
    }

    // Open preview should show placeholder
    let _ = editor.open_markdown_preview();
    assert_eq!(editor.window_manager.all_windows().len(), 2);
    assert!(editor.move_to_window_right());
    let preview_buf = editor.current_buffer().unwrap();
    assert!(
        preview_buf
            .lines
            .iter()
            .any(|l| l.contains("No markdown buffer"))
    );
    assert!(editor.move_to_window_left());

    // Create markdown buffer and switch to it, preview should update on refresh
    let md_id = editor.create_buffer(None).expect("md buf");
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.file_path = Some(std::path::PathBuf::from("notes.md"));
        buf.lines = vec!["# Heading".into(), "text".into()];
    }
    let _ = editor.refresh_markdown_preview_now();
    assert!(editor.move_to_window_right());
    let preview_after = editor.current_buffer().unwrap();
    assert!(preview_after.lines.iter().any(|l| l.contains("Heading")));
    assert!(editor.move_to_window_left());

    // Switch back to non-markdown buffer; preview should keep last markdown
    // (not revert to placeholder until closed) when refreshed.
    // Switch to first buffer (assumed id 1 if exists) if different
    if md_id != 1 {
        let _ = editor.switch_to_buffer(1);
    }
    let _ = editor.refresh_markdown_preview_now();
    assert!(editor.move_to_window_right());
    let preview_after2 = editor.current_buffer().unwrap();
    assert!(preview_after2.lines.iter().any(|l| l.contains("Heading")));
}

#[test]
fn markdown_preview_always_right_most_after_existing_split() {
    let mut editor = Editor::new().expect("editor");

    // Open markdown buffer
    let src_id = editor.create_buffer(None).expect("src");
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.file_path = Some(std::path::PathBuf::from("README.md"));
        buf.lines = vec!["# Title".into(), "Body".into()];
    }

    // Create a manual right split (simulating :vsplit)
    let original_wid = editor.window_manager.current_window_id().unwrap();
    let second_wid = editor
        .window_manager
        .split_current_window(oxidized::core::window::SplitDirection::VerticalRight)
        .expect("second split");
    assert_ne!(original_wid, second_wid);

    // Move focus to new right window (simulate user ctrl+w l)
    let _ = editor.window_manager.set_current_window(second_wid);
    // Open a non-markdown file in the right window
    let _ = editor.create_buffer(None).expect("non md");
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.file_path = Some(std::path::PathBuf::from("src/config/editor.rs"));
        buf.lines = vec!["// config".into()];
    }
    // Move focus back to left window
    let _ = editor.window_manager.set_current_window(original_wid);
    editor.current_buffer_id = Some(src_id);

    // Toggle preview (open)
    let _ = editor.open_markdown_preview();
    assert_eq!(editor.window_manager.all_windows().len(), 3);

    // Determine global right-most window x+width
    let mut right_most: Option<(usize, u16)> = None;
    for w in editor.window_manager.all_windows().values() {
        let right_edge = w.x + w.width;
        if right_most.map(|(_, r)| right_edge > r).unwrap_or(true) {
            right_most = Some((w.id, right_edge));
        }
    }
    let right_most_id = right_most.unwrap().0;
    // Ensure the right-most window currently shows the preview buffer by checking placeholder or rendered lines
    if editor.window_manager.set_current_window(right_most_id) {
        let buf = editor.current_buffer().expect("preview buffer");
        assert!(
            buf.lines.iter().any(|l| l.contains("Title"))
                || buf
                    .lines
                    .iter()
                    .any(|l| l.contains("No markdown buffer active")),
            "Right-most window should contain markdown preview buffer"
        );
    } else {
        panic!("Unable to focus right-most window");
    }
}

#[test]
fn markdown_preview_toggle_closes_after_manual_split() {
    let mut editor = Editor::new().expect("editor");
    // Simulate starting with README.md provided on command line
    let _ = editor.create_buffer(None).expect("buf");
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.file_path = Some(std::path::PathBuf::from("README.md"));
        buf.lines = vec!["# Doc".into(), "line".into()];
    }
    // Manual right split (like :vsplit)
    let left_id = editor.window_manager.current_window_id().unwrap();
    let right_id = editor
        .window_manager
        .split_current_window(oxidized::core::window::SplitDirection::VerticalRight)
        .unwrap();
    assert_ne!(left_id, right_id);
    // Open preview via toggle
    let msg = editor.toggle_markdown_preview();
    assert!(msg.to_lowercase().contains("opened"));
    assert!(editor.is_markdown_preview_open());
    let window_count_after_open = editor.window_manager.all_windows().len();
    assert_eq!(window_count_after_open, 3);
    // Toggle again to close
    let msg2 = editor.toggle_markdown_preview();
    assert!(msg2.to_lowercase().contains("closed"));
    assert!(
        !editor.is_markdown_preview_open(),
        "Preview should be closed"
    );
    assert_eq!(
        editor.window_manager.all_windows().len(),
        2,
        "Preview window should be removed"
    );
}

#[test]
fn markdown_preview_restores_focus_to_original_window_after_close() {
    let mut editor = Editor::new().expect("editor");
    // Create markdown buffer in initial window
    let md_id = editor.create_buffer(None).expect("md");
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.file_path = Some(std::path::PathBuf::from("guide.md"));
        buf.lines = vec!["# Guide".into(), "content".into()];
    }
    let original_window = editor.window_manager.current_window_id().unwrap();
    // Add a manual right split (non-markdown buffer)
    let _right = editor
        .window_manager
        .split_current_window(oxidized::core::window::SplitDirection::VerticalRight)
        .unwrap();
    // Ensure we are still focused on original window (left)
    let _ = editor.window_manager.set_current_window(original_window);
    editor.current_buffer_id = Some(md_id);
    // Toggle open preview
    let _ = editor.toggle_markdown_preview();
    assert!(editor.is_markdown_preview_open());
    // Move focus away intentionally to right window
    assert!(editor.move_to_window_right());
    // Toggle close preview
    let _ = editor.toggle_markdown_preview();
    assert!(!editor.is_markdown_preview_open());
    // Focus should be restored to original window
    assert_eq!(
        editor.window_manager.current_window_id(),
        Some(original_window),
        "Focus should return to window active when preview opened"
    );
    // And buffer should be the markdown source
    let cur_buf = editor.current_buffer().unwrap();
    assert_eq!(
        cur_buf.file_path.as_ref().unwrap().file_name().unwrap(),
        "guide.md"
    );
}
