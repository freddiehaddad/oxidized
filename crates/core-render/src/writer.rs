//! Terminal writer abstraction (Phase 3 Step 6).
//!
//! Breadth-first goal: introduce a thin layer that can batch primitive
//! terminal operations so later partial rendering can selectively emit
//! line updates without reconstructing / walking the entire `Frame`.
//!
//! Current scope (no behavior change): we still build a full `Frame`,
//! then translate it into writer commands and flush once. Future steps
//! will bypass full frame construction for partial paths.
//!
//! Design invariants:
//! * Commands preserve ordering; no flushing mid-frame.
//! * All positions are absolute (0,0) origin; caller ensures bounds.
//! * Styling minimal (reverse handled inline same as existing Renderer).
//! * Writer owns no global state; it is a short-lived object per frame.
//! * No async yet (will wrap stdout in async adapter in a later phase).
//!
//! Public API kept intentionally tiny until partial path activates.

use anyhow::Result;
use crossterm::{cursor::MoveTo, queue, style::Print};
use std::io::{Write, stdout};

#[derive(Debug)]
pub enum Command {
    MoveTo(u16, u16),
    ClearLine(u16, u16), // (x,y) start; clears full line logically (we just rewrite content)
    Print(String),
}

#[derive(Default)]
pub struct Writer {
    cmds: Vec<Command>,
}

impl Writer {
    pub fn new() -> Self {
        Self { cmds: Vec::new() }
    }
    pub fn move_to(&mut self, x: u16, y: u16) {
        self.cmds.push(Command::MoveTo(x, y));
    }
    pub fn clear_line(&mut self, x: u16, y: u16) {
        self.cmds.push(Command::ClearLine(x, y));
    }
    pub fn print<S: Into<String>>(&mut self, s: S) {
        let s: String = s.into();
        if !s.is_empty() {
            self.cmds.push(Command::Print(s));
        }
    }
    pub fn flush(self) -> Result<()> {
        let mut out = stdout();
        for c in self.cmds {
            match c {
                Command::MoveTo(x, y) => {
                    queue!(out, MoveTo(x, y))?;
                }
                Command::ClearLine(_, _) => {
                    // For now we rely on rewriting full content; explicit clear deferred.
                    // Could emit ClearLine crossterm command later when partial path active.
                }
                Command::Print(s) => {
                    queue!(out, Print(s))?;
                }
            }
        }
        out.flush()?;
        Ok(())
    }
}
