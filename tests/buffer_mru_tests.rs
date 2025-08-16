use anyhow::Result;
use oxidized::core::editor::Editor;
use oxidized::utils::command::execute_ex_command;
use std::sync::atomic::Ordering;

fn open_temp(editor: &mut Editor, name: &str, content: &str) -> Result<usize> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join(name);
    std::fs::write(&path, content)?;
    execute_ex_command(editor, &format!("e {}", path.to_string_lossy()));
    Ok(editor.current_buffer().unwrap().id)
}

#[test]
fn bd_switches_to_mru_and_updates_window() -> Result<()> {
    let mut editor = Editor::new()?;

    // Open two files; current will be second, MRU should be first
    let id_a = open_temp(&mut editor, "a.txt", "a")?;
    let id_b = open_temp(&mut editor, "b.txt", "b")?;
    assert_eq!(editor.current_buffer().map(|b| b.id), Some(id_b));

    // :bd closes current (b) and should switch to MRU (a)
    execute_ex_command(&mut editor, "bd");
    assert_eq!(editor.current_buffer().map(|b| b.id), Some(id_a));
    assert!(editor.status_message().contains("Buffer closed"));

    // Window retargeted to a
    let wid = editor.window_manager.current_window_id().unwrap();
    let win = editor.window_manager.get_window(wid).unwrap();
    assert_eq!(win.buffer_id, Some(id_a));

    // Redraw requested for immediate UI update
    assert!(editor.needs_redraw.load(Ordering::Relaxed));

    Ok(())
}

#[test]
fn bd_retargets_all_windows_showing_closed_buffer() -> Result<()> {
    let mut editor = Editor::new()?;

    // Open A and B, leave current on B
    let id_a = open_temp(&mut editor, "a.txt", "a")?;
    let id_b = open_temp(&mut editor, "b.txt", "b")?;
    assert_ne!(id_a, id_b);

    // Split window so both windows initially show current buffer (B)
    execute_ex_command(&mut editor, "vsplit");
    // Sanity: both windows should reference buffer B
    for w in editor.window_manager.all_windows().values() {
        assert_eq!(w.buffer_id, Some(id_b));
    }

    // Delete current buffer (B); both windows must be retargeted to fallback (A)
    execute_ex_command(&mut editor, "bd");
    for w in editor.window_manager.all_windows().values() {
        assert_eq!(w.buffer_id, Some(id_a));
    }

    Ok(())
}

#[test]
fn bd_chooses_true_mru_after_manual_switches() -> Result<()> {
    let mut editor = Editor::new()?;

    let id_a = open_temp(&mut editor, "a.txt", "a")?;
    let id_b = open_temp(&mut editor, "b.txt", "b")?;
    let id_c = open_temp(&mut editor, "c.txt", "c")?;
    assert_eq!(editor.current_buffer().map(|b| b.id), Some(id_c));

    // Switch to B, then to A to shape MRU order
    execute_ex_command(&mut editor, &format!("b {}", id_b));
    assert_eq!(editor.current_buffer().map(|b| b.id), Some(id_b));
    execute_ex_command(&mut editor, &format!("b {}", id_a));
    assert_eq!(editor.current_buffer().map(|b| b.id), Some(id_a));

    // Now closing A should fall back to MRU (B)
    execute_ex_command(&mut editor, "bd");
    assert_eq!(editor.current_buffer().map(|b| b.id), Some(id_b));

    Ok(())
}
