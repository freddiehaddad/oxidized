//! Status line formatter (Refactor R1 Step 1).
//!
//! Breadth-first: intentionally minimal. Later enhancements (file name, modified flag,
//! diagnostics, LSP state) extend this API without changing call sites in the main loop.

use core_state::Mode;

/// Simple DTO describing what we need to render a status line.
pub struct StatusContext<'a> {
    pub mode: Mode,
    pub line: usize,   // 0-based current line index
    pub col: usize,    // 0-based visual column
    pub command_active: bool,
    pub command_buffer: &'a str,
}

/// Build status line string. Format (Phase 1):
/// [MODE] Ln <1-based>, Col <1-based> :<cmd-buffer>
/// The trailing colon section appears only if command buffer active.
pub fn build_status(ctx: &StatusContext) -> String {
    let mode_str = match ctx.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
    };
    if ctx.command_active {
        format!(
            "[{}] Ln {}, Col {} :{}",
            mode_str,
            ctx.line + 1,
            ctx.col + 1,
            ctx.command_buffer
        )
    } else {
        format!("[{}] Ln {}, Col {} :", mode_str, ctx.line + 1, ctx.col + 1)
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
        });
        assert_eq!(s, "[NORMAL] Ln 1, Col 5 :");
    }

    #[test]
    fn builds_status_insert_with_cmd() {
        let s = build_status(&StatusContext {
            mode: Mode::Insert,
            line: 2,
            col: 10,
            command_active: true,
            command_buffer: ":wq",
        });
        assert_eq!(s, "[INSERT] Ln 3, Col 11 ::wq");
    }
}
