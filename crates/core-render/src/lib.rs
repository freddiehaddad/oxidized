//! Rendering primitives + frame assembly foundations.
//!
//! This crate currently exposes:
//! * Low-level `Cell` and `Frame` structures used to build a full terminal frame.
//! * A naive full-frame `Renderer` (breadth-first: always paints the entire surface).
//! * `render_engine` module (Refactor R2 Step 1) which constructs frames from editor state.
//! * `scheduler` (Refactor R2 Step 3) collecting fine‑grained semantic invalidation deltas
//!   (`RenderDelta`) while still always producing a full effective redraw.
//! * `status` segment model (Refactor R2 Step 4) enabling future dynamic status components.
//! * `timing` (Refactor R2 Step 11) minimal atomic storing the last full render duration
//!   in nanoseconds for early performance telemetry.
//!
//! Design Notes (Refactor R2):
//! - Partial rendering is intentionally deferred; semantic deltas + metrics allow us to
//!   validate optimization headroom before introducing diff complexity.
//! - The timing hook lives outside the engine for now to keep `RenderEngine` pure; it may
//!   migrate inward when incremental strategies require per-phase timings.
//! - The entire frame API favors simple `Vec<Cell>` storage; future phases may introduce
//!   line arenas or gap buffers for selective diff emission.

use anyhow::Result;
use bitflags::bitflags;
use crossterm::{
    cursor::MoveTo,
    queue,
    style::Print,
    terminal::{Clear, ClearType},
};
use std::io::{Write, stdout};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CellFlags: u8 {
        const REVERSE = 0b0000_0001; // reverse-video (software cursor)
        const CURSOR  = 0b0000_0010; // marks cell part of cursor span
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub flags: CellFlags,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            flags: CellFlags::empty(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<Cell>,
}

impl Frame {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); (width as usize) * (height as usize)],
        }
    }

    pub fn set(&mut self, x: u16, y: u16, ch: char) {
        if x < self.width && y < self.height {
            let idx = y as usize * self.width as usize + x as usize;
            self.cells[idx].ch = ch;
        }
    }

    pub fn set_with_flags(&mut self, x: u16, y: u16, ch: char, flags: CellFlags) {
        if x < self.width && y < self.height {
            let idx = y as usize * self.width as usize + x as usize;
            self.cells[idx].ch = ch;
            self.cells[idx].flags = flags;
        }
    }
}

pub struct Renderer;

impl Renderer {
    pub fn render(frame: &Frame) -> Result<()> {
        let mut out = stdout();
        queue!(out, Clear(ClearType::All))?;
        queue!(out, MoveTo(0, 0))?;
        let mut x = 0u16;
        let mut y = 0u16;
        for (i, cell) in frame.cells.iter().enumerate() {
            let expected_y = i as u16 / frame.width;
            let expected_x = i as u16 % frame.width;
            if expected_x != x || expected_y != y {
                queue!(out, MoveTo(expected_x, expected_y))?;
                x = expected_x;
                y = expected_y;
            }
            // For now, we only visually differentiate REVERSE (cursor span) by wrapping with simple ANSI invert if flag set.
            if cell.flags.contains(CellFlags::REVERSE) {
                queue!(out, Print(format!("\x1b[7m{}\x1b[0m", cell.ch)))?;
            } else {
                queue!(out, Print(cell.ch))?;
            }
        }
        out.flush()?;
        Ok(())
    }
}

pub mod render_engine;
pub mod scheduler;
pub mod status;
pub mod timing;
pub mod viewport; // (placeholder for future viewport helpers)
