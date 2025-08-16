use anyhow::Result;
use oxidized::core::editor::Editor;

#[test]
fn viewport_fields_in_snapshot_change_on_center_cursor() -> Result<()> {
    // Create an editor and event-driven wrapper (without running threads)
    let mut editor = Editor::new()?;

    // Prepare a buffer with many lines and put cursor far down
    editor.create_buffer(None)?;
    if let Some(buf) = editor.current_buffer_mut() {
        buf.lines = (0..200).map(|i| format!("line{}", i)).collect();
        buf.cursor.row = 100;
        buf.cursor.col = 0;
    }

    // Center the cursor (zz)
    editor.center_cursor();

    // After centering, viewport_top should have changed and needs_redraw should be set
    let viewport_top = editor
        .window_manager
        .current_window()
        .map(|w| w.viewport_top)
        .unwrap_or(0);
    assert!(
        viewport_top > 0,
        "viewport_top should move after center_cursor"
    );
    assert!(
        editor
            .needs_redraw
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    Ok(())
}

#[test]
fn viewport_fields_in_snapshot_change_on_top_bottom() -> Result<()> {
    let mut editor = Editor::new()?;

    editor.create_buffer(None)?;
    if let Some(buf) = editor.current_buffer_mut() {
        buf.lines = (0..200).map(|i| format!("line{}", i)).collect();
        buf.cursor.row = 150;
    }

    // Move line to top
    editor.cursor_to_top();
    let top = editor
        .window_manager
        .current_window()
        .map(|w| w.viewport_top)
        .unwrap_or(0);
    assert_eq!(top, 150);

    // Move line to bottom
    editor.cursor_to_bottom();
    let content_height = editor
        .window_manager
        .current_window()
        .map(|w| w.content_height())
        .unwrap_or(0);
    let bottom = editor
        .window_manager
        .current_window()
        .map(|w| w.viewport_top)
        .unwrap_or(0);
    assert_eq!(
        bottom,
        150usize.saturating_sub(content_height.saturating_sub(1))
    );
    Ok(())
}
