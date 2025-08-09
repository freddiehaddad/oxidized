use oxidized::core::mode::{Position, Selection, SelectionType};

#[test]
fn character_selection_single_line_span() {
    let sel = Selection::new(Position::new(2, 3), Position::new(2, 7));
    let span = sel.highlight_span_for_line(2, 20).unwrap();
    // Exclusive end for character selection
    assert_eq!(span, (3, 7));
}

#[test]
fn character_selection_multi_line_spans() {
    let sel = Selection::new(Position::new(1, 4), Position::new(3, 2));
    // start line spans from start.col to EOL
    assert_eq!(sel.highlight_span_for_line(1, 10).unwrap(), (4, 10));
    // middle line spans whole line
    assert_eq!(sel.highlight_span_for_line(2, 8).unwrap(), (0, 8));
    // end line spans from BOL to end.col
    assert_eq!(sel.highlight_span_for_line(3, 12).unwrap(), (0, 2));
}

#[test]
fn line_selection_full_lines() {
    let sel = Selection::new_line(Position::new(5, 0), Position::new(7, 0));
    assert_eq!(sel.highlight_span_for_line(4, 9), None);
    assert_eq!(sel.highlight_span_for_line(5, 9).unwrap(), (0, 9));
    assert_eq!(sel.highlight_span_for_line(6, 3).unwrap(), (0, 3));
    assert_eq!(sel.highlight_span_for_line(7, 1).unwrap(), (0, 1));
    assert_eq!(sel.highlight_span_for_line(8, 9), None);
}

#[test]
fn block_selection_inclusive_right_edge() {
    let sel = Selection::new_with_type(
        Position::new(0, 1),
        Position::new(2, 3),
        SelectionType::Block,
    );
    // For each line, the span should include the right column (exclusive end = right+1)
    assert_eq!(sel.highlight_span_for_line(0, 10).unwrap(), (1, 4));
    assert_eq!(sel.highlight_span_for_line(1, 2).unwrap(), (1, 2)); // short line clamps right
    assert_eq!(sel.highlight_span_for_line(2, 10).unwrap(), (1, 4));
}

#[test]
fn block_selection_single_column_when_same_cols() {
    let sel = Selection::new_with_type(
        Position::new(3, 5),
        Position::new(4, 5),
        SelectionType::Block,
    );
    // Should at least highlight one column (5..6)
    assert_eq!(sel.highlight_span_for_line(3, 20).unwrap(), (5, 6));
    // When the left column exceeds the line length, no highlight is produced
    assert_eq!(sel.highlight_span_for_line(4, 2), None);
}
