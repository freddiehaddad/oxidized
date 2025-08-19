use oxidized::core::buffer::Buffer;
use oxidized::core::editor::EditorRenderState;
use oxidized::core::mode::Mode;
use oxidized::core::window::WindowManager;
use oxidized::ui::UI;
use std::collections::HashMap;

#[test]
fn status_line_matches_terminal_width_on_resize() {
    // Build a minimal EditorRenderState without touching the real terminal
    let config = oxidized::config::EditorConfig::load();
    let window_manager = WindowManager::new(80, 24); // arbitrary terminal size

    // Create a simple buffer so the status line has content
    let mut buf = Buffer::new(1, config.editing.undo_levels);
    buf.file_path = None; // unnamed buffer shows [No Name]
    let current_buffer = Some(buf.clone());

    let mut all_buffers = HashMap::new();
    all_buffers.insert(buf.id, buf);

    let editor_state = EditorRenderState {
        mode: Mode::Normal,
        current_buffer,
        all_buffers,
        command_line: String::new(),
        status_message: String::new(),
        buffer_count: 1,
        current_buffer_id: Some(1),
        current_window_id: window_manager.current_window_id(),
        window_manager: window_manager.clone(),
        syntax_highlights: HashMap::new(),
        command_completion: Default::default(),
        config,
        markdown_preview_buffer_id: None,
        filetype: Some("Plain".to_string()),
        macro_recording: None,
        search_total: 0,
        search_index: None,
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

    // Layout differs with centering/right-align; only assert correct widths.
}
