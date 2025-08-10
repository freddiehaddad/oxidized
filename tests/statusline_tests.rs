use oxidized::core::buffer::Buffer;
use oxidized::core::editor::EditorRenderState;
use oxidized::core::mode::Mode;
use oxidized::core::window::WindowManager;
use oxidized::ui::UI;
use std::collections::HashMap;

fn make_base_state() -> (EditorRenderState, oxidized::config::EditorConfig) {
    let config = oxidized::config::EditorConfig::load();
    let window_manager = WindowManager::new(80, 24);

    let mut buf = Buffer::new(1, config.editing.undo_levels);
    buf.file_path = None;
    let current_buffer = Some(buf.clone());

    let mut all_buffers = HashMap::new();
    all_buffers.insert(buf.id, buf);

    (
        EditorRenderState {
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
            config: config.clone(),
            filetype: None,
            macro_recording: None,
            search_total: 0,
            search_index: None,
        },
        config,
    )
}

#[test]
fn statusline_segments_toggle() {
    let (mut state, mut config) = make_base_state();
    // Enable all right-side segments and customize some values
    config.statusline.show_indent = true;
    config.statusline.show_eol = true;
    config.statusline.show_encoding = true;
    config.statusline.show_type = true;
    config.statusline.show_macro = true;
    config.statusline.show_search = true;
    config.statusline.show_progress = true;
    config.statusline.separator = " | ".to_string();
    state.config = config.clone();

    state.filetype = Some("rust".to_string());
    state.macro_recording = Some('q');
    state.search_total = 10;
    state.search_index = Some(2);

    let ui = UI::new();
    let line = ui.compute_status_line_text(&state, 120);

    // Always present
    assert!(line.contains("Ln "));
    assert!(line.contains("Col "));

    // Indentation (Tabs default with width 4 unless config changed)
    assert!(line.contains("Tabs: 4") || line.contains("Spaces: 4"));
    // Encoding heuristic for empty buffer should be ASCII
    assert!(line.contains("ASCII"));
    // EOL placeholder
    assert!(line.contains("LF"));
    // Filetype
    assert!(line.contains("rust"));
    // Macro recording
    assert!(line.contains("REC @q"));
    // Search status 3/10 (index 2 is third)
    assert!(line.contains("3/10"));
    // Progress percentage (single line at row 0 -> 100%)
    assert!(line.contains("100%"));

    // Now disable optional segments; Ln/Col should remain, others disappear
    state.config.statusline.show_indent = false;
    state.config.statusline.show_encoding = false;
    state.config.statusline.show_eol = false;
    state.config.statusline.show_type = false;
    state.config.statusline.show_macro = false;
    state.config.statusline.show_search = false;
    state.config.statusline.show_progress = false;

    let line2 = ui.compute_status_line_text(&state, 120);
    assert!(line2.contains("Ln "));
    assert!(line2.contains("Col "));
    assert!(!line2.contains("Tabs: "));
    assert!(!line2.contains("Spaces: "));
    assert!(!line2.contains("ASCII"));
    assert!(!line2.contains("UTF-8"));
    assert!(!line2.contains("LF"));
    assert!(!line2.contains("rust"));
    assert!(!line2.contains("REC @"));
    assert!(!line2.contains("/")); // no search fraction
    assert!(!line2.contains("%")); // no progress
}

#[test]
fn statusline_separator_respected() {
    let (mut state, mut config) = make_base_state();
    config.statusline.separator = " | ".to_string();
    // Enable a few segments to ensure separators appear
    config.statusline.show_indent = true;
    config.statusline.show_encoding = true;
    config.statusline.show_eol = true;
    state.config = config;

    let ui = UI::new();
    let line = ui.compute_status_line_text(&state, 100);
    // At least two separators should appear between segments
    assert!(line.matches(" | ").count() >= 2);
}

#[test]
fn statusline_zero_width_is_empty() {
    let (state, _) = make_base_state();
    let ui = UI::new();
    let line = ui.compute_status_line_text(&state, 0);
    assert!(line.is_empty());
}

#[test]
fn statusline_message_is_included() {
    let (mut state, _) = make_base_state();
    state.status_message = "Build succeeded".to_string();
    let ui = UI::new();
    let line = ui.compute_status_line_text(&state, 120);
    assert!(line.contains("Build succeeded"));
}

#[test]
fn statusline_centers_message_when_space_available() {
    let (mut state, _) = make_base_state();
    // No right segments and no current buffer to simplify layout (right empty)
    state.current_buffer = None;
    state.all_buffers.clear();
    state.macro_recording = None;
    state.search_total = 0;
    state.status_message = "HELLO CENTER".to_string();

    // Disable all optional right-side segments just in case
    state.config.statusline.show_indent = false;
    state.config.statusline.show_eol = false;
    state.config.statusline.show_encoding = false;
    state.config.statusline.show_type = false;
    state.config.statusline.show_macro = false;
    state.config.statusline.show_search = false;
    state.config.statusline.show_progress = false;

    let ui = UI::new();
    let width = 60;
    let line = ui.compute_status_line_text(&state, width);

    // Ensure message is present
    let msg = &state.status_message;
    let pos = line.find(msg).expect("message should be present");

    // Count contiguous spaces directly around the message
    let left_spaces = line[..pos].chars().rev().take_while(|c| *c == ' ').count();
    let right_start = pos + msg.len();
    let right_spaces = line[right_start..]
        .chars()
        .take_while(|c| *c == ' ')
        .count();

    // Centering tolerance: difference at most 1
    assert!(
        (left_spaces as isize - right_spaces as isize).abs() <= 1,
        "left/right gap not centered: left={}, right={}, line='{}'",
        left_spaces,
        right_spaces,
        line
    );
}

#[test]
fn statusline_unicode_width_respected() {
    use unicode_width::UnicodeWidthStr;
    let (mut state, _) = make_base_state();
    state.status_message = "宽字符 e\u{301} — mixed".repeat(3);

    let ui = UI::new();
    let width: u16 = 40;
    let line = ui.compute_status_line_text(&state, width);
    assert_eq!(UnicodeWidthStr::width(line.as_str()), width as usize);
}

#[test]
fn statusline_shows_no_name_when_no_buffer() {
    let (mut state, _) = make_base_state();
    state.current_buffer = None;
    state.all_buffers.clear();

    let ui = UI::new();
    let line = ui.compute_status_line_text(&state, 80);
    assert!(line.contains("[No Name]"));
}

#[test]
fn statusline_search_matches_when_index_none() {
    let (mut state, _) = make_base_state();
    state.search_total = 7;
    state.search_index = None;
    state.config.statusline.show_search = true;

    let ui = UI::new();
    let line = ui.compute_status_line_text(&state, 120);
    assert!(line.contains("7 matches"));
}
