use anyhow::Result;
use oxidized::core::editor::Editor;
use oxidized::utils::command::execute_ex_command;

fn make_editor() -> Result<Editor> {
    Editor::new()
}

#[test]
fn ex_registers_shows_basic_registers() -> Result<()> {
    let mut editor = make_editor()?;
    // Create a buffer and insert two lines
    editor.create_buffer(None)?;
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.lines = vec!["alpha".to_string(), "beta".to_string()];
        buf.cursor.row = 0;
        buf.cursor.col = 0;
        // Yank first line (yy)
        let text = buf.lines[0].clone() + "\n";
        buf.write_register_content(
            text.clone(),
            oxidized::core::buffer::YankType::Line,
            oxidized::core::buffer::WriteKind::Yank,
        );
        // Delete second line (dd semantics)
        let deleted = buf.lines[1].clone() + "\n";
        buf.write_register_content(
            deleted,
            oxidized::core::buffer::YankType::Line,
            oxidized::core::buffer::WriteKind::Delete,
        );
    }

    // Open :registers view
    execute_ex_command(&mut editor, "registers");
    // Should set a status message about opening
    assert!(editor.status_message().to_lowercase().contains("registers"));

    // The current buffer is the view; inspect its contents
    let buf = editor.current_buffer().unwrap();
    let joined = buf.lines.join("\n");
    assert!(joined.contains("\"\"  ")); // unnamed
    assert!(joined.contains("\"0  alpha")); // yank register 0
    assert!(joined.contains("\"1  beta")); // numbered 1

    Ok(())
}
