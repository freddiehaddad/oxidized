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
    Quit,
    Write,
    Edit(PathBuf),
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
        // Fast path exact matches
        match body {
            "q" => return ParsedCommand::Quit,
            "w" => return ParsedCommand::Write,
            "metrics" => return ParsedCommand::Metrics,
            _ => {}
        }
        // Edit command: e <path>
        if let Some(rest) = body.strip_prefix('e') {
            // Accept forms: "e path", "e    path"
            let path_part = rest.trim_start();
            if !path_part.is_empty() {
                return ParsedCommand::Edit(PathBuf::from(path_part));
            }
        }
        ParsedCommand::Unknown(body.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quit() {
        assert_eq!(CommandParser::parse(":q"), ParsedCommand::Quit);
    }

    #[test]
    fn parse_write() {
        assert_eq!(CommandParser::parse(":w"), ParsedCommand::Write);
    }

    #[test]
    fn parse_edit() {
        match CommandParser::parse(":e  foo.txt") {
            ParsedCommand::Edit(p) => assert_eq!(p, PathBuf::from("foo.txt")),
            other => panic!("expected Edit, got {:?}", other),
        }
    }

    #[test]
    fn parse_metrics() {
        assert_eq!(CommandParser::parse(":metrics"), ParsedCommand::Metrics);
    }

    #[test]
    fn parse_unknown() {
        assert_eq!(
            CommandParser::parse(":doesnotexist"),
            ParsedCommand::Unknown("doesnotexist".into())
        );
    }
}
