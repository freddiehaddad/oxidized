use oxidized::core::buffer::Buffer;
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
fn unnamed_register_defaults_and_put() {
    let mut b = buf_from("alpha beta");
    b.cursor = Position::new(0, 6); // at 'b'
    b.yank_to_end_of_line();
    assert_eq!(b.get_register('"').unwrap().text, "beta");
    // Paste from unnamed
    b.cursor = Position::new(0, 5);
    b.put_before();
    assert_eq!(b.lines, vec!["alphabeta beta".to_string()]);
}

#[test]
fn named_register_yank_and_put() {
    let mut b = buf_from("one two");
    // Select register a
    b.set_active_register('a');
    b.cursor = Position::new(0, 4); // at 't'
    b.yank_to_end_of_line();
    assert_eq!(b.get_register('a').unwrap().text, "two");
    // Now paste from register a before cursor 0,2
    b.cursor = Position::new(0, 2);
    b.set_active_register('a');
    b.put_before();
    assert_eq!(b.lines[0], "ontwoe two");
}

#[test]
fn black_hole_register_drops_writes() {
    let mut b = buf_from("xyz");
    // Delete char into black-hole
    b.set_active_register('_');
    b.cursor = Position::new(0, 1);
    assert!(b.delete_char_at_cursor());
    // Unnamed should remain default empty
    assert_eq!(b.get_register('"').unwrap().text, "");
}

#[test]
fn upper_case_appends_to_lowercase_register() {
    let mut b = buf_from("ab cd");
    // Yank 'cd' into A
    b.cursor = Position::new(0, 3);
    b.set_active_register('A');
    b.yank_to_end_of_line();
    assert_eq!(b.get_register('a').unwrap().text, "cd");
    // Yank 'ab' into A again (append)
    b.cursor = Position::new(0, 0);
    b.set_active_register('A');
    b.yank_word();
    assert_eq!(b.get_register('a').unwrap().text, "cdab");
}
