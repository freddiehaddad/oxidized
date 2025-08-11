use anyhow::Result;
use oxidized::core::editor::Editor;
use oxidized::utils::command::{execute_ex_command, handle_set_command};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn make_editor() -> Result<Editor> {
    Editor::new()
}

#[test]
fn set_toggles_and_queries() -> Result<()> {
    let mut editor = make_editor()?;
    // Enable absolute numbers via execute_ex_command
    execute_ex_command(&mut editor, "set nu");
    let (abs, rel) = editor.get_line_number_state();
    assert!(abs && !rel);

    // Enable relative numbers
    execute_ex_command(&mut editor, "set rnu");
    let (abs, rel) = editor.get_line_number_state();
    assert!(!abs && rel);

    // Enable hybrid
    execute_ex_command(&mut editor, "set nu rnu");
    let (abs, rel) = editor.get_line_number_state();
    assert!(abs && rel);

    // Query reflects state
    execute_ex_command(&mut editor, "set number?");
    assert_eq!(editor.status_message(), "number: true");

    // key=value via centralized handler
    handle_set_command(&mut editor, "tabstop=4");
    assert_eq!(editor.get_config_value("tabstop").as_deref(), Some("4"));

    Ok(())
}

#[test]
fn write_and_open_commands() -> Result<()> {
    let mut editor = make_editor()?;

    // Create a temp file path
    let mut file = tempfile::NamedTempFile::new()?;
    writeln!(file, "hello")?; // initial content
    let path: PathBuf = file.path().to_path_buf();

    // :e <file>
    let cmd = format!("e {}", path.to_string_lossy());
    execute_ex_command(&mut editor, &cmd);
    assert!(editor.status_message().starts_with("Opened"));

    // Modify current buffer
    if let Some(buf) = editor.current_buffer_mut() {
        buf.lines.clear();
        buf.lines.push("edited".to_string());
        buf.modified = true;
    }

    // :w writes changes
    execute_ex_command(&mut editor, "w");
    let on_disk = fs::read_to_string(&path)?;
    assert!(on_disk.contains("edited"));

    Ok(())
}

#[test]
fn quit_and_force_quit_respect_unsaved() -> Result<()> {
    let mut editor = make_editor()?;
    editor.create_buffer(None)?;
    // Mark modified
    if let Some(buf) = editor.current_buffer_mut() {
        buf.modified = true;
    }

    // :q should refuse
    execute_ex_command(&mut editor, "q");
    assert!(!editor.should_quit());
    assert!(editor.status_message().contains("Unsaved changes"));

    // :q! should quit
    execute_ex_command(&mut editor, "q!");
    assert!(editor.should_quit());
    Ok(())
}

#[test]
fn buffer_management_commands() -> Result<()> {
    let mut editor = make_editor()?;

    // Create two buffers with names
    let dir = tempfile::tempdir()?;
    let a = dir.path().join("foo.txt");
    fs::write(&a, "a")?;
    let b = dir.path().join("bar.txt");
    fs::write(&b, "b")?;

    // Open both
    execute_ex_command(&mut editor, &format!("e {}", a.to_string_lossy()));
    let first_id = editor.current_buffer().map(|buf| buf.id).unwrap();
    execute_ex_command(&mut editor, &format!("e {}", b.to_string_lossy()));
    let second_id = editor.current_buffer().map(|buf| buf.id).unwrap();
    assert_ne!(first_id, second_id);

    // :ls shows buffers
    execute_ex_command(&mut editor, "ls");
    assert!(editor.status_message().starts_with("Buffers:"));

    // :b <id>
    execute_ex_command(&mut editor, &format!("b {}", first_id));
    assert_eq!(editor.current_buffer().map(|buf| buf.id), Some(first_id));

    // :b by name
    execute_ex_command(&mut editor, "b bar");
    assert_eq!(editor.current_buffer().map(|buf| buf.id), Some(second_id));

    // :bd refuses on modified
    if let Some(buf) = editor.current_buffer_mut() {
        buf.modified = true;
    }
    execute_ex_command(&mut editor, "bd");
    assert!(editor.status_message().to_lowercase().contains("unsaved"));

    // :bd! forces
    execute_ex_command(&mut editor, "bd!");
    assert!(editor.status_message().to_lowercase().contains("closed"));

    Ok(())
}

#[test]
fn window_split_commands_set_status() -> Result<()> {
    let mut editor = make_editor()?;
    execute_ex_command(&mut editor, "split");
    assert!(!editor.status_message().is_empty());
    execute_ex_command(&mut editor, "vsplit");
    assert!(!editor.status_message().is_empty());
    execute_ex_command(&mut editor, "close");
    assert!(!editor.status_message().is_empty());
    Ok(())
}
