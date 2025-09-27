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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClusterKind {
    Word,
    Blank,
    Other,
}

fn classify_cluster(cluster: &str) -> ClusterKind {
    if grapheme::is_word(cluster) {
        ClusterKind::Word
    } else if cluster.chars().all(|c| c.is_whitespace()) {
        ClusterKind::Blank
    } else {
        ClusterKind::Other
    }
}

fn line_without_newline(buf: &Buffer, idx: usize) -> String {
    buf.line(idx)
        .map(|mut l| {
            if l.ends_with('\n') {
                l.pop();
            }
            l
        })
        .unwrap_or_default()
}

fn advance_line_forward(
    buf: &Buffer,
    line: &mut usize,
    byte: &mut usize,
    line_content: &mut String,
) -> bool {
    if *line + 1 >= buf.line_count() {
        *byte = line_content.len();
        return false;
    }
    *line += 1;
    *line_content = line_without_newline(buf, *line);
    *byte = 0;
    true
}

fn skip_blanks_forward(
    buf: &Buffer,
    line: &mut usize,
    byte: &mut usize,
    line_content: &mut String,
) -> bool {
    loop {
        if *byte >= line_content.len() {
            if !advance_line_forward(buf, line, byte, line_content) {
                return false;
            }
            if line_content.is_empty() {
                continue;
            }
        }
        let nb = grapheme::next_boundary(line_content, *byte);
        let cluster = &line_content[*byte..nb];
        if classify_cluster(cluster) == ClusterKind::Blank {
            *byte = nb;
            continue;
        }
        return true;
    }
}

fn skip_kind_in_line(line: &str, mut byte: usize, kind: ClusterKind) -> usize {
    while byte < line.len() {
        let nb = grapheme::next_boundary(line, byte);
        let cluster = &line[byte..nb];
        if classify_cluster(cluster) != kind {
            break;
        }
        byte = nb;
    }
    byte
}

fn retreat_line(
    buf: &Buffer,
    line: &mut usize,
    byte: &mut usize,
    line_content: &mut String,
) -> bool {
    if *line == 0 {
        return false;
    }
    *line -= 1;
    *line_content = line_without_newline(buf, *line);
    *byte = line_content.len();
    true
}

/// Move forward to the start of the next token following Vim `w` semantics.
/// - Word tokens consist of Unicode letters, digits, underscores, and apostrophes (for contractions).
/// - Punctuation tokens (non-word, non-whitespace graphemes) are treated as standalone stops.
/// - Whitespace is skipped until the next word or punctuation token, traversing lines as needed.
pub fn word_forward(buf: &Buffer, pos: &mut Position) {
    if buf.line_count() == 0 {
        return;
    }
    let mut line = pos.line.min(buf.line_count() - 1);
    let mut line_content = line_without_newline(buf, line);
    let mut byte = pos.byte.min(line_content.len());
    if byte >= line_content.len() {
        let _ = skip_blanks_forward(buf, &mut line, &mut byte, &mut line_content);
        pos.line = line;
        pos.byte = byte;
        return;
    }

    let nb = grapheme::next_boundary(&line_content, byte);
    let cluster = &line_content[byte..nb];
    byte = match classify_cluster(cluster) {
        ClusterKind::Blank => nb,
        ClusterKind::Word => skip_kind_in_line(&line_content, byte, ClusterKind::Word),
        ClusterKind::Other => skip_kind_in_line(&line_content, byte, ClusterKind::Other),
    };
    let _ = skip_blanks_forward(buf, &mut line, &mut byte, &mut line_content);
    pos.line = line;
    pos.byte = byte;
}

/// Move backward to the start of the previous token following Vim `b` semantics.
/// If currently at the start of a token, move to the beginning of the prior word or punctuation token,
/// skipping intervening whitespace and blank lines.
pub fn word_backward(buf: &Buffer, pos: &mut Position) {
    if buf.line_count() == 0 {
        return;
    }
    let mut line = pos.line.min(buf.line_count() - 1);
    let mut line_content = line_without_newline(buf, line);
    let mut byte = pos.byte.min(line_content.len());

    loop {
        if byte == 0 {
            if !retreat_line(buf, &mut line, &mut byte, &mut line_content) {
                pos.line = 0;
                pos.byte = 0;
                return;
            }
            continue;
        }
        let prev_b = grapheme::prev_boundary(&line_content, byte);
        if prev_b == byte {
            if !retreat_line(buf, &mut line, &mut byte, &mut line_content) {
                pos.line = 0;
                pos.byte = 0;
                return;
            }
            continue;
        }
        let cluster = &line_content[prev_b..byte];
        let kind = classify_cluster(cluster);
        match kind {
            ClusterKind::Blank => {
                byte = prev_b;
                continue;
            }
            ClusterKind::Word | ClusterKind::Other => {
                byte = prev_b;
                while byte > 0 {
                    let before = grapheme::prev_boundary(&line_content, byte);
                    if before == byte {
                        break;
                    }
                    let prev_cluster = &line_content[before..byte];
                    if classify_cluster(prev_cluster) != kind {
                        break;
                    }
                    byte = before;
                }
                pos.line = line;
                pos.byte = byte;
                return;
            }
        }
    }
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
        let buf = Buffer::from_str("t", "foo, bar can't stop 123!").unwrap();
        let line0 = buf.line(0).unwrap();
        let comma_idx = line0.find(',').unwrap();
        let bar_idx = line0.find("bar").unwrap();
        let cant_idx = line0.find("can't").unwrap();
        let stop_idx = line0.find("stop").unwrap();
        let digits_idx = line0.find("123").unwrap();
        let excl_idx = line0.find('!').unwrap();

        let mut pos = Position::new(0, 0);
        word_forward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, comma_idx));
        word_forward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, bar_idx));
        word_forward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, cant_idx));
        word_forward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, stop_idx));
        word_forward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, digits_idx));
        word_forward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, excl_idx));

        // Starting on an internal apostrophe should treat it as part of the word
        let apost_idx = line0.find('\'').unwrap();
        pos.byte = apost_idx;
        word_forward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, stop_idx));

        // Backwards from end-of-line traverses punctuation and words
        pos.byte = buf.line_byte_len(0);
        word_backward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, excl_idx));
        word_backward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, digits_idx));
        word_backward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, stop_idx));
        word_backward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, cant_idx));
        word_backward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, bar_idx));
        word_backward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, comma_idx));
        word_backward(&buf, &mut pos);
        assert_eq!((pos.line, pos.byte), (0, 0));
    }

    #[test]
    fn word_motion_cross_line_edges() {
        // Ensures word_forward at end of line moves to first word of next non-empty line and backward wraps.
        let buf = Buffer::from_str("t", "alpha\n\n βeta γamma\n    \n😀 emoji\n").unwrap();
        let mut pos = Position::new(0, 0); // at 'alpha'

        // Forward jumps over blank lines and leading whitespace
        word_forward(&buf, &mut pos);
        assert_eq!(pos.line, 2);
        let beta_idx = buf.line(2).unwrap().find("βeta").unwrap();
        assert_eq!(pos.byte, beta_idx);

        // Next word on same line is γamma
        word_forward(&buf, &mut pos);
        let gamma_idx = buf.line(2).unwrap().find("γamma").unwrap();
        assert_eq!(pos.byte, gamma_idx);

        // Forward again should reach the emoji punctuation token on final line
        word_forward(&buf, &mut pos);
        assert_eq!(pos.line, 4);
        let emoji_idx = buf.line(4).unwrap().find("😀").unwrap();
        assert_eq!(pos.byte, emoji_idx);

        // Backwards from emoji returns to γamma despite intervening blank line
        word_backward(&buf, &mut pos);
        assert_eq!(pos.line, 2);
        assert_eq!(pos.byte, gamma_idx);
    }
}
