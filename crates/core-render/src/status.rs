//! Status line composition (Refactor R2 Step 4).
//!
//! Original (Phase 2) format:
//! `[MODE] <name>[*] Ln X, Col Y :` (command inactive) or
//! `[MODE] <name>[*] Ln X, Col Y :<command>` (command active).
//! * `<name>` is base file name or `[No Name]` for an unsaved buffer.
//! * `*` appears only when the buffer is dirty.
//! * A single colon precedes the command buffer; the internal stored buffer may begin with a
//!   sentinel ':' which we strip for display.
//!
//! Refactor R2 introduces a two‑stage pipeline:
//! 1. `compose_status` produces an ordered vector of `StatusSegment` items.
//! 2. `format_status` renders those segments into the legacy string (identical output).
//!
//! This preserves existing behavior (breadth‑first, zero user visible change) while setting up
//! future evolution (injecting VCS / diagnostics or truncation logic by manipulating segments).
//! All prior direct string construction logic was replaced; tests verify exact equivalence to a
//! "legacy" formatting function embedded in the test module.

use core_state::{Mode, SelectionKind};

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

/// Discrete status line segments (order-sensitive). Refactor R4 Step 6 expands the model to include
/// placeholders for future visual mode, register, and overlay indicators while keeping legacy
/// rendering behavior identical (tests assert string parity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusSegment<'a> {
    /// Current editor mode label ("NORMAL", "INSERT", etc.).
    Mode(&'static str),
    /// File name portion including leading space and optional trailing dirty marker `*` to preserve
    /// legacy formatting without additional glue logic.
    FileNameCow(std::borrow::Cow<'a, str>),
    /// 1-based cursor line & column for display.
    Position { line_1: usize, col_1: usize },
    /// Indicates command line inactive; legacy formatting keeps a trailing colon already emitted by Position.
    CommandInactive,
    /// Active command buffer content (without the leading ':' sentinel stored internally).
    CommandActive(&'a str),
    /// Placeholder for an active selection kind (visual mode) – not yet populated.
    Selection(Option<SelectionKind>),
    /// Placeholder for an explicit register hint (e.g., pending yank to a named register) – unused.
    RegisterHint(Option<char>),
    /// Placeholder flag indicating overlay (metrics) active – unused until Step 13.
    OverlayActive(bool),
    /// Generic placeholder string for future ad-hoc extensions (e.g., diagnostics count).
    Placeholder(&'static str),
}

/// Produce ordered segments representing the status line.
pub fn compose_status<'a>(ctx: &'a StatusContext<'a>) -> Vec<StatusSegment<'a>> {
    let mode_str = match ctx.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::VisualChar => "VISUAL",
    };
    let file_segment: std::borrow::Cow<'_, str> = if let Some(p) = ctx.file_name {
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            if ctx.dirty {
                format!(" {}*", name).into()
            } else {
                format!(" {}", name).into()
            }
        } else if ctx.dirty {
            " *".into()
        } else {
            "".into()
        }
    } else if ctx.dirty {
        " [No Name]*".into()
    } else {
        " [No Name]".into()
    };

    // Capacity now accounts for future placeholders though only legacy-impacting first five are rendered.
    let mut out = Vec::with_capacity(8);
    out.push(StatusSegment::Mode(mode_str));
    out.push(StatusSegment::FileNameCow(file_segment));
    out.push(StatusSegment::Position {
        line_1: ctx.line + 1,
        col_1: ctx.col + 1,
    });
    if ctx.command_active {
        let display = ctx
            .command_buffer
            .strip_prefix(':')
            .unwrap_or(ctx.command_buffer);
        out.push(StatusSegment::CommandActive(display));
    } else {
        out.push(StatusSegment::CommandInactive);
    }
    // Append scaffold placeholders with default inert values so tests can introspect presence.
    out.push(StatusSegment::Selection(None));
    out.push(StatusSegment::RegisterHint(None));
    out.push(StatusSegment::OverlayActive(false));
    out
}

/// Render ordered status segments into the final legacy string (exact match guaranteed by tests).
pub fn format_status(segments: &[StatusSegment<'_>]) -> String {
    // We know the approximate shape: [MODE]<file> Ln X, Col Y :<optional_cmd>
    // Pre-compute a conservative capacity to avoid many reallocations.
    let mut s = String::with_capacity(48);
    for seg in segments {
        match seg {
            StatusSegment::Mode(m) => {
                s.push('[');
                s.push_str(m);
                s.push(']');
            }
            StatusSegment::FileNameCow(name) => s.push_str(name),
            StatusSegment::Position { line_1, col_1 } => {
                use std::fmt::Write as _;
                let _ = write!(s, " Ln {}, Col {} :", line_1, col_1);
            }
            StatusSegment::CommandInactive => { /* colon already appended by Position */ }
            StatusSegment::CommandActive(cmd) => {
                // Position added trailing colon; append command directly.
                s.push_str(cmd);
            }
            // Placeholders intentionally not rendered in legacy string yet.
            StatusSegment::Selection(_) => {}
            StatusSegment::RegisterHint(_) => {}
            StatusSegment::OverlayActive(_) => {}
            StatusSegment::Placeholder(p) => s.push_str(p),
        }
    }
    s
}

/// Convenience wrapper (maintains external API stability if previously `build_status` was used).
pub fn build_status(ctx: &StatusContext) -> String {
    format_status(&compose_status(ctx))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builds_status_normal_no_cmd() {
        let ctx = StatusContext {
            mode: Mode::Normal,
            line: 0,
            col: 4,
            command_active: false,
            command_buffer: "",
            file_name: None,
            dirty: false,
        };
        let s = format_status(&compose_status(&ctx));
        assert_eq!(s, "[NORMAL] [No Name] Ln 1, Col 5 :");
    }

    #[test]
    fn builds_status_insert_with_cmd_single_colon() {
        let ctx = StatusContext {
            mode: Mode::Insert,
            line: 2,
            col: 10,
            command_active: true,
            command_buffer: ":wq",
            file_name: Some(std::path::Path::new("file.rs")),
            dirty: true,
        };
        let s = format_status(&compose_status(&ctx));
        assert_eq!(s, "[INSERT] file.rs* Ln 3, Col 11 :wq");
    }

    #[test]
    fn builds_status_named_clean() {
        let ctx = StatusContext {
            mode: Mode::Normal,
            line: 4,
            col: 0,
            command_active: false,
            command_buffer: "",
            file_name: Some(std::path::Path::new("main.rs")),
            dirty: false,
        };
        let s = format_status(&compose_status(&ctx));
        assert_eq!(s, "[NORMAL] main.rs Ln 5, Col 1 :");
    }

    #[test]
    fn builds_status_no_name_dirty() {
        let ctx = StatusContext {
            mode: Mode::Insert,
            line: 0,
            col: 0,
            command_active: false,
            command_buffer: "",
            file_name: None,
            dirty: true,
        };
        let s = format_status(&compose_status(&ctx));
        assert_eq!(s, "[INSERT] [No Name]* Ln 1, Col 1 :");
    }

    #[test]
    fn builds_status_no_name_clean_insert_mode_with_cmd() {
        let ctx = StatusContext {
            mode: Mode::Insert,
            line: 1,
            col: 2,
            command_active: true,
            command_buffer: ":e test.txt",
            file_name: None,
            dirty: false,
        };
        let s = format_status(&compose_status(&ctx));
        assert_eq!(s, "[INSERT] [No Name] Ln 2, Col 3 :e test.txt");
    }

    // Regression: compare segmented output with legacy formatting logic reproduction
    fn legacy_format(ctx: &StatusContext) -> String {
        let mode_str = match ctx.mode {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::VisualChar => "VISUAL",
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
                "".to_string()
            }
        } else if ctx.dirty {
            " [No Name]*".to_string()
        } else {
            " [No Name]".to_string()
        };
        if ctx.command_active {
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

    #[test]
    fn segmentation_regression_all_cases() {
        let cases = vec![
            StatusContext {
                mode: Mode::Normal,
                line: 0,
                col: 0,
                command_active: false,
                command_buffer: "",
                file_name: None,
                dirty: false,
            },
            StatusContext {
                mode: Mode::Insert,
                line: 10,
                col: 5,
                command_active: false,
                command_buffer: "",
                file_name: None,
                dirty: true,
            },
            StatusContext {
                mode: Mode::Insert,
                line: 2,
                col: 7,
                command_active: true,
                command_buffer: ":x",
                file_name: Some(std::path::Path::new("lib.rs")),
                dirty: false,
            },
            StatusContext {
                mode: Mode::Normal,
                line: 4,
                col: 9,
                command_active: true,
                command_buffer: ":write",
                file_name: Some(std::path::Path::new("main.rs")),
                dirty: true,
            },
        ];
        for ctx in cases {
            let seg = compose_status(&ctx);
            // Ensure new scaffold segments appended with default values.
            assert!(matches!(
                seg.last(),
                Some(StatusSegment::OverlayActive(false))
            ));
            // Find selection placeholder presence.
            assert!(
                seg.iter()
                    .any(|s| matches!(s, StatusSegment::Selection(None)))
            );
            let formatted = format_status(&seg);
            let legacy = legacy_format(&ctx);
            assert_eq!(formatted, legacy, "segmented output mismatch");
        }
    }
}
