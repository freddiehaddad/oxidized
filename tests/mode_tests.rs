#[cfg(test)]
mod mode_tests {
    use oxidized::core::{Mode, Position, Selection};

    #[test]
    fn test_mode_creation_and_equality() {
        let normal = Mode::Normal;
        let insert = Mode::Insert;
        let visual = Mode::Visual;

        assert_eq!(normal, Mode::Normal);
        assert_ne!(normal, insert);
        assert_ne!(insert, visual);
    }

    #[test]
    fn test_mode_display() {
        assert_eq!(Mode::Normal.to_string(), "NORMAL");
        assert_eq!(Mode::Insert.to_string(), "INSERT");
        assert_eq!(Mode::Visual.to_string(), "VISUAL");
        assert_eq!(Mode::VisualLine.to_string(), "V-LINE");
        assert_eq!(Mode::VisualBlock.to_string(), "V-BLOCK");
        assert_eq!(Mode::Command.to_string(), "COMMAND");
        assert_eq!(Mode::Replace.to_string(), "REPLACE");
        assert_eq!(Mode::Search.to_string(), "SEARCH");
        assert_eq!(Mode::OperatorPending.to_string(), "OP-PENDING");
    }

    #[test]
    fn test_mode_debug() {
        let mode = Mode::Visual;
        let debug_str = format!("{:?}", mode);
        assert_eq!(debug_str, "Visual");
    }

    #[test]
    fn test_mode_clone() {
        let original = Mode::VisualLine;
    let copied = original; // Mode is Copy
    assert_eq!(original, copied);
    }

    #[test]
    fn test_mode_copy() {
        let original = Mode::VisualBlock;
        let copied = original; // Copy trait allows this
        assert_eq!(original, copied);
    }

    #[test]
    fn test_position_creation() {
        let pos = Position::new(5, 10);
        assert_eq!(pos.row, 5);
        assert_eq!(pos.col, 10);
    }

    #[test]
    fn test_position_zero() {
        let pos = Position::zero();
        assert_eq!(pos.row, 0);
        assert_eq!(pos.col, 0);
    }

    #[test]
    fn test_position_equality() {
        let pos1 = Position::new(3, 7);
        let pos2 = Position::new(3, 7);
        let pos3 = Position::new(3, 8);

        assert_eq!(pos1, pos2);
        assert_ne!(pos1, pos3);
    }

    #[test]
    fn test_position_debug() {
        let pos = Position::new(2, 4);
        let debug_str = format!("{:?}", pos);
        assert!(debug_str.contains("2"));
        assert!(debug_str.contains("4"));
    }

    #[test]
    fn test_position_clone() {
        let original = Position::new(1, 2);
    let copied = original; // Position is Copy
    assert_eq!(original, copied);
    }

    #[test]
    fn test_position_copy() {
        let original = Position::new(8, 9);
        let copied = original; // Copy trait allows this
        assert_eq!(original, copied);
    }

    #[test]
    fn test_selection_creation() {
        let start = Position::new(1, 2);
        let end = Position::new(3, 4);
        let selection = Selection::new(start, end);

        assert_eq!(selection.start, start);
        assert_eq!(selection.end, end);
    }

    #[test]
    fn test_selection_equality() {
        let start1 = Position::new(1, 1);
        let end1 = Position::new(2, 2);
        let start2 = Position::new(1, 1);
        let end2 = Position::new(2, 2);

        let sel1 = Selection::new(start1, end1);
        let sel2 = Selection::new(start2, end2);
        let sel3 = Selection::new(start1, Position::new(2, 3));

        assert_eq!(sel1, sel2);
        assert_ne!(sel1, sel3);
    }

    #[test]
    fn test_selection_debug() {
        let selection = Selection::new(Position::new(0, 1), Position::new(2, 3));
        let debug_str = format!("{:?}", selection);
        assert!(debug_str.contains("Selection"));
    }

    #[test]
    fn test_selection_clone() {
        let original = Selection::new(Position::new(1, 2), Position::new(3, 4));
    let copied = original; // Selection is Copy
    assert_eq!(original, copied);
    }

    #[test]
    fn test_selection_copy() {
        let original = Selection::new(Position::new(5, 6), Position::new(7, 8));
        let copied = original; // Copy trait allows this
        assert_eq!(original, copied);
    }

    #[test]
    fn test_selection_zero_positions() {
        let selection = Selection::new(Position::zero(), Position::zero());
        assert_eq!(selection.start.row, 0);
        assert_eq!(selection.start.col, 0);
        assert_eq!(selection.end.row, 0);
        assert_eq!(selection.end.col, 0);
    }

    #[test]
    fn test_position_large_values() {
        let pos = Position::new(usize::MAX, usize::MAX - 1);
        assert_eq!(pos.row, usize::MAX);
        assert_eq!(pos.col, usize::MAX - 1);
    }

    #[test]
    fn test_all_mode_variants() {
    let modes = [
            Mode::Normal,
            Mode::Insert,
            Mode::Visual,
            Mode::VisualLine,
            Mode::VisualBlock,
            Mode::Command,
            Mode::Replace,
            Mode::Search,
            Mode::OperatorPending,
    ];

        // Ensure all modes are unique
        for (i, mode1) in modes.iter().enumerate() {
            for (j, mode2) in modes.iter().enumerate() {
                if i != j {
                    assert_ne!(
                        mode1, mode2,
                        "Modes at index {} and {} should not be equal",
                        i, j
                    );
                }
            }
        }
    }

    #[test]
    fn test_mode_transition_scenario() {
        // Simulate common mode transitions and verify each transition works

        // Normal -> Insert
        let mode = Mode::Insert;
        assert_eq!(mode, Mode::Insert);

        // Insert -> Normal (Escape)
        let mode = Mode::Normal;
        assert_eq!(mode, Mode::Normal);

        // Normal -> Visual
        let mode = Mode::Visual;
        assert_eq!(mode, Mode::Visual);

        // Visual -> Normal
        let mode = Mode::Normal;
        assert_eq!(mode, Mode::Normal);

        // Normal -> Command
        let mode = Mode::Command;
        assert_eq!(mode, Mode::Command);
    }

    #[test]
    fn test_selection_boundary_conditions() {
        // Test same start and end position
        let pos = Position::new(5, 5);
        let selection = Selection::new(pos, pos);
        assert_eq!(selection.start, selection.end);

        // Test reverse selection (end before start)
        let start = Position::new(10, 10);
        let end = Position::new(5, 5);
        let selection = Selection::new(start, end);
        assert_eq!(selection.start.row, 10);
        assert_eq!(selection.end.row, 5);
    }
}
