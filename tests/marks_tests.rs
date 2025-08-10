use oxidized::Buffer;
use oxidized::core::mode::Position;

#[test]
fn set_and_get_mark() {
    let mut buf = Buffer::new(1, 100);
    buf.lines = vec!["hello".into(), "  world".into(), "".into()];
    buf.move_cursor(Position::new(1, 3));

    buf.set_mark('a');
    let pos = buf.get_mark('a').expect("mark should be set");
    assert_eq!(pos, Position::new(1, 3));
}

#[test]
fn jump_to_mark_exact_moves_to_same_position() {
    let mut buf = Buffer::new(1, 100);
    buf.lines = vec!["abc".into(), "defg".into(), "hij".into()];

    // Set mark at line 1, col 2
    buf.move_cursor(Position::new(1, 2));
    buf.set_mark('x');

    // Move elsewhere
    buf.move_cursor(Position::new(0, 1));
    let jumped = buf.jump_to_mark_exact('x');
    assert!(jumped, "expected jump to succeed");
    assert_eq!(buf.cursor, Position::new(1, 2));
}

#[test]
fn jump_to_mark_line_moves_to_first_non_blank() {
    let mut buf = Buffer::new(1, 100);
    buf.lines = vec![
        "leading".into(),
        "    spaces then text".into(),
        "\t\tindented".into(),
        "".into(),
    ];

    // Set mark on line with spaces; col doesn't matter for line jump
    buf.move_cursor(Position::new(1, 12));
    buf.set_mark('l');

    // Move elsewhere and jump to line
    buf.move_cursor(Position::new(0, 0));
    let jumped = buf.jump_to_mark_line('l');
    assert!(jumped);
    assert_eq!(buf.cursor.row, 1);
    // First non-blank of line 1 is at column 4
    assert_eq!(buf.cursor.col, 4);
}

#[test]
fn jump_to_missing_mark_returns_false() {
    let mut buf = Buffer::new(1, 100);
    buf.lines = vec!["one".into(), "two".into()];
    buf.move_cursor(Position::new(0, 0));

    assert!(!buf.jump_to_mark_exact('z'));
    assert!(!buf.jump_to_mark_line('z'));
}
