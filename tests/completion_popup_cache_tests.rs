use oxidized::core::editor::Editor;
use oxidized::core::mode::Mode;

#[test]
fn completion_popup_cache_reduces_ops() {
    let mut editor = Editor::new().expect("editor");
    // Enter command mode and start completion for 'se' (set commands)
    editor.set_mode(Mode::Command);
    editor.set_command_line(":se".to_string());
    editor.start_command_completion("se");

    // First render (full) populates cache
    editor.reset_debug_terminal_ops();
    editor.render().expect("first render");
    let first_ops = editor.debug_terminal_ops();

    // Second render without changes should early-return for popup
    editor.reset_debug_terminal_ops();
    editor.render_with_flag(false).expect("second render");
    let second_ops = editor.debug_terminal_ops();

    // Expect fewer ops on second render (popup skipped). Allow equality fallback if headless logic changes.
    assert!(
        second_ops <= first_ops,
        "expected second render ops ({} ) <= first ({} )",
        second_ops,
        first_ops
    );
    assert!(
        second_ops < first_ops,
        "popup cache did not reduce operations ({} vs {})",
        second_ops,
        first_ops
    );
}
