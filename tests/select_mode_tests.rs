use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use oxidized::core::editor::Editor;
use oxidized::core::mode::Mode;
use oxidized::input::keymap::KeyHandler;

fn make() -> (KeyHandler, Editor) {
    let _ = std::env::set_current_dir(env!("CARGO_MANIFEST_DIR"));
    let mut editor = Editor::new().unwrap();
    let _ = editor.create_buffer(None).unwrap();
    (KeyHandler::new(), editor)
}

#[test]
fn gh_enters_select_mode() -> Result<()> {
    let (mut kh, mut ed) = make();
    kh.handle_key(
        &mut ed,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    kh.handle_key(
        &mut ed,
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
    )?;
    assert_eq!(ed.mode(), Mode::Select, "gh should enter Select mode");
    assert!(
        ed.current_buffer().and_then(|b| b.selection).is_some(),
        "selection should be started"
    );
    Ok(())
}

#[test]
#[allow(non_snake_case)]
fn gH_enters_select_line_mode() -> Result<()> {
    let (mut kh, mut ed) = make();
    kh.handle_key(
        &mut ed,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?; // prefix
    kh.handle_key(
        &mut ed,
        KeyEvent::new(KeyCode::Char('H'), KeyModifiers::NONE),
    )?; // completes gH
    assert_eq!(
        ed.mode(),
        Mode::SelectLine,
        "gH should enter SelectLine mode"
    );
    let sel = ed
        .current_buffer()
        .and_then(|b| b.selection)
        .expect("selection present");
    // In line mode selection start col should be 0
    assert_eq!(sel.start.col, 0);
    Ok(())
}

#[test]
fn select_mode_char_replaces_and_enters_insert() -> Result<()> {
    let (mut kh, mut ed) = make();
    // Enter select mode
    kh.handle_key(
        &mut ed,
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
    )?;
    kh.handle_key(
        &mut ed,
        KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
    )?;
    assert_eq!(
        ed.mode(),
        Mode::Select,
        "gh should enter Select before char"
    );
    // Type 'x' which should invoke select_insert_char -> replace selection & enter Insert
    kh.handle_key(
        &mut ed,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
    )?;
    assert_eq!(ed.mode(), Mode::Insert, "typing char should enter Insert");
    let buf = ed.current_buffer().unwrap();
    let line0 = buf.lines.first().cloned().unwrap_or_default();
    assert!(
        line0.contains('x'),
        "buffer should contain inserted character"
    );
    Ok(())
}
