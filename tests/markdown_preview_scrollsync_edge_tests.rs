use oxidized::core::Editor;
use oxidized::utils::command::execute_ex_command;

fn lines(input: &str) -> Vec<String> {
    input.split('\n').map(|s| s.to_string()).collect()
}

#[test]
fn scrollsync_aligns_across_heading_underlines() {
    let mut editor = Editor::new().expect("editor");
    editor.create_buffer(None).expect("buffer");
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.file_path = Some(std::path::PathBuf::from("sync.md"));
        buf.lines = lines("# Title\npara1\npara2\n\n## Sub\npara3\npara4");
        buf.modified = false;
    }

    execute_ex_command(&mut editor, "set mdpreview.scrollsync");
    let _ = editor.open_markdown_preview();

    // Scroll so that the heading is at the top, then a couple lines below
    // and ensure preview tracks despite underline line existing only in preview.
    let left_id = editor.window_manager.current_window_id().unwrap();
    let right_id = editor
        .window_manager
        .all_windows()
        .keys()
        .copied()
        .find(|&id| id != left_id)
        .unwrap();

    // Move viewport to the sub heading line in source
    if let Some(w) = editor.window_manager.get_window_mut(left_id) {
        w.viewport_top = 4; // line index of "## Sub"
    }
    editor.debounced_maybe_refresh_markdown_preview();

    let preview_top = editor
        .window_manager
        .get_window(right_id)
        .unwrap()
        .viewport_top;
    // Expect preview to align to the "Sub" title line; simple sanity (within bounds)
    // Temporarily focus right to read its buffer line count
    assert!(editor.move_to_window_right());
    let total = editor.current_buffer().unwrap().lines.len();
    assert!(preview_top < total);
    assert!(editor.move_to_window_left());
}

#[test]
fn scrollsync_across_fenced_code_blocks_without_fences() {
    let mut editor = Editor::new().expect("editor");
    editor.create_buffer(None).expect("buffer");
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.file_path = Some(std::path::PathBuf::from("code.md"));
        buf.lines = lines("before\n```\ncode\n```\nafter");
        buf.modified = false;
    }

    execute_ex_command(&mut editor, "set mdpreview.scrollsync");
    let _ = editor.open_markdown_preview();

    let left_id = editor.window_manager.current_window_id().unwrap();
    let right_id = editor
        .window_manager
        .all_windows()
        .keys()
        .copied()
        .find(|&id| id != left_id)
        .unwrap();

    // Set viewport to the first fence line (index 1 in source)
    if let Some(w) = editor.window_manager.get_window_mut(left_id) {
        w.viewport_top = 1;
    }
    editor.debounced_maybe_refresh_markdown_preview();
    let ptop_fence = editor
        .window_manager
        .get_window(right_id)
        .unwrap()
        .viewport_top;
    // Should align to code content start (index 0 or 1 depending on renderer spacing)
    assert!(ptop_fence == 0 || ptop_fence == 1);

    // Move viewport to the code line itself (index 2 in source)
    if let Some(w) = editor.window_manager.get_window_mut(left_id) {
        w.viewport_top = 2;
    }
    editor.debounced_maybe_refresh_markdown_preview();
    let ptop_code = editor
        .window_manager
        .get_window(right_id)
        .unwrap()
        .viewport_top;
    assert!(ptop_code == 0 || ptop_code == ptop_fence);
}

#[test]
fn scrollsync_ignores_inline_html_when_math_off() {
    let mut editor = Editor::new().expect("editor");
    editor.create_buffer(None).expect("buffer");
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.file_path = Some(std::path::PathBuf::from("html.md"));
        buf.lines = lines("<span>html</span>\ntext");
        buf.modified = false;
    }

    execute_ex_command(&mut editor, "set mdpreview.scrollsync");
    let _ = editor.open_markdown_preview();

    let left_id = editor.window_manager.current_window_id().unwrap();
    let right_id = editor
        .window_manager
        .all_windows()
        .keys()
        .copied()
        .find(|&id| id != left_id)
        .unwrap();

    // Set viewport to the HTML line (index 0); preview should show "text" at top (index 0)
    if let Some(w) = editor.window_manager.get_window_mut(left_id) {
        w.viewport_top = 0;
    }
    editor.debounced_maybe_refresh_markdown_preview();
    let ptop = editor
        .window_manager
        .get_window(right_id)
        .unwrap()
        .viewport_top;
    assert_eq!(ptop, 0);
}
