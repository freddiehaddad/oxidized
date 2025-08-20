use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::core::mode::Mode;
use oxidized::core::mode::Position;
use oxidized::input::keymap::KeyHandler;

fn make_editor_with_single_line(text: &str) -> Editor {
    let mut editor = Editor::new().expect("editor");
    let _ = editor.create_buffer(None).expect("buffer");
    if let Some(buf) = editor.current_buffer_mut() {
        buf.lines = vec![text.to_string()];
        buf.cursor = Position::new(0, 0);
    }
    editor
}

#[test]
fn y_dollar_yanks_to_end_of_line_then_pastes() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();

    // y$
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
        )
        .unwrap();

    // move to end and paste
    {
        let buf = editor.current_buffer_mut().unwrap();
        let len = buf.lines[0].len();
        buf.cursor = Position::new(0, len.saturating_sub(1));
    }
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
        )
        .unwrap();

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines[0].as_str(), "Hello worldHello world");
}

#[test]
fn d_dollar_deletes_to_end_of_line() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();

    // d$
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
        )
        .unwrap();

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines[0].as_str(), "");
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn c_dollar_changes_to_end_of_line_enters_insert() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();

    // c$
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE),
        )
        .unwrap();

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines[0].as_str(), "");
    assert_eq!(editor.mode(), Mode::Insert);
}
