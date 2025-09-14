//! Status line formatter (Refactor R1 Step 1).
//!
//! Breadth-first: intentionally minimal. Later enhancements (file name, modified flag,
//! diagnostics, LSP state) extend this API without changing call sites in the main loop.

use core_state::Mode;

/// Simple DTO describing what we need to render a status line.
pub struct StatusContext<'a> {
    pub mode: Mode,
    pub line: usize, // 0-based current line index
    pub col: usize,  // 0-based visual column
    pub command_active: bool,
    pub command_buffer: &'a str,
    /// Optional file name to display (Phase 2 Step 1). Only base file name shown for brevity.
    pub file_name: Option<&'a std::path::Path>,
    /// Dirty flag – when true, an asterisk is appended to the file name.
    pub dirty: bool,
}

/// Build status line string. Format (Phase 1):
/// [MODE] Ln <1-based>, Col <1-based> :<cmd-buffer-without-leading-colon>
/// When command is active we store the internal buffer with a leading ':' sentinel
/// but display only a single colon in the status line for visual cleanliness.
pub fn build_status(ctx: &StatusContext) -> String {
    let mode_str = match ctx.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
    };
    let file_segment = if let Some(p) = ctx.file_name {
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            if ctx.dirty {
                format!(" {}*", name)
            } else {
                format!(" {}", name)
            }
        } else if ctx.dirty {
            " *".to_string()
        } else {
            String::new()
        }
    } else if ctx.dirty {
        " [unnamed]*".to_string()
    } else {
        " [unnamed]".to_string()
    };
    if ctx.command_active {
        // Strip leading ':' if present so we only render one.
        let display = ctx
            .command_buffer
            .strip_prefix(':')
            .unwrap_or(ctx.command_buffer);
        format!(
            "[{}]{} Ln {}, Col {} :{}",
            mode_str,
            file_segment,
            ctx.line + 1,
            ctx.col + 1,
            display
        )
    } else {
        format!(
            "[{}]{} Ln {}, Col {} :",
            mode_str,
            file_segment,
            ctx.line + 1,
            ctx.col + 1
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builds_status_normal_no_cmd() {
        let s = build_status(&StatusContext {
            mode: Mode::Normal,
            line: 0,
            col: 4,
            command_active: false,
            command_buffer: "",
            file_name: None,
            dirty: false,
        });
        assert_eq!(s, "[NORMAL] [unnamed] Ln 1, Col 5 :");
    }

    #[test]
    fn builds_status_insert_with_cmd_single_colon() {
        let s = build_status(&StatusContext {
            mode: Mode::Insert,
            line: 2,
            col: 10,
            command_active: true,
            command_buffer: ":wq",
            file_name: Some(std::path::Path::new("file.rs")),
            dirty: true,
        });
        assert_eq!(s, "[INSERT] file.rs* Ln 3, Col 11 :wq");
    }
}
