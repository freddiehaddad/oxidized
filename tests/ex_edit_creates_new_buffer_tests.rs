use anyhow::Result;
use oxidized::core::editor::Editor;
use oxidized::utils::command::execute_ex_command;
use std::fs;

#[test]
fn ex_edit_nonexistent_creates_named_empty_buffer() -> Result<()> {
    let mut editor = Editor::new()?;

    // Choose a path inside a temp dir that does not exist
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("newfile_does_not_exist.txt");
    assert!(!path.exists());

    // :e <nonexistent>
    execute_ex_command(&mut editor, &format!("e {}", path.to_string_lossy()));

    // Should have current buffer with that file path set, empty content, unmodified
    let buf = editor.current_buffer().expect("buffer should exist");
    assert_eq!(buf.file_path.as_deref(), Some(path.as_path()));
    assert_eq!(buf.lines.len(), 1);
    assert_eq!(buf.lines[0], "");
    assert!(!buf.modified);

    // File should not exist on disk until written
    assert!(!path.exists());

    // :w should write it to disk
    execute_ex_command(&mut editor, "w");
    assert!(path.exists());
    let on_disk = fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(on_disk, "");

    Ok(())
}
