use oxidized::core::buffer::Buffer;
use oxidized::core::mode::Position;
use unicode_segmentation::UnicodeSegmentation;

fn grapheme_starts(s: &str) -> Vec<usize> {
    s.grapheme_indices(true).map(|(i, _)| i).collect()
}

#[test]
fn move_right_across_emoji_lands_after_emoji() {
    let line = "## 📋 Table of Contents".to_string();
    let mut buf = Buffer::new(1, 100);
    buf.lines = vec![line.clone()];
    buf.cursor = Position { row: 0, col: 0 };

    // Expected grapheme boundaries
    let starts = grapheme_starts(&line);
    // Press 'l' four times: after '#', '#', ' ', '📋' -> lands at start of space after emoji
    buf.move_cursor_right();
    buf.move_cursor_right();
    buf.move_cursor_right();
    buf.move_cursor_right();

    // The cursor should be at the start of the space after the emoji
    // That is the 4th index (0-based) + 1 => starts[4]
    assert_eq!(
        buf.cursor.col, starts[4],
        "Cursor should land after the emoji"
    );
}

#[test]
fn delete_char_at_cursor_removes_space_after_emoji() {
    let line = "## 📋 Table of Contents".to_string();
    let mut buf = Buffer::new(1, 100);
    buf.lines = vec![line.clone()];
    buf.cursor = Position { row: 0, col: 0 };

    // Move to the space after the emoji
    buf.move_cursor_right();
    buf.move_cursor_right();
    buf.move_cursor_right();
    buf.move_cursor_right();

    assert!(buf.delete_char_at_cursor(), "Deletion should occur");
    assert_eq!(buf.lines[0], "## 📋Table of Contents");
}

#[test]
fn delete_emoji_as_single_grapheme() {
    let line = "## 📋 Table of Contents".to_string();
    let mut buf = Buffer::new(1, 100);
    buf.lines = vec![line.clone()];
    buf.cursor = Position { row: 0, col: 0 };

    // Move to the emoji start (three moves: '#', '#', ' ')
    buf.move_cursor_right();
    buf.move_cursor_right();
    buf.move_cursor_right();

    // Now at emoji start; delete at cursor should remove the emoji cluster only
    assert!(
        buf.delete_char_at_cursor(),
        "Emoji grapheme should be deleted as one unit"
    );
    assert_eq!(
        buf.lines[0], "##  Table of Contents",
        "Emoji deletion should leave two spaces"
    );
}
