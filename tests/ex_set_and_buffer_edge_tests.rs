use anyhow::Result;
use oxidized::core::editor::Editor;
use oxidized::utils::command::execute_ex_command;

#[test]
fn set_queries_and_invalid_options() -> Result<()> {
    let mut editor = Editor::new()?;

    // Unknown setting query
    execute_ex_command(&mut editor, "set nosuch?");
    assert!(editor.status_message().contains("Unknown setting: nosuch"));

    // Invalid numeric value
    execute_ex_command(&mut editor, "set tabstop=abc");
    assert_eq!(editor.status_message(), "Invalid tab width value");

    // Unknown enable option
    execute_ex_command(&mut editor, "set foobar");
    assert!(editor.status_message().contains("Unknown option: foobar"));

    // Unknown disable option
    execute_ex_command(&mut editor, "set nofoobar");
    assert!(editor.status_message().contains("Unknown option: nofoobar"));

    Ok(())
}

#[test]
fn buffer_command_edge_cases() -> Result<()> {
    let mut editor = Editor::new()?;

    // :bd when no buffers
    execute_ex_command(&mut editor, "bd");
    assert!(editor.status_message().contains("No buffer to close"));

    // Create two named buffers
    let dir = tempfile::tempdir()?;
    let a = dir.path().join("one.txt");
    let b = dir.path().join("two.txt");
    std::fs::write(&a, "a")?;
    std::fs::write(&b, "b")?;

    execute_ex_command(&mut editor, &format!("e {}", a.to_string_lossy()));
    let first_id = editor.current_buffer().unwrap().id;

    execute_ex_command(&mut editor, &format!("e {}", b.to_string_lossy()));
    let second_id = editor.current_buffer().unwrap().id;
    assert_ne!(first_id, second_id);

    // :b invalid id
    execute_ex_command(&mut editor, "b 999999");
    assert!(editor.status_message().contains("No buffer with ID 999999"));

    // :b unknown name
    execute_ex_command(&mut editor, "b does-not-exist");
    assert!(
        editor
            .status_message()
            .contains("Unknown command: b does-not-exist")
    );

    // :bd closes when unmodified and creates new empty buffer if last
    // Close second buffer
    execute_ex_command(&mut editor, "b  "); // whitespace shouldn't switch
    execute_ex_command(&mut editor, "bd");
    // Close first buffer (switch to it first)
    execute_ex_command(&mut editor, &format!("b {}", first_id));
    execute_ex_command(&mut editor, "bd");
    // After closing last, new empty buffer is created
    assert!(
        editor
            .status_message()
            .to_lowercase()
            .contains("created new empty buffer")
    );

    Ok(())
}
