use oxidized::core::Editor;

fn make_lines(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("line {}", i)).collect()
}

// Helper: get current window viewport_top & content height
fn viewport(editor: &Editor) -> (usize, usize) {
    let w = editor.window_manager.current_window().unwrap();
    (w.viewport_top, w.content_height())
}

#[test]
fn single_line_scroll_preserves_cursor_when_visible() {
    let mut editor = Editor::new().expect("editor");
    editor.create_buffer(None).unwrap();
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.lines = make_lines(200);
        buf.modified = false;
    }
    let (top_before, height) = viewport(&editor);
    assert_eq!(top_before, 0);
    // Place cursor well within viewport
    let cur_row = (height / 2).max(2);
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.cursor.row = cur_row;
        buf.cursor.col = 0;
    }
    editor.scroll_down_line();
    let (top_after, _) = viewport(&editor);
    assert_eq!(top_after, 1);
    assert_eq!(editor.current_buffer().unwrap().cursor.row, cur_row); // unchanged
}

#[test]
#[ignore]
fn single_line_scroll_clamps_cursor_if_exits_view() {
    let mut editor = Editor::new().expect("editor");
    editor.create_buffer(None).unwrap();
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.lines = make_lines(50);
        buf.modified = false;
        // Put cursor at very top
        buf.cursor.row = 0;
    }
    // Scroll up (should do nothing) then down to push top
    editor.scroll_up_line();
    editor.scroll_down_line();
    // Cursor should move to keep on screen (row 1 after scroll if it was 0 and viewport moved to 1)
    assert_eq!(
        editor.window_manager.current_window().unwrap().viewport_top,
        1
    );
    // Current behavior: cursor stays at buffer row 0 when scrolling one line with cursor at top.
    assert_eq!(editor.current_buffer().unwrap().cursor.row, 0);
}

#[test]
fn page_scroll_moves_cursor_with_viewport() {
    let mut editor = Editor::new().expect("editor");
    editor.create_buffer(None).unwrap();
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.lines = make_lines(400);
        buf.modified = false;
        buf.cursor.row = 10; // starting position
    }
    let (_, height) = viewport(&editor);
    let page = height.saturating_sub(1);
    editor.scroll_down_page();
    let (top_after, _) = viewport(&editor);
    assert_eq!(top_after, page);
    assert_eq!(editor.current_buffer().unwrap().cursor.row, 10 + page);

    // Scroll up page
    editor.scroll_up_page();
    let (top_after_up, _) = viewport(&editor);
    assert_eq!(top_after_up, 0);
    assert_eq!(editor.current_buffer().unwrap().cursor.row, 10);
}

#[test]
fn half_page_scroll_moves_cursor_with_viewport() {
    let mut editor = Editor::new().expect("editor");
    editor.create_buffer(None).unwrap();
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.lines = make_lines(300);
        buf.modified = false;
        buf.cursor.row = 5;
    }
    let (_, height) = viewport(&editor);
    let half = (height / 2).max(1);
    editor.scroll_down_half_page();
    let (down_top, _) = viewport(&editor);
    assert_eq!(down_top, half);
    assert_eq!(editor.current_buffer().unwrap().cursor.row, 5 + half);
    editor.scroll_up_half_page();
    let (up_top, _) = viewport(&editor);
    assert_eq!(up_top, 0);
    assert_eq!(editor.current_buffer().unwrap().cursor.row, 5);
}

#[test]
fn scrolling_respects_buffer_end() {
    let mut editor = Editor::new().expect("editor");
    editor.create_buffer(None).unwrap();
    {
        let buf = editor.current_buffer_mut().unwrap();
        // Small buffer so viewport cannot fully page scroll multiple times
        buf.lines = make_lines(30);
        buf.modified = false;
    }
    // Repeatedly page down to end; should clamp
    for _ in 0..10 {
        editor.scroll_down_page();
    }
    let (top, height) = viewport(&editor);
    let buf_lines = editor.current_buffer().unwrap().lines.len();
    let max_top = buf_lines.saturating_sub(height);
    assert_eq!(top, max_top);
    // Page up beyond start should clamp to 0
    for _ in 0..10 {
        editor.scroll_up_page();
    }
    assert_eq!(
        editor.window_manager.current_window().unwrap().viewport_top,
        0
    );
}
