use std::fmt;

/// Represents different editor modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    VisualBlock,
    Command,
    Replace,
    Search,
    OperatorPending, // For waiting for text object after operator
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Normal => write!(f, "NORMAL"),
            Mode::Insert => write!(f, "INSERT"),
            Mode::Visual => write!(f, "VISUAL"),
            Mode::VisualLine => write!(f, "V-LINE"),
            Mode::VisualBlock => write!(f, "V-BLOCK"),
            Mode::Command => write!(f, "COMMAND"),
            Mode::Replace => write!(f, "REPLACE"),
            Mode::Search => write!(f, "SEARCH"),
            Mode::OperatorPending => write!(f, "OP-PENDING"),
        }
    }
}

/// Cursor position in the buffer
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}

impl Position {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }

    pub fn zero() -> Self {
        Self { row: 0, col: 0 }
    }
}

/// Type of visual selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionType {
    Character, // Character-wise selection (default visual mode)
    Line,      // Line-wise selection (visual line mode)
    Block,     // Block-wise selection (visual block mode)
}

/// Selection range for visual mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub start: Position,
    pub end: Position,
    pub selection_type: SelectionType,
}

impl Selection {
    pub fn new(start: Position, end: Position) -> Self {
        Self {
            start,
            end,
            selection_type: SelectionType::Character,
        }
    }

    /// Normalize selection so that for character & line types start <= end in row/col ordering.
    /// Block selections keep original start/end (anchor vs cursor) but still ensure returned
    /// highlight spans use ordered columns via highlight_span_for_line logic. This method is
    /// idempotent.
    pub fn normalize(&mut self) {
        if self.end.row < self.start.row {
            std::mem::swap(&mut self.start, &mut self.end);
        }
        // Do not swap purely by column for same-row selections; preserves anchor semantics.
    }
    pub fn new_with_type(start: Position, end: Position, selection_type: SelectionType) -> Self {
        Self {
            start,
            end,
            selection_type,
        }
    }

    /// Create a line-wise selection
    pub fn new_line(start: Position, end: Position) -> Self {
        Self {
            start,
            end,
            selection_type: SelectionType::Line,
        }
    }

    /// Compute the highlight span (start_col, end_col_exclusive) for a given line.
    /// Returns None if the selection does not cover the line.
    ///
    /// This centralizes inclusive/exclusive semantics so UI and core stay consistent:
    /// - Character: end is exclusive; multi-line spans cover start->EOL and BOL->end
    /// - Line: full line when within [start.row, end.row]
    /// - Block: rectangular selection inclusive of the cursor column; returns [left, right+1)
    pub fn highlight_span_for_line(
        &self,
        line_number: usize,
        line_length: usize,
    ) -> Option<(usize, usize)> {
        let (start, end) = if self.start.row <= self.end.row {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        };

        match self.selection_type {
            SelectionType::Character => {
                if line_number < start.row || line_number > end.row {
                    return None;
                }
                if start.row == end.row {
                    // Single-line character selection: ensure the anchor character remains
                    // included when extending backward. For forward (start <= end) we keep
                    // existing exclusive end semantics (end already points one past last char
                    // when moving right). For backward (start.col > end.col) we extend range to
                    // include original anchor by using anchor_col+1 as exclusive end.
                    if self.start.col <= self.end.col {
                        Some((self.start.col, self.end.col))
                    } else {
                        Some((self.end.col, self.start.col + 1))
                    }
                } else if line_number == start.row {
                    Some((start.col, line_length))
                } else if line_number == end.row {
                    // End row: include up to end.col (exclusive). If the original ordering was
                    // backward (anchor below), end represents original anchor; include anchor
                    // char by extending one column. Detect by comparing original start/end rows.
                    if self.start.row > self.end.row {
                        Some((0, (end.col + 1).min(line_length)))
                    } else {
                        Some((0, end.col))
                    }
                } else {
                    Some((0, line_length))
                }
            }
            SelectionType::Line => {
                if line_number >= start.row && line_number <= end.row {
                    Some((0, line_length))
                } else {
                    None
                }
            }
            SelectionType::Block => {
                if line_number < start.row || line_number > end.row {
                    return None;
                }
                let left_col = start.col.min(end.col);
                // +1 to make selection inclusive of the cursor column; return exclusive end
                let right_inclusive = start.col.max(end.col) + 1;
                let actual_right = right_inclusive.min(line_length);
                if left_col <= line_length {
                    Some((left_col, actual_right.max(left_col + 1)))
                } else {
                    None
                }
            }
        }
    }
}
