use anyhow::Result;
use oxidized::core::editor::Editor;
use oxidized::utils::command::execute_ex_command;
use std::fs;
use std::path::Path;

fn make_readonly(path: &Path) -> Result<()> {
    let meta = fs::metadata(path)?;
    let mut perms = meta.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o444);
        fs::set_permissions(path, perms)?;
    }
    #[cfg(windows)]
    {
        perms.set_readonly(true);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

#[test]
fn w_force_over_readonly_current_file() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("ro.txt");
    fs::write(&path, "orig")?;
    make_readonly(&path)?;

    let mut editor = Editor::new()?;
    execute_ex_command(&mut editor, &format!("e {}", path.to_string_lossy()));

    // Modify buffer
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.lines = vec!["new".into()];
        buf.modified = true;
    }

    // Non-force write should error and not change file
    execute_ex_command(&mut editor, "w");
    assert!(editor.status_message().to_lowercase().contains("error"));
    assert_eq!(fs::read_to_string(&path)?, "orig");

    // Force write should succeed and update file
    execute_ex_command(&mut editor, "w!");
    assert_eq!(fs::read_to_string(&path)?, "new");

    Ok(())
}

#[test]
fn w_force_with_path_over_readonly_target() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let target = dir.path().join("target.txt");
    fs::write(&target, "old")?;
    make_readonly(&target)?;

    let mut editor = Editor::new()?;
    editor.create_buffer(None)?;
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.lines = vec!["abc".into()];
        buf.modified = true;
    }

    // Non-force write to path should error
    execute_ex_command(&mut editor, &format!("w {}", target.to_string_lossy()));
    assert!(editor.status_message().to_lowercase().contains("error"));
    assert_eq!(fs::read_to_string(&target)?, "old");

    // Force write to path should succeed
    execute_ex_command(&mut editor, &format!("w! {}", target.to_string_lossy()));
    assert_eq!(fs::read_to_string(&target)?, "abc");

    Ok(())
}

#[test]
fn saveas_force_over_readonly_target() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let target = dir.path().join("rename.txt");
    fs::write(&target, "old")?;
    make_readonly(&target)?;

    let mut editor = Editor::new()?;
    editor.create_buffer(None)?;
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.lines = vec!["xyz".into()];
        buf.modified = true;
    }

    // Non-force saveas should error and not rename buffer
    execute_ex_command(&mut editor, &format!("saveas {}", target.to_string_lossy()));
    assert!(editor.status_message().to_lowercase().contains("error"));
    assert!(editor.current_buffer().unwrap().file_path.is_none());

    // Force saveas should succeed and update buffer state
    execute_ex_command(
        &mut editor,
        &format!("saveas! {}", target.to_string_lossy()),
    );
    let buf = editor.current_buffer().unwrap();
    assert_eq!(
        buf.file_path
            .as_ref()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy(),
        "rename.txt"
    );
    assert!(!buf.modified);
    assert_eq!(fs::read_to_string(&target)?, "xyz");

    Ok(())
}
