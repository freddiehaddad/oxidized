//! File IO helpers extracted from dispatcher (Refactor R2 Step 5).
//!
//! Breadth-first: synchronous, minimal, no async abstractions yet. These helpers isolate
//! normalization + reconstruction logic so the dispatcher focuses on command semantics.
//! Future (Phase 3+) replacements can provide async versions with identical signatures.

use core_state::{EditorState, LineEnding, normalize_line_endings};
use core_text::Buffer;

/// Result of attempting to open a file.
#[derive(Debug)]
pub enum OpenFileResult {
    Success(OpenSuccess),
    Error, // caller logs / sets ephemeral already
}

pub struct OpenSuccess {
    pub buffer: Buffer,
    pub file_name: std::path::PathBuf,
    pub original_line_ending: LineEnding,
    pub had_trailing_newline: bool,
    pub mixed_line_endings: bool,
}

impl std::fmt::Debug for OpenSuccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenSuccess")
            .field("file_name", &self.file_name)
            .field("original_line_ending", &self.original_line_ending)
            .field("had_trailing_newline", &self.had_trailing_newline)
            .field("mixed_line_endings", &self.mixed_line_endings)
            .finish()
    }
}

/// Open a file path into a new Buffer applying line ending normalization.
/// Returns structured metadata required to update EditorState.
pub fn open_file(path: &std::path::Path) -> OpenFileResult {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let norm = normalize_line_endings(&content);
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
            match Buffer::from_str(name, &norm.normalized) {
                Ok(buffer) => OpenFileResult::Success(OpenSuccess {
                    buffer,
                    file_name: path.to_path_buf(),
                    original_line_ending: norm.original,
                    had_trailing_newline: norm.had_trailing_newline,
                    mixed_line_endings: norm.mixed,
                }),
                Err(e) => {
                    tracing::error!(target: "io", ?e, "buffer_create_failed");
                    OpenFileResult::Error
                }
            }
        }
        Err(e) => {
            tracing::error!(target: "io", ?e, "file_open_error");
            OpenFileResult::Error
        }
    }
}

/// Result of a write attempt.
#[derive(Debug)]
pub enum WriteFileResult {
    Success,
    NoFilename,
    Error,
}

/// Serialize the active buffer out to its associated file name (or provided target)
/// honoring original line ending style and trailing newline presence.
pub fn write_file(state: &mut EditorState, target: Option<&std::path::Path>) -> WriteFileResult {
    let path = if let Some(p) = target {
        p.to_path_buf()
    } else if let Some(existing) = state.file_name.clone() {
        existing
    } else {
        return WriteFileResult::NoFilename;
    };
    // Re-expand line endings based on original metadata
    let mut content = String::new();
    let line_ending = state.original_line_ending.as_str();
    let last_index = state.active_buffer().line_count();
    for i in 0..last_index {
        if let Some(mut l) = state.active_buffer().line(i) {
            let ends_nl = l.ends_with('\n');
            if ends_nl {
                l.pop();
            }
            content.push_str(&l);
            if (i + 1 < last_index) || (state.had_trailing_newline && i + 1 == last_index) {
                content.push_str(line_ending);
            }
        }
    }
    match std::fs::write(&path, content.as_bytes()) {
        Ok(_) => {
            state.dirty = false; // mark clean after successful write
            WriteFileResult::Success
        }
        Err(e) => {
            tracing::error!(target: "io", ?e, "file_write_error");
            WriteFileResult::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_text::Buffer;

    #[test]
    fn open_file_normalizes_and_sets_metadata() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.txt");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            // Mixed line endings CRLF + LF + final CRLF
            write!(f, "line1\r\nline2\nline3\r\n").unwrap();
        }
        match open_file(&path) {
            OpenFileResult::Success(s) => {
                assert!(s.buffer.line(0).unwrap().starts_with("line1"));
                assert!(s.mixed_line_endings, "should detect mixed endings");
                assert!(s.had_trailing_newline, "should detect trailing newline");
            }
            _ => panic!("expected success"),
        }
    }

    #[test]
    fn write_file_preserves_original_style() {
        // Build state manually after open to focus on write serialization
        let buffer = Buffer::from_str("t", "a\nb\n").unwrap();
        let mut state = EditorState::new(buffer);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.txt");
        state.file_name = Some(path.clone());
        state.original_line_ending = LineEnding::Crlf;
        state.had_trailing_newline = true;
        state.dirty = true;
        let res = write_file(&mut state, None);
        assert!(matches!(res, WriteFileResult::Success));
        assert!(!state.dirty, "dirty cleared after write");
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("a\r\nb\r\n"));
    }

    #[test]
    fn write_file_no_filename() {
        let buffer = Buffer::from_str("t", "x").unwrap();
        let mut state = EditorState::new(buffer);
        state.dirty = true;
        let res = write_file(&mut state, None);
        assert!(matches!(res, WriteFileResult::NoFilename));
        assert!(state.dirty, "dirty unchanged when no filename");
    }
}
