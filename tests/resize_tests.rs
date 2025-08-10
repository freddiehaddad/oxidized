use oxidized::core::editor::{Editor, EditorRenderState};
use oxidized::ui::UI;

#[test]
fn status_line_matches_terminal_width_on_resize() {
    // Build a minimal editor to extract an EditorRenderState
    let mut editor = Editor::new().expect("editor init");

    // Ensure a buffer exists so the status line has content
    if editor.current_buffer().is_none() {
        editor.create_buffer(None).expect("create buffer");
    }

    // Render once to populate state and ensure paths are set up
    editor.render().expect("initial render");

    // Capture render state via a minimal clone through public API
    // We call render again which assembles EditorRenderState internally; to
    // construct one for testing, we replicate just enough fields.
    // Get current buffer clone and IDs through public getters
    let current_buffer = editor.current_buffer().cloned();
    let mut displayed_buffers = std::collections::HashMap::new();
    if let Some(buf) = current_buffer.clone() {
        displayed_buffers.insert(buf.id, buf.clone());
    }

    let editor_state = EditorRenderState {
        mode: editor.mode(),
        current_buffer,
        all_buffers: displayed_buffers,
        command_line: editor.command_line().to_string(),
        status_message: editor.status_message().to_string(),
        buffer_count: 1,
        current_buffer_id: editor.current_buffer_id,
        current_window_id: editor.window_manager.current_window_id(),
        window_manager: editor.window_manager.clone(),
        syntax_highlights: std::collections::HashMap::new(),
        command_completion: Default::default(),
        config: oxidized::config::EditorConfig::load(),
    };

    let ui = UI::new();

    // Simulate shrinking terminal width
    let small_width: u16 = 20;
    let status_small = ui.compute_status_line_text(&editor_state, small_width);
    assert_eq!(status_small.len(), small_width as usize);

    // Simulate growing terminal width
    let large_width: u16 = 120;
    let status_large = ui.compute_status_line_text(&editor_state, large_width);
    assert_eq!(status_large.len(), large_width as usize);

    // Ensure the small version is a prefix of the large (when content allows)
    assert!(status_large.starts_with(&status_small[..small_width as usize].to_string()));
}
