use oxidized::core::Editor;
use oxidized::utils::command::execute_ex_command;

fn make_lines(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("line {}", i)).collect()
}

#[test]
fn mdpreview_scrollsync_aligns_on_open() {
    let mut editor = Editor::new().expect("editor");

    // Source markdown buffer with many lines so we can scroll
    let src_id = editor.create_buffer(None).expect("buffer");
    {
        let buf = editor.current_buffer_mut().expect("cur buf");
        buf.file_path = Some(std::path::PathBuf::from("sync.md"));
        buf.lines = make_lines(200);
        buf.modified = false;
    }

    // Enable scroll sync
    execute_ex_command(&mut editor, "set mdpreview.scrollsync");

    // Scroll source down a page to get a non-zero viewport
    editor.scroll_down_page();

    // Capture source viewport_top
    let left_id = editor.window_manager.current_window_id().unwrap();
    let source_top_before = editor
        .window_manager
        .get_window(left_id)
        .map(|w| w.viewport_top)
        .unwrap();

    // Open preview; this should align the preview viewport to source_top + 2 (header offset)
    let _ = editor.open_markdown_preview();
    assert_eq!(editor.window_manager.all_windows().len(), 2);

    // Identify the right/preview window (not current)
    let right_id = editor
        .window_manager
        .all_windows()
        .keys()
        .copied()
        .find(|&id| id != left_id)
        .expect("right window id");

    let preview_height = editor
        .window_manager
        .get_window(right_id)
        .unwrap()
        .content_height();
    // Temporarily focus the right window to read its buffer line count
    assert!(editor.move_to_window_right());
    let preview_lines = editor.current_buffer().unwrap().lines.len();
    // Return focus to left/source window for further checks
    assert!(editor.move_to_window_left());
    let expected_top = (source_top_before + 2).min(preview_lines.saturating_sub(preview_height));
    let actual_top = editor
        .window_manager
        .get_window(right_id)
        .unwrap()
        .viewport_top;
    assert_eq!(actual_top, expected_top);

    // Ensure left window still shows the source buffer
    let left_after = editor.window_manager.current_window_id().unwrap();
    let left_win = editor.window_manager.get_window(left_after).unwrap();
    assert_eq!(left_win.buffer_id, Some(src_id));
}

#[test]
fn mdpreview_scrollsync_tracks_on_scroll() {
    let mut editor = Editor::new().expect("editor");

    // Source markdown buffer
    editor.create_buffer(None).expect("buffer");
    {
        let buf = editor.current_buffer_mut().expect("cur buf");
        buf.file_path = Some(std::path::PathBuf::from("track.md"));
        buf.lines = make_lines(300);
        buf.modified = false;
    }

    // Enable scroll sync and open preview
    execute_ex_command(&mut editor, "set mdpreview.scrollsync");
    let _ = editor.open_markdown_preview();

    // Capture window ids
    let left_id = editor.window_manager.current_window_id().unwrap();
    let right_id = editor
        .window_manager
        .all_windows()
        .keys()
        .copied()
        .find(|&id| id != left_id)
        .unwrap();

    // Record initial tops and preview total lines (by focusing preview briefly)
    let mut left_top = editor
        .window_manager
        .get_window(left_id)
        .map(|w| w.viewport_top)
        .unwrap();
    assert!(editor.move_to_window_right());
    let preview_total = editor.current_buffer().unwrap().lines.len();
    assert!(editor.move_to_window_left());

    // Scroll down a few lines and verify preview follows with +2 offset
    for _ in 0..5 {
        editor.scroll_down_line();
        left_top += 1;
        let preview_win = editor.window_manager.get_window(right_id).unwrap();
        let height = preview_win.content_height();
        let expected = (left_top + 2).min(preview_total.saturating_sub(height));
        assert_eq!(preview_win.viewport_top, expected);
    }

    // Scroll up a few lines and verify preview follows
    for _ in 0..3 {
        editor.scroll_up_line();
        left_top = left_top.saturating_sub(1);
        let preview_win = editor.window_manager.get_window(right_id).unwrap();
        let height = preview_win.content_height();
        let expected = (left_top + 2).min(preview_total.saturating_sub(height));
        assert_eq!(preview_win.viewport_top, expected);
    }
}
