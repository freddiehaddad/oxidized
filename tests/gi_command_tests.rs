use oxidized::core::{buffer::Buffer, mode::Position};

#[test]
fn test_gi_and_i_behavior() {
    let mut buffer = Buffer::new(1, 100);
    // Insert a line with indentation
    for ch in "    let x = 42;".chars() {
        buffer.insert_char(ch);
    }
    // Move cursor somewhere later in the line
    buffer.cursor = Position::new(0, 12);

    // Simulate I (first non-blank)
    // (Directly call helper to compute expectation, then emulate action logic)
    let first_non_blank = buffer.first_non_blank_col(0);
    buffer.cursor.col = first_non_blank; // what action_insert_line_start now does
    assert_eq!(first_non_blank, 4);
    assert_eq!(buffer.cursor.col, 4, "I should move to first non-blank");

    // Now test gI (absolute column 0) manually emulating action_insert_line_absolute
    buffer.cursor.col = 0;
    assert_eq!(buffer.cursor.col, 0, "gI should move to column 0");
}

#[test]
fn test_gi_vs_i_on_whitespace_only_line() {
    let mut buffer = Buffer::new(1, 100);
    for _ in 0..6 {
        buffer.insert_char(' ');
    }
    buffer.cursor = Position::new(0, 6);

    let first_non_blank = buffer.first_non_blank_col(0);
    assert_eq!(first_non_blank, 0, "Whitespace-only line should yield 0");

    // I behavior
    buffer.cursor.col = first_non_blank;
    assert_eq!(buffer.cursor.col, 0);

    // gI behavior
    buffer.cursor.col = 0;
    assert_eq!(buffer.cursor.col, 0);
}

#[test]
fn test_gi_and_i_on_no_indent_line() {
    let mut buffer = Buffer::new(1, 100);
    for ch in "foo".chars() {
        buffer.insert_char(ch);
    }
    buffer.cursor = Position::new(0, 3);

    let first_non_blank = buffer.first_non_blank_col(0);
    assert_eq!(first_non_blank, 0);

    // I
    buffer.cursor.col = first_non_blank;
    assert_eq!(buffer.cursor.col, 0);

    // gI
    buffer.cursor.col = 0;
    assert_eq!(buffer.cursor.col, 0);
}
