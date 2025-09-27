//! Structured command line parsing (Refactor R3 / Step 2).
//!
//! Converts the raw command buffer (always beginning with ':') into a
//! `ParsedCommand` enum. This replaces ad-hoc string prefix checks inside
//! the dispatcher and prepares for adding new commands (`:metrics`,
//! `:config-reload`, future buffer/window commands) without inflating
//! branching logic.
//!
//! Breadth-first constraints:
//! * Parsing is synchronous & allocation-light (only clones tail for
//!   unknown command variants or `Edit` path argument).
//! * Errors are represented as `ParsedCommand::Unknown(String)` which
//!   higher layers convert into ephemeral status messages.
//! * No side-effects here; pure classification.
//!
//! Future roadmap:
//! * Argument tokenization (quoted paths, flags).
//! * Validation errors separated from unknown commands.
//! * Async commands (e.g. LSP-driven) will use a follow-up event once
//!   implemented—parser remains pure.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Quit { force: bool },
    Write { force: bool, path: Option<PathBuf> },
    Edit { force: bool, path: Option<PathBuf> },
    Metrics, // placeholder for Step 11
    Unknown(String),
}

pub struct CommandParser;

impl CommandParser {
    pub fn parse(raw: &str) -> ParsedCommand {
        let s = raw.trim();
        if !s.starts_with(':') {
            return ParsedCommand::Unknown(s.to_string());
        }
        // Strip leading ':' for matching
        let body = &s[1..];
        if body.is_empty() {
            return ParsedCommand::Unknown(String::new());
        }
        let (head, tail) = split_head(body);
        match head {
            "q" => ParsedCommand::Quit { force: false },
            "q!" => ParsedCommand::Quit { force: true },
            "w" => ParsedCommand::Write {
                force: false,
                path: parse_path(tail),
            },
            "w!" => ParsedCommand::Write {
                force: true,
                path: parse_path(tail),
            },
            "e" => ParsedCommand::Edit {
                force: false,
                path: parse_path(tail),
            },
            "e!" => ParsedCommand::Edit {
                force: true,
                path: parse_path(tail),
            },
            "metrics" if tail.trim().is_empty() => ParsedCommand::Metrics,
            _ => ParsedCommand::Unknown(body.to_string()),
        }
    }
}

fn split_head(body: &str) -> (&str, &str) {
    let mut idx = 0usize;
    for (offset, ch) in body.char_indices() {
        if ch.is_whitespace() {
            break;
        }
        idx = offset + ch.len_utf8();
    }
    let (head, rest) = if idx == 0 || idx >= body.len() {
        (body, "")
    } else {
        body.split_at(idx)
    };
    (head, rest)
}

fn parse_path(rest: &str) -> Option<PathBuf> {
    let trimmed = rest.trim_start();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quit() {
        assert_eq!(
            CommandParser::parse(":q"),
            ParsedCommand::Quit { force: false }
        );
    }

    #[test]
    fn parse_quit_force() {
        assert_eq!(
            CommandParser::parse(":q!"),
            ParsedCommand::Quit { force: true }
        );
    }

    #[test]
    fn parse_write() {
        assert_eq!(
            CommandParser::parse(":w"),
            ParsedCommand::Write {
                force: false,
                path: None
            }
        );
    }

    #[test]
    fn parse_write_with_path_force() {
        assert_eq!(
            CommandParser::parse(":w!   foo.txt"),
            ParsedCommand::Write {
                force: true,
                path: Some(PathBuf::from("foo.txt"))
            }
        );
    }

    #[test]
    fn parse_edit() {
        assert_eq!(
            CommandParser::parse(":e  foo.txt"),
            ParsedCommand::Edit {
                force: false,
                path: Some(PathBuf::from("foo.txt"))
            }
        );
    }

    #[test]
    fn parse_metrics() {
        assert_eq!(CommandParser::parse(":metrics"), ParsedCommand::Metrics);
    }

    #[test]
    fn parse_edit_force_without_path() {
        assert_eq!(
            CommandParser::parse(":e!"),
            ParsedCommand::Edit {
                force: true,
                path: None
            }
        );
    }

    #[test]
    fn parse_unknown() {
        assert_eq!(
            CommandParser::parse(":doesnotexist"),
            ParsedCommand::Unknown("doesnotexist".into())
        );
    }
}
