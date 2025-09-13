//! Cursor motion helpers (Phase 1).
//!
//! These operate purely on a `Buffer` + `Position` pair and are free of global editor state.
//! Future phases (multi-cursor, selections, operators) can build on this without depending
//! on higher-level crates.

use crate::{Buffer, Position, grapheme};

/// Move left one grapheme boundary.
pub fn left(buf: &Buffer, pos: &mut Position) {
    if pos.byte == 0 {
        return;
    }
    if let Some(line) = buf.line(pos.line) {
        let content = if line.ends_with('\n') {
            &line[..line.len() - 1]
        } else {
            &line
        };
        let prev = grapheme::prev_boundary(content, pos.byte);
        pos.byte = prev;
    }
}

/// Move right one grapheme boundary.
pub fn right(buf: &Buffer, pos: &mut Position) {
    if let Some(line) = buf.line(pos.line) {
        let content = if line.ends_with('\n') {
            &line[..line.len() - 1]
        } else {
            &line
        };
        let next = grapheme::next_boundary(content, pos.byte);
        if next > pos.byte {
            pos.byte = next;
        }
    }
}

/// Move to start of line.
pub fn line_start(_buf: &Buffer, pos: &mut Position) {
    pos.byte = 0;
}

/// Move to end of line (after last grapheme).
pub fn line_end(buf: &Buffer, pos: &mut Position) {
    pos.byte = buf.line_byte_len(pos.line);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn horizontal_and_line_boundaries() {
        let buf = Buffer::from_str("t", "ab😀c").unwrap();
        let mut pos = Position::origin();
        // Move right until end
        while pos.byte < buf.line_byte_len(0) {
            right(&buf, &mut pos);
        }
        let end = buf.line_byte_len(0);
        assert_eq!(pos.byte, end);
        // Move left twice
        left(&buf, &mut pos);
        left(&buf, &mut pos);
        assert!(pos.byte < end);
        // Jump to start and back to end
        line_start(&buf, &mut pos);
        assert_eq!(pos.byte, 0);
        line_end(&buf, &mut pos);
        assert_eq!(pos.byte, end);
    }
}
