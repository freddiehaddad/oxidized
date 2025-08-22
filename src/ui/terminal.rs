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

// -------- Shadow Frame Types --------
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    ch: char,
    fg: Option<Color>,
    bg: Option<Color>,
}
impl Cell {
    fn blank(bg: Option<Color>) -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg,
        }
    }
}

#[derive(Clone, Debug)]
struct FrameBuffer {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
    cursor_visible: bool,
    cursor_pos: (u16, u16),
    cursor_style: Option<SetCursorStyle>,
}
impl FrameBuffer {
    fn new(w: u16, h: u16, bg: Option<Color>) -> Self {
        Self {
            width: w,
            height: h,
            cells: vec![Cell::blank(bg); w as usize * h as usize],
            cursor_visible: true,
            cursor_pos: (0, 0),
            cursor_style: None,
        }
    }
    #[inline]
    fn idx(&self, r: u16, c: u16) -> usize {
        r as usize * self.width as usize + c as usize
    }
    fn set_char(&mut self, r: u16, c: u16, ch: char, fg: Option<Color>, bg: Option<Color>) {
        if r < self.height && c < self.width {
            let idx = r as usize * self.width as usize + c as usize;
            self.cells[idx] = Cell { ch, fg, bg };
        }
    }
    fn fill_row_from(&mut self, r: u16, start: u16, fg: Option<Color>, bg: Option<Color>) {
        if r >= self.height || start >= self.width {
            return;
        }
        let s = self.idx(r, start);
        let e = self.idx(r, self.width - 1) + 1;
        for cell in &mut self.cells[s..e] {
            *cell = Cell { ch: ' ', fg, bg };
        }
    }
    fn clear_all(&mut self, bg: Option<Color>) {
        for cell in &mut self.cells {
            *cell = Cell::blank(bg);
        }
    }
}

// -------- Terminal --------
pub struct Terminal {
    stdout: Stdout,
    size: (u16, u16),
    is_tty: bool,
    prev_frame: Option<FrameBuffer>,
    cur_frame: Option<FrameBuffer>,
    capturing: bool,
    cap_cursor: (u16, u16),
    cap_fg: Option<Color>,
    cap_bg: Option<Color>,
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
                prev_frame: None,
                cur_frame: None,
                capturing: false,
                cap_cursor: (0, 0),
                cap_fg: None,
                cap_bg: None,
            })
        } else {
            // Headless/CI environment: skip TTY-dependent setup
            warn!("Stdout is not a TTY; running terminal in headless mode for CI/tests");
            let size = (80, 24); // sensible default
            Ok(Self {
                stdout,
                size,
                is_tty,
                prev_frame: None,
                cur_frame: None,
                capturing: false,
                cap_cursor: (0, 0),
                cap_fg: None,
                cap_bg: None,
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
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.clear_all(self.cap_bg);
            }
        } else {
            self.stdout.execute(Clear(ClearType::All))?;
        }
        Ok(())
    }

    pub fn clear_line(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.fill_row_from(self.cap_cursor.0, 0, self.cap_fg, self.cap_bg);
            }
        } else {
            self.stdout.execute(Clear(ClearType::CurrentLine))?;
        }
        Ok(())
    }

    pub fn move_cursor(&mut self, pos: Position) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            self.cap_cursor = (pos.row as u16, pos.col as u16);
            if let Some(f) = self.cur_frame.as_mut() {
                f.cursor_pos = self.cap_cursor;
            }
        } else {
            self.stdout
                .execute(cursor::MoveTo(pos.col as u16, pos.row as u16))?;
        }
        Ok(())
    }

    pub fn hide_cursor(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.cursor_visible = false;
            }
        } else {
            self.stdout.execute(cursor::Hide)?;
        }
        Ok(())
    }

    pub fn show_cursor(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.cursor_visible = true;
            }
        } else {
            self.stdout.execute(cursor::Show)?;
        }
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
        if self.capturing {
            self.cap_fg = Some(color);
        } else {
            self.stdout.execute(SetForegroundColor(color))?;
        }
        Ok(())
    }

    pub fn set_background_color(&mut self, color: Color) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            self.cap_bg = Some(color);
        } else {
            self.stdout.execute(SetBackgroundColor(color))?;
        }
        Ok(())
    }

    pub fn reset_color(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            self.cap_fg = None;
            self.cap_bg = None;
        } else {
            self.stdout.execute(ResetColor)?;
        }
        Ok(())
    }

    pub fn print(&mut self, text: &str) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                let (row, mut col) = self.cap_cursor;
                for ch in text.chars() {
                    if col >= f.width {
                        break;
                    }
                    f.set_char(row, col, ch, self.cap_fg, self.cap_bg);
                    col += 1;
                }
                self.cap_cursor = (row, col);
                f.cursor_pos = self.cap_cursor;
            }
        } else {
            self.stdout.execute(Print(text))?;
        }
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
        if self.capturing {
            // Reuse print logic to keep behavior consistent
            self.print(text)?;
        } else {
            self.stdout.queue(Print(text))?;
        }
        Ok(())
    }

    pub fn queue_move_cursor(&mut self, pos: Position) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            self.cap_cursor = (pos.row as u16, pos.col as u16);
            if let Some(f) = self.cur_frame.as_mut() {
                f.cursor_pos = self.cap_cursor;
            }
        } else {
            self.stdout
                .queue(cursor::MoveTo(pos.col as u16, pos.row as u16))?;
        }
        Ok(())
    }

    pub fn queue_set_fg_color(&mut self, color: Color) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            self.cap_fg = Some(color);
        } else {
            self.stdout.queue(SetForegroundColor(color))?;
        }
        Ok(())
    }

    pub fn queue_set_bg_color(&mut self, color: Color) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            self.cap_bg = Some(color);
        } else {
            self.stdout.queue(SetBackgroundColor(color))?;
        }
        Ok(())
    }

    pub fn queue_reset_color(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            self.cap_fg = None;
            self.cap_bg = None;
        } else {
            self.stdout.queue(ResetColor)?;
        }
        Ok(())
    }

    pub fn queue_clear_line(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.fill_row_from(self.cap_cursor.0, 0, self.cap_fg, self.cap_bg);
            }
        } else {
            self.stdout.queue(Clear(ClearType::CurrentLine))?;
        }
        Ok(())
    }

    pub fn queue_clear_screen(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.clear_all(self.cap_bg);
            }
        } else {
            self.stdout.queue(Clear(ClearType::All))?;
        }
        Ok(())
    }

    pub fn queue_hide_cursor(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.cursor_visible = false;
            }
        } else {
            self.stdout.queue(cursor::Hide)?;
        }
        Ok(())
    }

    pub fn queue_show_cursor(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.cursor_visible = true;
            }
        } else {
            self.stdout.queue(cursor::Show)?;
        }
        Ok(())
    }

    /// Queue cursor to block shape (normal mode)
    pub fn queue_cursor_block(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.cursor_style = Some(SetCursorStyle::SteadyBlock);
            }
        } else {
            self.stdout.queue(SetCursorStyle::SteadyBlock)?;
        }
        Ok(())
    }

    /// Queue cursor to vertical line shape (insert mode)
    pub fn queue_cursor_line(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.cursor_style = Some(SetCursorStyle::SteadyBar);
            }
        } else {
            self.stdout.queue(SetCursorStyle::SteadyBar)?;
        }
        Ok(())
    }

    /// Queue cursor to underline shape (replace mode)
    pub fn queue_cursor_underline(&mut self) -> io::Result<()> {
        if self.is_headless() {
            return Ok(());
        }
        if self.capturing {
            if let Some(f) = self.cur_frame.as_mut() {
                f.cursor_style = Some(SetCursorStyle::SteadyUnderScore);
            }
        } else {
            self.stdout.queue(SetCursorStyle::SteadyUnderScore)?;
        }
        Ok(())
    }

    /// Begin a new shadow frame capture. Any queued operations mutate the frame instead
    /// of emitting escape sequences. Call `flush_frame` to diff & display.
    pub fn begin_frame(&mut self, bg: Color) {
        if self.is_headless() {
            return;
        }
        let (w, h) = self.size;
        self.cur_frame = Some(FrameBuffer::new(w, h, Some(bg)));
        self.capturing = true;
        self.cap_cursor = (0, 0);
        self.cap_fg = None;
        self.cap_bg = Some(bg);
    }

    /// Invalidate previous frame forcing next flush to repaint all cells.
    pub fn invalidate_previous_frame(&mut self) {
        self.prev_frame = None;
    }

    /// Diff current captured frame with previous and emit minimal updates.
    pub fn flush_frame(&mut self) -> io::Result<()> {
        if self.is_headless() || !self.capturing {
            return self.flush(); // fallback
        }
        let Some(cur) = self.cur_frame.take() else {
            return Ok(());
        };
        let mut need_full = false;
        if let Some(prev) = &self.prev_frame {
            if prev.width != cur.width || prev.height != cur.height {
                need_full = true;
            }
        } else {
            need_full = true;
        }

        // Hide cursor during diff to avoid artifacts
        self.stdout.queue(cursor::Hide)?;

        let mut active_fg: Option<Color> = None;
        let mut active_bg: Option<Color> = None;

        if need_full {
            // Full repaint with run grouping for accurate colors
            self.stdout.queue(cursor::MoveTo(0, 0))?;
            self.stdout.queue(Clear(ClearType::All))?;
            for row in 0..cur.height {
                let mut col: u16 = 0;
                while col < cur.width {
                    let idx = (row * cur.width + col) as usize;
                    let cell = cur.cells[idx];
                    let run_fg = cell.fg;
                    let run_bg = cell.bg;
                    let mut run_end = col + 1;
                    while run_end < cur.width {
                        let c2 = cur.cells[(row * cur.width + run_end) as usize];
                        if c2.fg != run_fg || c2.bg != run_bg {
                            break;
                        }
                        run_end += 1;
                    }
                    self.stdout.queue(cursor::MoveTo(col, row))?;
                    if run_fg != active_fg || run_bg != active_bg {
                        self.stdout.queue(ResetColor)?; // reset ensures clean state before applying new fg/bg
                        if let Some(bg) = run_bg {
                            self.stdout.queue(SetBackgroundColor(bg))?;
                        }
                        if let Some(fg) = run_fg {
                            self.stdout.queue(SetForegroundColor(fg))?;
                        }
                        active_fg = run_fg;
                        active_bg = run_bg;
                    }
                    let mut s = String::with_capacity((run_end - col) as usize);
                    for ccol in col..run_end {
                        s.push(cur.cells[(row * cur.width + ccol) as usize].ch);
                    }
                    self.stdout.queue(Print(s))?;
                    col = run_end;
                }
            }
        } else if let Some(prev) = &self.prev_frame {
            // Diff pass: iterate cells, group contiguous runs of changed cells sharing fg/bg
            let mut row: u16 = 0;
            while row < cur.height {
                let mut col: u16 = 0;
                while col < cur.width {
                    let idx = (row * cur.width + col) as usize;
                    let new_cell = cur.cells[idx];
                    let old_cell = prev.cells[idx];
                    if new_cell == old_cell {
                        col += 1;
                        continue;
                    }
                    // Start run
                    let run_fg = new_cell.fg;
                    let run_bg = new_cell.bg;
                    let mut run_end = col + 1;
                    while run_end < cur.width {
                        let j = (row * cur.width + run_end) as usize;
                        let nc = cur.cells[j];
                        let oc = prev.cells[j];
                        if nc == oc {
                            break;
                        }
                        if nc.fg != run_fg || nc.bg != run_bg {
                            break;
                        }
                        run_end += 1;
                    }
                    // Move & set colors
                    self.stdout.queue(cursor::MoveTo(col, row))?;
                    if run_fg != active_fg || run_bg != active_bg {
                        self.stdout.queue(ResetColor)?;
                        if let Some(bg) = run_bg {
                            self.stdout.queue(SetBackgroundColor(bg))?;
                        }
                        if let Some(fg) = run_fg {
                            self.stdout.queue(SetForegroundColor(fg))?;
                        }
                        active_fg = run_fg;
                        active_bg = run_bg;
                    }
                    // Build run string
                    let mut s = String::with_capacity((run_end - col) as usize);
                    for ccol in col..run_end {
                        s.push(cur.cells[(row * cur.width + ccol) as usize].ch);
                    }
                    self.stdout.queue(Print(s))?;
                    col = run_end;
                }
                row += 1;
            }
        }

        // Restore cursor visibility and position
        if cur.cursor_visible {
            self.stdout
                .queue(cursor::MoveTo(cur.cursor_pos.1, cur.cursor_pos.0))?;
            self.stdout.queue(cursor::Show)?;
        } else {
            self.stdout.queue(cursor::Hide)?; // keep hidden
        }
        // Cursor style if set
        if let Some(style) = cur.cursor_style {
            self.stdout.queue(style)?;
        }

        self.stdout.flush()?;
        self.prev_frame = Some(cur);
        self.capturing = false;
        Ok(())
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        debug!("Cleaning up terminal: restoring cursor and colors");
        if !self.is_headless() {
            let _ = self.stdout.execute(cursor::Show);
            let _ = self.stdout.execute(SetCursorStyle::DefaultUserShape);
            let _ = self.stdout.execute(ResetColor);
            let _ = terminal::disable_raw_mode();
            let _ = self.stdout.execute(LeaveAlternateScreen);
        }
    }
}
