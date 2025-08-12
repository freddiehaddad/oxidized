use oxidized::core::buffer::{Buffer, YankType};
use oxidized::core::mode::Position;

fn buf_from(content: &str) -> Buffer {
    let mut b = Buffer::new(1, 100);
    b.lines.clear();
    for line in content.lines() {
        b.lines.push(line.to_string());
    }
    if b.lines.is_empty() {
        b.lines.push(String::new());
    }
    b
}

#[test]
fn yank_line_and_put_after() {
    let mut b = buf_from("one\ntwo\nthree");
    b.cursor = Position::new(1, 1); // on line "two"
    b.yank_line();
    assert_eq!(b.clipboard.yank_type, YankType::Line);
    assert_eq!(b.clipboard.text, "two\n");

    b.put_after();
    // Should insert a new line after current line
    assert_eq!(b.lines, vec!["one", "two", "two\n", "three"]);
    assert_eq!(b.cursor, Position::new(2, 0));
}

#[test]
fn yank_to_eol_and_put_before() {
    let mut b = buf_from("hello world");
    b.cursor = Position::new(0, 6); // at 'w'
    b.yank_to_end_of_line();
    assert_eq!(b.clipboard.yank_type, YankType::Character);
    assert_eq!(b.clipboard.text, "world");

    b.cursor = Position::new(0, 5); // before space's previous index
    b.put_before();
    assert_eq!(b.lines, vec!["helloworld world"]);
    // Cursor moves to col + (len(text) - 1) = 5 + 4 = 9
    assert_eq!(b.cursor, Position::new(0, 9));
}

#[test]
fn yank_word_and_put_after_multi_line() {
    let mut b = buf_from("abc def\nXYZ");
    b.cursor = Position::new(0, 4); // at 'd' of def
    b.yank_word();
    assert_eq!(b.clipboard.yank_type, YankType::Character);
    assert_eq!(b.clipboard.text, "def");

    // Move to end of first line and put after -> appends to the same line
    b.cursor = Position::new(0, b.lines[0].len());
    b.put_after();

    assert_eq!(b.lines, vec!["abc defdef", "XYZ"]);
    // Cursor at insert_pos + (len(text) - 1) = 7 + (3 - 1) = 9
    assert_eq!(b.cursor, Position::new(0, 9));
}

#[test]
fn block_paste_after_extends_on_last_line() {
    let mut b = buf_from("A\nB");
    // Prepare block content (two lines)
    b.clipboard.text = "11\n22".into();
    b.clipboard.yank_type = YankType::Block;

    // Place cursor on last line -> put_after should start at next row
    b.cursor = Position::new(1, 0);
    b.put_after();

    assert_eq!(b.lines, vec!["A", "B", "11", "22"]);
    assert_eq!(b.cursor, Position::new(2, 1));
}

#[test]
fn block_paste_before_inserts_at_cursor_col() {
    let mut b = buf_from("abcd\nef");
    b.clipboard.text = "XX\nYY".into();
    b.clipboard.yank_type = YankType::Block;

    b.cursor = Position::new(0, 2);
    b.put_before();

    assert_eq!(b.lines, vec!["abXXcd", "efYY"]);
    assert_eq!(b.cursor, Position::new(0, 3));
}
