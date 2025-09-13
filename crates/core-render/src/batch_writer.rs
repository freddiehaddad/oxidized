//! BatchWriter (Refactor R3 Step 7)
//!
//! Groups consecutive plain single-width cell prints into a single terminal
//! `Print` command to establish a baseline command/cell ratio metric before
//! introducing more advanced scroll‑region or diff shrinking optimizations.
//!
//! Batching Rules (intentionally conservative breadth‑first approach):
//! * A "plain cell" is a single visible character with no ANSI escape
//!   sequences (we treat any string containing `\x1b` as styled and thus
//!   a hard batch boundary).
//! * Only 1‑char plain strings are aggregated. Multi‑char plain strings
//!   (e.g. status line) are passed through as their own command so later
//!   phases can reason about higher‑level semantic segments if desired.
//! * Any movement (`MoveTo`) or `ClearLine` flushes the current batch.
//! * Styled prints (reverse video sequences used for cursor) flush the
//!   current batch first, then are emitted directly.
//!
//! Metrics Semantics:
//! * `print_commands` – number of terminal `Print` commands issued after
//!   batching (the lower the better for throughput).
//! * `cells_printed` – logical cells written (count of plain chars batched
//!   plus one per styled / multi‑char command). This guarantees
//!   `print_commands <= cells_printed` making simple assertions possible.
//!   (Future refinement may count visible width of multi‑char commands.)
//!
//! Design Tenets Applied:
//! * Breadth‑first: minimal safe batching without premature complexity.
//! * Modularity: isolated; render paths depend only on the public API.
//! * Unicode correctness: batching only applies after grapheme shaping;
//!   we only aggregate already separated single‑width cells.
//!
use crate::writer::Command;
use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    queue,
    style::Print,
    terminal::{Clear, ClearType},
};
use std::io::{Write, stdout};

#[derive(Default)]
pub struct BatchWriter {
    cmds: Vec<Command>,
    pending_plain: String,
    pub print_commands: u64,
    pub cells_printed: u64,
}

impl BatchWriter {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn flush_pending(&mut self) {
        if self.pending_plain.is_empty() {
            return;
        }
        let s = std::mem::take(&mut self.pending_plain);
        self.cmds.push(Command::Print(s));
        self.print_commands += 1;
        // cells already counted during accumulation
    }

    pub fn move_to(&mut self, x: u16, y: u16) {
        self.flush_pending();
        self.cmds.push(Command::MoveTo(x, y));
    }

    pub fn clear_line(&mut self, x: u16, y: u16) {
        self.flush_pending();
        self.cmds.push(Command::ClearLine(x, y));
    }

    pub fn print<S: Into<String>>(&mut self, s: S) {
        let s: String = s.into();
        if s.is_empty() {
            return;
        }
        let is_plain_single = s.len() == 1 && !s.contains('\x1b');
        if is_plain_single {
            self.pending_plain.push_str(&s);
            self.cells_printed += 1;
            return;
        }
        // Styled or multi-char: flush batch then emit directly, counting 1 logical cell.
        self.flush_pending();
        self.cmds.push(Command::Print(s));
        self.print_commands += 1;
        self.cells_printed += 1; // treat as one logical cell for baseline
    }

    pub fn flush(mut self) -> Result<(u64, u64)> {
        self.flush_pending();
        let mut out = stdout();
        for c in self.cmds {
            match c {
                Command::MoveTo(x, y) => {
                    queue!(out, MoveTo(x, y))?;
                }
                Command::ClearLine(_, _) => {
                    queue!(out, Clear(ClearType::CurrentLine))?;
                }
                Command::Print(s) => {
                    queue!(out, Print(s))?;
                }
            }
        }
        out.flush()?;
        Ok((self.print_commands, self.cells_printed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batches_consecutive_plain_chars() {
        let mut w = BatchWriter::new();
        w.move_to(0, 0);
        w.print("a");
        w.print("b");
        w.print("c");
        // styled boundary flushes
        w.print("\x1b[7mx\x1b[0m");
        let (print_cmds, cells) = w.flush().unwrap();
        // Expect 2 print commands: one batched abc, one styled x
        assert_eq!(print_cmds, 2);
        assert!(cells >= print_cmds);
    }
}
