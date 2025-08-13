use crate::core::mode::Position;
use crossterm::{
    ExecutableCommand, QueueableCommand,
    cursor::{self, SetCursorStyle},
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
    tty::IsTty,
};
use log::{debug, trace, warn};
use std::io::{self, Stdout, Write};

pub struct Terminal {
    stdout: Stdout,
    size: (u16, u16), // (width, height)
    is_tty: bool,
}

impl Terminal {
    #[inline]
    fn is_headless(&self) -> bool {
        !self.is_tty
    }

    pub fn new() -> io::Result<Self> {
        trace!("Initializing terminal with alternate screen and raw mode");
        let mut stdout = io::stdout();
        let is_tty = stdout.is_tty();

        if is_tty {
            // Enter alternate screen before enabling raw mode
            stdout.execute(EnterAlternateScreen)?;
            trace!("Entered alternate screen mode");

            terminal::enable_raw_mode()?;
            trace!("Enabled raw terminal mode");

            stdout.execute(terminal::Clear(ClearType::All))?;
            stdout.execute(cursor::Hide)?;
            trace!("Cleared screen and hid cursor");

            // Flush stdout and give terminal time to settle
            stdout.flush()?;

            let size = terminal::size()?;
            debug!("Terminal initialized with size: {}x{}", size.0, size.1);
            Ok(Self {
                stdout,
                size,
                is_tty,
            })
        } else {
            // Headless/CI environment: skip TTY-dependent setup
            warn!("Stdout is not a TTY; running terminal in headless mode for CI/tests");
            let size = (80, 24); // sensible default
            Ok(Self {
                stdout,
                size,
                is_tty,
            })
        }
    }

    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    pub fn update_size(&mut self) -> io::Result<()> {
        // In headless/CI (non-TTY), keep the fixed default size to avoid OS errors
        if self.is_tty {
            let old_size = self.size;
            self.size = terminal::size()?;
            if old_size != self.size {
                debug!(
                    "Terminal size updated from {}x{} to {}x{}",
                    old_size.0, old_size.1, self.size.0, self.size.1
                );
            }
        }
        Ok(())
    }

    pub fn clear_screen(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.execute(Clear(ClearType::All))?;
        Ok(())
    }

    pub fn clear_line(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.execute(Clear(ClearType::CurrentLine))?;
        Ok(())
    }

    pub fn move_cursor(&mut self, pos: Position) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout
            .execute(cursor::MoveTo(pos.col as u16, pos.row as u16))?;
        Ok(())
    }

    pub fn hide_cursor(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.execute(cursor::Hide)?;
        Ok(())
    }

    pub fn show_cursor(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.execute(cursor::Show)?;
        Ok(())
    }

    /// Set cursor to block shape (normal mode)
    pub fn set_cursor_block(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        debug!("Setting cursor to block shape (normal mode)");
        self.stdout.execute(SetCursorStyle::SteadyBlock)?;
        Ok(())
    }

    /// Set cursor to vertical line shape (insert mode)
    pub fn set_cursor_line(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        debug!("Setting cursor to line shape (insert mode)");
        self.stdout.execute(SetCursorStyle::SteadyBar)?;
        Ok(())
    }

    /// Set cursor to underline shape (replace mode)
    pub fn set_cursor_underline(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        debug!("Setting cursor to underline shape (replace mode)");
        self.stdout.execute(SetCursorStyle::SteadyUnderScore)?;
        Ok(())
    }

    /// Restore cursor to default system shape
    pub fn restore_cursor_shape(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        debug!("Restoring cursor to default system shape");
        self.stdout.execute(SetCursorStyle::DefaultUserShape)?;
        Ok(())
    }

    pub fn set_foreground_color(&mut self, color: Color) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.execute(SetForegroundColor(color))?;
        Ok(())
    }

    pub fn set_background_color(&mut self, color: Color) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.execute(SetBackgroundColor(color))?;
        Ok(())
    }

    pub fn reset_color(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.execute(ResetColor)?;
        Ok(())
    }

    pub fn print(&mut self, text: &str) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.execute(Print(text))?;
        Ok(())
    }

    pub fn print_at(&mut self, pos: Position, text: &str) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.move_cursor(pos)?;
        self.print(text)?;
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.flush()
    }

    pub fn queue_print(&mut self, text: &str) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(Print(text))?;
        Ok(())
    }

    pub fn queue_move_cursor(&mut self, pos: Position) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout
            .queue(cursor::MoveTo(pos.col as u16, pos.row as u16))?;
        Ok(())
    }

    pub fn queue_set_fg_color(&mut self, color: Color) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(SetForegroundColor(color))?;
        Ok(())
    }

    pub fn queue_set_bg_color(&mut self, color: Color) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(SetBackgroundColor(color))?;
        Ok(())
    }

    pub fn queue_reset_color(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(ResetColor)?;
        Ok(())
    }

    pub fn queue_clear_line(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(Clear(ClearType::CurrentLine))?;
        Ok(())
    }

    pub fn queue_clear_screen(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(Clear(ClearType::All))?;
        Ok(())
    }

    pub fn queue_hide_cursor(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(cursor::Hide)?;
        Ok(())
    }

    pub fn queue_show_cursor(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(cursor::Show)?;
        Ok(())
    }

    /// Queue cursor to block shape (normal mode)
    pub fn queue_cursor_block(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(SetCursorStyle::SteadyBlock)?;
        Ok(())
    }

    /// Queue cursor to vertical line shape (insert mode)
    pub fn queue_cursor_line(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(SetCursorStyle::SteadyBar)?;
        Ok(())
    }

    /// Queue cursor to underline shape (replace mode)
    pub fn queue_cursor_underline(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        self.stdout.queue(SetCursorStyle::SteadyUnderScore)?;
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        debug!("Cleaning up terminal: restoring cursor and colors");
        if !self.is_headless() {
            // Restore cursor visibility, shape, and colors
            let _ = self.stdout.execute(cursor::Show);
            let _ = self.stdout.execute(SetCursorStyle::DefaultUserShape);
            let _ = self.stdout.execute(ResetColor);

            debug!("Disabling raw terminal mode");
            // Disable raw mode before leaving alternate screen
            let _ = terminal::disable_raw_mode();

            debug!("Leaving alternate screen mode");
            // Leave alternate screen to restore original terminal content
            let _ = self.stdout.execute(LeaveAlternateScreen);
            debug!("Terminal cleanup completed");
        }
    }
}
