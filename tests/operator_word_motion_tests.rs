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
fn dw_deletes_word_and_space() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();

    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE),
        )
        .unwrap();

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines[0].as_str(), "world"); // deletes word and trailing space
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn de_deletes_word_only() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();

    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        )
        .unwrap();

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines[0].as_str(), " world"); // deletes word but keeps following space
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn db_deletes_previous_word_and_space() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.cursor = Position::new(0, 6); // at start of "world"
    }

    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        )
        .unwrap();

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines[0].as_str(), "world");
    assert_eq!(buf.cursor.col, 0);
}

#[test]
fn cw_changes_word_and_space_and_enters_insert() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();

    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE),
        )
        .unwrap();

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines[0].as_str(), "world");
    assert_eq!(editor.mode(), Mode::Insert);
}

#[test]
fn ce_changes_word_only_and_enters_insert() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();

    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        )
        .unwrap();

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines[0].as_str(), " world");
    assert_eq!(editor.mode(), Mode::Insert);
}

#[test]
fn cb_changes_previous_word_and_space_and_enters_insert() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.cursor = Position::new(0, 6); // at start of "world"
    }

    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        )
        .unwrap();

    let buf = editor.current_buffer().unwrap();
    assert_eq!(buf.lines[0].as_str(), "world");
    assert_eq!(editor.mode(), Mode::Insert);
}

#[test]
fn yw_yanks_word_and_space_then_pastes() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();

    // yank using w (includes trailing space)
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE),
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
    assert_eq!(buf.lines[0].as_str(), "Hello worldHello ");
}

#[test]
fn ye_yanks_word_only_then_pastes() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();

    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
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
    assert_eq!(buf.lines[0].as_str(), "Hello worldHello");
}

#[test]
fn yb_yanks_previous_word_and_space_then_pastes() {
    let mut editor = make_editor_with_single_line("Hello world");
    let mut key_handler = KeyHandler::test_with_embedded();

    // place cursor at start of second word
    {
        let buf = editor.current_buffer_mut().unwrap();
        buf.cursor = Position::new(0, 6);
    }

    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
        )
        .unwrap();
    key_handler
        .handle_key(
            &mut editor,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
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
    assert_eq!(buf.lines[0].as_str(), "Hello worldHello ");
}
