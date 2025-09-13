//! Cursor motion helpers (Phase 1).
//!
//! These operate purely on a `Buffer` + `Position` pair and are free of global editor state.
//! Future phases (multi-cursor, selections, operators) can build on this without depending
//! on higher-level crates.

use crate::{Buffer, Position, grapheme};

/// Normalize a position for Vim Normal-mode semantics:
/// If the cursor byte is exactly at the end-of-line (line_len) and the line is non-empty,
/// clamp it to the start byte of the last grapheme cluster so the cursor rests on a real
/// character cell (Vim block cursor behavior). No change for empty lines.
pub fn normalize_normal_mode_position(buf: &Buffer, pos: &mut Position) {
    if pos.line >= buf.line_count() {
        return;
    }
    let line_len = buf.line_byte_len(pos.line); // excludes trailing newline
    if line_len == 0 {
        return;
    }
    if pos.byte == line_len {
        // clamp
        if let Some(line_full) = buf.line(pos.line) {
            let content = if line_full.ends_with('\n') {
                &line_full[..line_full.len() - 1]
            } else {
                &line_full
            };
            let prev = grapheme::prev_boundary(content, content.len());
            pos.byte = prev;
        }
    } else if pos.byte > line_len {
        // defensive clamp if ever past end
        pos.byte = line_len.saturating_sub(1); // will be normalized again if needed
        if let Some(line_full) = buf.line(pos.line) {
            let content = if line_full.ends_with('\n') {
                &line_full[..line_full.len() - 1]
            } else {
                &line_full
            };
            pos.byte = grapheme::prev_boundary(content, content.len());
        }
    }
}

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

/// Move up one line preserving a target visual column (sticky). Returns the updated sticky column.
/// Caller should maintain the returned sticky column across successive vertical motions. If `sticky_col`
/// is `None`, it will be computed from the current position's visual column.
pub fn up(buf: &Buffer, pos: &mut Position, mut sticky_col: Option<usize>) -> Option<usize> {
    if pos.line == 0 {
        return sticky_col;
    }
    let current_line = buf.line(pos.line).unwrap_or_default();
    let current_content = if current_line.ends_with('\n') {
        &current_line[..current_line.len() - 1]
    } else {
        &current_line
    };
    if sticky_col.is_none() {
        sticky_col = Some(grapheme::visual_col(current_content, pos.byte));
    }
    pos.line -= 1;
    pos.byte = byte_for_visual_col(buf, pos.line, sticky_col.unwrap());
    sticky_col
}

/// Move down one line preserving sticky visual column.
pub fn down(buf: &Buffer, pos: &mut Position, mut sticky_col: Option<usize>) -> Option<usize> {
    if pos.line + 1 >= buf.line_count() {
        return sticky_col;
    }
    let current_line = buf.line(pos.line).unwrap_or_default();
    let current_content = if current_line.ends_with('\n') {
        &current_line[..current_line.len() - 1]
    } else {
        &current_line
    };
    if sticky_col.is_none() {
        sticky_col = Some(grapheme::visual_col(current_content, pos.byte));
    }
    pos.line += 1;
    pos.byte = byte_for_visual_col(buf, pos.line, sticky_col.unwrap());
    sticky_col
}

/// Convert a target visual column into a byte offset on a given line, clamping to line end.
fn byte_for_visual_col(buf: &Buffer, line: usize, target_col: usize) -> usize {
    if let Some(line_str) = buf.line(line) {
        let content = if line_str.ends_with('\n') {
            &line_str[..line_str.len() - 1]
        } else {
            &line_str
        };
        let mut col = 0;
        let mut byte = 0;
        for (b, g) in unicode_segmentation::UnicodeSegmentation::grapheme_indices(content, true) {
            let w = grapheme::cluster_width(g);
            if col + w > target_col {
                return b;
            }
            col += w;
            byte = b + g.len();
        }
        return byte; // end of line
    }
    0
}

/// Move forward to the start of the next word. Semantics (naive):
/// - If currently on a word cluster, advance to boundary after the current word, then skip any non-word clusters, landing on first cluster of next word.
/// - If on whitespace / punctuation, skip them until a word cluster; if at EOL, move to start of next line's first word.
pub fn word_forward(buf: &Buffer, pos: &mut Position) {
    let mut line = pos.line;
    let mut byte = pos.byte;
    // Helper to get line content sans newline
    let get_line = |buf: &Buffer, idx: usize| -> Option<String> {
        buf.line(idx).map(|l| {
            if l.ends_with('\n') {
                l[..l.len() - 1].to_string()
            } else {
                l
            }
        })
    };
    let mut line_content = match get_line(buf, line) {
        Some(s) => s,
        None => return,
    };
    if byte > line_content.len() {
        byte = line_content.len();
    }
    if byte >= line_content.len() {
        // Move to next line start
        if line + 1 >= buf.line_count() {
            return;
        }
        line += 1;
        byte = 0;
        line_content = get_line(buf, line).unwrap();
    }
    // If starting on a word char, skip the rest of this word first
    let next_b = grapheme::next_boundary(&line_content, byte);
    if next_b > byte {
        let current = &line_content[byte..next_b];
        if grapheme::is_word(current) {
            let mut b = next_b;
            while b < line_content.len() {
                let nb = grapheme::next_boundary(&line_content, b);
                let c = &line_content[b..nb];
                if !grapheme::is_word(c) {
                    break;
                }
                b = nb;
            }
            byte = b;
        }
    }
    // Skip non-word clusters to next word start (could be same line or next line)
    loop {
        if byte >= line_content.len() {
            if line + 1 >= buf.line_count() {
                pos.line = line;
                pos.byte = line_content.len();
                return;
            }
            line += 1;
            byte = 0;
            line_content = get_line(buf, line).unwrap();
        }
        let nb = grapheme::next_boundary(&line_content, byte);
        let cluster = &line_content[byte..nb];
        if grapheme::is_word(cluster) {
            pos.line = line;
            pos.byte = byte;
            return;
        }
        byte = nb;
    }
}

/// Move backward to the start of the previous word. If currently at start of a word, move to start of previous word.
pub fn word_backward(buf: &Buffer, pos: &mut Position) {
    if pos.line >= buf.line_count() {
        return;
    }
    // Helper to get line content sans newline
    let get_line = |buf: &Buffer, idx: usize| -> Option<String> {
        buf.line(idx).map(|l| {
            if l.ends_with('\n') {
                l[..l.len() - 1].to_string()
            } else {
                l
            }
        })
    };
    let mut line = pos.line;
    let mut line_content = get_line(buf, line).unwrap_or_default();
    let mut byte = pos.byte;
    if byte == 0 {
        if line == 0 {
            pos.byte = 0;
            return;
        }
        line -= 1;
        line_content = get_line(buf, line).unwrap_or_default();
        byte = line_content.len();
    }
    // Step back one cluster if currently at a word start to move inside previous region
    let prev = grapheme::prev_boundary(&line_content, byte);
    if prev < byte {
        byte = prev;
    }
    // Skip punctuation/whitespace backwards
    while byte > 0 {
        let prev_b = grapheme::prev_boundary(&line_content, byte);
        let cluster = &line_content[prev_b..byte];
        if grapheme::is_word(cluster) {
            break;
        }
        byte = prev_b;
    }
    // Skip word chars backwards to start of word
    while byte > 0 {
        let prev_b = grapheme::prev_boundary(&line_content, byte);
        let cluster = &line_content[prev_b..byte];
        if !grapheme::is_word(cluster) {
            break;
        }
        byte = prev_b;
    }
    pos.line = line;
    pos.byte = byte;
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

    #[test]
    fn vertical_motions_basic_and_sticky() {
        let buf = Buffer::from_str("t", "a😀\nxyz\nlonger line here").unwrap();
        let mut pos = Position::new(0, 0);
        // Move right over 'a'
        right(&buf, &mut pos);
        // Now over emoji
        right(&buf, &mut pos);
        // Capture sticky by moving down twice
        let mut sticky = None;
        sticky = down(&buf, &mut pos, sticky);
        let first_col_byte = pos.byte;
        sticky = down(&buf, &mut pos, sticky);
        // Move up and ensure we land at same visual column (or line end if shorter)
        sticky = up(&buf, &mut pos, sticky);
        assert_eq!(pos.line, 1); // moved back to second line
        up(&buf, &mut pos, sticky); // now top line again
        assert_eq!(pos.line, 0);
        assert!(pos.byte >= first_col_byte); // On first line width may clamp (emoji wide)
    }

    #[test]
    fn word_forward_and_backward_basic() {
        let buf = Buffer::from_str("t", "foo, bar baz\nqux!! zip").unwrap();
        let mut pos = Position::new(0, 0);
        // forward through words
        word_forward(&buf, &mut pos); // at foo already, move to bar
        assert_eq!(pos.line, 0);
        let line0 = buf.line(0).unwrap();
        let bar_idx = line0.find("bar").unwrap();
        assert_eq!(pos.byte, bar_idx);
        word_forward(&buf, &mut pos); // baz
        let baz_idx = line0.find("baz").unwrap();
        assert_eq!(pos.byte, baz_idx);
        word_forward(&buf, &mut pos); // next line qux
        assert_eq!(pos.line, 1);
        let line1 = buf.line(1).unwrap();
        let qux_idx = line1.find("qux").unwrap();
        assert_eq!(pos.byte, qux_idx);
        // backward
        word_backward(&buf, &mut pos); // back to baz
        assert_eq!(pos.line, 0);
        assert_eq!(pos.byte, baz_idx);
        word_backward(&buf, &mut pos); // bar
        assert_eq!(pos.byte, bar_idx);
        word_backward(&buf, &mut pos); // foo
        assert_eq!(pos.byte, 0);
    }

    #[test]
    fn word_motion_cross_line_edges() {
        // Ensures word_forward at end of line moves to first word of next non-empty line and backward wraps.
        let buf = Buffer::from_str("t", "alpha\n\n beta gamma\n\nzzz").unwrap();
        let mut pos = Position::new(0, 0); // at 'alpha'
        // Move to next line's first word (beta) skipping blank line
        word_forward(&buf, &mut pos); // should go to beta
        // Ensure current line contains beta
        let current_line_str = buf.line(pos.line).unwrap();
        let beta_idx = current_line_str.find("beta").expect("beta present");
        assert_eq!(pos.byte, beta_idx);
        // Move forward to gamma (same line)
        word_forward(&buf, &mut pos);
        let gamma_line_str = buf.line(pos.line).unwrap();
        let gamma_idx = gamma_line_str.find("gamma").unwrap();
        assert_eq!(pos.byte, gamma_idx);
        // Move forward to zzz (final non-empty line)
        word_forward(&buf, &mut pos);
        let z_line = buf.line(pos.line).unwrap();
        assert!(z_line.contains("zzz"));
        assert_eq!(pos.byte, 0, "should land at start of final word line");
        // Move backward returns to a prior non-empty line (gamma or beta depending on naive parsing)
        word_backward(&buf, &mut pos);
        assert!(
            pos.line < buf.line_count() - 1,
            "should have moved off final line"
        );
        let mut back_line = buf.line(pos.line).unwrap();
        if back_line.trim().is_empty() {
            // Move backward again to reach previous word line
            word_backward(&buf, &mut pos);
            back_line = buf.line(pos.line).unwrap();
        }
        assert!(
            back_line.contains("beta") || back_line.contains("gamma"),
            "expected to reach a word line (beta|gamma)"
        );
    }
}
