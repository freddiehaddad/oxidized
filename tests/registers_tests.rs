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

#[test]
fn yank_sets_register_0_and_is_stable_across_deletes() {
    let mut b = buf_from("one two\nthree");
    // Yank 'two' to set register 0
    b.cursor = Position::new(0, 4);
    b.yank_to_end_of_line();
    assert_eq!(b.get_register('0').unwrap().text, "two");
    // Now delete a line to change numbered/unnamed, but 0 must remain 'two'
    b.cursor = Position::new(1, 0);
    assert!(b.delete_line());
    assert_eq!(b.get_register('0').unwrap().text, "two");
    // Put from register 0 before cursor (into whatever line exists)
    b.set_active_register('0');
    b.put_before();
    assert!(b.lines.iter().any(|ln| ln.contains("two")));
}

#[test]
fn delete_line_rotates_numbered_registers() {
    let mut b = buf_from("L1\nL2\nL3\nL4");
    // Delete L1, then L2, then L3
    b.cursor = Position::new(0, 0);
    assert!(b.delete_line());
    assert_eq!(b.get_register('1').unwrap().text, "L1\n");
    b.cursor = Position::new(0, 0);
    assert!(b.delete_line());
    assert_eq!(b.get_register('1').unwrap().text, "L2\n");
    assert_eq!(b.get_register('2').unwrap().text, "L1\n");
    b.cursor = Position::new(0, 0);
    assert!(b.delete_line());
    assert_eq!(b.get_register('1').unwrap().text, "L3\n");
    assert_eq!(b.get_register('2').unwrap().text, "L2\n");
    assert_eq!(b.get_register('3').unwrap().text, "L1\n");
}

#[test]
fn small_delete_writes_to_dash() {
    let mut b = buf_from("abc");
    b.cursor = Position::new(0, 1); // on 'b'
    assert!(b.delete_char_at_cursor()); // delete 'b'
    assert_eq!(b.get_register('-').unwrap().text, "b");
    // Numbered delete registers should remain empty
    assert_eq!(b.get_register('1').unwrap().text, "");
}

#[test]
fn explicit_write_to_numbered_does_not_rotate() {
    let mut b = buf_from("xyz");
    b.set_active_register('2');
    b.yank_line();
    assert_eq!(b.get_register('2').unwrap().text, "xyz\n");
    // '1' should still be empty
    assert_eq!(b.get_register('1').unwrap().text, "");
}

#[test]
fn black_hole_delete_does_not_touch_0_or_numbered() {
    let mut b = buf_from("A\nB\nC");
    // Prime register 0 with a yank
    b.cursor = Position::new(0, 0);
    b.yank_line();
    assert_eq!(b.get_register('0').unwrap().text, "A\n");
    // Black-hole a delete-line
    b.set_active_register('_');
    b.cursor = Position::new(1, 0);
    assert!(b.delete_line());
    // 0 unchanged, numbered unchanged
    assert_eq!(b.get_register('0').unwrap().text, "A\n");
    assert_eq!(b.get_register('1').unwrap().text, "");
}
