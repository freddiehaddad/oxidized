use oxidized::core::buffer::Buffer;
use oxidized::core::mode::Position;

fn create_test_buffer() -> Buffer {
    Buffer::new(1, 100)
}

fn buffer_from(content: &str) -> Buffer {
    let mut b = create_test_buffer();
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
fn get_text_in_range_single_line() {
    let b = buffer_from("hello world");
    assert_eq!(
        b.get_text_in_range(Position::new(0, 0), Position::new(0, 5)),
        "hello"
    );
    assert_eq!(
        b.get_text_in_range(Position::new(0, 6), Position::new(0, 11)),
        "world"
    );
    // empty range
    assert_eq!(
        b.get_text_in_range(Position::new(0, 3), Position::new(0, 3)),
        ""
    );
}

#[test]
fn get_text_in_range_multi_line() {
    let b = buffer_from("hello\nworld\nrust");
    // From "he[llo" to "ru]st" (exclusive end)
    let text = b.get_text_in_range(Position::new(0, 2), Position::new(2, 2));
    assert_eq!(text, "llo\nworld\nru");
}

#[test]
fn delete_range_single_line() {
    let mut b = buffer_from("abcdef");
    let deleted = b.delete_range(Position::new(0, 2), Position::new(0, 4));
    assert_eq!(deleted, "cd");
    assert_eq!(b.lines, vec!["abef".to_string()]);
    assert_eq!(b.cursor, Position::new(0, 2));
    assert!(b.modified);
}

#[test]
fn delete_range_multi_line() {
    let mut b = buffer_from("012345\nabcdef\nXYZ");
    // delete from after '3' in first line through 'X' (index 1) in third line
    let deleted = b.delete_range(Position::new(0, 4), Position::new(2, 1));
    assert_eq!(deleted, "45\nabcdef\nX");
    // Remaining should be first_part ("0123") + last_part ("YZ")
    assert_eq!(b.lines, vec!["0123YZ".to_string()]);
    assert_eq!(b.cursor, Position::new(0, 4));
    assert!(b.modified);
}

#[test]
fn replace_range_single_line() {
    let mut b = buffer_from("hello world");
    b.replace_range(Position::new(0, 6), Position::new(0, 11), "rust");
    assert_eq!(b.lines, vec!["hello rust".to_string()]);
    // Cursor placed at end of inserted text on same line
    assert_eq!(b.cursor, Position::new(0, 10));
    assert!(b.modified);
}

#[test]
fn replace_range_multi_line() {
    let mut b = buffer_from("abc\n12345\nXYZ");
    // Replace from after 'a' through after 'X' (index 2 exclusive)
    b.replace_range(Position::new(0, 1), Position::new(2, 2), "MIDDLE\nLINES");
    // After deletion, base line becomes "aZ"; inserting creates two lines
    assert_eq!(b.lines, vec!["aMIDDLE".to_string(), "LINESZ".to_string()]);
    // Cursor ends at end of inserted text
    assert_eq!(b.cursor, Position::new(1, 5));
    assert!(b.modified);
}

#[test]
fn range_clamping_out_of_bounds() {
    let mut b = buffer_from("short");
    // Start/end cols beyond line length should clamp safely
    let deleted = b.delete_range(Position::new(0, 10), Position::new(0, 50));
    assert_eq!(deleted, "");
    assert_eq!(b.lines, vec!["short".to_string()]);
    // get_text_in_range with large indices should also be empty and not panic
    let text = b.get_text_in_range(Position::new(0, 10), Position::new(0, 50));
    assert_eq!(text, "");
}

#[test]
fn get_text_in_range_entire_file() {
    let content = "line1\nline2\nline3";
    let b = buffer_from(content);
    // Select entire file
    let text = b.get_text_in_range(Position::new(0, 0), Position::new(2, 5));
    assert_eq!(text, content);
}
