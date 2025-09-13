//! Terminal backend abstraction and crossterm implementation.
//!
//! Refactor R3: Introduced `TerminalCapabilities` stub (scroll region support flag)
//! consumed by the render engine to gate forthcoming scroll-delta optimizations.

use anyhow::Result;
use crossterm::{
    cursor::Hide,
    cursor::Show,
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, SetTitle, disable_raw_mode, enable_raw_mode,
    },
};
use std::io::stdout;

pub mod capabilities;
pub use capabilities::TerminalCapabilities;

pub trait TerminalBackend {
    fn enter(&mut self) -> Result<()>;
    fn leave(&mut self) -> Result<()>;
    fn set_title(&mut self, title: &str) -> Result<()>;
}

pub struct CrosstermBackend {
    entered: bool,
}

/// RAII guard ensuring terminal state restoration even if caller early-returns or panics.
pub struct TerminalGuard<'a> {
    backend: &'a mut CrosstermBackend,
    active: bool,
}

impl Default for CrosstermBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CrosstermBackend {
    pub fn new() -> Self {
        Self { entered: false }
    }

    /// Enter and return a guard that will leave on drop.
    pub fn enter_guard(&mut self) -> Result<TerminalGuard<'_>> {
        self.enter()?;
        Ok(TerminalGuard {
            backend: self,
            active: true,
        })
    }
}

impl TerminalBackend for CrosstermBackend {
    fn enter(&mut self) -> Result<()> {
        if !self.entered {
            enable_raw_mode()?;
            execute!(stdout(), EnterAlternateScreen, Hide)?;
            self.entered = true;
        }
        Ok(())
    }

    fn leave(&mut self) -> Result<()> {
        if self.entered {
            execute!(stdout(), LeaveAlternateScreen, Show)?;
            disable_raw_mode()?;
            self.entered = false;
        }
        Ok(())
    }

    fn set_title(&mut self, title: &str) -> Result<()> {
        execute!(stdout(), SetTitle(title))?;
        Ok(())
    }
}

impl Drop for CrosstermBackend {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

impl<'a> Drop for TerminalGuard<'a> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.backend.leave();
        }
    }
}
