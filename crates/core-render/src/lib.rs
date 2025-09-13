//! Rendering primitives: Cell, Frame, and a naive full-screen renderer.

use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    queue,
    style::Print,
    terminal::{Clear, ClearType},
};
use std::io::{Write, stdout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
}

impl Default for Cell {
    fn default() -> Self {
        Self { ch: ' ' }
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
}

pub struct Renderer;

impl Renderer {
    pub fn render(frame: &Frame) -> Result<()> {
        let mut out = stdout();
        queue!(out, Clear(ClearType::All))?;
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
            queue!(out, Print(cell.ch))?;
        }
        out.flush()?;
        Ok(())
    }
}

pub mod status;
